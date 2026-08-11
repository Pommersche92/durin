//! Live tracking visualization page.
//!
//! This page presents:
//! - active profile metadata,
//! - RAM time-series chart per configured group,
//! - current heap inspection readiness state,
//! - last persistence status message.

use eframe::egui;
use egui_plot::{Legend, Line, Plot};

use crate::app::DurinApp;

/// Renders the central tracking page and chart content.
///
/// Chart lines are derived from the tracker iterator and rendered with
/// deterministic labels equal to configured group names.
pub fn render(app: &mut DurinApp, ui: &mut egui::Ui) {
    ui.heading(app.t("tracking.heading"));
    ui.separator();

    if let Some(profile) = app.active_profile() {
        ui.label(format!("{}: {}", app.t("tracking.active_profile"), profile.name));
        ui.label(profile.description.clone());
    } else {
        ui.label(app.t("tracking.no_profile"));
    }

    ui.separator();
    Plot::new("ram_plot")
        .legend(Legend::default())
        .height(280.0)
        .show(ui, |plot_ui| {
            for (group_name, series) in app.ram_tracker.group_series() {
                let points: Vec<[f64; 2]> = series.iter().map(|p| [p.t_sec, p.value_mib]).collect();
                let line = Line::new(group_name.to_string(), points);
                plot_ui.line(line);
            }
        });

    ui.separator();
    ui.heading(app.t("tracking.heap_heading"));
    if app.heap_backend.is_supported() {
        ui.label(app.t("tracking.heap_backend_ready"));
    } else {
        ui.label(app.t("tracking.heap_not_implemented"));
    }

    if let Some(message) = &app.status_message {
        ui.separator();
        ui.label(message);
    }
}
