//! Live tracking visualization page.
//!
//! This page presents:
//! - active profile metadata,
//! - RAM time-series chart per configured group,
//! - current heap inspection readiness state,
//! - last persistence status message.

use eframe::egui;
use egui_plot::{GridMark, HoverPosition, Legend, Line, Plot};

use crate::app::DurinApp;

/// Renders the central tracking page and chart content.
///
/// Chart lines are derived from the tracker iterator and rendered with
/// deterministic labels equal to configured group names.
pub fn render(app: &mut DurinApp, ui: &mut egui::Ui) {
    let base_opacity = 1.0;
    let bg_alpha = (base_opacity * 255.0) as u8;
    let bg = egui::Color32::from_rgba_unmultiplied(14, 18, 28, bg_alpha);
    ui.painter().rect_filled(ui.max_rect(), 0.0, bg);

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
        .x_axis_formatter(|mark: GridMark, _x_range| format_duration_compact(mark.value))
        .y_axis_formatter(|mark: GridMark, y_range| {
            let unit = choose_memory_unit_for_range(y_range);
            format_memory_value_from_mib(mark.value, unit)
        })
        .label_formatter(|hover_pos: &HoverPosition<'_>| {
            Some(match hover_pos {
                HoverPosition::NearDataPoint {
                    plot_name,
                    position,
                    index: _,
                } => format!(
                    "{}\nt = {}\nRAM = {}",
                    plot_name,
                    format_duration_compact(position.x),
                    format_memory_value_adaptive(position.y)
                ),
                HoverPosition::Elsewhere { position } => format!(
                    "t = {}\nRAM = {}",
                    format_duration_compact(position.x),
                    format_memory_value_adaptive(position.y)
                ),
            })
        })
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

#[derive(Clone, Copy)]
enum MemoryUnit {
    B,
    KiB,
    MiB,
    GiB,
    TiB,
}

fn choose_memory_unit_for_range(y_range: &std::ops::RangeInclusive<f64>) -> MemoryUnit {
    let max_abs_mib = y_range.start().abs().max(y_range.end().abs());

    if max_abs_mib >= 1024.0 * 1024.0 {
        MemoryUnit::TiB
    } else if max_abs_mib >= 1024.0 {
        MemoryUnit::GiB
    } else if max_abs_mib >= 1.0 {
        MemoryUnit::MiB
    } else if max_abs_mib >= (1.0 / 1024.0) {
        MemoryUnit::KiB
    } else {
        MemoryUnit::B
    }
}

fn format_memory_value_adaptive(value_mib: f64) -> String {
    let unit = choose_memory_unit_for_range(&(0.0..=value_mib.abs()));
    format_memory_value_from_mib(value_mib, unit)
}

fn format_memory_value_from_mib(value_mib: f64, unit: MemoryUnit) -> String {
    let (value, suffix) = match unit {
        MemoryUnit::B => (value_mib * 1024.0 * 1024.0, "B"),
        MemoryUnit::KiB => (value_mib * 1024.0, "KB"),
        MemoryUnit::MiB => (value_mib, "MB"),
        MemoryUnit::GiB => (value_mib / 1024.0, "GB"),
        MemoryUnit::TiB => (value_mib / (1024.0 * 1024.0), "TB"),
    };

    let decimals = if value.abs() >= 100.0 {
        0
    } else if value.abs() >= 10.0 {
        1
    } else {
        2
    };

    format!("{:.*} {}", decimals, value, suffix)
}

fn format_duration_compact(seconds: f64) -> String {
    let sign = if seconds.is_sign_negative() { "-" } else { "" };
    let total = seconds.abs();

    let hours = (total / 3600.0).floor() as u64;
    let minutes = ((total % 3600.0) / 60.0).floor() as u64;
    let secs = (total % 60.0).round() as u64;

    if hours > 0 {
        format!("{sign}{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{sign}{minutes}m {secs}s")
    } else if total >= 10.0 {
        format!("{sign}{:.0}s", total)
    } else {
        format!("{sign}{:.1}s", total)
    }
}
