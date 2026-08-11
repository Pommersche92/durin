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

use std::{path::PathBuf, time::Duration};

use eframe::{App, egui};
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
    locale::{Localization, detect_system_locale, locales_path},
    process::{RunningProcess, list_running_processes},
    tracking::RamTracker,
    ui::pages,
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
    pub(crate) last_sample: std::time::Instant,
    pub(crate) ram_tracker: RamTracker,
    pub(crate) profile_editor: Option<ProfileEditorState>,
    _hotkey_manager: Option<GlobalHotKeyManager>,
    overlay_hotkey: HotKey,
    tray: Option<TrayState>,
    pub(crate) heap_backend: Box<dyn HeapInspectorBackend + Send + Sync>,
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
        let localization = Localization::load(&locales_path(), requested_locale.as_deref())
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

        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(self.settings.overlay_visible));
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.t("app.window_title").to_string()));

        ui.columns(3, |columns| {
            pages::profiles_page::render(self, &mut columns[0]);
            pages::tracking_page::render(self, &mut columns[1]);
            pages::grouping_page::render(self, &mut columns[2]);
        });

        pages::profile_editor_page::render(self, &ctx);
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
