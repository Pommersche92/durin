//! Profile editor dialog page.
//!
//! This module provides the modal window used for creating and editing
//! profile definitions before persisting them to settings.
//!
//! The dialog includes:
//! - profile identity fields (name, description),
//! - group editing,
//! - per-group process target editing from live and manual inputs.

use eframe::egui;

use crate::{
    app::DurinApp,
    config::{ProcessGroup, ProcessTarget},
    ui::controls::{process_list::running_process_selector, target_list::process_targets_with_remove},
};

/// Renders and manages the profile editor modal lifecycle.
///
/// Behavior summary:
/// - opens only when `app.profile_editor` is set,
/// - edits are performed on draft state,
/// - `Speichern` commits draft into persistent profile collection,
/// - `Abbrechen` drops all unsaved draft modifications.
pub fn render(app: &mut DurinApp, ctx: &egui::Context) {
    let mut close_editor = false;
    let mut save_profile = false;

    if let Some(editor) = &mut app.profile_editor {
        egui::Window::new(if editor.edit_index.is_some() {
            "Profil bearbeiten"
        } else {
            "Profil erstellen"
        })
        .collapsible(false)
        .resizable(true)
        .default_width(540.0)
        .show(ctx, |ui| {
            ui.label("Profilname");
            ui.text_edit_singleline(&mut editor.draft.name);

            ui.label("Beschreibung");
            ui.text_edit_multiline(&mut editor.draft.description);

            ui.separator();
            ui.heading("Gruppen");
            let mut remove_group = None;

            for idx in 0..editor.draft.groups.len() {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(editor.selected_group_idx == Some(idx), "")
                        .clicked()
                    {
                        editor.selected_group_idx = Some(idx);
                    }
                    ui.text_edit_singleline(&mut editor.draft.groups[idx].name);
                    if ui.small_button("entfernen").clicked() {
                        remove_group = Some(idx);
                    }
                });
            }

            if let Some(idx) = remove_group {
                editor.draft.groups.remove(idx);
                if editor.draft.groups.is_empty() {
                    editor.selected_group_idx = None;
                } else if idx == 0 {
                    editor.selected_group_idx = Some(0);
                } else {
                    editor.selected_group_idx = Some(idx.saturating_sub(1));
                }
            }

            if ui.button("Leere Gruppe hinzufuegen").clicked() {
                editor.draft.groups.push(ProcessGroup {
                    name: "Neue Gruppe".to_string(),
                    targets: Vec::new(),
                });
                editor.selected_group_idx = Some(editor.draft.groups.len().saturating_sub(1));
            }

            ui.separator();
            ui.heading("Prozesse in ausgewaehlter Gruppe");

            if let Some(group_idx) = editor.selected_group_idx {
                if let Some(group) = editor.draft.groups.get_mut(group_idx) {
                    if let Some(remove_target) = process_targets_with_remove(ui, &group.targets) {
                        group.targets.remove(remove_target);
                    }

                    ui.separator();
                    ui.label("Laufende Prozesse auswaehlen");
                    running_process_selector(
                        ui,
                        &app.running_processes,
                        &mut editor.process_search,
                        &mut editor.selected_running_process,
                        140.0,
                    );

                    if ui
                        .add_enabled(
                            editor.selected_running_process.is_some(),
                            egui::Button::new("Laufenden Prozess hinzufuegen"),
                        )
                        .clicked()
                    {
                        if let Some(proc_idx) = editor.selected_running_process {
                            if let Some(proc_info) = app.running_processes.get(proc_idx) {
                                group.targets.push(ProcessTarget {
                                    display_name: format!("{} (PID {})", proc_info.name, proc_info.pid),
                                    process_name: proc_info.name.clone(),
                                    pid: Some(proc_info.pid),
                                    manual: false,
                                });
                            }
                        }
                    }

                    ui.separator();
                    ui.label("Prozessname manuell eingeben");
                    ui.text_edit_singleline(&mut editor.manual_process_name);
                    if ui
                        .add_enabled(
                            !editor.manual_process_name.trim().is_empty(),
                            egui::Button::new("Manuellen Prozessnamen hinzufuegen"),
                        )
                        .clicked()
                    {
                        let process_name = editor.manual_process_name.trim().to_string();
                        group.targets.push(ProcessTarget {
                            display_name: format!("{} (Name Match)", process_name),
                            process_name,
                            pid: None,
                            manual: true,
                        });
                        editor.manual_process_name.clear();
                    }
                }
            } else {
                ui.label("Lege mindestens eine Gruppe an und waehle sie aus.");
            }

            ui.separator();
            ui.horizontal(|ui| {
                let can_save = !editor.draft.name.trim().is_empty();
                if ui
                    .add_enabled(can_save, egui::Button::new("Speichern"))
                    .clicked()
                {
                    save_profile = true;
                }

                if ui.button("Abbrechen").clicked() {
                    close_editor = true;
                }
            });
        });
    }

    if save_profile {
        if let Some(editor) = &app.profile_editor {
            let mut profile = editor.draft.clone();
            profile.name = profile.name.trim().to_string();
            profile.description = profile.description.trim().to_string();

            if let Some(edit_idx) = editor.edit_index {
                if let Some(existing) = app.settings.profiles.get_mut(edit_idx) {
                    *existing = profile;
                    app.selected_profile_idx = Some(edit_idx);
                }
            } else {
                app.settings.profiles.push(profile);
                app.selected_profile_idx = Some(app.settings.profiles.len().saturating_sub(1));
            }

            app.settings.set_active_profile_by_index(app.selected_profile_idx);
            app.ram_tracker.clear();
            app.save_settings();
        }
        app.profile_editor = None;
    }

    if close_editor {
        app.profile_editor = None;
    }
}
