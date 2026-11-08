use crate::I18nError;
use std::collections::HashMap;

/// Parse a UTF-8 `.lang`/`.properties` catalog.
///
/// Example: `menu.save = Save` or `files = {n, plural, one {# file} other {# files}}`.
/// The first unescaped equals sign separates key and value. Duplicates error.
pub(crate) fn parse_properties(source: &str) -> Result<HashMap<String, String>, I18nError> {
    let mut entries = HashMap::new();
    let mut lines = source.trim_start_matches('\u{feff}').lines().enumerate();
    while let Some((index, line)) = lines.next() {
        let line_number = index + 1;
        let mut logical = line.trim_end_matches('\r').to_string();
        // A trailing odd number of backslashes joins the next line.
        while logical.bytes().rev().take_while(|b| *b == b'\\').count() % 2 == 1 {
            logical.pop();
            if let Some((_, next)) = lines.next() { logical.push_str(next.trim_start()); }
            else { return Err(I18nError::Resource { line: line_number, message: "continuation without a next line".into() }); }
        }
        let line = logical.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') { continue; }
        let mut escaped = false;
        let mut divider = None;
        for (i, c) in line.char_indices() {
            if c == '=' && !escaped { divider = Some(i); break; }
            if c == '\\' && !escaped { escaped = true; } else { escaped = false; }
        }
        let i = divider.ok_or_else(|| I18nError::Resource { line: line_number, message: "expected 'key = value'".into() })?;
        let key = unescape(line[..i].trim(), line_number)?;
        if key.is_empty() || key.len() > 256 || !key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
            return Err(I18nError::Resource { line: line_number, message: format!("invalid key '{key}'") });
        }
        let value = unescape(line[i + 1..].trim(), line_number)?;
        if entries.insert(key.clone(), value).is_some() {
            return Err(I18nError::Resource { line: line_number, message: format!("duplicate key '{key}'") });
        }
    }
    Ok(entries)
}
fn unescape(input: &str, line: usize) -> Result<String, I18nError> {
    let mut chars = input.chars().peekable();
    let mut out = String::with_capacity(input.len());
    while let Some(c) = chars.next() {
        if c != '\\' { out.push(c); continue; }
        let escaped = chars.next().ok_or_else(|| I18nError::Resource { line, message: "unfinished escape".into() })?;
        match escaped {
            'n' => out.push('\n'), 't' => out.push('\t'), 'r' => out.push('\r'),
            '\\' => out.push('\\'), '=' => out.push('='), '#' => out.push('#'),
            '{' => { out.push('\\'); out.push('{'); },
            '}' => { out.push('\\'); out.push('}'); },
            'u' if chars.peek() == Some(&'{') => {
                chars.next();
                let mut hex = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '}' { break; }
                    hex.push(c); chars.next();
                    if hex.len() > 6 { break; }
                }
                if chars.next() != Some('}') { return Err(I18nError::Resource { line, message: "invalid unicode escape".into() }); }
                let code = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                    .ok_or_else(|| I18nError::Resource { line, message: "invalid unicode scalar".into() })?;
                out.push(code);
            }
            _ => return Err(I18nError::Resource { line, message: format!("unsupported escape '\\{escaped}'") }),
        }
    }
    Ok(out)
}

/// Parse a nested JSON string-to-string dictionary, flattened to dotted keys.
/// Arrays, booleans and numbers are intentionally disallowed in catalogs.
pub(crate) fn parse_json(source: &str) -> Result<HashMap<String, String>, I18nError> {
    let mut p = JsonParser { source, pos: 0 };
    let mut result = HashMap::new();
    p.object("", &mut result, 0)?;
    p.ws();
    if p.pos != source.len() { return Err(p.err("trailing content")); }
    Ok(result)
}
struct JsonParser<'a> { source: &'a str, pos: usize }
impl<'a> JsonParser<'a> {
    fn rem(&self) -> &'a str { &self.source[self.pos..] }
    fn peek(&self) -> Option<char> { self.rem().chars().next() }
    fn bump(&mut self) -> Option<char> { let c = self.peek()?; self.pos += c.len_utf8(); Some(c) }
    fn ws(&mut self) { while self.peek().is_some_and(|c| matches!(c, ' ' | '\n' | '\r' | '\t')) { self.bump(); } }
    fn err(&self, msg: impl Into<String>) -> I18nError { I18nError::Json { position: self.pos, message: msg.into() } }
    fn take(&mut self, ch: char) -> Result<(), I18nError> {
        self.ws();
        if self.bump() == Some(ch) { Ok(()) } else { Err(self.err(format!("expected '{ch}'"))) }
    }
    fn hex4(&mut self) -> Result<u16, I18nError> {
        let mut v = 0u16;
        for _ in 0..4 {
            let c = self.bump().ok_or_else(|| self.err("incomplete \\u escape"))?;
            let n = c.to_digit(16).ok_or_else(|| self.err("invalid hex digit"))?;
            v = (v << 4) | n as u16;
        }
        Ok(v)
    }
    fn string(&mut self) -> Result<String, I18nError> {
        self.take('"')?;
        let mut out = String::new();
        loop {
            let c = self.bump().ok_or_else(|| self.err("unterminated string"))?;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let e = self.bump().ok_or_else(|| self.err("unfinished escape"))?;
                    match e {
                        '"' => out.push('"'), '\\' => out.push('\\'), '/' => out.push('/'),
                        'b' => out.push('\u{0008}'), 'f' => out.push('\u{000c}'),
                        'n' => out.push('\n'), 'r' => out.push('\r'), 't' => out.push('\t'),
                        'u' => {
                            let lead = self.hex4()?;
                            let scalar = if (0xD800..=0xDBFF).contains(&lead) {
                                if self.bump() != Some('\\') || self.bump() != Some('u') {
                                    return Err(self.err("high surrogate requires low surrogate"));
                                }
                                let low = self.hex4()?;
                                if !(0xDC00..=0xDFFF).contains(&low) { return Err(self.err("invalid low surrogate")); }
                                0x10000 + (((lead as u32 - 0xD800) << 10) | (low as u32 - 0xDC00))
                            } else { lead as u32 };
                            out.push(char::from_u32(scalar).ok_or_else(|| self.err("invalid unicode scalar"))?);
                        }
                        _ => return Err(self.err("invalid JSON escape")),
                    }
                }
                c if c < '\u{20}' => return Err(self.err("unescaped control character")),
                _ => out.push(c),
            }
        }
    }
    fn object(&mut self, prefix: &str, out: &mut HashMap<String, String>, depth: usize) -> Result<(), I18nError> {
        if depth > 32 { return Err(self.err("JSON nesting exceeds 32 levels")); }
        self.take('{')?;
        self.ws();
        if self.peek() == Some('}') { self.bump(); return Ok(()); }
        loop {
            self.ws();
            let key = self.string()?;
            if key.is_empty() || key.len() > 256 || !key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) {
                return Err(self.err(format!("invalid JSON catalog key '{key}'")));
            }
            let full = if prefix.is_empty() { key } else { format!("{prefix}.{key}") };
            self.take(':')?;
            self.ws();
            if self.peek() == Some('{') {
                self.object(&full, out, depth + 1)?;
            } else if self.peek() == Some('"') {
                let value = self.string()?;
                if out.insert(full.clone(), value).is_some() { return Err(self.err(format!("duplicate key '{full}'"))); }
            } else { return Err(self.err("catalog values must be strings or nested objects")); }
            self.ws();
            match self.bump() {
                Some(',') => continue,
                Some('}') => return Ok(()),
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn properties() {
        let a = parse_properties("# comment\nmenu.save = Save\\nnow\nkey = equals\\=ok\n").unwrap();
        assert_eq!(&a["menu.save"], "Save\nnow");
        assert_eq!(&a["key"], "equals=ok");
        assert!(parse_properties("a = x\na = y").is_err());
    }
    #[test] fn nested_json_and_surrogates() {
        let a = parse_json(r#"{"menu":{"save":"Save"},"emoji":"\uD83D\uDE80"}"#).unwrap();
        assert_eq!(&a["menu.save"], "Save");
        assert_eq!(&a["emoji"], "🚀");
        assert!(parse_json(r#"{"a":1}"#).is_err());
        assert!(parse_json(r#"{"a":"x","a":"y"}"#).is_err());
    }
}
