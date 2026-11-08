//! Incremental, bounded XML event reader. Input is read from `Read` without
//! buffering an entire part or building a DOM. The lexical unit (a tag, text
//! node or CDATA node) is bounded by `Limits::max_value_bytes`.
//!
//! UTF-8 is supported directly; callers with UTF-16 parts can transcode to
//! UTF-8 before using the streaming API. DTD and external entities are rejected.
use crate::xml::{self, Element, Limits};
use std::collections::BTreeMap;
use std::io::{self, BufReader, Error, ErrorKind, Read};

fn invalid(s: &str) -> io::Error { Error::new(ErrorKind::InvalidData, s) }

#[derive(Debug, Clone, PartialEq)]
pub enum Event { Start(Element), Text(String), End(String) }

struct Input<R: Read> {
    inner: BufReader<R>, unread: Option<u8>, consumed: usize, limit: usize,
}
impl<R: Read> Input<R> {
    fn new(read: R, limit: usize) -> Self { Self { inner: BufReader::new(read), unread: None, consumed: 0, limit } }
    fn byte(&mut self) -> io::Result<Option<u8>> {
        if let Some(byte) = self.unread.take() { return Ok(Some(byte)); }
        let mut buf = [0u8; 1];
        match self.inner.read(&mut buf)? {
            0 => Ok(None),
            _ => { self.consumed = self.consumed.checked_add(1).ok_or_else(|| invalid("XML size overflow"))?;
                if self.consumed > self.limit { return Err(invalid("XML byte limit exceeded")); }
                Ok(Some(buf[0])) }
        }
    }
    fn peek(&mut self) -> io::Result<Option<u8>> {
        let b = self.byte()?; self.unread = b; Ok(b)
    }
}
fn append(buf: &mut Vec<u8>, byte: u8, limit: usize) -> io::Result<()> {
    if buf.len() >= limit { return Err(invalid("XML token limit exceeded")); }
    buf.push(byte); Ok(())
}
fn token_string(buf: Vec<u8>) -> io::Result<String> {
    String::from_utf8(buf).map_err(|_| invalid("non-UTF-8 streaming XML token"))
}
fn text_token(s: &str, limits: Limits) -> io::Result<String> {
    // Reuse the DOM parser's strict entity and character validation, but only
    // for one bounded text token. No complete XML document is buffered.
    let wrapped = format!("<text>{s}</text>");
    Ok(xml::parse_with_limits(&wrapped, Limits { max_bytes: limits.max_value_bytes.saturating_add(32), ..limits })?.text())
}
fn emit_text<F: FnMut(Event) -> io::Result<()>>(buf: Vec<u8>, limits: Limits, sink: &mut F, nodes: &mut usize) -> io::Result<()> {
    if buf.is_empty() { return Ok(()); }
    let text = text_token(&token_string(buf)?, limits)?;
    *nodes += 1;
    if *nodes > limits.max_nodes { return Err(invalid("XML node limit exceeded")); }
    sink(Event::Text(text))
}

/// Incrementally parse a UTF-8 XML stream and invoke `sink` for each event.
/// The sink may stop parsing by returning an error. No DTD, external entities,
/// or arbitrary input-size allocations are permitted.
pub fn read_events<R: Read, F: FnMut(Event) -> io::Result<()>>(
    reader: R, limits: Limits, mut sink: F,
) -> io::Result<()> {
    let mut input = Input::new(reader, limits.max_bytes);
    let mut stack: Vec<String> = Vec::new();
    let mut namespaces: Vec<BTreeMap<String, String>> = Vec::new();
    let mut nodes = 0usize;
    let mut had_root = false;
    let mut closed_root = false;
    // BOM must be handled before the first XML token.
    if input.peek()? == Some(0xef) {
        let mut bom = Vec::new(); for _ in 0..3 { if let Some(b)=input.byte()? { bom.push(b); } }
        if bom != [0xef, 0xbb, 0xbf] { return Err(invalid("invalid UTF-8 BOM")); }
    }
    loop {
        let mut data = Vec::new();
        while let Some(b) = input.peek()? {
            if b == b'<' { break; }
            input.byte()?;
            append(&mut data, b, limits.max_value_bytes)?;
        }
        if !data.is_empty() {
            let value = token_string(data)?;
            if stack.is_empty() && !value.trim().is_empty() { return Err(invalid("text outside XML root")); }
            emit_text(value.into_bytes(), limits, &mut sink, &mut nodes)?;
        }
        if input.byte()?.is_none() { break; } // '<'
        let next = input.byte()?.ok_or_else(|| invalid("unterminated XML markup"))?;
        if next == b'!' {
            let marker = input.byte()?.ok_or_else(|| invalid("truncated XML markup"))?;
            if marker == b'-' {
                if input.byte()? != Some(b'-') { return Err(invalid("invalid XML comment")); }
                let mut buf = Vec::new();
                loop {
                    let b = input.byte()?.ok_or_else(|| invalid("unterminated XML comment"))?;
                    append(&mut buf, b, limits.max_value_bytes)?;
                    if buf.ends_with(b"-->") { break; }
                }
                if buf[..buf.len()-3].windows(2).any(|v|v == b"--") { return Err(invalid("invalid XML comment")); }
            } else if marker == b'[' {
                for &expected in b"CDATA[" { if input.byte()? != Some(expected) { return Err(invalid("invalid CDATA")); } }
                let mut buf = Vec::new();
                loop {
                    let b = input.byte()?.ok_or_else(|| invalid("unterminated CDATA"))?;
                    append(&mut buf, b, limits.max_value_bytes)?;
                    if buf.ends_with(b"]]>") { break; }
                }
                buf.truncate(buf.len()-3);
                let value = token_string(buf)?;
                if stack.is_empty() && !value.trim().is_empty() { return Err(invalid("CDATA outside root")); }
                // CDATA is literal, not entity expanded; validate characters.
                let wrapped = format!("<text><![CDATA[{value}]]></text>");
                xml::parse_with_limits(&wrapped, Limits { max_bytes: limits.max_value_bytes.saturating_add(40), ..limits })?;
                nodes += 1; if nodes > limits.max_nodes { return Err(invalid("XML node limit exceeded")); }
                sink(Event::Text(value))?;
            } else { return Err(invalid("DTD and declarations are forbidden")); }
            continue;
        }
        if next == b'?' {
            let mut data = Vec::new();
            loop {
                let b = input.byte()?.ok_or_else(|| invalid("unterminated processing instruction"))?;
                append(&mut data, b, limits.max_value_bytes)?;
                if data.ends_with(b"?>") { break; }
            }
            continue;
        }
        let mut data = vec![next]; let mut quote = None;
        loop {
            let b = input.byte()?.ok_or_else(|| invalid("unterminated XML tag"))?;
            append(&mut data, b, limits.max_value_bytes)?;
            if let Some(q) = quote { if q == b { quote = None; } }
            else { match b { b'\'' | b'"' => quote = Some(b), b'>' => break, _=>{} } }
        }
        let raw = token_string(data)?;
        if raw.starts_with('/') {
            let name = raw[1..raw.len()-1].trim();
            let prior = stack.pop().ok_or_else(|| invalid("unexpected close tag"))?;
            namespaces.pop();
            if prior != name { return Err(invalid("mismatched XML close tag")); }
            sink(Event::End(name.to_owned()))?;
            if stack.is_empty() { closed_root = true; }
        } else {
            if closed_root { return Err(invalid("multiple XML roots")); }
            let selfclose = raw.ends_with("/>");
            let name = raw.split(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>').next().unwrap_or("");
            if name.is_empty() { return Err(invalid("invalid start tag")); }
            // Namespace definitions inherited from ancestors are injected into
            // a temporary self-contained element for strict parser validation.
            let inherited = namespaces.last().cloned().unwrap_or_default();
            let suffix = if selfclose { &raw[..raw.len()-2] } else { &raw[..raw.len()-1] };
            let mut stand_alone = format!("<{suffix}");
            for (prefix, uri) in &inherited {
                let declared = if prefix.is_empty() { "xmlns".to_owned() } else { format!("xmlns:{prefix}") };
                if raw.contains(&format!("{declared}=")) || raw.contains(&format!("{declared} =")) { continue; }
                if prefix.is_empty() { stand_alone.push_str(&format!(" xmlns=\"{}\"", xml::escape(uri))); }
                else { stand_alone.push_str(&format!(" xmlns:{}=\"{}\"",prefix,xml::escape(uri))); }
            }
            stand_alone.push_str("/>");
            let mut node = xml::parse_with_limits(&stand_alone, Limits {
                max_bytes: limits.max_value_bytes.saturating_mul(2).saturating_add(4096),
                max_depth: 2, max_nodes: 8, ..limits
            })?;
            let mut current = inherited;
            for (key, val) in &node.attrs {
                if key == "xmlns" { current.insert(String::new(), val.clone()); }
                else if let Some(prefix) = key.strip_prefix("xmlns:") { current.insert(prefix.to_owned(),val.clone()); }
            }
            // Do not report injected namespace attributes in the user event.
            for (prefix, uri) in namespaces.last().into_iter().flat_map(|m|m.iter()) {
                let key = if prefix.is_empty() { "xmlns".to_owned() } else { format!("xmlns:{prefix}") };
                if !raw.contains(&format!("{key}=")) && !raw.contains(&format!("{key} =")) && node.attrs.get(&key) == Some(uri) { node.attrs.remove(&key); }
            }
            nodes += 1; if nodes > limits.max_nodes { return Err(invalid("XML node limit exceeded")); }
            if stack.len() >= limits.max_depth && !selfclose { return Err(invalid("XML depth limit exceeded")); }
            sink(Event::Start(node))?;
            if selfclose { sink(Event::End(name.to_owned()))?; if stack.is_empty() { closed_root = true; } }
            else { stack.push(name.to_owned()); namespaces.push(current); }
            had_root = true;
        }
    }
    if !had_root || !stack.is_empty() { return Err(invalid("incomplete XML stream")); }
    Ok(())
}
