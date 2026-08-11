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
    let profile_name_label = app.t("editor.profile_name").to_string();
    let description_label = app.t("editor.description").to_string();
    let groups_heading = app.t("editor.groups").to_string();
    let remove_label = app.t("common.remove").to_string();
    let add_empty_group_label = app.t("editor.add_empty_group").to_string();
    let new_group_name = app.t("editor.new_group_name").to_string();
    let processes_in_group_heading = app.t("editor.processes_in_selected_group").to_string();
    let select_running_processes_label = app.t("editor.select_running_processes").to_string();
    let add_running_process_label = app.t("editor.add_running_process").to_string();
    let manual_process_name_label = app.t("editor.manual_process_name").to_string();
    let add_manual_process_label = app.t("editor.add_manual_process").to_string();
    let create_and_select_group_label = app.t("editor.create_and_select_group").to_string();
    let save_label = app.t("common.save").to_string();
    let cancel_label = app.t("common.cancel").to_string();
    let name_match_label = app.t("process.name_match").to_string();
    let is_edit = app
        .profile_editor
        .as_ref()
        .and_then(|editor| editor.edit_index)
        .is_some();
    let window_title = if is_edit {
        app.t("editor.edit_title")
    } else {
        app.t("editor.create_title")
    }
    .to_string();

    if let Some(editor) = &mut app.profile_editor {
        egui::Window::new(window_title)
        .collapsible(false)
        .resizable(true)
        .default_width(540.0)
        .show(ctx, |ui| {
            ui.label(&profile_name_label);
            ui.text_edit_singleline(&mut editor.draft.name);

            ui.label(&description_label);
            ui.text_edit_multiline(&mut editor.draft.description);

            ui.separator();
            ui.heading(&groups_heading);
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
                    if ui.small_button(&remove_label).clicked() {
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

            if ui.button(&add_empty_group_label).clicked() {
                editor.draft.groups.push(ProcessGroup {
                    name: new_group_name.clone(),
                    targets: Vec::new(),
                });
                editor.selected_group_idx = Some(editor.draft.groups.len().saturating_sub(1));
            }

            ui.separator();
            ui.heading(&processes_in_group_heading);

            if let Some(group_idx) = editor.selected_group_idx {
                if let Some(group) = editor.draft.groups.get_mut(group_idx) {
                    if let Some(remove_target) =
                        process_targets_with_remove(
                            ui,
                            &group.targets,
                            &remove_label,
                            &name_match_label,
                        )
                    {
                        group.targets.remove(remove_target);
                    }

                    ui.separator();
                    ui.label(&select_running_processes_label);
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
                            egui::Button::new(&add_running_process_label),
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
                    ui.label(&manual_process_name_label);
                    ui.text_edit_singleline(&mut editor.manual_process_name);
                    if ui
                        .add_enabled(
                            !editor.manual_process_name.trim().is_empty(),
                            egui::Button::new(&add_manual_process_label),
                        )
                        .clicked()
                    {
                        let process_name = editor.manual_process_name.trim().to_string();
                        group.targets.push(ProcessTarget {
                            display_name: format!("{} ({})", process_name, name_match_label),
                            process_name,
                            pid: None,
                            manual: true,
                        });
                        editor.manual_process_name.clear();
                    }
                }
            } else {
                ui.label(&create_and_select_group_label);
            }

            ui.separator();
            ui.horizontal(|ui| {
                let can_save = !editor.draft.name.trim().is_empty();
                if ui
                    .add_enabled(can_save, egui::Button::new(&save_label))
                    .clicked()
                {
                    save_profile = true;
                }

                if ui.button(&cancel_label).clicked() {
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
