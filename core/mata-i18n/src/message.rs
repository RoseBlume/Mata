use std::collections::HashMap;
use crate::{I18nError, Locale};

/// A typed substitution value. Numeric arguments are required for plural forms.
#[derive(Debug, Clone, PartialEq)]
pub enum Value { Text(String), Integer(i64), Decimal(f64), Boolean(bool) }
impl Value {
    fn as_number(&self) -> Option<f64> {
        match self { Self::Integer(n) => Some(*n as f64), Self::Decimal(n) if n.is_finite() => Some(*n), _ => None }
    }
    fn display(&self, locale: &Locale) -> String {
        match self {
            Self::Text(text) => text.clone(),
            Self::Integer(number) => crate::format_integer(locale, *number),
            Self::Decimal(number) => {
                if !number.is_finite() { return number.to_string(); }
                let s = number.to_string();
                if s.contains('e') || s.contains('E') { s } else { crate::format::format_decimal(locale, &s) }
            }
            Self::Boolean(b) => b.to_string(),
        }
    }
}
impl From<&str> for Value { fn from(v: &str) -> Self { Self::Text(v.into()) } }
impl From<String> for Value { fn from(v: String) -> Self { Self::Text(v) } }
impl From<&String> for Value { fn from(v: &String) -> Self { Self::Text(v.clone()) } }
impl From<bool> for Value { fn from(v: bool) -> Self { Self::Boolean(v) } }
macro_rules! into_int { ($($t:ty),*) => { $(impl From<$t> for Value { fn from(v: $t) -> Self { Self::Integer(v as i64) } })* }; }
into_int!(i8, i16, i32, i64, u8, u16, u32);
impl From<usize> for Value { fn from(v: usize) -> Self { Self::Integer(i64::try_from(v).unwrap_or(i64::MAX)) } }
impl From<u64> for Value { fn from(v: u64) -> Self { Self::Integer(i64::try_from(v).unwrap_or(i64::MAX)) } }
impl From<isize> for Value { fn from(v: isize) -> Self { Self::Integer(i64::try_from(v).unwrap_or(if v < 0 { i64::MIN } else { i64::MAX })) } }
impl From<f32> for Value { fn from(v: f32) -> Self { Self::Decimal(v as f64) } }
impl From<f64> for Value { fn from(v: f64) -> Self { Self::Decimal(v) } }
pub type Args = HashMap<String, Value>;

#[macro_export]
macro_rules! args {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = $crate::Args::new();
        $(map.insert(($key).to_string(), $crate::Value::from($value));)*
        map
    }};
}

#[derive(Debug, Clone)]
pub(crate) enum Node {
    Text(String), Argument(String),
    Plural { variable: String, offset: f64, forms: Vec<(String, Vec<Node>)> },
    Select { variable: String, forms: Vec<(String, Vec<Node>)> },
}
#[derive(Debug, Clone)]
pub(crate) struct Template(pub(crate) Vec<Node>);
#[derive(Debug)]
pub(crate) struct ParseError { pub(crate) pos: usize, pub(crate) message: String }

struct Parser<'a> { s: &'a str, i: usize }
impl<'a> Parser<'a> {
    fn remaining(&self) -> &'a str { &self.s[self.i..] }
    fn peek(&self) -> Option<char> { self.remaining().chars().next() }
    fn bump(&mut self) -> Option<char> { let c = self.peek()?; self.i += c.len_utf8(); Some(c) }
    fn skip_ws(&mut self) { while self.peek().is_some_and(char::is_whitespace) { self.bump(); } }
    fn error(&self, message: impl Into<String>) -> ParseError { ParseError { pos: self.i, message: message.into() } }
    fn ident(&mut self) -> Result<String, ParseError> {
        self.skip_ws();
        let start = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')) { self.bump(); }
        if self.i == start { Err(self.error("expected a variable or selector name")) }
        else { Ok(self.s[start..self.i].to_string()) }
    }
    fn nodes(&mut self, in_branch: bool, depth: usize) -> Result<Vec<Node>, ParseError> {
        if depth > 24 { return Err(self.error("message nesting exceeds 24 levels")); }
        let mut result = Vec::new();
        let mut literal = String::new();
        while let Some(c) = self.peek() {
            if self.remaining().starts_with("\\{") { self.i += 2; literal.push('{'); continue; }
            if self.remaining().starts_with("\\}") { self.i += 2; literal.push('}'); continue; }
            if self.remaining().starts_with("{{") { self.i += 2; literal.push('{'); continue; }
            if c == '}' {
                if in_branch { break; }
                if self.remaining().starts_with("}}") { self.i += 2; literal.push('}'); continue; }
                return Err(self.error("unexpected closing brace; use '}}' for a literal brace"));
            }
            if c == '{' {
                if !literal.is_empty() { result.push(Node::Text(std::mem::take(&mut literal))); }
                result.push(self.placeholder(depth)?);
            } else { literal.push(c); self.bump(); }
        }
        if !literal.is_empty() { result.push(Node::Text(literal)); }
        Ok(result)
    }
    fn placeholder(&mut self, depth: usize) -> Result<Node, ParseError> {
        self.bump(); // '{'
        let variable = self.ident()?;
        self.skip_ws();
        match self.bump() {
            Some('}') => Ok(Node::Argument(variable)),
            Some(',') => {
                let kind = self.ident()?;
                self.skip_ws();
                if self.bump() != Some(',') { return Err(self.error("expected ',' after plural/select")); }
                let mut offset = 0.0;
                self.skip_ws();
                if kind == "plural" && self.remaining().starts_with("offset:") {
                    self.i += "offset:".len();
                    self.skip_ws();
                    let from = self.i;
                    while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '-' || c == '.') { self.bump(); }
                    offset = self.s[from..self.i].parse::<f64>().map_err(|_| self.error("invalid plural offset"))?;
                    if !offset.is_finite() { return Err(self.error("non-finite offset")); }
                }
                if kind != "plural" && kind != "select" { return Err(self.error(format!("unknown message kind '{kind}'"))); }
                let mut forms = Vec::<(String, Vec<Node>)>::new();
                loop {
                    self.skip_ws();
                    if self.peek() == Some('}') { self.bump(); break; }
                    let selector = if self.peek() == Some('=') {
                        if kind != "plural" { return Err(self.error("numeric selectors are only valid for plurals")); }
                        self.bump();
                        let start = self.i;
                        if self.peek() == Some('-') { self.bump(); }
                        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '.') { self.bump(); }
                        if self.s[start..self.i].parse::<f64>().is_err() { return Err(self.error("invalid exact-number plural selector")); }
                        format!("={}", &self.s[start..self.i])
                    } else { self.ident()? };
                    if forms.iter().any(|(s, _)| s == &selector) { return Err(self.error(format!("duplicate selector '{selector}'"))); }
                    self.skip_ws();
                    if self.bump() != Some('{') { return Err(self.error("expected '{' after selector")); }
                    let branch = self.nodes(true, depth + 1)?;
                    if self.bump() != Some('}') { return Err(self.error("unterminated selector body")); }
                    forms.push((selector, branch));
                }
                if !forms.iter().any(|(name, _)| name.as_str() == "other") { return Err(self.error("plural/select must have an 'other' branch")); }
                if kind == "plural" { Ok(Node::Plural { variable, offset, forms }) }
                else { Ok(Node::Select { variable, forms }) }
            }
            _ => Err(self.error("expected '}' or ',' after variable")),
        }
    }
}

impl Template {
    pub(crate) fn parse(s: &str) -> Result<Self, ParseError> {
        if s.len() > 1024 * 1024 { return Err(ParseError { pos: 0, message: "message too long".into() }); }
        let mut parser = Parser { s, i: 0 };
        let nodes = parser.nodes(false, 0)?;
        Ok(Self(nodes))
    }
    pub(crate) fn format(&self, args: &Args, locale: &Locale) -> Result<String, I18nError> {
        let mut out = String::new();
        render(&self.0, &mut out, args, locale, None)?;
        Ok(out)
    }
}

fn render(nodes: &[Node], out: &mut String, args: &Args, locale: &Locale, pound: Option<f64>) -> Result<(), I18nError> {
    for node in nodes {
        match node {
            Node::Text(text) => {
                if let Some(value) = pound {
                    let replacement = if value.fract() == 0.0 && value.abs() <= i64::MAX as f64 {
                        crate::format_integer(locale, value as i64)
                    } else {
                        let plain = value.to_string();
                        if plain.contains('e') || plain.contains('E') { plain }
                        else { crate::format::format_decimal(locale, &plain) }
                    };
                    let mut chars = text.chars().peekable();
                    while let Some(ch) = chars.next() {
                        if ch == '#' {
                            if chars.peek() == Some(&'#') { chars.next(); out.push('#'); }
                            else { out.push_str(&replacement); }
                        } else { out.push(ch); }
                    }
                } else { out.push_str(text); }
            }
            Node::Argument(key) => {
                let val = args.get(key).ok_or_else(|| I18nError::MissingArgument(key.clone()))?;
                out.push_str(&val.display(locale));
            }
            Node::Plural { variable, offset, forms } => {
                let value = args.get(variable).ok_or_else(|| I18nError::MissingArgument(variable.clone()))?;
                let number = value.as_number().ok_or_else(|| I18nError::InvalidArgument { name: variable.clone(), message: "plural arguments must be finite numbers".into() })?;
                let exact = forms.iter().find(|(s, _)| s.strip_prefix('=').and_then(|v| v.parse::<f64>().ok()).is_some_and(|v| v == number));
                let adjusted = number - offset;
                let category = plural_category(locale, adjusted);
                let body = exact.or_else(|| forms.iter().find(|(s, _)| s.as_str() == category))
                    .or_else(|| forms.iter().find(|(s, _)| s.as_str() == "other"));
                if let Some((_, nodes)) = body { render(nodes, out, args, locale, Some(adjusted))?; }
            }
            Node::Select { variable, forms } => {
                let value = args.get(variable).ok_or_else(|| I18nError::MissingArgument(variable.clone()))?;
                let requested = value.display(locale);
                let body = forms.iter().find(|(s, _)| s.as_str() == requested)
                    .or_else(|| forms.iter().find(|(s, _)| s.as_str() == "other"));
                if let Some((_, nodes)) = body { render(nodes, out, args, locale, pound)?; }
            }
        }
    }
    Ok(())
}

fn plural_category(locale: &Locale, value: f64) -> &'static str {
    let n = value.abs();
    let whole = n.fract() == 0.0;
    let i = n as u64;
    let mod10 = i % 10;
    let mod100 = i % 100;
    match locale.language() {
        // Arabic zero, one, two, few, many, other.
        "ar" if n == 0.0 => "zero",
        "ar" if n == 1.0 => "one",
        "ar" if n == 2.0 => "two",
        "ar" if whole && (3..=10).contains(&mod100) => "few",
        "ar" if whole && (11..=99).contains(&mod100) => "many",
        "ar" => "other",
        // East Slavic rules for integer counts; non-integer is other.
        "ru" | "uk" | "be" if whole && mod10 == 1 && mod100 != 11 => "one",
        "ru" | "uk" | "be" if whole && (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) => "few",
        "ru" | "uk" | "be" if whole && (mod10 == 0 || (5..=9).contains(&mod10) || (11..=14).contains(&mod100)) => "many",
        "ru" | "uk" | "be" => "other",
        "pl" if n == 1.0 => "one",
        "pl" if whole && (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) => "few",
        "pl" if whole && (i != 1 && (mod10 <= 1 || (5..=9).contains(&mod10) || (12..=14).contains(&mod100))) => "many",
        "pl" => "other",
        "cs" | "sk" if n == 1.0 => "one",
        "cs" | "sk" if whole && (2.0..=4.0).contains(&n) => "few",
        "cs" | "sk" if !whole => "many",
        "cs" | "sk" => "other",
        "sl" if whole && i % 100 == 1 => "one",
        "sl" if whole && i % 100 == 2 => "two",
        "sl" if !whole || (3..=4).contains(&(i % 100)) => "few",
        "sl" => "other",
        "fr" | "pt" if n < 2.0 => "one",
        "fr" | "pt" => "other",
        "ja" | "ko" | "zh" | "vi" | "th" | "id" => "other",
        _ if n == 1.0 => "one",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn escaping_plural_and_select() {
        let msg = Template::parse("{{{name}}}: {count, plural, =0 {no items} one {# item} other {# items}}; {gender, select, f {she} other {they}}!").unwrap();
        let en = Locale::parse("en-US").unwrap();
        assert_eq!(msg.format(&crate::args! {"name" => "Ada", "count" => 2, "gender" => "f"}, &en).unwrap(), "{Ada}: 2 items; she!");
        assert_eq!(msg.format(&crate::args! {"name" => "Ada", "count" => 0, "gender" => "m"}, &en).unwrap(), "{Ada}: no items; they!");
    }
    #[test] fn languages() {
        let r = Locale::parse("ru").unwrap();
        assert_eq!(plural_category(&r, 1.0), "one");
        assert_eq!(plural_category(&r, 2.0), "few");
        assert_eq!(plural_category(&r, 5.0), "many");
        assert_eq!(plural_category(&r, 1.5), "other");
        let ar = Locale::parse("ar").unwrap();
        assert_eq!(plural_category(&ar, 0.0), "zero");
        assert_eq!(plural_category(&ar, 7.0), "few");
    }
    #[test] fn rejects_malformed() {
        assert!(Template::parse("hello {name").is_err());
        assert!(Template::parse("{n, plural, one {one}}").is_err());
        assert!(Template::parse("{s, select, other {x} other {y}}").is_err());
    }
}
