//! XDG Desktop Portal FileChooser client over the Rust std-only D-Bus client.
//!
//! Matches the Request::Response signal *before* sending OpenFile/SaveFile
//! and retains early signals so dialogs cannot lose a fast response.

use std::{ffi::OsString, io, os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf}, time::Duration};
use super::{dbus_std::{Session, Writer, PORTAL_DEST, PORTAL_OBJECT},
    file_dialog::{FileDialog, Kind}};

const REQUEST_PREFIX: &str = "/org/freedesktop/portal/desktop/request/";
const CHOOSER: &str = "org.freedesktop.portal.FileChooser";

/// Only an unavailable portal (before a chooser has been accepted) may fall
/// back to a helper. Never fall back after cancellation or a response error.
#[derive(Debug)]
pub(super) enum PortalError {
    Unavailable(io::Error),
    Failed(io::Error),
}
impl PortalError {
    pub(super) fn into_io(self) -> io::Error {
        match self { Self::Unavailable(e) | Self::Failed(e) => e }
    }
}
fn unavailable(error: impl std::fmt::Display) -> PortalError {
    PortalError::Unavailable(io::Error::new(io::ErrorKind::NotConnected,
        format!("XDG file chooser portal unavailable: {error}")))
}
fn failed(error: impl std::fmt::Display) -> PortalError {
    PortalError::Failed(io::Error::other(format!("XDG file chooser portal: {error}")))
}

pub(super) fn run(dialog: &FileDialog<'_>, multiple: bool)
    -> Result<Option<Vec<PathBuf>>, PortalError>
{
    // Keep portal setup bounded. The actual user dialog may remain open much
    // longer, so increase the timeout *after* subscribing to signals.
    let mut session=Session::connect(Duration::from_secs(4)).map_err(unavailable)?;
    if dialog.kind==Kind::Folder {
        let mut args=Writer::new();
        args.string(CHOOSER);args.string("version");
        let version=session.call(PORTAL_DEST, PORTAL_OBJECT,
            "org.freedesktop.DBus.Properties", "Get", "ss", args)
            .map_err(unavailable)?;
        let version=version.reader().variant_u32().map_err(unavailable)?;
        if version<3 {
            return Err(unavailable(format!("FileChooser version {version} does not support folders")));
        }
    }
    // AddMatch is acknowledged by the bus before OpenFile is sent, preventing
    // a response from escaping the subscription. Signals received during the
    // method reply are queued by Session::call.
    session.add_match("type='signal',interface='org.freedesktop.portal.Request',member='Response',path_namespace='/org/freedesktop/portal/desktop/request'")
        .map_err(unavailable)?;
    session.set_timeout(Duration::from_secs(300)).map_err(unavailable)?;

    let mut args=Writer::new();
    let title=dialog.title.as_deref().unwrap_or(match dialog.kind {
        Kind::Open=>"Open File",Kind::Save=>"Save File",Kind::Folder=>"Select Folder"
    });
    let parent=dialog.portal_parent.clone()
        .unwrap_or_else(||dialog.window.portal_parent_window());
    args.string(&parent); args.string(title);
    build_options(&mut args,dialog,multiple);
    let method=if dialog.kind==Kind::Save {"SaveFile"} else {"OpenFile"};
    let response=session.call(PORTAL_DEST,PORTAL_OBJECT,CHOOSER,method,"ssa{sv}",args)
        .map_err(unavailable)?;
    if response.signature.as_deref()!=Some("o") {
        return Err(failed("portal returned a handle with unexpected type"));
    }
    let handle=response.reader().string().map_err(failed)?.to_owned();
    if !handle.starts_with(REQUEST_PREFIX) {return Err(failed("invalid request object path"));}
    let response=session.wait_signal("org.freedesktop.portal.Request","Response",&handle)
        .map_err(failed)?;
    if response.signature.as_deref()!=Some("ua{sv}") {
        return Err(failed("portal returned an invalid response signature"));
    }
    let (status,uris)=response.reader().portal_response().map_err(failed)?;
    match status {
        0=>{
            let uris=uris.ok_or_else(||failed("successful response contains no uris"))?;
            if uris.is_empty() {return Err(failed("portal selected no paths"));}
            let paths=uris.iter().map(|uri|file_uri_path(uri))
                .collect::<io::Result<Vec<_>>>().map_err(PortalError::Failed)?;
            Ok(Some(paths))
        }
        1=>Ok(None),
        2=>Err(failed("file chooser request failed or was dismissed")),
        other=>Err(failed(format!("unknown response code {other}"))),
    }
}

/// Encode exactly the FileChooser spec's `a{sv}` options. Each dictionary
/// entry has 8-byte alignment; variants carry their own value signature.
fn build_options(args:&mut Writer,dialog:&FileDialog<'_>,multiple:bool){
    args.array(8,|out|{
        option(out,"modal","b",|w|w.u32(1));
        if multiple {option(out,"multiple","b",|w|w.u32(1));}
        if dialog.kind==Kind::Folder {option(out,"directory","b",|w|w.u32(1));}
        if dialog.kind==Kind::Save {
            let filename=dialog.suggested_name.as_deref().or_else(||{
                dialog.initial.as_deref().filter(|p|!p.is_dir())
                    .and_then(Path::file_name).and_then(|name|name.to_str())
            });
            if let Some(name)=filename {
                option(out,"current_name","s",|w|w.string(name));
            }
        }
        if let Some(initial)=dialog.initial.as_ref(){
            let folder=if initial.is_dir(){Some(initial.as_path())}
                else{initial.parent().filter(|p|!p.as_os_str().is_empty())};
            if let Some(folder)=folder {
                let mut bytes=folder.as_os_str().as_bytes().to_vec();bytes.push(0);
                option(out,"current_folder","ay",|w|{
                    w.array(1,|w|w.bytes(&bytes));
                });
            }
        }
        if dialog.kind!=Kind::Folder {
            let filters:Vec<_>=dialog.filters.iter().filter(|f|!f.extensions.is_empty()).collect();
            if !filters.is_empty(){
                option(out,"filters","a(sa(us))",|w|{
                    w.array(8,|w|{
                        for filter in filters {
                            w.structure(|w|{
                                w.string(&filter.name);
                                w.array(8,|w|{
                                    for ext in &filter.extensions {
                                        w.structure(|w|{
                                            w.u32(0);w.string(&format!("*.{ext}"));
                                        });
                                    }
                                });
                            });
                        }
                    });
                });
            }
        }
    });
}
fn option<F:FnOnce(&mut Writer)>(w:&mut Writer,name:&str,sig:&str,value:F){
    w.structure(|w|{w.string(name);w.variant(sig,value);});
}

/// XDG uses file:// URIs, whereas mata-ui returns filesystem paths. Decode
/// percent escapes as filesystem bytes, not UTF-8; Unix filenames need not be
/// UTF-8. Refuse remote authorities and non-file schemes: they cannot be
/// represented safely by this path-only API.
fn file_uri_path(uri: &str) -> io::Result<PathBuf> {
    let rest = uri.strip_prefix("file://").ok_or_else(|| invalid_uri("not a file URI"))?;
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    if !path.starts_with('/') { return Err(invalid_uri("non-local file URI")); }
    let raw = path.as_bytes();
    let mut decoded = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            if i + 2 >= raw.len() { return Err(invalid_uri("incomplete percent escape")); }
            let h = hex(raw[i + 1]).ok_or_else(|| invalid_uri("invalid percent escape"))?;
            let l = hex(raw[i + 2]).ok_or_else(|| invalid_uri("invalid percent escape"))?;
            decoded.push((h << 4) | l);
            i += 3;
        } else {
            decoded.push(raw[i]); i += 1;
        }
    }
    if decoded.contains(&0) { return Err(invalid_uri("NUL in filesystem path")); }
    Ok(PathBuf::from(OsString::from_vec(decoded)))
}
fn hex(b: u8) -> Option<u8> {
    match b { b'0'..=b'9' => Some(b - b'0'), b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10), _ => None }
}
fn invalid_uri(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn percent_decodes_local_files() {
        assert_eq!(file_uri_path("file:///home/u/My%20Folder/%F0%9F%8C%B7.png").unwrap(),
            PathBuf::from("/home/u/My Folder/🌷.png"));
        assert_eq!(file_uri_path("file://localhost/tmp/a%23b").unwrap(),
            PathBuf::from("/tmp/a#b"));
    }
    #[test]
    fn rejects_nonlocal_and_malformed_uris() {
        for uri in ["https://example.com/a", "file://remotehost/a", "file:///a%XZ",
            "file:///bad%", "file:///a%00b"] {
            assert!(file_uri_path(uri).is_err(), "{uri}");
        }
    }
    #[test]
    fn preserves_non_utf8_filenames() {
        use std::os::unix::ffi::OsStrExt;
        let path = file_uri_path("file:///tmp/%FF").unwrap();
        assert_eq!(path.as_os_str().as_bytes(), b"/tmp/\xff");
    }
}
