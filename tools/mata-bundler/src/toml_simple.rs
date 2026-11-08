//! TOML subset for Mata.toml, plus minimally invasive table updates for `android init`.
//! Preserves unrelated config text and comments. Unsupported syntax is reported for fields Mata consumes.
use std::collections::BTreeMap;
use crate::{bail, Result};
#[derive(Debug, Clone, PartialEq)]
pub enum Val { String(String), Integer(i64), Bool(bool), Strings(Vec<String>) }

pub fn values(src: &str) -> Result<BTreeMap<String, Val>> {
    let mut map = BTreeMap::new(); let mut table = String::new();
    // Join multiline arrays (ordinary TOML arrays only).
    let mut source = String::new(); let mut pending = String::new();
    for line in src.lines() {
        if pending.is_empty() {
            let trimmed = strip_comment(line).trim().to_owned();
            if trimmed.contains('=') && trimmed.split_once('=').is_some_and(|(_,v)| v.trim().starts_with('[') && !v.trim().ends_with(']')) {
                pending.push_str(&trimmed); pending.push('\n'); continue;
            }
            source.push_str(line); source.push('\n');
        } else {
            pending.push_str(strip_comment(line).trim());
            if pending.ends_with(']') { source.push_str(&pending); source.push('\n'); pending.clear(); }
        }
    }
    if !pending.is_empty() { return bail("unterminated TOML array"); }
    for (i, line) in source.lines().enumerate() {
        let line = strip_comment(line).trim();
        if line.is_empty() { continue; }
        if line.starts_with('[') {
            if let Some(t) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) { table = t.trim().to_owned(); continue; }
            return bail(format!("invalid TOML section at line {}",i+1));
        }
        let Some((key, val)) = line.split_once('=') else { return bail(format!("invalid TOML assignment at line {}", i+1)); };
        let key = key.trim().trim_matches('"');
        // An unknown table/key can contain syntax that Mata doesn't consume.
        if !matches!(table.as_str(), "app" | "android" | "linux" | "windows") { continue; }
        let known = match table.as_str() {
            "app" => matches!(key,"name" | "package" | "bin" | "version" | "maintainer" | "license" | "assets" | "files"),
            "android" => matches!(key,"application_id" | "version_code" | "min_sdk" | "target_sdk" | "compile_sdk" | "abis" | "activity" | "lib_name" | "manifest" | "java_sources" | "resources" | "assets" | "native_libs" | "permissions"),
            "linux" => matches!(key,"target" | "files"),
            "windows" => matches!(key,"target" | "files" | "icon" | "desktop_shortcut"), _=>false,
        };
        if known {
            let val = parse_val(val.trim()).map_err(|e| format!("Mata.toml line {} ({table}.{key}): {e}", i+1))?;
            let full = format!("{table}.{key}");
            if map.insert(full.clone(), val).is_some() { return bail(format!("duplicate {full} in Mata.toml")); }
        }
    }
    Ok(map)
}
fn strip_comment(s: &str) -> &str {
    let mut quoted = false; let mut literal = false; let mut escaped = false;
    for (i,c) in s.char_indices() {
        if escaped { escaped = false; continue; }
        if c == '\\' && quoted { escaped = true; continue; }
        if c=='"' && !literal { quoted = !quoted; }
        if c=='\'' && !quoted { literal = !literal; }
        if c=='#' && !quoted && !literal { return &s[..i]; }
    }
    s
}
fn quoted(s: &str) -> Result<String> {
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 { return Ok(s[1..s.len()-1].to_owned()); }
    if !s.starts_with('"') || !s.ends_with('"') || s.len()<2 { return bail("expected a quoted string"); }
    let mut out=String::new(); let mut chars=s[1..s.len()-1].chars();
    while let Some(c)=chars.next() {
        if c!='\\' {out.push(c);continue;}
        match chars.next().ok_or("unterminated string escape")? {
            'n'=>out.push('\n'), 'r'=>out.push('\r'), 't'=>out.push('\t'), '"'=>out.push('"'), '\\'=>out.push('\\'),
            _=>return bail("unsupported TOML escape"),
        }
    }
    Ok(out)
}
fn parse_val(s: &str)->Result<Val> {
    if s.starts_with('"') || s.starts_with('\'') { return Ok(Val::String(quoted(s)?)); }
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len()-1];
        let mut elems=Vec::new(); let mut start=0; let mut quote=None; let mut esc=false;
        for (i,c) in inner.char_indices() {
            if esc {esc=false;continue;}
            if c=='\\' && quote==Some('"') {esc=true;continue;}
            if c=='"' || c=='\'' { if quote==Some(c) {quote=None;} else if quote.is_none(){quote=Some(c);} }
            if c==',' && quote.is_none(){let v=inner[start..i].trim();if !v.is_empty(){elems.push(quoted(v)?);}start=i+1;}
        }
        if quote.is_some() {return bail("unterminated TOML array string");}
        let v=inner[start..].trim();if !v.is_empty(){elems.push(quoted(v)?);}
        return Ok(Val::Strings(elems));
    }
    if s=="true" || s=="false" {return Ok(Val::Bool(s=="true"));}
    let number = s.replace('_',"").parse::<i64>().map_err(|_|"unsupported TOML scalar")?;
    Ok(Val::Integer(number))
}

/// Replace a simple TOML value without reformatting the rest of the file.
pub fn set(src: &str, section: &str, key: &str, value: &str) -> String {
    let mut lines:Vec<String>=src.lines().map(str::to_owned).collect();
    let header=format!("[{section}]"); let mut pos=None;
    for (i,l) in lines.iter().enumerate() { if l.trim()==header {pos=Some(i);break;} }
    if let Some(start)=pos {
        let end=(start+1..lines.len()).find(|&i|lines[i].trim().starts_with('[')).unwrap_or(lines.len());
        if let Some(i)=(start+1..end).find(|&i|strip_comment(&lines[i]).split_once('=').is_some_and(|(k,_)|k.trim()==key)) {
            // Keep original inline comment where possible.
            let comment=lines[i].find('#').map(|n|format!(" {}", &lines[i][n..])).unwrap_or_default();
            lines[i]=format!("{key} = {value}{comment}");
        } else {lines.insert(end,format!("{key} = {value}"));}
    } else {lines.push(String::new());lines.push(header);lines.push(format!("{key} = {value}"));}
    format!("{}\n",lines.join("\n"))
}
pub fn table_exists(src: &str, table: &str) -> bool { src.lines().any(|l|l.trim()==format!("[{table}]")) }
#[cfg(test)]mod tests {use super::*;#[test]fn parse_and_update(){let m=values("[app]\nname = 'A'\nfiles = [\"a\",\n\"b\"]\n[android]\nmin_sdk = 26\n").unwrap();assert_eq!(m.get("app.files"),Some(&Val::Strings(vec!["a".into(),"b".into()])));let s=set("[app]\nname=\"x\"\n","app","name","\"y\"");assert!(s.contains("name = \"y\""));}}
