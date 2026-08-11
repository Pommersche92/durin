//! Running process selector control.
//!
//! This control centralizes search/filter/select behavior for process lists so
//! page modules can reuse identical UX semantics without duplicating logic.

use eframe::egui;

use crate::process::RunningProcess;

/// Renders a searchable process list and updates selected process index.
///
/// Parameters:
/// - `running_processes`: source process snapshot,
/// - `process_search`: in/out text buffer for case-insensitive filter,
/// - `selected_running_process`: in/out selected row index,
/// - `max_height`: vertical clamp for scroll region.
///
/// Selection is index-based against the currently displayed backing slice.
pub fn running_process_selector(
    ui: &mut egui::Ui,
    running_processes: &[RunningProcess],
    process_search: &mut String,
    selected_running_process: &mut Option<usize>,
    max_height: f32,
) {
    ui.text_edit_singleline(process_search);

    egui::ScrollArea::vertical().max_height(max_height).show(ui, |ui| {
        for (idx, proc_info) in running_processes
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                process_search.is_empty()
                    || p
                        .name
                        .to_lowercase()
                        .contains(&process_search.to_lowercase())
            })
        {
            let selected = *selected_running_process == Some(idx);
            if ui
                .selectable_label(selected, format!("{} (PID {})", proc_info.name, proc_info.pid))
                .clicked()
            {
                *selected_running_process = Some(idx);
            }
        }
    });
}
