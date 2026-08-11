//! Application entry point and runtime bootstrap for Durin.
//!
//! This module is intentionally small and focused on orchestration:
//! it initializes logging, loads persisted configuration from `settings.toml`,
//! configures the eframe native viewport options, and starts the UI app.
//!
//! Architectural rationale:
//! - Keep process setup concerns separate from UI and domain logic.
//! - Fail fast with contextual error messages while still providing actionable output.
//! - Preserve a deterministic startup path for future features such as
//!   privilege checks (heap inspection), backend selection, and telemetry.

mod app;
mod config;
mod heap;
mod locale;
mod process;
mod tracking;
mod ui;

use anyhow::Context;
use eframe::{egui, egui_wgpu, wgpu};
use tracing_subscriber::EnvFilter;

use crate::app::DurinApp;

/// Starts the Durin desktop application and returns a fallible result.
///
/// Workflow:
/// 1. Configure tracing/logging.
/// 2. Load or create persisted settings (`settings.toml`).
/// 3. Build native viewport options for the overlay window.
/// 4. Launch eframe with the application state.
///
/// The function uses `anyhow::Context` at every major boundary so that
/// startup failures are understandable when propagated to users or logs.
fn main() -> anyhow::Result<()> {
    init_tracing();

    let settings_path = config::settings_path();
    let settings = config::Settings::load_or_default(&settings_path)
        .with_context(|| format!("Konnte {} nicht laden", settings_path.display()))?;

    let mut native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Durin Overlay")
            .with_always_on_top()
            .with_transparent(true)
            .with_decorations(true)
            .with_inner_size([1040.0, 720.0]),
        ..Default::default()
    };

    #[cfg(windows)]
    {
        if let egui_wgpu::WgpuSetup::CreateNew(create_new) = &mut native_options.wgpu_options.wgpu_setup
        {
            // Avoid fragile Vulkan startup paths on older Intel Windows drivers.
            create_new.instance_descriptor.backends = wgpu::Backends::DX12 | wgpu::Backends::GL;
        }
    }

    eframe::run_native(
        "Durin",
        native_options,
        Box::new(move |_cc| Ok(Box::new(DurinApp::new(settings, settings_path)))),
    )
    .context("eframe konnte nicht gestartet werden")?;

    Ok(())
}

/// Initializes global tracing subscriber configuration.
///
/// The log filter is read from the environment first (for reproducible
/// diagnostics in CI/dev shells), and falls back to an `info` baseline.
///
/// This function is intentionally idempotent in current usage (called once
/// from `main`) and centralizes future logging policy updates.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
