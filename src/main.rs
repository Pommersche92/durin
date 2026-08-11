#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

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
#[cfg(not(windows))]
use image::ImageReader;
use tracing_subscriber::EnvFilter;

#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection,
            DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ, ReleaseDC, SelectObject,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            DI_NORMAL, DestroyIcon, DrawIconEx, HICON, IMAGE_FLAGS, IMAGE_ICON, LR_DEFAULTSIZE,
            LoadImageW,
        },
    },
    core::PCWSTR,
};

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
            .with_icon(load_app_icon().context("Konnte icon.png nicht als Fenster-Icon laden")?)
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

#[cfg(windows)]
fn load_app_icon() -> anyhow::Result<egui::IconData> {
    const APP_ICON_RESOURCE_ID: u16 = 1;
    const APP_ICON_SIZE: i32 = 256;

    unsafe {
        let module_handle =
            GetModuleHandleW(None).context("Konnte Modul-Handle der Anwendung nicht laden")?;
        let icon_handle = load_icon_resource(module_handle.into(), APP_ICON_RESOURCE_ID, APP_ICON_SIZE)
            .context("Konnte eingebettetes Fenster-Icon nicht aus den Exe-Ressourcen laden")?;

        let icon_data = icon_data_from_hicon(icon_handle, APP_ICON_SIZE as u32, APP_ICON_SIZE as u32)
            .context("Konnte eingebettetes Fenster-Icon nicht in RGBA-Daten umwandeln")?;

        let _ = DestroyIcon(icon_handle);

        Ok(icon_data)
    }
}

#[cfg(not(windows))]
fn load_app_icon() -> anyhow::Result<egui::IconData> {
    let icon_bytes = include_bytes!("../icon.png");
    let image = ImageReader::new(std::io::Cursor::new(icon_bytes))
        .with_guessed_format()
        .context("Konnte PNG-Format nicht erkennen")?
        .decode()
        .context("Konnte icon.png nicht decodieren")?
        .into_rgba8();

    let (width, height) = image.dimensions();

    Ok(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

#[cfg(windows)]
unsafe fn load_icon_resource(
    module_handle: windows::Win32::Foundation::HINSTANCE,
    resource_id: u16,
    size: i32,
) -> anyhow::Result<HICON> {
    let resource_name = PCWSTR(resource_id as usize as *const u16);
    let handle = unsafe {
        LoadImageW(
            Some(module_handle),
            resource_name,
            IMAGE_ICON,
            size,
            size,
            IMAGE_FLAGS(LR_DEFAULTSIZE.0),
        )
    }
    .context("LoadImageW für die eingebettete Icon-Ressource ist fehlgeschlagen")?;

    Ok(HICON(handle.0))
}

#[cfg(windows)]
unsafe fn icon_data_from_hicon(icon: HICON, width: u32, height: u32) -> anyhow::Result<egui::IconData> {
    let screen_dc = unsafe { GetDC(Some(HWND(std::ptr::null_mut()))) };
    anyhow::ensure!(
        screen_dc.0 != std::ptr::null_mut(),
        "GetDC lieferte keinen Device Context"
    );

    let memory_dc = unsafe { CreateCompatibleDC(Some(screen_dc)) };
    if memory_dc.0 == std::ptr::null_mut() {
        let _ = unsafe { ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc) };
        anyhow::bail!("CreateCompatibleDC ist fehlgeschlagen");
    }

    let mut pixels_ptr = std::ptr::null_mut();
    let mut bitmap_info = BITMAPINFO::default();
    bitmap_info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width as i32,
        biHeight: -(height as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };

    let bitmap = unsafe {
        CreateDIBSection(
            Some(memory_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut pixels_ptr,
            None,
            0,
        )
    }
    .context("CreateDIBSection ist fehlgeschlagen")?;

    if pixels_ptr.is_null() {
        let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        let _ = unsafe { DeleteDC(memory_dc) };
        let _ = unsafe { ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc) };
        anyhow::bail!("CreateDIBSection ist fehlgeschlagen");
    }

    let previous_bitmap = unsafe { SelectObject(memory_dc, HGDIOBJ(bitmap.0)) };
    if previous_bitmap.0 == std::ptr::null_mut() {
        let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
        let _ = unsafe { DeleteDC(memory_dc) };
        let _ = unsafe { ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc) };
        anyhow::bail!("SelectObject ist fehlgeschlagen");
    }

    unsafe {
        DrawIconEx(
            memory_dc,
            0,
            0,
            icon,
            width as i32,
            height as i32,
            0,
            None,
            DI_NORMAL,
        )
    }
    .context("DrawIconEx ist fehlgeschlagen")?;

    let pixel_count = (width * height) as usize;
    let bgra_pixels = unsafe { std::slice::from_raw_parts(pixels_ptr as *const u8, pixel_count * 4) };
    let mut rgba_pixels = Vec::with_capacity(pixel_count * 4);

    for chunk in bgra_pixels.chunks_exact(4) {
        rgba_pixels.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
    }

    let _ = unsafe { SelectObject(memory_dc, previous_bitmap) };
    let _ = unsafe { DeleteObject(HGDIOBJ(bitmap.0)) };
    let _ = unsafe { DeleteDC(memory_dc) };
    let _ = unsafe { ReleaseDC(Some(HWND(std::ptr::null_mut())), screen_dc) };

    Ok(egui::IconData {
        rgba: rgba_pixels,
        width,
        height,
    })
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
