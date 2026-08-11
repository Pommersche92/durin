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
    ui.heading("Live RAM Tracking");
    ui.separator();

    if let Some(profile) = app.active_profile() {
        ui.label(format!("Aktives Profil: {}", profile.name));
        ui.label(profile.description.clone());
    } else {
        ui.label("Kein Profil aktiv");
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
    ui.heading("Heap Inspection (Vorbereitung)");
    if app.heap_backend.is_supported() {
        ui.label("Heap-Backend erkannt.");
    } else {
        ui.label("Live Heap Inspection ist vorbereitet, aber noch nicht implementiert.");
    }

    if let Some(message) = &app.status_message {
        ui.separator();
        ui.label(message);
    }
}
