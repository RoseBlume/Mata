# MIVL v0.6 — Rust ↔ MIVL command and event bridge

All code in this update uses **Rust `std` and existing Mata crates only**. No Tokio, futures crate, syn, or quote.

## Commands, including nested Rust modules

Functions decorated with `#[mivl::command]` expose a registrar in the *type namespace*, so `resources::add_numbers::register()` is possible while `resources::add_numbers(...)` remains an ordinary function.

```rust
use mivl::command;

mod resources {
    use mivl::command;
    #[command]
    pub fn add_numbers(a: i32, b: i32) -> i32 { a + b }
    #[command]
    pub async fn multiply(a: i32, b: i32) -> i32 { a * b }
}
#[command]
fn greet(name: String) -> String { format!("Hello, {name}") }

fn main() -> mivl::Result<()> {
    // Registers on this thread for Engine / MainWindow instances created *after* it.
    mivl::register_commands![
        resources::add_numbers,
        resources::multiply,
        greet
    ];

    let mut engine = mivl::Engine::parse(include_str!("ui/main.mivl"))?;
    // If the Engine has already been constructed, use the explicit variant instead:
    // mivl::register_commands![in &mut engine; resources::add_numbers, greet]?;
    Ok(())
}
```

`register_commands![...]` **replaces** the current thread's default set; it does *not* mutate already-instantiated windows. It is thread-local, not process-global. Use `clear_default_commands()` to reset between embedded applications. Prefer `register_commands![in &mut engine; ...]?` for independent multi-window registration. Existing `register_greet(engine)?` functions remain supported. Duplicate command names fail with an error when installing commands on an engine.

## MIVL → synchronous Rust

```qml
Button {
    onClicked: { result = invoke("add_numbers", 20, 22) }
}
```

## MIVL → asynchronous Rust

```qml
Window {
    property int requestId: 0
    property var answer: null
    property string lastError: ""

    // Emitted by Engine::pump_messages() on the UI thread.
    onCommandCompleted: { answer = commandResult }
    onCommandFailed: { lastError = commandError }

    Button {
        onClicked: { requestId = invoke_async("multiply", 6, 7) }
    }
}
```

`invoke_async()` immediately returns a numeric request ID (not the command result). When it finishes, `Window.commandName`, `Window.commandRequestId`, `Window.commandResult`, and `Window.commandError` are set and `onCommandCompleted` or `onCommandFailed` fires. The result is handled on the UI thread, never on the worker thread. If multiple tasks complete out of order, **match `commandRequestId` against your stored request ID** in the event handler (e.g. via the ternary expression). Calling asynchronous commands through `invoke()` raises an error. Commands run on lightweight OS threads with an std-only, condvar-backed future executor. Futures must own their input and be `Send + 'static`.

Callbacks returning a `Result<T, E>` propagate errors via `IntoCommandResult`, including async callbacks.

## Rust → MIVL events

On the Rust side, get a thread-safe event publisher **before** moving the engine into native UI:

```rust
let mut engine = mivl::Engine::parse(include_str!("ui/main.mivl"))?;
let frontend = engine.frontend_handle();
std::thread::spawn(move || {
    frontend.emit("message", "Hello from Rust").unwrap();
});
// engine.pump_messages()?; for manual/headless applications.
// mivl::native::NativeApp::new(engine)?.run()?; for desktop UI (automatic pumping).
```

On the MIVL side, a nonvisual `EventListener` handles matching messages by event name:

```qml
Window {
    property string textFromRust: ""
    property int limitedTotal: 0

    EventListener {
        id: once
        event: "message"
        // Default: times: 1
        onTriggered: { textFromRust = eventData }
    }
    EventListener {
        id: limited
        event: "counter"
        times: 5
        onTriggered: { limitedTotal += eventData }
    }
    EventListener {
        id: persistent
        event: "message"
        repeat: true
        enabled: true
        onTriggered: { textFromRust = eventData }
    }
    Button {
        text: "Toggle notifications"
        onClicked: { persistent.enabled = !persistent.enabled }
    }
}
```

Listener rules:

- `times` defaults to **1** (one-shot).
- `times: N` permits exactly **N** successful deliveries.
- `times: -1` or `repeat: true` accepts unlimited events until disabled.
- `enabled: false` pauses event handling immediately. Re-enabling does **not** reset the counter; Rust can call `engine.reset_listener("id")` to re-arm a completed one-shot listener.
- `eventData` contains the payload, `eventName` the event name, and `onTriggered` is an ordinary MIVL event block.
- `Engine::dispatch_frontend(name, payload)` delivers immediately when on the UI thread; `FrontendHandle::emit(name, payload)` queues a thread-safe message; `Engine::pump_messages()` drains it.
- Listener delivery counts survive list virtualization/recomposition (static nodes preserve IDs).

## Notes and limitations

- The frontend event queue uses `std::sync::mpsc` and is unbounded; do not flood it with untrusted traffic.
- Each async request currently gets its own OS thread, with at most **128** concurrent workers. Additional requests fail explicitly; for high-volume workloads, a worker pool or queue would be more efficient.
- No cancellation API for already-running async command futures; close/drop the receiving engine to ignore further completions.
- Host command arguments must be owned types implementing `FromCommandArg`; return types must implement `IntoCommandResult`. Generic functions, unsafe functions, and `self` methods are not supported by `#[command]` yet.
- Host callbacks are not transactional. If subsequent MIVL statements fail, prior Rust side effects and already-spawned async calls cannot be undone.
- The `.mivl` files remain interpreted, despite the `include_modules!()` build step.
