//! OPC package inspection, strict URI resolution and conservative repairs.
//! Package-level signature parts are preserved on unchanged roundtrips. They are
//! deliberately removed on any edited document because signatures would be stale.
use crate::{xml,zip};
use std::collections::{BTreeMap,BTreeSet};
use std::io::{self,Error,ErrorKind};
fn bad(s:&str)->io::Error{Error::new(ErrorKind::InvalidData,s)}

/// Percent-decode OPC relative part URIs, reject absolute traversal beyond the
/// package root and URI schemes in internal relationships.
pub fn resolve_target(source_part:&str,target:&str)->io::Result<String>{
    if source_part.contains('\\')||source_part.starts_with('/')||source_part.split('/').any(|s|s=="..") {return Err(bad("invalid OPC source part"));}
    let target=target.split('#').next().unwrap_or("");
    if target.is_empty() || target.contains('?') || target.contains('\\') || target.starts_with("//") {return Err(bad("unsupported OPC target"));}
    let mut decoded=Vec::with_capacity(target.len());let input=target.as_bytes();let mut i=0;
    while i<input.len(){
        if input[i]==b'%' {
            if i+2>=input.len(){return Err(bad("malformed OPC percent escape"));}
            let h=|b:u8| -> Option<u8>{match b {b'0'..=b'9'=>Some(b-b'0'),b'a'..=b'f'=>Some(b-b'a'+10),b'A'..=b'F'=>Some(b-b'A'+10),_=>None}};
            let hi=h(input[i+1]).ok_or_else(||bad("invalid OPC hex escape"))?;
            let lo=h(input[i+2]).ok_or_else(||bad("invalid OPC hex escape"))?;
            let value=hi*16+lo;
            if matches!(value,0|b'/'|b'\\'|b'?'|b'#'){return Err(bad("unsafe encoded OPC target delimiter"));}
            decoded.push(value);i+=3;
        } else {decoded.push(input[i]);i+=1;}
    }
    let decoded=String::from_utf8(decoded).map_err(|_|bad("invalid UTF-8 OPC target"))?;
    if decoded.contains(':') {return Err(bad("OPC internal relationships must not use URI schemes"));}
    let mut pieces:Vec<&str>=if target.starts_with('/') {Vec::new()} else {source_part.rsplit_once('/').map(|(dir,_)|dir.split('/').collect()).unwrap_or_default()};
    for seg in decoded.trim_start_matches('/').split('/'){
        match seg{""|"."=>{},".."=>{pieces.pop().ok_or_else(||bad("OPC relationship escapes package root"))?;},_=>pieces.push(seg)}
    }
    if pieces.is_empty(){return Err(bad("OPC relationship target resolves to package root"));}
    Ok(pieces.join("/"))
}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Relationship {pub id:String,pub kind:String,pub target:String,pub external:bool}
#[derive(Debug,Clone,Default)]
pub struct Package {pub parts:BTreeMap<String,Vec<u8>>}
impl Package {
    pub fn from_bytes(data:&[u8])->io::Result<Self>{Ok(Self{parts:zip::decode(data)?})}
    pub fn to_bytes(&self)->io::Result<Vec<u8>>{zip::encode(&self.parts)}
    pub fn relationships(&self,source_part:&str)->io::Result<Vec<Relationship>>{
        let path=if source_part.is_empty(){"_rels/.rels".to_owned()}else if let Some((folder,name))=source_part.rsplit_once('/'){
            format!("{folder}/_rels/{name}.rels")
        }else{format!("_rels/{source_part}.rels")};
        let Some(bytes)=self.parts.get(&path) else {return Ok(Vec::new())};
        let root=xml::parse(&xml::decode_bytes(bytes)?)?;
        if root.local()!="Relationships"{return Err(bad("not an OPC relationships part"));}
        let mut seen=BTreeSet::new();let mut result=Vec::new();
        for el in root.elements("Relationship"){
            let id=el.attr("Id").ok_or_else(||bad("relationship missing ID"))?;
            let kind=el.attr("Type").ok_or_else(||bad("relationship missing type"))?;
            let raw=el.attr("Target").ok_or_else(||bad("relationship missing target"))?;
            let external=el.attr("TargetMode")==Some("External");
            if !seen.insert(id.to_owned()){return Err(bad("duplicate relationship ID"));}
            let target=if external{raw.to_owned()}else{resolve_target(source_part,raw)?};
            result.push(Relationship{id:id.to_owned(),kind:kind.to_owned(),target,external});
        }
        Ok(result)
    }
    /// Validate OPC target references and check minimal required Word parts.
    pub fn validate(&self)->io::Result<()> {
        if !self.parts.contains_key("[Content_Types].xml"){return Err(bad("missing OPC content types"));}
        if !self.parts.contains_key("_rels/.rels"){return Err(bad("missing OPC root relationships"));}
        for path in self.parts.keys().filter(|p|p.ends_with(".rels")){
            let source=if path=="_rels/.rels"{"".to_owned()}else{
                let (dir,name)=path.rsplit_once("/_rels/").or_else(||path.strip_prefix("_rels/").map(|n|("",n))).ok_or_else(||bad("invalid relationship part path"))?;
                let name=name.strip_suffix(".rels").ok_or_else(||bad("invalid OPC rel extension"))?;
                if dir.is_empty(){name.to_owned()}else{format!("{dir}/{name}")}
            };
            for rel in self.relationships(&source)?{if !rel.external && !self.parts.contains_key(&rel.target){return Err(bad("dangling OPC relationship"));}}
        }
        Ok(())
    }
    pub fn signed_parts(&self)->Vec<&str>{self.parts.keys().filter(|p|p.starts_with("_xmlsignatures/")).map(String::as_str).collect()}
    /// Remove digital signature artifacts after editing package content.
    /// This avoids delivering a misleading or invalid document signature.
    pub fn invalidate_signatures(&mut self){
        self.parts.retain(|p,_|!p.starts_with("_xmlsignatures/"));
        for name in ["_rels/.rels"] {
            if let Some(bytes)=self.parts.get_mut(name){
                if let Ok(s)=xml::decode_bytes(bytes){if let Ok(mut root)=xml::parse(&s){
                    root.children.retain(|child|match child {xml::Node::Element(e)=>!e.attr("Type").unwrap_or("").contains("digital-signature"),_=>true});
                    *bytes=xml::serialize(&root).into_bytes();
                }}
            }
        }
    }
    /// Remove relationships to missing internal targets; does not synthesize
    /// missing content or guess missing Word document structure.
    pub fn repair_dangling_relationships(&mut self)->io::Result<usize>{
        let paths:Vec<String>=self.parts.keys().filter(|p|p.ends_with(".rels")).cloned().collect();
        let mut dropped=0;
        for path in paths {
            let before=dropped;
            let source=if path=="_rels/.rels"{String::new()}else{
                let (dir,name)=path.rsplit_once("/_rels/").or_else(||path.strip_prefix("_rels/").map(|n|("",n))).ok_or_else(||bad("invalid relationship part path"))?;
                let name=name.strip_suffix(".rels").ok_or_else(||bad("invalid rel path"))?;
                if dir.is_empty(){name.to_owned()}else{format!("{dir}/{name}")}
            };
            let bytes=self.parts.get(&path).unwrap();
            let mut root=xml::parse(&xml::decode_bytes(bytes)?)?;
            root.children.retain(|node|{
                if let xml::Node::Element(e)=node{if e.local()=="Relationship"&&e.attr("TargetMode")!=Some("External"){
                    let target=e.attr("Target").unwrap_or("");
                    if resolve_target(&source,target).map(|p|self.parts.contains_key(&p)).unwrap_or(false)==false {dropped+=1;return false;}
                }}true
            });
            if dropped>before {self.parts.insert(path,xml::serialize(&root).into_bytes());}
        }
        Ok(dropped)
    }
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn uri_resolution(){assert_eq!(resolve_target("word/document.xml","media/pic.png").unwrap(),"word/media/pic.png");assert_eq!(resolve_target("word/document.xml","../customXml/item1.xml").unwrap(),"customXml/item1.xml");assert!(resolve_target("word/document.xml","../../evil.xml").is_err());assert!(resolve_target("word/document.xml","%2e%2e/%2e%2e/evil").is_err());assert!(resolve_target("word/document.xml","http://evil").is_err());}
}

/// The result of conservative OPC repair. Reconstruction never pretends to
/// recover content whose underlying bytes were lost.
#[derive(Debug, Clone, Default)]
pub struct RepairReport {
    pub repaired: usize,
    pub warnings: Vec<String>,
}

const OFFICE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const OPC_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

/// Repair standard OOXML namespace declarations which were omitted from a root
/// element. Never invent a binding for an unknown or conflicting prefix.
pub fn repair_known_namespace_declarations(data: &[u8]) -> io::Result<Option<Vec<u8>>> {
    let input = xml::decode_bytes(data)?;
    if xml::parse(&input).is_ok() { return Ok(None); }
    if input.contains("<!DOCTYPE") || input.contains("<!ENTITY") { return Err(bad("cannot repair XML with a DTD")); }
    let known = [
        ("w", "http://schemas.openxmlformats.org/wordprocessingml/2006/main"),
        ("r", "http://schemas.openxmlformats.org/officeDocument/2006/relationships"),
        ("a", "http://schemas.openxmlformats.org/drawingml/2006/main"),
        ("wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
        ("m", "http://schemas.openxmlformats.org/officeDocument/2006/math"),
        ("mc", "http://schemas.openxmlformats.org/markup-compatibility/2006"),
        ("w14", "http://schemas.microsoft.com/office/word/2010/wordml"),
        ("w15", "http://schemas.microsoft.com/office/word/2012/wordml"),
        ("v", "urn:schemas-microsoft-com:vml"),
        ("o", "urn:schemas-microsoft-com:office:office"),
        ("cp", "http://schemas.openxmlformats.org/package/2006/metadata/core-properties"),
        ("dc", "http://purl.org/dc/elements/1.1/"),
        ("dcterms", "http://purl.org/dc/terms/"),
        ("xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    ];
    let mut start = 0;
    while input[start..].starts_with("<?") {
        let end = input[start..].find("?>").ok_or_else(||bad("bad XML declaration"))?;
        start += end + 2;
        while input.as_bytes().get(start).map(|c|c.is_ascii_whitespace()).unwrap_or(false) { start += 1; }
    }
    let open = input[start..].find('<').map(|n|start+n).ok_or_else(||bad("no XML root"))?;
    let close = input[open..].find('>').map(|n|open+n).ok_or_else(||bad("unclosed XML root"))?;
    let root_tag = &input[open..=close];
    let mut extra = String::new();
    for (prefix, namespace) in known {
        if (input.contains(&format!("<{prefix}:")) || input.contains(&format!(" {prefix}:")))
            && !root_tag.contains(&format!("xmlns:{prefix}="))
            && !root_tag.contains(&format!("xmlns:{prefix} =")) {
            extra.push_str(&format!(" xmlns:{prefix}=\"{namespace}\""));
        }
    }
    if extra.is_empty() { return Ok(None); }
    let insert_at = if input.as_bytes()[close.saturating_sub(1)] == b'/' { close-1 } else { close };
    let mut fixed = input;
    fixed.insert_str(insert_at, &extra);
    xml::parse(&fixed)?; // no repair result unless valid XML
    Ok(Some(fixed.into_bytes()))
}

impl Package {
    /// Reconstruct the minimal Office package metadata, content type registry,
    /// and missing relationship declarations. A missing document is replaced
    /// by a minimal empty document, and is recorded in `warnings` as data loss.
    /// Does not repair corrupted ZIP member bytes or verify signatures.
    pub fn repair_word_package(&mut self) -> io::Result<RepairReport> {
        let mut report=RepairReport::default();
        let paths: Vec<String> = self.parts.keys().filter(|p|p.ends_with(".xml")||p.ends_with(".rels")).cloned().collect();
        for path in paths {
            if let Some(data) = self.parts.get(&path) {
                match repair_known_namespace_declarations(data) {
                    Ok(Some(repaired)) => { self.parts.insert(path,repaired);report.repaired+=1; },
                    Ok(None)=>{},
                    Err(err) => report.warnings.push(format!("Could not repair {path}: {err}")),
                }
            }
        }
        if !self.parts.contains_key("word/document.xml") {
            report.warnings.push("Document body was missing; inserted an empty document (content cannot be recovered)".into());
            self.parts.insert("word/document.xml".into(),b"<?xml version=\"1.0\"?><w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body><w:p/></w:body></w:document>".to_vec());
            report.repaired+=1;
        }
        let root_rel_path="_rels/.rels";
        let mut root = self.parts.get(root_rel_path).and_then(|bytes|xml::decode_bytes(bytes).ok()).and_then(|s|xml::parse(&s).ok())
            .filter(|root|root.local()=="Relationships")
            .unwrap_or_else(||xml::Element{name:"Relationships".into(),attrs:BTreeMap::from([("xmlns".into(),OPC_REL.into())]),children:Vec::new()});
        let has_document = root.elements("Relationship").any(|r|r.attr("Type")==Some(OFFICE_REL));
        if !has_document {
            let mut el = xml::Element{name:"Relationship".into(),..Default::default()};
            let ids:BTreeSet<String>=root.elements("Relationship").filter_map(|e|e.attr("Id").map(str::to_owned)).collect();
            let mut n=1;while ids.contains(&format!("rId{n}")){n+=1;}
            el.attrs.insert("Id".into(),format!("rId{n}"));
            el.attrs.insert("Type".into(),OFFICE_REL.into());
            el.attrs.insert("Target".into(),"word/document.xml".into());
            root.children.push(xml::Node::Element(el));report.repaired+=1;
        }
        self.parts.insert(root_rel_path.into(),xml::serialize(&root).into_bytes());
        if !self.parts.contains_key("[Content_Types].xml") || self.parts.get("[Content_Types].xml").and_then(|d|xml::decode_bytes(d).ok()).and_then(|s|xml::parse(&s).ok()).is_none() {
            self.parts.insert("[Content_Types].xml".into(),b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"/>".to_vec());
            report.repaired+=1;
        }
        let bytes = self.parts.get("[Content_Types].xml").unwrap();
        let mut types=xml::parse(&xml::decode_bytes(bytes)?)?;
        if types.local()!="Types"{return Err(bad("invalid content types root"));}
        let mut existing:BTreeSet<String>=types.elements("Override").filter_map(|e|e.attr("PartName").map(str::to_owned)).collect();
        let mut defaults:BTreeSet<String>=types.elements("Default").filter_map(|e|e.attr("Extension").map(str::to_owned)).collect();
        let mut add_default=|extension:&str,mime:&str| {
            if defaults.insert(extension.to_owned()) {
                let mut e=xml::Element{name:"Default".into(),..Default::default()};
                e.attrs.insert("Extension".into(),extension.into());e.attrs.insert("ContentType".into(),mime.into());
                types.children.push(xml::Node::Element(e));report.repaired+=1;
            }
        };
        add_default("rels","application/vnd.openxmlformats-package.relationships+xml");
        add_default("xml","application/xml");
        drop(add_default);
        let paths:Vec<String>=self.parts.keys().cloned().collect();
        for path in paths {
            let mime=match path.as_str() {
                "word/document.xml"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"),
                "word/styles.xml"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"),
                "word/settings.xml"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"),
                "word/numbering.xml"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"),
                "word/footnotes.xml"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"),
                "word/endnotes.xml"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml"),
                "word/comments.xml"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"),
                "docProps/core.xml"=>Some("application/vnd.openxmlformats-package.core-properties+xml"),
                _=>None,
            };
            if let Some(mime)=mime { let part=format!("/{path}");if existing.insert(part.clone()){
                let mut e=xml::Element{name:"Override".into(),..Default::default()};
                e.attrs.insert("PartName".into(),part);e.attrs.insert("ContentType".into(),mime.into());
                types.children.push(xml::Node::Element(e));report.repaired+=1;
            }}
        }
        self.parts.insert("[Content_Types].xml".into(),xml::serialize(&types).into_bytes());
        report.repaired+=self.repair_dangling_relationships()?;
        if report.repaired>0 && !self.signed_parts().is_empty(){self.invalidate_signatures();report.warnings.push("Removed invalidated digital signatures".into());}
        self.validate()?;
        Ok(report)
    }
}
