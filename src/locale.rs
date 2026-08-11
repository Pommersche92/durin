//! Runtime localization loading and locale resolution.
//!
//! Localization files are plain TOML key-value maps stored in `locales/`
//! and named by language tags such as `de-DE.toml`.

use std::{
    collections::HashMap,
    env,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

pub const DEFAULT_LOCALE: &str = "en-GB";

#[derive(Debug, Clone)]
pub struct Localization {
    current_locale: String,
    fallback_locale: String,
    translations: HashMap<String, HashMap<String, String>>,
}

impl Localization {
    pub fn load(locales_dir: &Path, requested_locale: Option<&str>) -> Result<Self> {
        let translations = load_translation_maps(locales_dir)?;
        if !translations.contains_key(DEFAULT_LOCALE) {
            anyhow::bail!(
                "Missing fallback locale file: {}",
                locales_dir.join(format!("{DEFAULT_LOCALE}.toml")).display()
            );
        }

        let resolved_locale = resolve_locale(requested_locale, &translations)
            .unwrap_or_else(|| DEFAULT_LOCALE.to_string());

        Ok(Self {
            current_locale: resolved_locale,
            fallback_locale: DEFAULT_LOCALE.to_string(),
            translations,
        })
    }

    pub fn current_locale(&self) -> &str {
        &self.current_locale
    }

    pub fn available_locales(&self) -> Vec<&str> {
        let mut locales: Vec<&str> = self.translations.keys().map(String::as_str).collect();
        locales.sort_unstable();
        locales
    }

    pub fn set_locale(&mut self, locale: &str) -> bool {
        if let Some(resolved) = resolve_locale(Some(locale), &self.translations) {
            let changed = self.current_locale != resolved;
            self.current_locale = resolved;
            return changed;
        }

        false
    }

    pub fn resolve_supported_locale(&self, locale: Option<&str>) -> String {
        resolve_locale(locale, &self.translations).unwrap_or_else(|| self.fallback_locale.clone())
    }

    pub fn text<'a>(&'a self, key: &'a str) -> &'a str {
        self.translations
            .get(&self.current_locale)
            .and_then(|map| map.get(key))
            .or_else(|| {
                self.translations
                    .get(&self.fallback_locale)
                    .and_then(|map| map.get(key))
            })
            .map(String::as_str)
            .unwrap_or(key)
    }
}

pub fn locales_path() -> PathBuf {
    candidate_locales_paths()
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("locales"))
}

pub fn detect_system_locale() -> Option<String> {
    sys_locale::get_locale()
}

fn load_translation_maps(locales_dir: &Path) -> Result<HashMap<String, HashMap<String, String>>> {
    let mut translations = HashMap::new();

    for entry in fs::read_dir(locales_dir)
        .with_context(|| format!("Failed to read locales directory {}", locales_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "Failed to inspect an entry in locales directory {}",
                locales_dir.display()
            )
        })?;
        let path = entry.path();

        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }

        let locale_code = match path.file_stem().and_then(|stem| stem.to_str()) {
            Some(code) if !code.trim().is_empty() => code.to_string(),
            _ => continue,
        };

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read locale file {}", path.display()))?;

        let map = toml::from_str::<HashMap<String, String>>(&content)
            .with_context(|| format!("Failed to parse locale file {}", path.display()))?;

        translations.insert(locale_code, map);
    }

    Ok(translations)
}

fn resolve_locale(
    requested_locale: Option<&str>,
    translations: &HashMap<String, HashMap<String, String>>,
) -> Option<String> {
    let requested = requested_locale?.trim();
    if requested.is_empty() {
        return None;
    }

    if translations.contains_key(requested) {
        return Some(requested.to_string());
    }

    let normalized_requested = requested.replace('_', "-");
    if translations.contains_key(&normalized_requested) {
        return Some(normalized_requested);
    }

    let language = normalized_requested.split('-').next()?;
    if let Some((locale, _)) = translations
        .iter()
        .find(|(locale, _)| locale.eq_ignore_ascii_case(language))
    {
        return Some(locale.clone());
    }

    translations
        .keys()
        .find(|locale| {
            locale
                .split('-')
                .next()
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(language))
        })
        .cloned()
}

fn candidate_locales_paths() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("locales"));
            if let Some(parent) = exe_dir.parent() {
                candidates.push(parent.join("locales"));
                if let Some(grandparent) = parent.parent() {
                    candidates.push(grandparent.join("locales"));
                }
            }
        }
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("locales"));
    }

    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales"));

    dedupe_paths(candidates)
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique = Vec::new();

    for path in paths {
        if unique.iter().any(|existing| existing == &path) {
            continue;
        }
        unique.push(path);
    }

    unique
}