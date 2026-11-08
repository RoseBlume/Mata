use crate::{Args, I18nError, Locale, LocaleError, TextDirection};
use crate::message::Template;
use crate::resource::{parse_json, parse_properties};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

#[derive(Debug)]
struct State {
    locale: Locale,
    default: Locale,
    catalogs: HashMap<Locale, HashMap<String, Arc<Template>>>,
}

/// A cheaply cloneable, thread-safe translation manager.
/// Loaded message templates are parsed once and reused during rendering.
/// Locale switching is shared by clones; no global mutable singleton is used.
#[derive(Debug, Clone)]
pub struct I18n { state: Arc<RwLock<State>> }
impl I18n {
    /// Create an empty translation catalog and establish the default locale.
    pub fn new(default: &str) -> Result<Self, LocaleError> {
        let locale = Locale::parse(default)?;
        Ok(Self { state: Arc::new(RwLock::new(State { locale: locale.clone(), default: locale, catalogs: HashMap::new() })) })
    }
    /// Create a catalog with system-derived initial locale and explicit fallback.
    pub fn with_system_locale(default: &str) -> Result<Self, LocaleError> {
        let manager = Self::new(default)?;
        manager.set_locale(crate::system_locale().as_str())?;
        Ok(manager)
    }
    /// All clones share the active locale; the method does not reload files.
    pub fn set_locale(&self, locale: &str) -> Result<(), LocaleError> {
        self.state.write().unwrap_or_else(|e| e.into_inner()).locale = Locale::parse(locale)?;
        Ok(())
    }
    pub fn locale(&self) -> Locale { self.state.read().unwrap_or_else(|e| e.into_inner()).locale.clone() }
    pub fn default_locale(&self) -> Locale { self.state.read().unwrap_or_else(|e| e.into_inner()).default.clone() }
    pub fn direction(&self) -> TextDirection { self.locale().direction() }
    pub fn available_locales(&self) -> Vec<Locale> {
        let mut locales: Vec<_> = self.state.read().unwrap_or_else(|e| e.into_inner()).catalogs.keys().cloned().collect();
        locales.sort();
        locales
    }
    /// Add/override a subset of translations in `.lang` / `.properties` syntax.
    /// Parsing is atomic: an error leaves all preexisting translations unchanged.
    pub fn add_resource(&self, locale: &str, source: &str) -> Result<usize, I18nError> {
        let locale = Locale::parse(locale)?;
        let resources = parse_properties(source)?;
        self.insert(locale, resources)
    }
    /// Add/override translations from nested-object JSON resources, parsed internally.
    pub fn add_json(&self, locale: &str, source: &str) -> Result<usize, I18nError> {
        let locale = Locale::parse(locale)?;
        let resources = parse_json(source.trim_start_matches('\u{feff}'))?;
        self.insert(locale, resources)
    }
    fn insert(&self, locale: Locale, resources: HashMap<String, String>) -> Result<usize, I18nError> {
        let mut parsed = HashMap::with_capacity(resources.len());
        for (key, value) in resources {
            let template = Template::parse(&value).map_err(|e| I18nError::Message {
                key: key.clone(), position: e.pos, message: e.message,
            })?;
            parsed.insert(key, Arc::new(template));
        }
        let len = parsed.len();
        self.state.write().unwrap_or_else(|e| e.into_inner())
            .catalogs.entry(locale).or_default().extend(parsed);
        Ok(len)
    }
    /// Read `.json`, `.lang`, or `.properties` from a file.
    pub fn load_file(&self, locale: &str, path: impl AsRef<Path>) -> Result<usize, I18nError> {
        let path = path.as_ref();
        let source = fs::read_to_string(path)?;
        match path.extension().and_then(|e| e.to_str()) {
            Some("json") => self.add_json(locale, &source),
            Some("lang" | "properties") => self.add_resource(locale, &source),
            _ => Err(I18nError::Resource { line: 0, message: format!("unsupported resource extension: {}", path.display()) }),
        }
    }
    /// Load all immediate `.lang`, `.properties`, and `.json` children.
    /// Filenames (minus extension) are locale tags, e.g. `en-US.lang`, `fr.json`.
    /// Each file is atomic but the operation is not a directory-wide transaction.
    pub fn load_directory(&self, directory: impl AsRef<Path>) -> Result<usize, I18nError> {
        let mut files = Vec::new();
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_file() && matches!(path.extension().and_then(|ext| ext.to_str()), Some("lang" | "properties" | "json")) {
                files.push(path);
            }
        }
        files.sort();
        let mut sum = 0;
        for path in files {
            let locale = path.file_stem().and_then(|s| s.to_str()).ok_or_else(|| I18nError::Resource {
                line: 0, message: "resource filename must be a UTF-8 locale".into(),
            })?;
            sum += self.load_file(locale, &path)?;
        }
        Ok(sum)
    }
    /// Look up a message in the selected locale, its parents, then the default
    /// locale and its parents. Missing keys and arguments are reported, not hidden.
    pub fn translate(&self, key: &str, args: &Args) -> Result<String, I18nError> {
        let (template, requested_locale) = {
            let state = self.state.read().unwrap_or_else(|e| e.into_inner());
            let requested_locale = state.locale.clone();
            let mut candidates = requested_locale.fallback_chain();
            candidates.extend(state.default.fallback_chain());
            let mut seen = HashSet::new();
            let mut result = None;
            for locale in candidates {
                if !seen.insert(locale.clone()) { continue; }
                if let Some(template) = state.catalogs.get(&locale).and_then(|cat| cat.get(key)) {
                    result = Some(Arc::clone(template));
                    break;
                }
            }
            (result.ok_or_else(|| I18nError::MissingKey(key.into()))?, requested_locale)
        };
        // Plural selection and numeric formatting follow the requested locale,
        // not the language from which the fallback text was retrieved.
        template.format(args, &requested_locale)
    }
    /// Convenience for static UI labels. Missing keys yield the key itself.
    pub fn tr(&self, key: &str) -> String {
        self.translate(key, &Args::new()).unwrap_or_else(|_| key.to_string())
    }
    /// Convenience for dynamic UI labels. Missing keys and args yield the key.
    pub fn tr_with(&self, key: &str, args: &Args) -> String {
        self.translate(key, args).unwrap_or_else(|_| key.to_string())
    }
    pub fn contains(&self, key: &str) -> bool {
        let state = self.state.read().unwrap_or_else(|e| e.into_inner());
        state.locale.fallback_chain().into_iter().chain(state.default.fallback_chain())
            .any(|locale| state.catalogs.get(&locale).is_some_and(|cat| cat.contains_key(key)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn fallback_switch_and_rollback() {
        let i18n = I18n::new("en-US").unwrap();
        i18n.add_resource("en", "title = Hello {name}\ncommon = Always").unwrap();
        i18n.add_resource("fr", "title = Salut {name}").unwrap();
        i18n.set_locale("fr-CA").unwrap();
        assert_eq!(i18n.translate("title", &crate::args! {"name" => "Zoé"}).unwrap(), "Salut Zoé");
        assert_eq!(i18n.tr("common"), "Always");
        assert!(i18n.add_resource("fr", "common = Oui\ninvalid = {name").is_err());
        assert_eq!(i18n.tr("common"), "Always");
        assert_eq!(i18n.tr("unknown"), "unknown");
    }
    #[test] fn shared_clone() {
        let first = I18n::new("en").unwrap();
        let second = first.clone();
        first.set_locale("fr").unwrap();
        assert_eq!(second.locale().as_str(), "fr");
    }
}
