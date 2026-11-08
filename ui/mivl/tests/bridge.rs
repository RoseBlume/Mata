use mivl::{command, Engine, Value};

#[command]
fn greet(name: String) -> String {format!("Hello {name}")}

pub mod resources {
    use mivl::command;
    #[command]
    pub fn add_numbers(a: i32, b: i32) -> i32 {a+b}
    #[command]
    pub async fn multiply(a: i32, b: i32) -> i32 {a*b}
}

#[test]
fn exact_macro_registers_nested_modules_and_simple_functions() {
    mivl::register_commands![resources::add_numbers, greet];
    let mut app=Engine::parse(r#"
        Window {
            property int total: 0
            property string hello: ""
            Button {id: go; onClicked: {
                total = invoke("add_numbers", 20, 22)
                hello = invoke("greet", "MIVL")
            }}
        }
    "#).unwrap();
    app.emit("go","clicked").unwrap();
    assert_eq!(app.get("total").unwrap(),Value::Number(42.0));
    assert_eq!(app.get("hello").unwrap(),Value::String("Hello MIVL".into()));
    mivl::clear_default_commands();
}

#[test]
fn explicit_engine_registration_works_after_parse() {
    let mut engine=Engine::parse("Window { }").unwrap();
    mivl::register_commands![in &mut engine; resources::add_numbers, greet].unwrap();
    assert_eq!(engine.invoke_command("add_numbers",&[Value::Number(2.0),Value::Number(3.0)]).unwrap(),Value::Number(5.0));
    assert!(engine.invoke_command("greet",&[Value::from("Hi")]).is_ok());
    assert!(mivl::register_commands![in &mut engine; greet].is_err());
}

#[test]
fn async_commands_return_request_id_and_complete_on_ui_thread() {
    let mut engine=Engine::parse(r#"
        Window {
            property int request: 0
            property int answer: 0
            onCommandCompleted: {answer = commandResult}
            Button { id: start; onClicked: {request = invoke_async("multiply", 6, 7)} }
        }
    "#).unwrap();
    mivl::register_commands![in &mut engine; resources::multiply].unwrap();
    engine.emit("start","clicked").unwrap();
    assert!(engine.get("request").unwrap().number().unwrap()>0.0);
    for _ in 0..250 {
        engine.pump_messages().unwrap();
        if engine.get("answer").unwrap()==Value::Number(42.0) {break;}
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(engine.get("answer").unwrap(),Value::Number(42.0));
    assert_eq!(engine.get("commandName").unwrap(),Value::from("multiply"));
    assert!(engine.invoke_command("multiply",&[]).is_err());
}

#[test]
fn rust_emitted_events_respect_once_times_and_repeat_and_enable() {
    let mut engine=Engine::parse(r#"
        Window {
            property int onceTotal: 0
            property int threeTotal: 0
            property int continuousTotal: 0
            EventListener {id: one; event: "tick"; onTriggered: {onceTotal += eventData}}
            EventListener {id: three; event: "tick"; times: 3; onTriggered: {threeTotal += eventData}}
            EventListener {id: continuous; event: "tick"; repeat: true; onTriggered: {continuousTotal += eventData}}
        }
    "#).unwrap();
    let handle=engine.frontend_handle();
    let worker=std::thread::spawn(move||{
        for _ in 0..5 {handle.emit("tick",2).unwrap();}
    });
    worker.join().unwrap();
    assert_eq!(engine.pump_messages().unwrap(),5);
    assert_eq!(engine.get("onceTotal").unwrap(),Value::Number(2.0));
    assert_eq!(engine.get("threeTotal").unwrap(),Value::Number(6.0));
    assert_eq!(engine.get("continuousTotal").unwrap(),Value::Number(10.0));
    engine.set_listener_enabled("continuous",false).unwrap();
    assert_eq!(engine.dispatch_frontend("tick",2).unwrap(),0);
    engine.set_listener_enabled("continuous",true).unwrap();
    assert_eq!(engine.dispatch_frontend("tick",3).unwrap(),1);
    assert_eq!(engine.get("continuousTotal").unwrap(),Value::Number(13.0));
    engine.reset_listener("one").unwrap();
    engine.dispatch_frontend("tick",1).unwrap();
    assert_eq!(engine.get("onceTotal").unwrap(),Value::Number(3.0));
}

#[test]
fn subscriptions_survive_virtualized_model_rebuilds() {
    let mut app=Engine::parse(r#"
        Window {
            property int count: 100000
            property int messages: 0
            EventListener {id: receiver; event:"update"; repeat:true; onTriggered:{messages += eventData}}
            ListView {id: rows; width:250; height:100; model:count; itemHeight:32
                delegate: Text {height:32; text: index}
            }
        }
    "#).unwrap();
    app.dispatch_frontend("update",2).unwrap();
    app.scroll("rows",0.0,5000.0).unwrap();
    app.dispatch_frontend("update",2).unwrap();
    assert_eq!(app.get("messages").unwrap(),Value::Number(4.0));
}
