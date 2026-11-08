//! Small, dependency-free JSON parser for `cargo metadata` output.
use std::collections::BTreeMap;
use crate::{bail, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Value { Null, Bool(bool), Number(String), String(String), Array(Vec<Value>), Object(BTreeMap<String, Value>) }
impl Value {
    pub fn get(&self, key: &str) -> Option<&Self> { if let Self::Object(o) = self { o.get(key) } else { None } }
    pub fn string(&self) -> Option<&str> { if let Self::String(s) = self { Some(s) } else { None } }
    pub fn array(&self) -> Option<&[Self]> { if let Self::Array(a) = self { Some(a) } else { None } }
    pub fn field_str(&self, key: &str) -> Result<&str> { self.get(key).and_then(Self::string).ok_or_else(|| format!("JSON field {key} must be a string").into()) }
    pub fn field_array(&self, key: &str) -> Result<&[Self]> { self.get(key).and_then(Self::array).ok_or_else(|| format!("JSON field {key} must be an array").into()) }
}
struct Parser<'a> { data: &'a [u8], pos: usize }
impl Parser<'_> {
    fn skip(&mut self) { while self.pos < self.data.len() && self.data[self.pos].is_ascii_whitespace() { self.pos += 1; } }
    fn byte(&mut self) -> Result<u8> { let b = *self.data.get(self.pos).ok_or("unexpected JSON EOF")?; self.pos += 1; Ok(b) }
    fn expect(&mut self, val: u8) -> Result<()> { if self.byte()? != val { bail("unexpected JSON punctuation") } else { Ok(()) } }
    fn quoted(&mut self) -> Result<String> {
        self.expect(b'"')?;
        let mut out = String::new(); let mut from = self.pos;
        while let Some(&b) = self.data.get(self.pos) {
            if b == b'"' || b == b'\\' {
                out.push_str(std::str::from_utf8(&self.data[from..self.pos])?);
                self.pos += 1;
                if b == b'"' { return Ok(out); }
                match self.byte()? {
                    b'"' => out.push('"'), b'\\' => out.push('\\'), b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'), b'f' => out.push('\u{000c}'), b'n' => out.push('\n'),
                    b'r' => out.push('\r'), b't' => out.push('\t'),
                    b'u' => {
                        let first = self.hex4()?;
                        let cp = if (0xd800..=0xdbff).contains(&first) {
                            self.expect(b'\\')?; self.expect(b'u')?;
                            let second = self.hex4()?;
                            if !(0xdc00..=0xdfff).contains(&second) { return bail("invalid UTF-16 surrogate pair"); }
                            0x10000 + ((first as u32 - 0xd800) << 10) + (second as u32 - 0xdc00)
                        } else { first as u32 };
                        out.push(char::from_u32(cp).ok_or("invalid JSON unicode escape")?);
                    }
                    _ => return bail("invalid JSON string escape"),
                }
                from = self.pos;
            } else if b < 0x20 { return bail("unescaped control character in JSON string"); }
            else { self.pos += 1; }
        }
        bail("unterminated JSON string")
    }
    fn hex4(&mut self) -> Result<u16> {
        let mut value = 0u16;
        for _ in 0..4 { let b = self.byte()?; value = value.checked_mul(16).ok_or("bad unicode escape")? + (b as char).to_digit(16).ok_or("bad unicode escape")? as u16; }
        Ok(value)
    }
    fn parse(&mut self, depth: usize) -> Result<Value> {
        if depth > 96 { return bail("JSON nesting too deep"); }
        self.skip();
        match self.data.get(self.pos).copied().ok_or("missing JSON value")? {
            b'"' => Ok(Value::String(self.quoted()?)),
            b'{' => {
                self.pos += 1; let mut out = BTreeMap::new(); self.skip();
                if self.data.get(self.pos) == Some(&b'}') { self.pos += 1; return Ok(Value::Object(out)); }
                loop {
                    self.skip(); let key = self.quoted()?; self.skip(); self.expect(b':')?;
                    let value = self.parse(depth + 1)?; out.insert(key, value);
                    self.skip(); match self.byte()? { b'}' => break, b',' => {}, _ => return bail("bad JSON object separator") }
                }
                Ok(Value::Object(out))
            }
            b'[' => {
                self.pos += 1; let mut out = Vec::new(); self.skip();
                if self.data.get(self.pos) == Some(&b']') { self.pos += 1; return Ok(Value::Array(out)); }
                loop { out.push(self.parse(depth + 1)?); self.skip(); match self.byte()? { b']' => break, b',' => {}, _ => return bail("bad JSON array separator") } }
                Ok(Value::Array(out))
            }
            b't' => { self.literal(b"true")?; Ok(Value::Bool(true)) }
            b'f' => { self.literal(b"false")?; Ok(Value::Bool(false)) }
            b'n' => { self.literal(b"null")?; Ok(Value::Null) }
            b'-' | b'0'..=b'9' => {
                let from = self.pos;
                while self.data.get(self.pos).is_some_and(|b| b"0123456789+-.eE".contains(b)) { self.pos += 1; }
                let val = std::str::from_utf8(&self.data[from..self.pos])?;
                if val.parse::<f64>().is_err() { return bail("invalid JSON number"); }
                Ok(Value::Number(val.into()))
            }
            _ => bail("invalid JSON value")
        }
    }
    fn literal(&mut self, lit: &[u8]) -> Result<()> {
        if self.data.get(self.pos..self.pos + lit.len()) != Some(lit) { return bail("invalid JSON literal"); }
        self.pos += lit.len(); Ok(())
    }
}
pub fn parse(data: &[u8]) -> Result<Value> {
    let mut p = Parser { data, pos: 0 };
    let val = p.parse(0)?; p.skip();
    if p.pos != data.len() { return bail("unexpected characters after JSON document"); }
    Ok(val)
}
#[cfg(test)] mod tests { use super::*; #[test] fn metadata_objects() { let v=parse(br#"{"target_directory":"C:\\Temp","workspace_members":["a"],"other":{"x":true}}"#).unwrap(); assert_eq!(v.field_str("target_directory").unwrap(),r"C:\Temp"); }}
