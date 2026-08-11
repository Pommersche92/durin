//! Group and process-target configuration page.
//!
//! This page lets users map tracked processes to logical groups used for
//! RAM aggregation and chart rendering.
//!
//! It combines:
//! - group CRUD interactions,
//! - running-process selection,
//! - manual process-name target entry.

use eframe::egui;

use crate::{
    app::DurinApp,
    config::ProcessGroup,
    ui::controls::{process_list::running_process_selector, target_list::process_targets_with_remove},
};

/// Renders group and process-target management controls.
///
/// The function enforces that a profile must be selected before any group
/// mutation can occur.
pub fn render(app: &mut DurinApp, ui: &mut egui::Ui) {
    ui.heading(app.t("groups.heading"));
    ui.separator();

    if app.selected_profile_idx.is_none() {
        ui.label(app.t("groups.select_profile_first"));
        return;
    }

    render_groups_editor(app, ui);

    ui.separator();
    ui.label(app.t("groups.running_processes"));
    running_process_selector(
        ui,
        &app.running_processes,
        &mut app.process_search,
        &mut app.selected_running_process,
        180.0,
    );

    if ui
        .add_enabled(
            app.selected_running_process.is_some() && app.selected_group_idx.is_some(),
            egui::Button::new(app.t("groups.add_running_process")),
        )
        .clicked()
    {
        app.add_selected_running_process();
    }

    ui.separator();
    ui.label(app.t("groups.add_manual_process"));
    ui.text_edit_singleline(&mut app.manual_process_name);

    if ui
        .add_enabled(
            !app.manual_process_name.trim().is_empty() && app.selected_group_idx.is_some(),
            egui::Button::new(app.t("groups.add_manual_process_button")),
        )
        .clicked()
    {
        app.add_manual_process_name();
    }
}

/// Renders group editor rows and applies group-level mutations.
///
/// Mutations include:
/// - selecting active group,
/// - removing groups,
/// - adding new groups,
/// - removing process targets from groups.
///
/// Persistence is triggered once after batched changes.
fn render_groups_editor(app: &mut DurinApp, ui: &mut egui::Ui) {
    let mut selected_group_idx = app.selected_group_idx;
    let mut remove_group_idx = None;
    let mut add_group = false;
    let mut changed = false;
    let remove_label = app.t("common.remove").to_string();
    let name_match_label = app.t("process.name_match").to_string();
    let add_group_label = app.t("groups.add_group").to_string();

    if let Some(profile_idx) = app.selected_profile_idx {
        let profile = &mut app.settings.profiles[profile_idx];

        for g_idx in 0..profile.groups.len() {
            let group_name = profile.groups[g_idx].name.clone();
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(selected_group_idx == Some(g_idx), group_name)
                    .clicked()
                {
                    selected_group_idx = Some(g_idx);
                }

                if ui.small_button("x").clicked() {
                    remove_group_idx = Some(g_idx);
                }
            });

            if let Some(remove_target_idx) =
                process_targets_with_remove(
                    ui,
                    &profile.groups[g_idx].targets,
                    &remove_label,
                    &name_match_label,
                )
            {
                profile.groups[g_idx].targets.remove(remove_target_idx);
                changed = true;
            }
        }

        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut app.new_group_name);
            if ui
                .add_enabled(
                    !app.new_group_name.trim().is_empty(),
                    egui::Button::new(&add_group_label),
                )
                .clicked()
            {
                add_group = true;
            }
        });

        if add_group {
            profile.groups.push(ProcessGroup {
                name: app.new_group_name.trim().to_string(),
                targets: Vec::new(),
            });
            app.new_group_name.clear();
            selected_group_idx = Some(profile.groups.len().saturating_sub(1));
            changed = true;
        }

        if let Some(idx) = remove_group_idx {
            if idx < profile.groups.len() {
                profile.groups.remove(idx);
                if profile.groups.is_empty() {
                    selected_group_idx = None;
                } else if idx == 0 {
                    selected_group_idx = Some(0);
                } else {
                    selected_group_idx = Some(idx - 1);
                }
                changed = true;
            }
        }
    }

    app.selected_group_idx = selected_group_idx;
    if changed {
        app.save_settings();
    }
}
