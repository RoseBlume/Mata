//! Bounded, namespace-aware XML 1.0 reader for OPC and OOXML.
//! No DTD, entity declarations, XInclude, external resource resolution, or crates.
use std::collections::BTreeMap;
use std::io::{Error, ErrorKind, Result};
fn bad(s: &str)->Error {Error::new(ErrorKind::InvalidData,s)}
#[derive(Debug,Clone,PartialEq)]
pub enum Node {Element(Element),Text(String)}
#[derive(Debug,Clone,PartialEq,Default)]
pub struct Element {pub name:String,pub attrs:BTreeMap<String,String>,pub children:Vec<Node>}
impl Element {
    pub fn local(&self)->&str {self.name.rsplit(':').next().unwrap_or(&self.name)}
    pub fn attr(&self,key:&str)->Option<&str>{self.attrs.get(key).or_else(||self.attrs.iter().find(|(k,_)|k.rsplit(':').next()==Some(key)).map(|(_,v)|v)).map(String::as_str)}
    pub fn child(&self,name:&str)->Option<&Element>{self.children.iter().filter_map(|n|if let Node::Element(e)=n{Some(e)}else{None}).find(|e|e.local()==name)}
    pub fn elements<'a>(&'a self,name:&'a str)->impl Iterator<Item=&'a Element>+'a{self.children.iter().filter_map(move |n|if let Node::Element(e)=n{if e.local()==name{Some(e)}else{None}}else{None})}
    pub fn text(&self)->String {let mut s=String::new();self.gather(&mut s);s}
    fn gather(&self,s:&mut String){for n in &self.children{match n{Node::Text(t)=>s.push_str(t),Node::Element(e)=>e.gather(s)}}}
    pub fn descendants<'a>(&'a self,name:&str,out:&mut Vec<&'a Element>){for n in &self.children{if let Node::Element(e)=n{if e.local()==name{out.push(e);}e.descendants(name,out);}}}
}
/// Upper bounds prevent pathological XML memory usage / excessive recursion.
#[derive(Debug,Clone,Copy)]
pub struct Limits {pub max_bytes:usize,pub max_depth:usize,pub max_nodes:usize,pub max_attributes:usize,pub max_value_bytes:usize}
impl Default for Limits {fn default()->Self{Self{max_bytes:64*1024*1024,max_depth:256,max_nodes:500_000,max_attributes:256,max_value_bytes:1024*1024}}}
fn xml_char(c:char)->bool{matches!(c,'\t'|'\n'|'\r') || (c >= '\u{20}' && c <= '\u{D7FF}') || (c >= '\u{E000}' && c <= '\u{FFFD}') || (c >= '\u{10000}' && c <= '\u{10FFFF}')}
pub fn escape(s:&str)->String{let mut r=String::new();for c in s.chars(){match c{'&'=>r.push_str("&amp;"),'<'=>r.push_str("&lt;"),'>'=>r.push_str("&gt;"),'"'=>r.push_str("&quot;"),'\''=>r.push_str("&apos;"),_=>r.push(c)}}r}
pub fn unescape(s:&str)->String{decode_entities(s).unwrap_or_else(|_|s.to_string())}
fn decode_entities(s:&str)->Result<String>{
    let mut result=String::with_capacity(s.len());let mut left=s;
    while let Some(pos)=left.find('&'){
        result.push_str(&left[..pos]);left=&left[pos+1..];
        let end=left.find(';').ok_or_else(||bad("unterminated XML entity"))?;
        if end>12{return Err(bad("XML entity name too long"));}
        let name=&left[..end];
        let ch=match name{"amp"=>'&',"lt"=>'<',"gt"=>'>',"quot"=>'"',"apos"=>'\'',
            _ if name.starts_with("#x")=>u32::from_str_radix(&name[2..],16).ok().and_then(char::from_u32).ok_or_else(||bad("invalid XML character reference"))?,
            _ if name.starts_with('#')=>name[1..].parse::<u32>().ok().and_then(char::from_u32).ok_or_else(||bad("invalid XML character reference"))?,
            _=>return Err(bad("named or external XML entities are disabled"))};
        if !xml_char(ch){return Err(bad("invalid XML character"));}
        result.push(ch);left=&left[end+1..];
    }
    result.push_str(left);
    if !result.chars().all(xml_char){return Err(bad("illegal XML character"));}
    Ok(result)
}
fn space(b:u8)->bool{matches!(b,b' '|b'\n'|b'\r'|b'\t')}
fn is_name(c:u8)->bool{c.is_ascii_alphanumeric()||matches!(c,b'_'|b'-'|b'.'|b':')||c>=128}
fn check_qname(name:&str,ns:&BTreeMap<String,String>,attribute:bool)->Result<()> {
    if name.is_empty() || name.starts_with(':') || name.ends_with(':') || name.bytes().filter(|b|*b==b':').count()>1 {return Err(bad("invalid XML QName"));}
    if let Some((prefix,_))=name.split_once(':') {
        if prefix=="xmlns" && attribute {return Ok(());}
        if prefix!="xml" && !ns.contains_key(prefix){return Err(bad("unbound XML namespace prefix"));}
    }
    Ok(())
}
/// Parse XML into a bounded tree. Rejects DOCTYPE and all custom entities.
pub fn parse(input:&str)->Result<Element>{parse_with_limits(input,Limits::default())}
pub fn parse_with_limits(input:&str,limits:Limits)->Result<Element>{
    if input.len()>limits.max_bytes{return Err(bad("XML input size limit exceeded"));}
    let bytes=input.as_bytes();let mut i=0;let mut stack:Vec<Element>=Vec::new();let mut ns_stack:Vec<BTreeMap<String,String>>=Vec::new();
    let mut root:Option<Element>=None;let mut nodes=0usize;
    while i<bytes.len(){
        if bytes[i]!=b'<'{
            let end=input[i..].find('<').map(|v|i+v).unwrap_or(bytes.len());
            let value=decode_entities(&input[i..end])?;
            if !value.trim().is_empty() && stack.is_empty(){return Err(bad("text outside XML root"));}
            if value.len()>limits.max_value_bytes{return Err(bad("XML text node too large"));}
            if let Some(e)=stack.last_mut(){e.children.push(Node::Text(value));}
            nodes+=1;i=end;continue;
        }
        if input[i..].starts_with("<!--"){
            let at=input[i+4..].find("-->").ok_or_else(||bad("unterminated XML comment"))?;
            if input[i+4..i+4+at].contains("--"){return Err(bad("invalid XML comment"));}
            i+=4+at+3;continue;
        }
        if input[i..].starts_with("<![CDATA["){
            let at=input[i+9..].find("]]>").ok_or_else(||bad("unterminated CDATA"))?;let end=i+9+at;
            if end-(i+9)>limits.max_value_bytes || !input[i+9..end].chars().all(xml_char){return Err(bad("invalid CDATA"));}
            if let Some(e)=stack.last_mut(){e.children.push(Node::Text(input[i+9..end].to_owned()));}
            i=end+3;nodes+=1;continue;
        }
        if input[i..].starts_with("<?"){
            let at=input[i+2..].find("?>").ok_or_else(||bad("unterminated XML processing instruction"))?;
            i+=2+at+2;continue;
        }
        if input[i..].starts_with("<!"){return Err(bad("DTD and XML declarations with external entities are disabled"));}
        if input[i..].starts_with("</"){
            let end=input[i+2..].find('>').ok_or_else(||bad("unterminated closing element"))?;
            let name=input[i+2..i+2+end].trim();
            let closed=stack.pop().ok_or_else(||bad("orphan XML closing tag"))?;
            ns_stack.pop();
            if name!=closed.name{return Err(bad("mismatched XML close tag"));}
            if let Some(parent)=stack.last_mut(){parent.children.push(Node::Element(closed));}
            else if root.replace(closed).is_some(){return Err(bad("multiple XML roots"));}
            i+=2+end+1;continue;
        }
        i+=1;let start=i;
        while i<bytes.len() && is_name(bytes[i]) {i+=1;}
        if start==i || i-start>1024{return Err(bad("invalid XML element name"));}
        let mut element=Element{name:input[start..i].to_owned(),..Element::default()};
        let mut selfclose=false;let mut count=0;
        loop {
            while i<bytes.len()&&space(bytes[i]){i+=1;}
            if i>=bytes.len(){return Err(bad("unterminated XML start tag"));}
            if bytes[i]==b'>'{i+=1;break;}
            if bytes[i]==b'/' && bytes.get(i+1)==Some(&b'>'){i+=2;selfclose=true;break;}
            let a=i;
            while i<bytes.len() && is_name(bytes[i]){i+=1;}
            if i==a || i-a>1024{return Err(bad("invalid XML attribute name"));}
            let key=&input[a..i];
            while i<bytes.len()&&space(bytes[i]){i+=1;}
            if bytes.get(i)!=Some(&b'='){return Err(bad("attribute missing equals"));}i+=1;
            while i<bytes.len()&&space(bytes[i]){i+=1;}
            let quote=*bytes.get(i).ok_or_else(||bad("attribute missing quote"))?;
            if quote!=b'\'' && quote!=b'"'{return Err(bad("unquoted XML attribute"));}i+=1;
            let a=i;
            while i<bytes.len() && bytes[i]!=quote {if bytes[i]==b'<'{return Err(bad("XML attribute contains '<'"));}i+=1;}
            if i>=bytes.len() || i-a>limits.max_value_bytes{return Err(bad("unterminated or oversized attribute"));}
            let value=decode_entities(&input[a..i])?;i+=1;
            if element.attrs.insert(key.to_owned(),value).is_some(){return Err(bad("duplicate XML attribute"));}
            count+=1;if count>limits.max_attributes{return Err(bad("too many XML attributes"));}
        }
        let mut ns=ns_stack.last().cloned().unwrap_or_default();
        for (key,value) in &element.attrs {
            if key=="xmlns"{ns.insert(String::new(),value.clone());}
            else if let Some(prefix)=key.strip_prefix("xmlns:"){
                if prefix.is_empty() || prefix=="xmlns" {return Err(bad("invalid XML namespace declaration"));}
                if prefix=="xml" {
                    if value!="http://www.w3.org/XML/1998/namespace" {return Err(bad("invalid XML namespace declaration"));}
                } else {ns.insert(prefix.to_owned(),value.to_owned());}
            }
        }
        check_qname(&element.name,&ns,false)?;
        for key in element.attrs.keys(){if key!="xmlns"{check_qname(key,&ns,true)?;}}
        nodes+=1;if nodes>limits.max_nodes{return Err(bad("XML node limit exceeded"));}
        if selfclose {
            if let Some(parent)=stack.last_mut(){parent.children.push(Node::Element(element));}
            else if root.replace(element).is_some(){return Err(bad("multiple XML roots"));}
        } else {
            if stack.len()>=limits.max_depth{return Err(bad("XML tree depth exceeded"));}
            stack.push(element);ns_stack.push(ns);
        }
    }
    if !stack.is_empty(){return Err(bad("unclosed XML elements"));}
    root.ok_or_else(||bad("empty XML document"))
}
/// Decode XML encoded as UTF-8 (with optional BOM) or UTF-16 with a BOM.
pub fn decode_bytes(bytes:&[u8])->Result<String>{
    if bytes.starts_with(&[0xef,0xbb,0xbf]){return std::str::from_utf8(&bytes[3..]).map(str::to_owned).map_err(|_|bad("invalid UTF-8 XML"));}
    if bytes.starts_with(&[0xff,0xfe]) || bytes.starts_with(&[0xfe,0xff]){
        if bytes.len()%2!=0{return Err(bad("odd UTF-16 XML length"));}
        let le=bytes[0]==0xff;
        let units=bytes[2..].chunks_exact(2).map(|c|if le{u16::from_le_bytes([c[0],c[1]])}else{u16::from_be_bytes([c[0],c[1]])});
        return std::char::decode_utf16(units).map(|v|v.map_err(|_|bad("invalid XML UTF-16 surrogate"))).collect();
    }
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|_|bad("invalid UTF-8 XML"))
}
/// Serialize the parsed tree. Namespace and extension attributes survive.
pub fn serialize(element:&Element)->String{
    let mut out=format!("<{}",element.name);
    for (name,value) in &element.attrs {out.push_str(&format!(" {}=\"{}\"",name,escape(value)));}
    if element.children.is_empty(){out.push_str("/>");return out;}
    out.push('>');for n in &element.children{match n{Node::Text(t)=>out.push_str(&escape(t)),Node::Element(e)=>out.push_str(&serialize(e))}}
    out.push_str(&format!("</{}>",element.name));out
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn namespaces(){let e=parse("<w:a xmlns:w='urn:example'><w:b w:id='2'>&lt;ok&gt;</w:b></w:a>").unwrap();assert_eq!(e.child("b").unwrap().attr("w:id"),Some("2"));assert_eq!(e.text(),"<ok>");}
    #[test]fn blocks_entities(){assert!(parse("<!DOCTYPE x [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><x>&x;</x>").is_err());assert!(parse("<a>&unknown;</a>").is_err());assert!(parse("<a x='1' x='2'/>").is_err());}
    #[test]fn bounds(){let limits=Limits{max_depth:1,..Limits::default()};assert!(parse_with_limits("<a><b/></a>",limits).is_ok());assert!(parse_with_limits("<a><b><c/></b></a>",limits).is_err());}
}
