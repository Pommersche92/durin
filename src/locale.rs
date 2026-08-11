//! Runtime localization loading and locale resolution.
//!
//! Localization files are embedded from `locales/` at build time and loaded
//! directly from memory. Optional file-based overrides can be placed in a
//! `locales/` directory next to the persisted `settings.toml`.

use std::{
    collections::HashMap,
    fs,
    path::Path,
};

use anyhow::{Context, Result};
use rust_embed::RustEmbed;

pub const DEFAULT_LOCALE: &str = "en-GB";

#[derive(RustEmbed)]
#[folder = "locales/"]
struct EmbeddedLocales;

#[derive(Debug, Clone)]
pub struct Localization {
    current_locale: String,
    fallback_locale: String,
    translations: HashMap<String, HashMap<String, String>>,
}

impl Localization {
    pub fn load(locales_override_dir: Option<&Path>, requested_locale: Option<&str>) -> Result<Self> {
        let translations = load_translation_maps(locales_override_dir)?;
        if !translations.contains_key(DEFAULT_LOCALE) {
            anyhow::bail!("Missing embedded fallback locale file: {DEFAULT_LOCALE}.toml");
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

pub fn detect_system_locale() -> Option<String> {
    sys_locale::get_locale()
}

fn load_translation_maps(
    locales_override_dir: Option<&Path>,
) -> Result<HashMap<String, HashMap<String, String>>> {
    let mut translations = load_embedded_translation_maps()?;

    if let Some(override_dir) = locales_override_dir.filter(|path| path.is_dir()) {
        load_override_translation_maps(override_dir, &mut translations)?;
    }

    Ok(translations)
}

fn load_embedded_translation_maps() -> Result<HashMap<String, HashMap<String, String>>> {
    let mut translations = HashMap::new();

    for embedded_path in EmbeddedLocales::iter() {
        let embedded_path = embedded_path.as_ref();
        let embedded_file_path = Path::new(embedded_path);

        if embedded_file_path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }

        let locale_code = match embedded_file_path.file_stem().and_then(|stem| stem.to_str()) {
            Some(code) if !code.trim().is_empty() => code.to_string(),
            _ => continue,
        };

        let content = EmbeddedLocales::get(embedded_path)
            .with_context(|| format!("Failed to read embedded locale file {embedded_path}"))?;
        let content = std::str::from_utf8(content.data.as_ref())
            .with_context(|| format!("Embedded locale file {embedded_path} is not valid UTF-8"))?;

        let map = toml::from_str::<HashMap<String, String>>(content)
            .with_context(|| format!("Failed to parse embedded locale file {embedded_path}"))?;

        translations.insert(locale_code, map);
    }

    Ok(translations)
}

fn load_override_translation_maps(
    locales_override_dir: &Path,
    translations: &mut HashMap<String, HashMap<String, String>>,
) -> Result<()> {
    for entry in fs::read_dir(locales_override_dir).with_context(|| {
        format!(
            "Failed to read override locales directory {}",
            locales_override_dir.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "Failed to inspect an entry in override locales directory {}",
                locales_override_dir.display()
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
            .with_context(|| format!("Failed to read override locale file {}", path.display()))?;

        let map = toml::from_str::<HashMap<String, String>>(&content)
            .with_context(|| format!("Failed to parse override locale file {}", path.display()))?;

        translations.insert(locale_code, map);
    }

    Ok(())
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