//! Console interrupt handling via operating-system APIs, without ctrlc crate.
use std::sync::atomic::{AtomicBool, Ordering};
use crate::{bail, Result};
static STOP: AtomicBool = AtomicBool::new(false);
pub fn requested() -> bool {STOP.load(Ordering::SeqCst)}
#[cfg(unix)]
extern "C" fn handler(_:i32){STOP.store(true, Ordering::SeqCst);}
#[cfg(unix)]
unsafe extern "C" {fn signal(sig:i32, action:extern "C" fn(i32))->usize;}
#[cfg(windows)]
unsafe extern "system" fn handler(code:u32)->i32 {
    if code==0 || code==1 {STOP.store(true,Ordering::SeqCst);1}else{0}
}
#[cfg(windows)]
unsafe extern "system" {
    fn SetConsoleCtrlHandler(handler:Option<unsafe extern "system" fn(u32)->i32>,add:i32)->i32;
}
pub fn install()->Result<()> {
    STOP.store(false,Ordering::SeqCst);
    #[cfg(unix)]unsafe {signal(2,handler);signal(15,handler);}
    #[cfg(windows)]unsafe {if SetConsoleCtrlHandler(Some(handler),1)==0{return bail("cannot install console interrupt handler");}}
    #[cfg(not(any(unix,windows)))]{return bail("unsupported platform for console interrupt handling");}
    Ok(())
}
