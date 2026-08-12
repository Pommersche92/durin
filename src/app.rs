//! Primary application state container and non-UI control flow.
//!
//! This module owns the long-lived runtime state for Durin and coordinates:
//! - process discovery and RAM sampling cadence,
//! - global hotkey and tray event handling,
//! - persistence hooks for `settings.toml`,
//! - delegation to UI page modules under `src/ui`.
//!
//! Design notes:
//! - UI rendering lives in dedicated page modules; this file keeps orchestration logic.
//! - Platform integration objects (hotkey manager/tray icon) are kept alive in the app state.
//! - Private helper functions encapsulate deterministic parsing and icon generation.

use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use eframe::{App, egui};
use egui_plot::{GridMark, HoverPosition, Legend, Line, Plot};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use sysinfo::{ProcessesToUpdate, System};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuId, MenuItem},
};

use crate::{
    config::{ProcessTarget, Profile, Settings},
    heap::{HeapInspectorBackend, StubHeapInspector},
    locale::{Localization, detect_system_locale},
    process::{RunningProcess, list_running_processes},
    tracking::RamTracker,
    ui::pages,
    windowing::{ChartPopoutController, create_chart_popout_controller},
};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(1000);

/// Central state object shared by all UI pages and background polling logic.
///
/// The struct intentionally groups state by responsibility:
/// - persisted user-facing config (`settings`),
/// - dynamic runtime caches (`running_processes`, tracker state),
/// - transient UI state (selection/edit buffers),
/// - OS integration handles (hotkey/tray).
pub struct DurinApp {
    pub(crate) settings: Settings,
    pub(crate) settings_path: PathBuf,
    pub(crate) localization: Localization,
    pub(crate) status_message: Option<String>,
    pub(crate) system: System,
    pub(crate) running_processes: Vec<RunningProcess>,
    pub(crate) selected_profile_idx: Option<usize>,
    pub(crate) selected_group_idx: Option<usize>,
    pub(crate) selected_running_process: Option<usize>,
    pub(crate) new_group_name: String,
    pub(crate) manual_process_name: String,
    pub(crate) process_search: String,
    last_sample: Instant,
    pub(crate) ram_tracker: RamTracker,
    pub(crate) profile_editor: Option<ProfileEditorState>,
    _hotkey_manager: Option<GlobalHotKeyManager>,
    overlay_hotkey: HotKey,
    tray: Option<TrayState>,
    pub(crate) chart_popout: Box<dyn ChartPopoutController>,
    chart_popout_viewport_id: Option<egui::ViewportId>,
    chart_snapshot: Arc<RwLock<Vec<ChartSeries>>>,
    pub(crate) heap_backend: Box<dyn HeapInspectorBackend + Send + Sync>,
}

#[derive(Clone, Debug, Default)]
struct ChartSeries {
    name: String,
    points: Vec<[f64; 2]>,
}

/// Runtime tray integration state.
///
/// Keeps icon/menu alive and stores stable menu IDs used to map incoming
/// events to commands (`toggle overlay`, `quit app`).
struct TrayState {
    _icon: TrayIcon,
    _menu: Menu,
    toggle_item: MenuItem,
    quit_item: MenuItem,
    toggle_id: MenuId,
    quit_id: MenuId,
}

/// Scratchpad state for profile create/edit flows.
///
/// This state is intentionally detached from persisted settings until the
/// user confirms with `Speichern`, enabling cancel/revert semantics.
pub(crate) struct ProfileEditorState {
    pub(crate) edit_index: Option<usize>,
    pub(crate) draft: Profile,
    pub(crate) selected_group_idx: Option<usize>,
    pub(crate) process_search: String,
    pub(crate) selected_running_process: Option<usize>,
    pub(crate) manual_process_name: String,
}

impl DurinApp {
    /// Constructs a new application state from persisted settings.
    ///
    /// During initialization, the function:
    /// - boots a full `sysinfo::System` snapshot,
    /// - resolves currently running processes,
    /// - restores active profile selection,
    /// - parses and registers the global overlay hotkey,
    /// - creates tray resources when supported by the platform.
    ///
    /// If optional integrations fail (e.g., hotkey registration), the app
    /// remains usable and logs a warning rather than aborting startup.
    pub fn new(settings: Settings, settings_path: PathBuf) -> Self {
        let requested_locale = settings
            .ui_language
            .clone()
            .or_else(detect_system_locale);
        let locales_override_dir = settings_path.parent().map(|dir| dir.join("locales"));
        let localization = Localization::load(locales_override_dir.as_deref(), requested_locale.as_deref())
            .unwrap_or_else(|err| panic!("Failed to load localization files: {err}"));

        let mut system = System::new_all();
        system.refresh_processes(ProcessesToUpdate::All, true);

        let running_processes = list_running_processes(&system);
        let selected_profile_idx = settings.active_profile_index();

        let overlay_hotkey = parse_hotkey(&settings.hotkey)
            .unwrap_or_else(|| HotKey::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyR));

        let hotkey_manager = GlobalHotKeyManager::new().ok();
        if let Some(manager) = &hotkey_manager {
            if let Err(err) = manager.register(overlay_hotkey) {
                tracing::warn!("Hotkey konnte nicht registriert werden: {err}");
            }
        }

        let tray = create_tray(&localization);
        let mut chart_popout = create_chart_popout_controller();
        chart_popout.set_enabled(settings.chart_popout_enabled);
        chart_popout.set_opacity(1.0);
        chart_popout.set_always_on_top(settings.chart_popout_always_on_top);

        Self {
            settings,
            settings_path,
            localization,
            status_message: None,
            system,
            running_processes,
            selected_profile_idx,
            selected_group_idx: Some(0),
            selected_running_process: None,
            new_group_name: String::new(),
            manual_process_name: String::new(),
            process_search: String::new(),
            last_sample: std::time::Instant::now(),
            ram_tracker: RamTracker::new(),
            profile_editor: None,
            _hotkey_manager: hotkey_manager,
            overlay_hotkey,
            tray,
            chart_popout,
            chart_popout_viewport_id: None,
            chart_snapshot: Arc::new(RwLock::new(Vec::new())),
            heap_backend: Box::<StubHeapInspector>::default(),
        }
    }

    /// Persists current settings to disk and records a user-visible status message.
    ///
    /// This is the canonical persistence endpoint used by both UI interactions
    /// and background event handlers, ensuring consistent feedback behavior.
    pub(crate) fn save_settings(&mut self) {
        match self.settings.save(&self.settings_path) {
            Ok(()) => {
                self.status_message = Some(self.t("status.settings_saved").to_string());
            }
            Err(err) => {
                self.status_message = Some(format!("{}: {err}", self.t("status.settings_save_failed")));
            }
        }
    }

    pub(crate) fn t<'a>(&'a self, key: &'a str) -> &'a str {
        self.localization.text(key)
    }

    pub(crate) fn current_ui_language(&self) -> &str {
        self.localization.current_locale()
    }

    pub(crate) fn available_ui_languages(&self) -> Vec<&str> {
        self.localization.available_locales()
    }

    pub(crate) fn toggle_chart_popout(&mut self) {
        self.settings.chart_popout_enabled = !self.settings.chart_popout_enabled;
        self.chart_popout.set_enabled(self.settings.chart_popout_enabled);
        self.chart_popout.set_click_through(false);
        self.save_settings();
    }

    pub(crate) fn toggle_chart_popout_pin(&mut self) {
        self.settings.chart_popout_pinned = !self.settings.chart_popout_pinned;
        self.save_settings();
    }

    pub(crate) fn sync_chart_popout_input(&mut self, ctrl_down: bool, hovered: bool) {
        self.chart_popout.update_from_input(ctrl_down, hovered, 1.0);
    }

    fn refresh_chart_snapshot(&mut self) {
        let series = self
            .ram_tracker
            .group_series()
            .map(|(name, values)| ChartSeries {
                name: name.to_string(),
                points: values.iter().map(|p| [p.t_sec, p.value_mib]).collect(),
            })
            .collect();

        *self.chart_snapshot.write().expect("chart snapshot lock poisoned") = series;
    }

    fn render_chart_popout(&mut self, ctx: &egui::Context) {
        if !self.settings.chart_popout_enabled {
            if let Some(id) = self.chart_popout_viewport_id {
                ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Close);
                self.chart_popout_viewport_id = None;
            }
            return;
        }

        let viewport_id = self
            .chart_popout_viewport_id
            .unwrap_or_else(|| {
                let id = egui::ViewportId::from_hash_of("durin_chart_popout");
                self.chart_popout_viewport_id = Some(id);
                id
            });

        self.chart_popout.set_viewport_id(viewport_id);
        let snapshot = self.chart_snapshot.clone();
        let opacity = 1.0;
        let pinned = self.settings.chart_popout_pinned;
        let saved_position = self.settings.chart_popout_position.unwrap_or([120.0, 120.0]);
        let saved_size = self.settings.chart_popout_size.unwrap_or([760.0, 360.0]);

        self.chart_popout.apply_platform_state(ctx, viewport_id, false, false, 1.0);
        ctx.request_repaint_of(viewport_id);

        let pinned_state = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(pinned));
        let pinned_state_for_closure = pinned_state.clone();
        let geometry_state = std::sync::Arc::new(std::sync::Mutex::new((
            self.settings.chart_popout_position,
            self.settings.chart_popout_size,
        )));
        let geometry_state_for_closure = geometry_state.clone();
        let settings_path_for_closure = self.settings_path.clone();

        ctx.show_viewport_deferred(
            viewport_id,
            egui::ViewportBuilder::default()
                .with_title("Durin RAM Chart")
                .with_decorations(false)
                .with_transparent(false)
                .with_always_on_top()
                .with_resizable(true)
                .with_visible(true)
                .with_position(saved_position)
                .with_inner_size(saved_size),
            move |ui, _class| {
                let title_bar_height = 32.0;
                let chart_top_margin = 6.0;
                let effective_pinned = pinned_state_for_closure.load(std::sync::atomic::Ordering::Relaxed);
                let max_rect = ui.max_rect();
                let background = egui::Color32::from_rgba_unmultiplied(10, 12, 18, (opacity * 255.0) as u8);
                ui.painter().rect_filled(max_rect, 0.0, background);

                let title_rect = egui::Rect::from_min_size(
                    max_rect.min,
                    egui::vec2(max_rect.width(), title_bar_height),
                );
                ui.painter().rect_filled(title_rect, 0.0, egui::Color32::from_rgba_unmultiplied(18, 22, 32, 220));

                let title_response = ui.allocate_rect(title_rect, egui::Sense::drag());
                if title_response.drag_started() && !effective_pinned {
                    ui.ctx().send_viewport_cmd_to(viewport_id, egui::ViewportCommand::StartDrag);
                }

                let label_rect = egui::Rect::from_min_size(
                    title_rect.min + egui::vec2(12.0, 6.0),
                    egui::vec2(120.0, 18.0),
                );
                ui.painter().text(
                    label_rect.min,
                    egui::Align2::LEFT_TOP,
                    "RAM Chart",
                    egui::FontId::proportional(16.0),
                    egui::Color32::WHITE,
                );

                let pin_button_rect = egui::Rect::from_min_size(
                    egui::pos2(title_rect.max.x - 72.0, title_rect.min.y + 5.0),
                    egui::vec2(60.0, 22.0),
                );
                let pin_label = if effective_pinned { "Unpin" } else { "Pin" };
                let pin_button = ui.put(pin_button_rect, egui::Button::new(pin_label));
                if pin_button.clicked() {
                    let next = !effective_pinned;
                    pinned_state_for_closure.store(next, std::sync::atomic::Ordering::Relaxed);
                }

                ui.ctx().send_viewport_cmd_to(
                    viewport_id,
                    egui::ViewportCommand::Resizable(!effective_pinned),
                );

                let resize_rect = egui::Rect::from_min_size(
                    egui::pos2(max_rect.max.x - 18.0, max_rect.max.y - 18.0),
                    egui::vec2(18.0, 18.0),
                );
                let resize_response = ui.allocate_rect(resize_rect, egui::Sense::drag());
                if resize_response.drag_started() && !effective_pinned {
                    ui.ctx().send_viewport_cmd_to(
                        viewport_id,
                        egui::ViewportCommand::BeginResize(egui::viewport::ResizeDirection::SouthEast),
                    );
                }

                let chart_rect = egui::Rect::from_min_max(
                    egui::pos2(max_rect.min.x + 6.0, max_rect.min.y + title_bar_height + chart_top_margin),
                    egui::pos2(max_rect.max.x - 6.0, max_rect.max.y - 6.0),
                );
                let mut chart_ui = ui.new_child(egui::UiBuilder::new().max_rect(chart_rect));

                if let Some(rect) = ui.ctx().input(|i| i.viewport().outer_rect) {
                    let next_position = [rect.min.x, rect.min.y];
                    let next_size = [rect.width(), rect.height()];
                    let mut geometry = geometry_state_for_closure.lock().expect("chart geometry mutex poisoned");
                    let should_save = geometry.0 != Some(next_position) || geometry.1 != Some(next_size);
                    if should_save {
                        geometry.0 = Some(next_position);
                        geometry.1 = Some(next_size);
                        let mut settings = crate::config::Settings::load_or_default(&settings_path_for_closure)
                            .unwrap_or_default();
                        settings.chart_popout_position = Some(next_position);
                        settings.chart_popout_size = Some(next_size);
                        let _ = settings.save(&settings_path_for_closure);
                    }
                }

                if let Ok(series) = snapshot.read() {
                    if series.is_empty() {
                        chart_ui.label("No data yet");
                    } else {
                        let plot = Plot::new("chart_popout_plot")
                            .legend(Legend::default())
                            .show_background(false)
                            .x_axis_formatter(|mark: GridMark, _x_range| format_duration_compact(mark.value))
                            .y_axis_formatter(|mark: GridMark, y_range| {
                                let unit = choose_memory_unit_for_range(y_range);
                                format_memory_value_from_mib(mark.value, unit)
                            })
                            .label_formatter(|hover_pos: &HoverPosition<'_>| {
                                Some(match hover_pos {
                                    HoverPosition::NearDataPoint { plot_name, position, index: _ } => format!(
                                        "{}\nt = {}\nRAM = {}",
                                        plot_name,
                                        format_duration_compact(position.x),
                                        format_memory_value_adaptive(position.y)
                                    ),
                                    HoverPosition::Elsewhere { position } => format!(
                                        "t = {}\nRAM = {}",
                                        format_duration_compact(position.x),
                                        format_memory_value_adaptive(position.y)
                                    ),
                                })
                            });

                        plot.show(&mut chart_ui, |plot_ui| {
                            for group in series.iter() {
                                plot_ui.line(Line::new(group.name.clone(), group.points.clone()));
                            }
                        });
                    }
                }
            },
        );

        let geometry = geometry_state.lock().expect("chart geometry mutex poisoned");
        self.settings.chart_popout_position = geometry.0;
        self.settings.chart_popout_size = geometry.1;
        self.settings.chart_popout_pinned = pinned_state.load(std::sync::atomic::Ordering::Relaxed);
    }

    pub(crate) fn set_ui_language(&mut self, language: Option<String>) {
        let requested = language.as_deref().filter(|value| !value.trim().is_empty());
        let runtime_locale = match requested {
            Some(locale) => self.localization.resolve_supported_locale(Some(locale)),
            None => self
                .localization
                .resolve_supported_locale(detect_system_locale().as_deref()),
        };

        self.settings.ui_language = language;
        self.localization.set_locale(&runtime_locale);
        self.refresh_tray_labels();
        self.save_settings();
    }

    fn refresh_tray_labels(&self) {
        if let Some(tray) = &self.tray {
            tray.toggle_item.set_text(self.t("tray.toggle_overlay"));
            tray.quit_item.set_text(self.t("tray.quit"));
            let _ = tray._icon.set_tooltip(Some(self.t("tray.tooltip")));
        }
    }

    /// Returns a mutable reference to the currently selected profile, if any.
    ///
    /// The lookup is index-based and tied to transient UI selection state.
    /// Callers should gracefully handle `None` for empty profile collections
    /// or no active selection.
    pub(crate) fn active_profile_mut(&mut self) -> Option<&mut Profile> {
        let idx = self.selected_profile_idx?;
        self.settings.profiles.get_mut(idx)
    }

    /// Returns an immutable reference to the currently selected profile.
    ///
    /// This accessor is used by rendering paths and sampling logic that only
    /// need read access to profile/group membership.
    pub(crate) fn active_profile(&self) -> Option<&Profile> {
        let idx = self.selected_profile_idx?;
        self.settings.profiles.get(idx)
    }

    /// Performs one runtime maintenance tick.
    ///
    /// Responsibilities per tick:
    /// - refresh process table snapshot,
    /// - update live process list used by selectors,
    /// - sample RAM usage at fixed interval,
    /// - poll OS event channels for hotkey and tray commands.
    ///
    /// This function is called from the UI loop, making all side effects
    /// frame-synchronized and avoiding additional background threads.
    fn tick(&mut self) {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        self.running_processes = list_running_processes(&self.system);

        if self.last_sample.elapsed() >= SAMPLE_INTERVAL {
            if let Some(profile) = self.active_profile().cloned() {
                self.ram_tracker.sample_profile(&profile, &mut self.system);
            }
            self.last_sample = std::time::Instant::now();
        }

        self.poll_hotkey();
        self.poll_tray();
    }

    /// Drains global hotkey events and applies overlay visibility toggles.
    ///
    /// Toggle semantics are bound to key release events to avoid repeated
    /// rapid toggles while a key chord remains pressed.
    fn poll_hotkey(&mut self) {
        while let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.overlay_hotkey.id() && event.state == HotKeyState::Released {
                self.settings.overlay_visible = !self.settings.overlay_visible;
                self.save_settings();
            }
        }
    }

    /// Drains tray menu/click events and translates them into app commands.
    ///
    /// Supported commands:
    /// - toggle overlay visibility,
    /// - terminate the process (`quit`).
    ///
    /// Event IDs are cloned up front to satisfy Rust borrowing rules while
    /// mutating application state inside the receive loop.
    fn poll_tray(&mut self) {
        if let Some(tray) = &self.tray {
            let toggle_id = tray.toggle_id.clone();
            let quit_id = tray.quit_id.clone();
            let tray_icon_id = tray._icon.id().clone();

            while let Ok(event) = MenuEvent::receiver().try_recv() {
                if event.id == toggle_id {
                    self.settings.overlay_visible = !self.settings.overlay_visible;
                    self.save_settings();
                }

                if event.id == quit_id {
                    std::process::exit(0);
                }
            }

            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                match event {
                    TrayIconEvent::Click { id, .. } | TrayIconEvent::DoubleClick { id, .. }
                        if id == tray_icon_id =>
                    {
                        self.settings.overlay_visible = !self.settings.overlay_visible;
                        self.save_settings();
                    }
                    _ => {}
                }
            }
        }
    }

    /// Adds the currently selected running process to the selected group.
    ///
    /// The created target stores both display metadata and PID-based identity,
    /// enabling precise matching while the process is alive.
    pub(crate) fn add_selected_running_process(&mut self) {
        let running_idx = match self.selected_running_process {
            Some(idx) => idx,
            None => return,
        };
        let group_idx = match self.selected_group_idx {
            Some(idx) => idx,
            None => return,
        };

        if let Some(proc_info) = self.running_processes.get(running_idx).cloned() {
            if let Some(profile) = self.active_profile_mut() {
                if let Some(group) = profile.groups.get_mut(group_idx) {
                    group.targets.push(ProcessTarget {
                        display_name: format!("{} (PID {})", proc_info.name, proc_info.pid),
                        process_name: proc_info.name,
                        pid: Some(proc_info.pid),
                        manual: false,
                    });
                    self.save_settings();
                }
            }
        }
    }

    /// Adds a manual process-name target to the selected group.
    ///
    /// This path is useful for future/ephemeral processes that are not present
    /// in the current process list. Matching is done by lowercase name equality
    /// during RAM aggregation.
    pub(crate) fn add_manual_process_name(&mut self) {
        let group_idx = match self.selected_group_idx {
            Some(idx) => idx,
            None => return,
        };

        let process_name = self.manual_process_name.trim().to_string();
        let name_match_label = self.t("process.name_match").to_string();
        if process_name.is_empty() {
            return;
        }

        if let Some(profile) = self.active_profile_mut() {
            if let Some(group) = profile.groups.get_mut(group_idx) {
                group.targets.push(ProcessTarget {
                    display_name: format!("{} ({})", process_name, name_match_label),
                    process_name,
                    pid: None,
                    manual: true,
                });
                self.manual_process_name.clear();
                self.save_settings();
            }
        }
    }
}

impl App for DurinApp {
    /// Renders one UI frame and advances frame-coupled runtime logic.
    ///
    /// The method keeps a strict ordering:
    /// 1. Run maintenance tick (refresh/process events/sampling).
    /// 2. Apply viewport visibility command from settings.
    /// 3. Render page layout (profiles, tracking, grouping).
    /// 4. Render modal profile editor window.
    /// 5. Request next repaint with a controlled cadence.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.tick();
        self.refresh_chart_snapshot();

        let ctrl_down = ui.input(|i| i.modifiers.ctrl);
        let hovered = ui.input(|i| i.pointer.hover_pos().is_some());

        if let Some(rect) = ui.ctx().input(|i| i.viewport().outer_rect) {
            let next_position = [rect.min.x, rect.min.y];
            let next_size = [rect.width(), rect.height()];
            let settings_changed = self.settings.main_window_position != Some(next_position)
                || self.settings.main_window_size != Some(next_size);
            if settings_changed {
                self.settings.main_window_position = Some(next_position);
                self.settings.main_window_size = Some(next_size);
                let _ = self.settings.save(&self.settings_path);
            }
        }

        self.sync_chart_popout_input(ctrl_down, hovered);

        ui.horizontal_wrapped(|ui| {
            ui.heading(self.t("app.window_title"));
            ui.separator();

            let popout_label = if self.settings.chart_popout_enabled {
                "Close pop-out"
            } else {
                "Open pop-out"
            };

            if ui.button(popout_label).clicked() {
                self.toggle_chart_popout();
            }

        });

        ui.add_space(4.0);

        ui.columns(3, |columns| {
            pages::profiles_page::render(self, &mut columns[0]);
            pages::tracking_page::render(self, &mut columns[1]);
            pages::grouping_page::render(self, &mut columns[2]);
        });

        pages::profile_editor_page::render(self, &ctx);
        self.render_chart_popout(&ctx);
        ctx.request_repaint_after(Duration::from_millis(120));
    }
}

/// Creates tray icon resources and returns initialized tray state.
///
/// Returns `None` when tray setup is not available on the current platform
/// or when any intermediate build step fails.
fn create_tray(localization: &Localization) -> Option<TrayState> {
    let menu = Menu::new();
    let toggle_item = MenuItem::new(localization.text("tray.toggle_overlay"), true, None);
    let quit_item = MenuItem::new(localization.text("tray.quit"), true, None);
    let toggle_id = toggle_item.id().clone();
    let quit_id = quit_item.id().clone();

    if menu.append(&toggle_item).is_err() || menu.append(&quit_item).is_err() {
        return None;
    }

    let icon = create_overlay_icon().ok()?;

    let icon = TrayIconBuilder::new()
        .with_tooltip(localization.text("tray.tooltip"))
        .with_menu(Box::new(menu.clone()))
        .with_icon(icon)
        .build()
        .ok()?;

    Some(TrayState {
        _icon: icon,
        _menu: menu,
        toggle_item,
        quit_item,
        toggle_id,
        quit_id,
    })
}

/// Builds a tiny in-memory RGBA icon used for tray representation.
///
/// The generated icon intentionally avoids external assets to keep startup
/// deterministic and packaging-simple across Windows and Linux targets.
fn create_overlay_icon() -> anyhow::Result<Icon> {
    let width = 32;
    let height = 32;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let is_bar = x > 6 && x < 26 && y > (28 - x / 2) && y < 30;
            let (r, g, b, a) = if is_bar {
                (24, 196, 127, 255)
            } else {
                (14, 22, 34, 255)
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }

    Icon::from_rgba(rgba, width, height).map_err(Into::into)
}

/// Parses a user-configured hotkey string into a `global_hotkey::HotKey`.
///
/// Supported examples include:
/// - `Ctrl+Shift+R`
/// - `Alt+F8`
/// - `Super+1`
///
/// Unknown or incomplete values return `None`, allowing caller-provided fallback.
#[derive(Clone, Copy)]
enum MemoryUnit {
    B,
    KiB,
    MiB,
    GiB,
    TiB,
}

fn choose_memory_unit_for_range(y_range: &std::ops::RangeInclusive<f64>) -> MemoryUnit {
    let max_abs_mib = y_range.start().abs().max(y_range.end().abs());

    if max_abs_mib >= 1024.0 * 1024.0 {
        MemoryUnit::TiB
    } else if max_abs_mib >= 1024.0 {
        MemoryUnit::GiB
    } else if max_abs_mib >= 1.0 {
        MemoryUnit::MiB
    } else if max_abs_mib >= (1.0 / 1024.0) {
        MemoryUnit::KiB
    } else {
        MemoryUnit::B
    }
}

fn format_memory_value_adaptive(value_mib: f64) -> String {
    let unit = choose_memory_unit_for_range(&(0.0..=value_mib.abs()));
    format_memory_value_from_mib(value_mib, unit)
}

fn format_memory_value_from_mib(value_mib: f64, unit: MemoryUnit) -> String {
    let (value, suffix) = match unit {
        MemoryUnit::B => (value_mib * 1024.0 * 1024.0, "B"),
        MemoryUnit::KiB => (value_mib * 1024.0, "KB"),
        MemoryUnit::MiB => (value_mib, "MB"),
        MemoryUnit::GiB => (value_mib / 1024.0, "GB"),
        MemoryUnit::TiB => (value_mib / (1024.0 * 1024.0), "TB"),
    };

    let decimals = if value.abs() >= 100.0 {
        0
    } else if value.abs() >= 10.0 {
        1
    } else {
        2
    };

    format!("{:.*} {}", decimals, value, suffix)
}

fn format_duration_compact(seconds: f64) -> String {
    let sign = if seconds.is_sign_negative() { "-" } else { "" };
    let total = seconds.abs();

    let hours = (total / 3600.0).floor() as u64;
    let minutes = ((total % 3600.0) / 60.0).floor() as u64;
    let secs = (total % 60.0).round() as u64;

    if hours > 0 {
        format!("{sign}{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{sign}{minutes}m {secs}s")
    } else if total >= 10.0 {
        format!("{sign}{:.0}s", total)
    } else {
        format!("{sign}{:.1}s", total)
    }
}

fn parse_hotkey(input: &str) -> Option<HotKey> {
    let parts: Vec<String> = input
        .split('+')
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect();

    if parts.is_empty() {
        return None;
    }

    let mut modifiers = Modifiers::empty();
    let mut key_code = None;

    for part in parts {
        match part.as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
            "super" | "win" | "meta" => modifiers |= Modifiers::SUPER,
            other => {
                key_code = parse_key_code(other);
            }
        }
    }

    let key_code = key_code?;
    Some(HotKey::new(Some(modifiers), key_code))
}

/// Parses a textual key token into a keyboard code.
///
/// Supports single alphanumeric keys and function keys (`F1`..`F12`).
/// The parser is case-insensitive and deliberately conservative to avoid
/// ambiguous mappings across platforms/layouts.
fn parse_key_code(value: &str) -> Option<Code> {
    let upper = value.to_ascii_uppercase();
    let mut chars = upper.chars();

    if let (Some(ch), None) = (chars.next(), chars.next()) {
        return match ch {
            'A' => Some(Code::KeyA),
            'B' => Some(Code::KeyB),
            'C' => Some(Code::KeyC),
            'D' => Some(Code::KeyD),
            'E' => Some(Code::KeyE),
            'F' => Some(Code::KeyF),
            'G' => Some(Code::KeyG),
            'H' => Some(Code::KeyH),
            'I' => Some(Code::KeyI),
            'J' => Some(Code::KeyJ),
            'K' => Some(Code::KeyK),
            'L' => Some(Code::KeyL),
            'M' => Some(Code::KeyM),
            'N' => Some(Code::KeyN),
            'O' => Some(Code::KeyO),
            'P' => Some(Code::KeyP),
            'Q' => Some(Code::KeyQ),
            'R' => Some(Code::KeyR),
            'S' => Some(Code::KeyS),
            'T' => Some(Code::KeyT),
            'U' => Some(Code::KeyU),
            'V' => Some(Code::KeyV),
            'W' => Some(Code::KeyW),
            'X' => Some(Code::KeyX),
            'Y' => Some(Code::KeyY),
            'Z' => Some(Code::KeyZ),
            '0' => Some(Code::Digit0),
            '1' => Some(Code::Digit1),
            '2' => Some(Code::Digit2),
            '3' => Some(Code::Digit3),
            '4' => Some(Code::Digit4),
            '5' => Some(Code::Digit5),
            '6' => Some(Code::Digit6),
            '7' => Some(Code::Digit7),
            '8' => Some(Code::Digit8),
            '9' => Some(Code::Digit9),
            _ => None,
        };
    }

    match upper.as_str() {
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        _ => None,
    }
}
