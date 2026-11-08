//! Cross-platform UTF-8 clipboard helpers for native widgets.
//! On Linux these use available Wayland/X11 clipboard utilities, without
//! launching a shell or interpreting copied text as commands.
use std::{io::{self, Write},process::{Command,Stdio}};

fn tool_candidates(copy: bool) -> Vec<(&'static str, Vec<&'static str>)> {
    #[cfg(target_os="linux")] {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let mut commands=Vec::new();
        if wayland {commands.push(if copy {("wl-copy", vec!["--type", "text/plain;charset=utf-8"])} else {("wl-paste",vec!["--no-newline", "--type", "text/plain;charset=utf-8"])});}
        commands.push(if copy {("xclip",vec!["-selection","clipboard","-in"])} else {("xclip",vec!["-selection","clipboard","-out"])});
        commands.push(if copy {("xsel",vec!["--clipboard","--input"])} else {("xsel",vec!["--clipboard","--output"])});
        return commands;
    }
    #[cfg(target_os="macos")] { return vec![if copy {("pbcopy",vec![])} else {("pbpaste",vec![])}]; }
    #[cfg(target_os="windows")] {
        return vec![if copy {("powershell.exe",vec!["-NoProfile","-NonInteractive","-Command","$input | Set-Clipboard"])} else {("powershell.exe",vec!["-NoProfile","-NonInteractive","-Command","Get-Clipboard -Raw"])}];
    }
    #[cfg(not(any(target_os="linux",target_os="macos",target_os="windows")))] {vec![]}
}

/// Copy UTF-8 text into the desktop clipboard.
pub fn copy_text(value: &str) -> io::Result<()> {
    let mut failure=None;
    for (binary,args) in tool_candidates(true) {
        let child=Command::new(binary).args(args).stdin(Stdio::piped())
            .stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        match child {
            Ok(mut child)=>{
                let result=child.stdin.take().expect("piped stdin").write_all(value.as_bytes());
                let status=child.wait()?;
                if result.is_ok() && status.success() {return Ok(());}
                failure=Some(io::Error::other(format!("{binary} clipboard write failed")));
            },
            Err(err)=>failure=Some(err),
        }
    }
    Err(failure.unwrap_or_else(||io::Error::new(io::ErrorKind::Unsupported,"No clipboard provider available")))
}

/// Read current UTF-8 text from the desktop clipboard.
pub fn paste_text() -> io::Result<String> {
    let mut failure=None;
    for (binary,args) in tool_candidates(false) {
        match Command::new(binary).args(args).output() {
            Ok(out) if out.status.success()=>return String::from_utf8(out.stdout)
                .map_err(|err|io::Error::new(io::ErrorKind::InvalidData,err)),
            Ok(_)=>failure=Some(io::Error::other(format!("{binary} clipboard read failed"))),
            Err(err)=>failure=Some(err),
        }
    }
    Err(failure.unwrap_or_else(||io::Error::new(io::ErrorKind::Unsupported,"No clipboard provider available")))
}
