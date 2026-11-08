//! Thread-safe, std-only dispatch for Rust -> MIVL messages and async command completions.
use std::{future::Future, pin::Pin, sync::{Arc, Condvar, Mutex, mpsc, atomic::{AtomicU64, AtomicUsize, Ordering}}, task::{Context, Poll, Wake, Waker}, thread};
use crate::{Error, Result, Value, Engine};

/// A clonable, Send handle used by worker threads to publish events to the UI thread.
#[derive(Clone, Debug)]
pub struct FrontendHandle(pub(crate) mpsc::Sender<FrontendEvent>);
#[derive(Debug, Clone)]
pub(crate) struct FrontendEvent { pub name: String, pub payload: Value }
impl FrontendHandle {
    /// Queue an event; delivery occurs on Engine::pump_messages(), never on this worker thread.
    pub fn emit(&self, event: impl Into<String>, payload: impl Into<Value>) -> Result<()> {
        let name=event.into();
        if name.is_empty() || name.len()>256 {return Err(Error::command("invalid frontend event name"));}
        self.0.send(FrontendEvent {name,payload:payload.into()})
            .map_err(|_|Error::command("frontend engine has been dropped"))
    }
}

/// An asynchronous command returns an owned, Send future. No external executor is needed.
pub type CommandFuture = Pin<Box<dyn Future<Output=Result<Value>> + Send + 'static>>;
#[derive(Debug)]
pub(crate) struct Completion {pub id: u64, pub name: String, pub result: Result<Value>}

#[derive(Debug)]
struct WakerGate { ready: Mutex<bool>, signal: Condvar }
impl Wake for WakerGate {
    fn wake(self: Arc<Self>) {self.wake_by_ref();}
    fn wake_by_ref(self: &Arc<Self>) {
        if let Ok(mut ready)=self.ready.lock() {*ready=true; self.signal.notify_one();}
    }
}
/// Sleep between future wakeups (no spin polling). Each invocation uses its own worker.
fn run_future(mut future: CommandFuture) -> Result<Value> {
    let gate=Arc::new(WakerGate{ready:Mutex::new(false),signal:Condvar::new()});
    let waker=Waker::from(gate.clone());
    let mut cx=Context::from_waker(&waker);
    loop {
        *gate.ready.lock().map_err(|_|Error::command("async waker poisoned"))?=false;
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value)=>return value,
            Poll::Pending=>{
                let mut ready=gate.ready.lock().map_err(|_|Error::command("async waker poisoned"))?;
                while !*ready {ready=gate.signal.wait(ready).map_err(|_|Error::command("async waker poisoned"))?;}
            }
        }
    }
}

pub(crate) fn spawn_command(id: u64, name: String, future: CommandFuture, sender: mpsc::Sender<Completion>, active: Arc<AtomicUsize>) -> Result<()> {
    // Bound the number of worker threads; callers can retry later.
    active.fetch_update(Ordering::AcqRel,Ordering::Acquire,|count| (count < 128).then_some(count+1))
        .map_err(|_|Error::command("too many concurrent asynchronous commands (limit 128)"))?;
    let release=active.clone();
    let launched=thread::Builder::new().name(format!("mivl-async-{id}")).spawn(move || {
        // Catch panics so an async worker cannot silently strand its caller.
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||run_future(future)))
            .unwrap_or_else(|_|Err(Error::command("async command panicked")));
        let _=sender.send(Completion{id,name,result});
        release.fetch_sub(1,Ordering::AcqRel);
    });
    match launched {
        Ok(_handle)=>Ok(()),
        Err(error)=> {active.fetch_sub(1,Ordering::AcqRel);Err(Error::command(format!("cannot start async command: {error}")))}
    }
}

/// A request ID factory shared across engine re-compositions.
#[derive(Debug, Clone)]
pub(crate) struct Requests {seq: Arc<AtomicU64>, active: Arc<AtomicUsize>}
impl Default for Requests {fn default()->Self {Self{seq:Arc::new(AtomicU64::new(1)),active:Arc::new(AtomicUsize::new(0))}}}
impl Requests {
    pub fn active(&self)->Arc<AtomicUsize> {self.active.clone()}
    pub fn next(&self)->Result<u64> {
        let id=self.seq.fetch_add(1,Ordering::Relaxed);
        if id>9_007_199_254_740_991 {return Err(Error::command("async request ID limit reached"));}
        Ok(id)
    }
}

// A no-argument register_commands![...] statement installs these registrars for
// engines created later on the current UI thread. Explicit-engine form works
// for already-created engines and is preferable in multi-window applications.
pub type CommandRegistrar = fn(&mut Engine) -> Result<()>;
thread_local! {
    static DEFAULT_COMMANDS: std::cell::RefCell<Vec<CommandRegistrar>> = const {std::cell::RefCell::new(Vec::new())};
}
/// Replace current-thread default registrations (does not mutate existing engines).
pub fn install_default_commands(registrars: &[CommandRegistrar]) {
    DEFAULT_COMMANDS.with(|defaults| {*defaults.borrow_mut()=registrars.to_vec();});
}
/// Clear command defaults, useful when initializing isolated applications/tests.
pub fn clear_default_commands() {install_default_commands(&[])}
pub(crate) fn apply_defaults(engine: &mut Engine)->Result<()> {
    DEFAULT_COMMANDS.with(|defaults| {
        for registrar in defaults.borrow().iter() { registrar(engine)?; }
        Ok(())
    })
}
