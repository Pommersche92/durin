#[cfg(windows)]
fn main() {
    use std::{env, path::PathBuf};

    println!("cargo:rerun-if-changed=icon.png");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR fehlt"));
    let icon_png = PathBuf::from("icon.png");
    let icon_ico = out_dir.join("durin-icon.ico");

    image::open(&icon_png)
        .expect("icon.png konnte nicht geladen werden")
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
        .save_with_format(&icon_ico, image::ImageFormat::Ico)
        .expect("ICO-Datei konnte nicht erzeugt werden");

    winres::WindowsResource::new()
        .set_icon(icon_ico.to_string_lossy().as_ref())
        .compile()
        .expect("Windows-Ressource konnte nicht kompiliert werden");
}

#[cfg(not(windows))]
fn main() {}