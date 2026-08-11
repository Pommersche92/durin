//! Persistent settings and profile configuration model.
//!
//! This module defines the durable configuration schema stored in
//! `settings.toml` and provides load/save helpers with contextual errors.
//!
//! Key goals of the schema:
//! - Human-editable and source-control-friendly TOML structure.
//! - Separation between UI state (`overlay_visible`, active profile) and
//!   profile composition data (groups and process targets).
//! - Forward compatibility for later expansion (heap inspection options,
//!   per-profile sampling intervals, export controls).

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Root persisted application settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub overlay_visible: bool,
    pub hotkey: String,
    pub ui_language: Option<String>,
    pub active_profile: Option<String>,
    pub profiles: Vec<Profile>,
}

/// Named profile that groups process target collections.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub groups: Vec<ProcessGroup>,
}

/// Logical process grouping used for aggregated RAM chart lines.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessGroup {
    pub name: String,
    pub targets: Vec<ProcessTarget>,
}

/// A process target reference stored in profile groups.
///
/// Target matching can happen either by exact PID (runtime-specific) or
/// by process-name match (future-friendly/manual entry).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProcessTarget {
    pub display_name: String,
    pub process_name: String,
    pub pid: Option<u32>,
    pub manual: bool,
}

impl Default for Settings {
    /// Builds default in-memory settings used for first launch.
    fn default() -> Self {
        Self {
            overlay_visible: true,
            hotkey: "Ctrl+Shift+R".to_string(),
            ui_language: None,
            active_profile: None,
            profiles: Vec::new(),
        }
    }
}

impl Settings {
    /// Loads settings from disk or creates a new default file if absent.
    ///
    /// This function establishes the expected startup behavior:
    /// a missing config file is not treated as an error; instead, defaults
    /// are persisted immediately so users can inspect/edit the generated file.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            let settings = Self::default();
            settings.save(path)?;
            return Ok(settings);
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Konnte Datei {} nicht lesen", path.display()))?;

        let settings = toml::from_str::<Self>(&content)
            .with_context(|| format!("Konnte TOML in {} nicht parsen", path.display()))?;

        Ok(settings)
    }

    /// Persists settings as pretty TOML to the provided path.
    ///
    /// Pretty serialization is intentionally used to keep manual edits easy
    /// and diff-friendly for debugging or synchronization use cases.
    pub fn save(&self, path: &Path) -> Result<()> {
        let serialized = toml::to_string_pretty(self).context("Konnte Settings nicht serialisieren")?;
        fs::write(path, serialized)
            .with_context(|| format!("Konnte Datei {} nicht schreiben", path.display()))
    }

    /// Resolves the active profile name to its current index.
    ///
    /// Returns `None` when no active profile exists or the configured name
    /// no longer matches an entry (e.g., after manual edits).
    pub fn active_profile_index(&self) -> Option<usize> {
        self.active_profile
            .as_ref()
            .and_then(|name| self.profiles.iter().position(|p| &p.name == name))
    }

    /// Updates `active_profile` by selecting an index from `profiles`.
    ///
    /// Passing `None` clears the active profile name.
    pub fn set_active_profile_by_index(&mut self, index: Option<usize>) {
        self.active_profile = index.and_then(|i| self.profiles.get(i)).map(|p| p.name.clone());
    }
}

/// Returns the canonical settings file path used by the application.
///
/// Kept as a function so path policy can later evolve (e.g., XDG/AppData)
/// without touching call-sites.
pub fn settings_path() -> PathBuf {
    PathBuf::from("settings.toml")
}
