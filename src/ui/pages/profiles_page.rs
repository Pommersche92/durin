//! Profile selection and lifecycle page.
//!
//! This page handles high-level profile operations:
//! - choose active profile,
//! - create profile draft,
//! - edit selected profile,
//! - delete selected profile.
//!
//! It also surfaces quick status hints for hotkey/tray controls.

use eframe::egui;

use crate::{
    app::{DurinApp, ProfileEditorState},
    config::{ProcessGroup, Profile},
};

/// Renders the profile management sidebar.
///
/// Side effects:
/// - updates active profile selection,
/// - initializes profile editor draft state,
/// - persists settings when profile operations change durable state.
pub fn render(app: &mut DurinApp, ui: &mut egui::Ui) {
    ui.heading(app.t("profiles.heading"));
    ui.separator();

    for i in 0..app.settings.profiles.len() {
        let profile_name = app.settings.profiles[i].name.clone();
        let selected = app.selected_profile_idx == Some(i);
        if ui.selectable_label(selected, profile_name).clicked() {
            app.selected_profile_idx = Some(i);
            app.settings.set_active_profile_by_index(app.selected_profile_idx);
            app.selected_group_idx = Some(0);
            app.ram_tracker.clear();
            app.save_settings();
        }
    }

    ui.separator();

    if ui.button(app.t("profiles.new")) .clicked() {
        app.profile_editor = Some(ProfileEditorState {
            edit_index: None,
            draft: Profile {
                name: "".to_string(),
                description: "".to_string(),
                groups: vec![ProcessGroup {
                    name: app.t("profiles.default_group_name").to_string(),
                    targets: Vec::new(),
                }],
            },
            selected_group_idx: Some(0),
            process_search: String::new(),
            selected_running_process: None,
            manual_process_name: String::new(),
        });
    }

    if ui
        .add_enabled(app.selected_profile_idx.is_some(), egui::Button::new(app.t("profiles.edit")))
        .clicked()
    {
        if let Some(idx) = app.selected_profile_idx {
            app.profile_editor = Some(ProfileEditorState {
                edit_index: Some(idx),
                draft: app.settings.profiles[idx].clone(),
                selected_group_idx: Some(0),
                process_search: String::new(),
                selected_running_process: None,
                manual_process_name: String::new(),
            });
        }
    }

    if ui
        .add_enabled(app.selected_profile_idx.is_some(), egui::Button::new(app.t("profiles.delete")))
        .clicked()
    {
        if let Some(idx) = app.selected_profile_idx {
            app.settings.profiles.remove(idx);
            app.selected_profile_idx = None;
            app.settings.set_active_profile_by_index(None);
            app.ram_tracker.clear();
            app.save_settings();
        }
    }

    ui.separator();
    ui.label(format!("{}: {}", app.t("settings.hotkey"), app.settings.hotkey));
    ui.label(app.t("settings.tray_hint"));

    ui.separator();
    ui.label(app.t("settings.language"));

    let mut selected_language = app.settings.ui_language.clone().unwrap_or_default();
    let selected_label = if selected_language.is_empty() {
        format!(
            "{} ({})",
            app.t("settings.language_system"),
            app.current_ui_language()
        )
    } else {
        selected_language.clone()
    };

    egui::ComboBox::from_id_salt("ui_language_select")
        .selected_text(selected_label)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut selected_language,
                String::new(),
                app.t("settings.language_system"),
            );

            for locale in app.available_ui_languages() {
                ui.selectable_value(&mut selected_language, locale.to_string(), locale);
            }
        });

    let desired_language = if selected_language.is_empty() {
        None
    } else {
        Some(selected_language)
    };

    if desired_language != app.settings.ui_language {
        app.set_ui_language(desired_language);
    }
}
