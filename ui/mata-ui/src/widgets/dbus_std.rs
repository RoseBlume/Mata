//! Minimal *client-only* D-Bus session transport for the two XDG portal APIs
//! used by Mata UI. Linux `std` only: no FFI, subprocess, or extra crate.
//!
//! Supported: Unix path/abstract sockets, EXTERNAL auth, Hello, method calls,
//! method returns/errors, signal subscriptions and a bounded message decoder.
//! Not a general-purpose bus implementation (no FD passing, TCP or services).
use std::{env, fs, io::{self, Read, Write}, os::{linux::net::SocketAddrExt,
    unix::{ffi::OsStringExt, net::{SocketAddr, UnixStream}}}, path::Path, time::Duration};

const MAX_MESSAGE: usize = 16 * 1024 * 1024;
const DESKTOP: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub(super) const PORTAL_DEST: &str = DESKTOP;
pub(super) const PORTAL_OBJECT: &str = PORTAL_PATH;

fn invalid(detail: &str) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, detail.to_owned()) }
fn align_up(n: usize, alignment: usize) -> usize { (n + alignment - 1) & !(alignment - 1) }

/// D-Bus values are aligned from the start of a body or header-field array.
#[derive(Default)]
pub(super) struct Writer { data: Vec<u8> }
impl Writer {
    pub(super) fn new() -> Self { Self::default() }
    fn align(&mut self, n: usize) { self.data.resize(align_up(self.data.len(), n), 0); }
    pub(super) fn byte(&mut self, value: u8) { self.data.push(value); }
    pub(super) fn u32(&mut self, value: u32) {
        self.align(4); self.data.extend_from_slice(&value.to_le_bytes());
    }
    pub(super) fn string(&mut self, value: &str) {
        self.align(4);
        self.u32(value.len() as u32);
        self.data.extend_from_slice(value.as_bytes()); self.byte(0);
    }
    fn signature(&mut self, value: &str) {
        debug_assert!(value.len() < 256);
        self.byte(value.len() as u8);
        self.data.extend_from_slice(value.as_bytes()); self.byte(0);
    }
    pub(super) fn variant<F: FnOnce(&mut Self)>(&mut self, signature: &str, encode: F) {
        self.signature(signature);
        self.align(type_alignment(signature.as_bytes().first().copied().unwrap_or(b'y')));
        encode(self);
    }
    /// D-Bus array length excludes the element-alignment padding.
    pub(super) fn array<F: FnOnce(&mut Self)>(&mut self, element_alignment: usize, encode: F) {
        self.align(4);
        let length_pos = self.data.len(); self.u32(0);
        self.align(element_alignment);
        let start = self.data.len(); encode(self);
        let len = (self.data.len() - start) as u32;
        self.data[length_pos..length_pos + 4].copy_from_slice(&len.to_le_bytes());
    }
    pub(super) fn structure<F: FnOnce(&mut Self)>(&mut self, encode: F) {
        self.align(8); encode(self);
    }
    pub(super) fn bytes(&mut self, data: &[u8]) { self.data.extend_from_slice(data); }
}

fn type_alignment(ty: u8) -> usize {
    match ty { b'x'|b't'|b'd'|b'('|b'{' => 8,
        b'u'|b'i'|b'b'|b'h'|b's'|b'o'|b'a' => 4,
        b'n'|b'q' => 2, _ => 1 }
}

pub(super) struct Reader<'a> { data: &'a [u8], pos: usize, little: bool }
impl<'a> Reader<'a> {
    pub(super) fn new(data: &'a [u8], little: bool) -> Self { Self { data, pos: 0, little } }
    fn align(&mut self, n: usize) -> io::Result<()> {
        self.pos = align_up(self.pos,n);
        if self.pos > self.data.len() { return Err(invalid("D-Bus alignment outside message")); }
        Ok(())
    }
    fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        let end = self.pos.checked_add(count).ok_or_else(||invalid("D-Bus length overflow"))?;
        let v = self.data.get(self.pos..end).ok_or_else(||invalid("truncated D-Bus value"))?;
        self.pos = end; Ok(v)
    }
    fn byte(&mut self) -> io::Result<u8> { Ok(self.take(1)?[0]) }
    pub(super) fn u32(&mut self) -> io::Result<u32> {
        self.align(4)?;
        let b: [u8;4] = self.take(4)?.try_into().unwrap();
        Ok(if self.little {u32::from_le_bytes(b)}else{u32::from_be_bytes(b)})
    }
    fn len_string(&mut self, len: usize) -> io::Result<&'a str> {
        if len > MAX_MESSAGE { return Err(invalid("D-Bus string too long")); }
        let bytes=self.take(len)?;
        if self.byte()? != 0 || bytes.contains(&0) {return Err(invalid("bad D-Bus string terminator"));}
        std::str::from_utf8(bytes).map_err(|_|invalid("invalid D-Bus UTF-8"))
    }
    pub(super) fn string(&mut self) -> io::Result<&'a str> {
        let len=self.u32()? as usize; self.len_string(len)
    }
    fn signature(&mut self) -> io::Result<&'a str> {
        let n=self.byte()? as usize; self.len_string(n)
    }
    pub(super) fn array(&mut self, element_alignment: usize) -> io::Result<usize> {
        let len=self.u32()? as usize;
        self.align(element_alignment)?;
        let end=self.pos.checked_add(len).ok_or_else(||invalid("D-Bus array overflow"))?;
        if end>self.data.len() || len>MAX_MESSAGE {return Err(invalid("truncated D-Bus array"));}
        Ok(end)
    }
    pub(super) fn string_array(&mut self) -> io::Result<Vec<String>> {
        let end=self.array(4)?;
        let mut values=Vec::new();
        while self.pos < end {
            if values.len() > 65536 { return Err(invalid("too many D-Bus strings")); }
            values.push(self.string()?.to_owned());
        }
        if self.pos != end { return Err(invalid("D-Bus array exceeds length")); }
        Ok(values)
    }
    pub(super) fn variant_u32(&mut self) -> io::Result<u32> {
        for _ in 0..4 {
            let signature=self.signature()?;
            if signature == "u" { return self.u32(); }
            if signature != "v" { return Err(invalid("expected uint32 portal value")); }
        }
        Err(invalid("too many nested D-Bus variants"))
    }
    pub(super) fn variant_strings(&mut self) -> io::Result<Vec<String>> {
        let sig=self.signature()?;
        if sig!="as" {return Err(invalid("expected string-array portal value"));}
        self.string_array()
    }
    pub(super) fn portal_response(&mut self) -> io::Result<(u32, Option<Vec<String>>)> {
        let response=self.u32()?;
        let end=self.array(8)?;
        let mut uris=None;
        while self.pos < end {
            self.align(8)?;
            let key=self.string()?.to_owned();
            let signature=self.signature()?;
            if key=="uris" {
                if signature!="as" {return Err(invalid("portal uris have wrong type"));}
                uris=Some(self.string_array()?);
            }else{
                self.skip_value(signature,0)?;
            }
        }
        if self.pos!=end {return Err(invalid("portal results exceed D-Bus array"));}
        Ok((response,uris))
    }
    /// Skip any well-formed wire value, including nested variant/struct/array.
    fn skip_value(&mut self, sig: &str, depth: usize) -> io::Result<()> {
        if depth > 32 { return Err(invalid("D-Bus type nesting limit")); }
        let b=sig.as_bytes();
        let Some(&kind)=b.first() else {return Err(invalid("empty D-Bus signature"));};
        self.align(type_alignment(kind))?;
        match kind {
            b'y' => {self.take(1)?;}
            b'n'|b'q' => {self.take(2)?;}
            b'b'|b'u'|b'i'|b'h' => {self.take(4)?;}
            b't'|b'x'|b'd' => {self.take(8)?;}
            b's'|b'o' => {self.string()?;}
            b'g' => {self.signature()?;}
            b'v' => {let nested=self.signature()?;self.skip_value(nested,depth+1)?;}
            b'a' => {
                let item=&sig[1..];
                let alignment=type_alignment(item.as_bytes().first().copied().ok_or_else(||invalid("bad array signature"))?);
                let end=self.array(alignment)?;
                self.pos=end;
            }
            b'('|b'{' => {
                let closing=if kind==b'(' {b')'} else {b'}'};
                if b.last()!=Some(&closing) {return Err(invalid("invalid struct signature"));}
                let mut at=1;
                while at+1 < b.len() {
                    let next=type_end(b,at)?;
                    self.skip_value(&sig[at..next],depth+1)?;
                    at=next;
                }
            }
            _ => return Err(invalid("unsupported D-Bus type")),
        }
        Ok(())
    }
}
fn type_end(sig:&[u8],start:usize)->io::Result<usize>{
    let ty=*sig.get(start).ok_or_else(||invalid("truncated type signature"))?;
    match ty {
        b'a'=>type_end(sig,start+1),
        b'('|b'{'=>{
            let close=if ty==b'(' {b')'}else{b'}'};
            let mut pos=start+1;
            loop {
                let n=*sig.get(pos).ok_or_else(||invalid("unterminated type signature"))?;
                if n==close{return Ok(pos+1);}
                pos=type_end(sig,pos)?;
            }
        }
        b'y'|b'b'|b'n'|b'q'|b'i'|b'u'|b'x'|b't'|b'd'|b'h'|b's'|b'o'|b'g'|b'v' =>Ok(start+1),
        _=>Err(invalid("invalid D-Bus signature element"))
    }
}

#[derive(Debug)]
pub(super) struct Message {
    kind: u8,
    reply_serial: Option<u32>,
    pub(super) path: Option<String>,
    pub(super) interface: Option<String>,
    pub(super) member: Option<String>,
    pub(super) signature: Option<String>,
    body: Vec<u8>,
    little: bool,
    error_name: Option<String>,
}
impl Message {
    pub(super) fn reader(&self)->Reader<'_>{Reader::new(&self.body,self.little)}
    pub(super) fn is_signal(&self,interface:&str,member:&str)->bool{
        self.kind==4 && self.interface.as_deref()==Some(interface) && self.member.as_deref()==Some(member)
    }
}

pub(super) struct Session {
    stream: UnixStream,
    serial: u32,
    queued: Vec<Message>,
}
impl Session {
    pub(super) fn connect(timeout: Duration) -> io::Result<Self> {
        let mut stream=connect_bus()?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(Duration::from_secs(4)))?;
        authenticate(&mut stream)?;
        let mut session=Self{stream,serial:0,queued:Vec::new()};
        let hello=session.call("org.freedesktop.DBus","/org/freedesktop/DBus",
            "org.freedesktop.DBus","Hello","",Writer::new())?;
        let mut reader=hello.reader();
        if hello.signature.as_deref()!=Some("s") || !reader.string()?.starts_with(':') {
            return Err(invalid("invalid D-Bus Hello response"));
        }
        Ok(session)
    }
    pub(super) fn set_timeout(&self, duration: Duration) -> io::Result<()> {
        self.stream.set_read_timeout(Some(duration))
    }
    pub(super) fn add_match(&mut self, rule: &str)->io::Result<()> {
        let mut body=Writer::new();body.string(rule);
        self.call("org.freedesktop.DBus","/org/freedesktop/DBus",
            "org.freedesktop.DBus","AddMatch","s",body)?;
        Ok(())
    }
    pub(super) fn call(&mut self, dest:&str,path:&str,interface:&str,member:&str,
        signature:&str,body:Writer)->io::Result<Message>{
        self.serial=self.serial.checked_add(1).ok_or_else(||invalid("D-Bus serial exhausted"))?;
        let serial=self.serial;
        let mut fields=Writer::new();
        header_field(&mut fields,1,"o",path);
        header_field(&mut fields,2,"s",interface);
        header_field(&mut fields,3,"s",member);
        header_field(&mut fields,6,"s",dest);
        if !signature.is_empty(){header_field(&mut fields,8,"g",signature);}
        let wire=encode_message(1,serial,fields,body)?;
        self.stream.write_all(&wire)?;
        loop {
            let msg=read_message(&mut self.stream)?;
            if msg.reply_serial==Some(serial) && (msg.kind==2 || msg.kind==3) {
                if msg.kind==3 {
                    let description=msg.reader().string().unwrap_or("remote call failed").to_owned();
                    return Err(io::Error::other(format!("{}: {}",
                        msg.error_name.as_deref().unwrap_or("D-Bus error"),description)));
                }
                return Ok(msg);
            }
            if msg.kind==4 {
                if self.queued.len()>=64 {self.queued.remove(0);}
                self.queued.push(msg);
            }
        }
    }
    pub(super) fn wait_signal(&mut self,iface:&str,member:&str,path:&str)->io::Result<Message>{
        if let Some(index)=self.queued.iter().position(|m|m.is_signal(iface,member)
            && m.path.as_deref()==Some(path)) {
            return Ok(self.queued.remove(index));
        }
        loop {
            let msg=read_message(&mut self.stream)?;
            if msg.is_signal(iface,member) && msg.path.as_deref()==Some(path) {
                return Ok(msg);
            }
        }
    }
}
fn header_field(fields:&mut Writer,code:u8,signature:&str,value:&str){
    fields.align(8); fields.byte(code);
    fields.variant(signature,|out|if signature=="g" {out.signature(value)} else {out.string(value)});
}
fn encode_message(kind:u8,serial:u32,fields:Writer,body:Writer)->io::Result<Vec<u8>>{
    if body.data.len()>MAX_MESSAGE || fields.data.len()>MAX_MESSAGE{return Err(invalid("message too large"));}
    let mut wire=Vec::with_capacity(16+fields.data.len()+8+body.data.len());
    wire.extend_from_slice(&[b'l',kind,0,1]);
    wire.extend_from_slice(&(body.data.len() as u32).to_le_bytes());
    wire.extend_from_slice(&serial.to_le_bytes());
    wire.extend_from_slice(&(fields.data.len() as u32).to_le_bytes());
    wire.extend_from_slice(&fields.data);
    wire.resize(align_up(wire.len(),8),0);
    wire.extend_from_slice(&body.data);
    Ok(wire)
}
fn read_message(stream:&mut UnixStream)->io::Result<Message>{
    let mut fixed=[0u8;16];stream.read_exact(&mut fixed)?;
    let little=match fixed[0]{b'l'=>true,b'B'=>false,_=>return Err(invalid("invalid D-Bus byte order"))};
    if fixed[3]!=1{return Err(invalid("unsupported D-Bus protocol version"));}
    let uint=|bytes:&[u8]|->u32{
        let b:[u8;4]=bytes.try_into().unwrap();
        if little{u32::from_le_bytes(b)}else{u32::from_be_bytes(b)}
    };
    let body_len=uint(&fixed[4..8]) as usize;
    let fields_len=uint(&fixed[12..16]) as usize;
    if body_len>MAX_MESSAGE || fields_len>MAX_MESSAGE || body_len+fields_len>MAX_MESSAGE {
        return Err(invalid("D-Bus message exceeds size limit"));
    }
    let header_pad=align_up(16+fields_len,8)-16;
    let mut headers=vec![0u8;header_pad];stream.read_exact(&mut headers)?;
    let mut body=vec![0u8;body_len];stream.read_exact(&mut body)?;
    let mut r=Reader::new(&headers[..fields_len],little);
    let mut msg=Message{kind:fixed[1],reply_serial:None,path:None,interface:None,
        member:None,signature:None,body,little,error_name:None};
    while r.pos < fields_len {
        r.align(8)?;
        if r.pos == fields_len {break;}
        let field=r.byte()?;
        let signature=r.signature()?;
        match (field,signature) {
            (5,"u") => msg.reply_serial=Some(r.u32()?),
            (1,"o") => msg.path=Some(r.string()?.to_owned()),
            (2,"s") => msg.interface=Some(r.string()?.to_owned()),
            (3,"s") => msg.member=Some(r.string()?.to_owned()),
            (4,"s") => msg.error_name=Some(r.string()?.to_owned()),
            (8,"g") => msg.signature=Some(r.signature()?.to_owned()),
            _=>r.skip_value(signature,0)?,
        }
    }
    Ok(msg)
}
fn connect_bus()->io::Result<UnixStream>{
    if let Ok(bus)=env::var("DBUS_SESSION_BUS_ADDRESS") {
        for address in bus.split(';') {
            let Some(parts)=address.strip_prefix("unix:") else {continue};
            for part in parts.split(',') {
                if let Some(encoded)=part.strip_prefix("path=") {
                    let path=decode_address(encoded)?;
                    if let Ok(stream)=UnixStream::connect(Path::new(&std::ffi::OsString::from_vec(path))) {
                        return Ok(stream);
                    }
                }
                if let Some(encoded)=part.strip_prefix("abstract=") {
                    let bytes=decode_address(encoded)?;
                    if let Ok(address)=SocketAddr::from_abstract_name(&bytes) {
                        if let Ok(stream)=UnixStream::connect_addr(&address) {return Ok(stream);}
                    }
                }
            }
        }
    }
    if let Some(runtime)=env::var_os("XDG_RUNTIME_DIR") {
        if let Ok(stream)=UnixStream::connect(Path::new(&runtime).join("bus")){return Ok(stream);}
    }
    Err(io::Error::new(io::ErrorKind::NotConnected,"no supported D-Bus session socket"))
}
fn decode_address(encoded:&str)->io::Result<Vec<u8>>{
    let mut result=Vec::with_capacity(encoded.len());let bytes=encoded.as_bytes();let mut i=0;
    while i<bytes.len(){
        if bytes[i]==b'%' {
            if i+2>=bytes.len(){return Err(invalid("bad percent-encoded D-Bus address"));}
            let h=(bytes[i+1] as char).to_digit(16).ok_or_else(||invalid("bad D-Bus address"))?;
            let l=(bytes[i+2] as char).to_digit(16).ok_or_else(||invalid("bad D-Bus address"))?;
            result.push(((h<<4)|l) as u8);i+=3;
        }else{result.push(bytes[i]);i+=1;}
    }
    if result.contains(&0){return Err(invalid("NUL in D-Bus address"));}
    Ok(result)
}
fn authenticate(stream:&mut UnixStream)->io::Result<()>{
    // Linux exposes the real/effective UID without relying on libc FFI.
    let status=fs::read_to_string("/proc/self/status")?;
    let uid=status.lines().find_map(|line|line.strip_prefix("Uid:")
        .and_then(|line|line.split_whitespace().next()))
        .ok_or_else(||invalid("cannot determine Linux UID"))?;
    let mut command=Vec::from(&b"\0AUTH EXTERNAL "[..]);
    for ch in uid.bytes() {command.extend_from_slice(format!("{ch:02X}").as_bytes());}
    command.extend_from_slice(b"\r\n");stream.write_all(&command)?;
    let mut line=Vec::new();
    loop {
        let mut b=[0]; stream.read_exact(&mut b)?;
        if b[0]==b'\n'{break;}
        if line.len()>512{return Err(invalid("invalid D-Bus authentication response"));}
        line.push(b[0]);
    }
    if !line.starts_with(b"OK ") {return Err(io::Error::new(io::ErrorKind::PermissionDenied,
        format!("session bus rejected EXTERNAL auth: {}",String::from_utf8_lossy(&line))));}
    stream.write_all(b"BEGIN\r\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn array_length_excludes_padding() {
        let mut w=Writer::new();w.byte(7);
        w.array(8,|w|w.structure(|w|{w.u32(3);w.string("ab");}));
        let mut r=Reader::new(&w.data,true);
        assert_eq!(r.byte().unwrap(),7);
        let end=r.array(8).unwrap();r.align(8).unwrap();
        assert_eq!(r.u32().unwrap(),3);assert_eq!(r.string().unwrap(),"ab");
        assert_eq!(r.pos,end);
    }
    #[test] fn nested_variants_and_skips() {
        let mut w=Writer::new();w.variant("v",|w|w.variant("u",|w|w.u32(1)));
        assert_eq!(Reader::new(&w.data,true).variant_u32().unwrap(),1);
        let mut r=Reader::new(&w.data,true);r.skip_value("v",0).unwrap();
        assert_eq!(r.pos,w.data.len());
    }
    #[test] fn percent_decodes_bus_address() {
        assert_eq!(decode_address("/run/user/1000/my%2Cbus").unwrap(),b"/run/user/1000/my,bus");
    }
    #[test] fn message_headers_and_portal_result() {
        // A simulated Request.Response signal, including a result field that
        // this client does not interpret (it must still skip it correctly).
        let mut fields=Writer::new();
        header_field(&mut fields,1,"o","/org/freedesktop/portal/desktop/request/1_30/mata_1");
        header_field(&mut fields,2,"s","org.freedesktop.portal.Request");
        header_field(&mut fields,3,"s","Response");
        header_field(&mut fields,8,"g","ua{sv}");
        let mut body=Writer::new();body.u32(0);
        body.array(8,|w|{
            w.structure(|w|{
                w.string("choices");
                w.variant("a(ss)",|w|w.array(8,|w|{
                    w.structure(|w|{w.string("theme");w.string("dark");});
                }));
            });
            w.structure(|w|{
                w.string("uris");
                w.variant("as",|w|w.array(4,|w|{
                    w.string("file:///tmp/StudyForge%20Library.json");
                }));
            });
        });
        let wire=encode_message(4,42,fields,body).unwrap();
        let (mut sender,mut receiver)=UnixStream::pair().unwrap();
        sender.write_all(&wire).unwrap();
        let message=read_message(&mut receiver).unwrap();
        assert!(message.is_signal("org.freedesktop.portal.Request","Response"));
        assert_eq!(message.path.as_deref(),Some("/org/freedesktop/portal/desktop/request/1_30/mata_1"));
        assert_eq!(message.signature.as_deref(),Some("ua{sv}"));
        let (status,uris)=message.reader().portal_response().unwrap();
        assert_eq!(status,0);
        assert_eq!(uris.unwrap(),vec!["file:///tmp/StudyForge%20Library.json"]);
    }
    #[test] fn big_endian_message_header_fields() {
        // A server may use big-endian framing even on a little-endian host.
        let (mut sender,mut receiver)=UnixStream::pair().unwrap();
        let mut wire=vec![b'B',2,0,1];
        wire.extend_from_slice(&4u32.to_be_bytes());
        wire.extend_from_slice(&19u32.to_be_bytes());
        wire.extend_from_slice(&0u32.to_be_bytes());
        wire.extend_from_slice(&7u32.to_be_bytes());
        sender.write_all(&wire).unwrap();
        let msg=read_message(&mut receiver).unwrap();
        assert_eq!(msg.reader().u32().unwrap(),7);
    }
}
