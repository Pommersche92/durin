//! Process target listing control with inline remove action.
//!
//! This widget renders all targets in display form and reports which entry,
//! if any, should be removed by the calling page.

use eframe::egui;

use crate::config::ProcessTarget;

/// Renders process targets and returns the index requested for removal.
///
/// The control itself does not mutate the input slice; callers decide when
/// and how to apply removal in their own state management flow.
pub fn process_targets_with_remove(
    ui: &mut egui::Ui,
    targets: &[ProcessTarget],
    remove_label: &str,
    name_match_label: &str,
) -> Option<usize> {
    let mut remove_idx = None;

    for (idx, target) in targets.iter().enumerate() {
        let display_name = if let Some(pid) = target.pid {
            format!("{} (PID {})", target.process_name, pid)
        } else if target.manual {
            format!("{} ({})", target.process_name, name_match_label)
        } else {
            target.display_name.clone()
        };

        ui.horizontal(|ui| {
            ui.label(format!("- {}", display_name));
            if ui.small_button(remove_label).clicked() {
                remove_idx = Some(idx);
            }
        });
    }

    remove_idx
}
