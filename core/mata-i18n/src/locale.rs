use std::env;
use std::fmt;

/// A validated, consistently-cased language tag (common BCP 47 forms).
///
/// This lightweight parser does not validate tags against the IANA registry,
/// nor does it implement BCP 47 extension semantics.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Locale(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleError(pub String);
impl fmt::Display for LocaleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "invalid locale: {}", self.0) }
}
impl std::error::Error for LocaleError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDirection { LeftToRight, RightToLeft }

impl Locale {
    /// Accepts language tags such as `en`, `en-CA`, `zh-Hant-TW`, `sr_Latn_RS`.
    pub fn parse(input: &str) -> Result<Self, LocaleError> {
        let raw = input.trim().replace('_', "-");
        if raw.len() > 120 { return Err(LocaleError("tag exceeds 120 bytes".into())); }
        let parts: Vec<&str> = raw.split('-').collect();
        if parts.is_empty() || !(2..=8).contains(&parts[0].len())
            || !parts[0].bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(LocaleError(input.into()));
        }
        let mut segments = vec![parts[0].to_ascii_lowercase()];
        for &part in &parts[1..] {
            if part.is_empty() || part.len() > 8 || !part.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return Err(LocaleError(input.into()));
            }
            let normalized = if part.len() == 4 && part.bytes().all(|b| b.is_ascii_alphabetic()) {
                let mut chars = part.chars();
                format!("{}{}", chars.next().unwrap().to_ascii_uppercase(), chars.as_str().to_ascii_lowercase())
            } else if (part.len() == 2 && part.bytes().all(|b| b.is_ascii_alphabetic()))
                || (part.len() == 3 && part.bytes().all(|b| b.is_ascii_digit())) {
                part.to_ascii_uppercase()
            } else {
                part.to_ascii_lowercase()
            };
            segments.push(normalized);
        }
        Ok(Self(segments.join("-")))
    }
    pub fn language(&self) -> &str { self.0.split('-').next().unwrap_or("en") }
    pub fn as_str(&self) -> &str { &self.0 }
    pub fn direction(&self) -> TextDirection {
        let script_rtl = self.0.split('-').any(|s| matches!(s, "Arab" | "Hebr" | "Syrc" | "Thaa" | "Nkoo" | "Adlm"));
        if script_rtl || matches!(self.language(), "ar" | "fa" | "he" | "ur" | "ps" | "sd" | "ug" | "yi" | "dv") {
            TextDirection::RightToLeft
        } else { TextDirection::LeftToRight }
    }
    /// From most specific to least specific, excluding any global default.
    pub fn fallback_chain(&self) -> Vec<Self> {
        let mut chain = vec![self.clone()];
        let mut tag = self.0.as_str();
        while let Some(index) = tag.rfind('-') {
            tag = &tag[..index];
            chain.push(Self(tag.to_owned()));
        }
        chain
    }
}
impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
}

/// Find the preferred locale without platform APIs or native libraries.
/// Checks common environment variables and falls back to `en-US`.
/// On systems without those variables, the application should call `set_locale`.
pub fn system_locale() -> Locale {
    for key in ["LC_ALL", "LANGUAGE", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = env::var(key) {
            for item in value.split(':') {
                let first = item.split(['.', '@']).next().unwrap_or("");
                if let Ok(tag) = Locale::parse(first) { return tag; }
            }
        }
    }
    Locale::parse("en-US").expect("hardcoded locale must parse")
}

/// Choose the best supported locale from an HTTP `Accept-Language` header.
/// Weighted values are honored, ties use the original preference ordering.
/// Returns the default if nothing matches. `*` matches the default.
pub fn negotiate(header: &str, supported: &[Locale], default: &Locale) -> Locale {
    let mut weighted = Vec::new();
    for (index, item) in header.split(',').take(64).enumerate() {
        let mut fields = item.trim().split(';');
        let tag = fields.next().unwrap_or("").trim();
        let mut quality = 1.0f32;
        let mut valid = true;
        for f in fields {
            let f = f.trim();
            if let Some(q) = f.strip_prefix("q=") {
                if let Ok(n) = q.trim().parse::<f32>() {
                    if (0.0..=1.0).contains(&n) { quality = n; } else { valid = false; }
                } else { valid = false; }
            }
        }
        if valid && quality > 0.0 { weighted.push((quality, index, tag)); }
    }
    weighted.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (_, _, requested) in weighted {
        if requested == "*" { return default.clone(); }
        if let Ok(requested) = Locale::parse(requested) {
            for locale in requested.fallback_chain() {
                if let Some(found) = supported.iter().find(|s| **s == locale) { return found.clone(); }
            }
            if let Some(found) = supported.iter().find(|s| s.language() == requested.language()) {
                return found.clone();
            }
        }
    }
    default.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn normalization_and_fallback() {
        let locale = Locale::parse("ZH_hANT_tw").unwrap();
        assert_eq!(locale.to_string(), "zh-Hant-TW");
        assert_eq!(locale.fallback_chain().iter().map(Locale::as_str).collect::<Vec<_>>(), ["zh-Hant-TW", "zh-Hant", "zh"]);
        assert!(Locale::parse("en/../../x").is_err());
    }
    #[test] fn direction_and_negotiation() {
        assert_eq!(Locale::parse("ar-EG").unwrap().direction(), TextDirection::RightToLeft);
        let supported = vec![Locale::parse("en-US").unwrap(), Locale::parse("fr").unwrap()];
        assert_eq!(negotiate("en;q=0.3, fr-CA;q=0.9", &supported, &supported[0]).as_str(), "fr");
        assert_eq!(negotiate("fr;q=0", &supported, &supported[0]).as_str(), "en-US");
    }
}
