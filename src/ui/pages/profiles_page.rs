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
    ui.heading("Profile");
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

    if ui.button("Neues Profil").clicked() {
        app.profile_editor = Some(ProfileEditorState {
            edit_index: None,
            draft: Profile {
                name: "".to_string(),
                description: "".to_string(),
                groups: vec![ProcessGroup {
                    name: "Default".to_string(),
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
        .add_enabled(app.selected_profile_idx.is_some(), egui::Button::new("Profil bearbeiten"))
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
        .add_enabled(app.selected_profile_idx.is_some(), egui::Button::new("Profil loeschen"))
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
    ui.label(format!("Hotkey: {}", app.settings.hotkey));
    ui.label("Tray: Klick auf Icon oder Menu");
}
