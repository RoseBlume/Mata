use mivl::{command, Engine, Value};
use std::{cell::RefCell, rc::Rc};

#[command]
fn greet(name: String) -> String {format!("Hello, {name}!")}

#[command]
fn add_numbers(a: i32, b: i32) -> i32 {a+b}

#[command]
fn is_even(value: i64) -> Result<bool,&'static str> {
    if value<0 {Err("negative values are not supported")} else {Ok(value%2==0)}
}

#[test]
fn attribute_commands_can_return_values() {
    let mut ui=Engine::parse(r#"
        Window {
            property string message: "Waiting"
            property int total: 0
            Button {
                id: go
                onClicked: {
                    message = invoke("greet", "Mata")
                    total = invoke("add_numbers", 19, 23)
                }
            }
            Text {id: caption; text: message + " " + total}
        }
    "#).unwrap();
    register_greet(&mut ui).unwrap();
    register_add_numbers(&mut ui).unwrap();
    ui.emit("go","clicked").unwrap();
    assert_eq!(ui.get("message").unwrap(),Value::from("Hello, Mata!"));
    assert_eq!(ui.get("total").unwrap(),Value::Number(42.0));
    assert_eq!(ui.get("caption.text").unwrap(),Value::from("Hello, Mata! 42"));
}

#[test]
fn standalone_command_invocations_and_mutable_rust_captures() {
    let mut ui=Engine::parse(r#"
        Window {
            property string name: "Ada"
            Button {id: go; onClicked: {invoke("record", name); name = "Grace"; invoke("record", name)}}
        }
    "#).unwrap();
    let values=Rc::new(RefCell::new(Vec::new()));
    let receiver=Rc::clone(&values);
    ui.register_command("record",move |args| {receiver.borrow_mut().push(args[0].display());Ok(Value::Null)}).unwrap();
    ui.emit("go","clicked").unwrap();
    assert_eq!(*values.borrow(),vec!["Ada", "Grace"]);
    assert!(ui.register_command("record",|_|Ok(Value::Null)).is_err());
    assert!(ui.unregister_command("record"));
    assert!(!ui.has_command("record"));
}

#[test]
fn argument_checks_and_error_propagation() {
    let mut ui=Engine::parse(r#"
        Window {
            property bool ok: false
            Button { id: go; onClicked: {ok = invoke("is_even", 4)} }
        }
    "#).unwrap();
    register_is_even(&mut ui).unwrap();
    ui.emit("go","clicked").unwrap();
    assert_eq!(ui.get("ok").unwrap(),Value::Bool(true));
    assert!(ui.invoke_command("is_even",&[Value::Number(-1.0)]).is_err());
    assert!(ui.invoke_command("is_even",&[Value::String("4".into())]).is_err());
    assert!(ui.invoke_command("is_even",&[]).is_err());
    assert!(ui.invoke_command("nonexistent",&[]).is_err());
}

#[test]
fn commands_survive_view_virtualization_and_model_recomposition() {
    let mut ui=Engine::parse(r#"
        Window {
            property int rows: 100000
            property string result: ""
            ListView {
                id: records
                width: 260; height: 150
                model: rows
                itemHeight: 30
                delegate: Button {
                    height: 30
                    text: index
                    onClicked: {result = invoke("greet", "list")}
                }
            }
        }
    "#).unwrap();
    register_greet(&mut ui).unwrap();
    ui.scroll("records",0.0,50000.0).unwrap();
    assert!(ui.has_command("greet"));
    ui.set("rows",100001).unwrap();
    assert!(ui.has_command("greet"));
    let delegate=ui.children(ui.lookup("records").unwrap())[0];
    ui.emit_at(delegate,"clicked").unwrap();
    assert_eq!(ui.get("result").unwrap(),Value::from("Hello, list!"));
}

#[test]
fn side_effect_commands_cannot_be_used_in_bindings() {
    let bad=Engine::parse(r#"Window { property string message: invoke("greet", "x") }"#);
    assert!(bad.is_err());
}

#[test]
fn unknown_invocation_rolls_back_property_mutations() {
    let mut ui=Engine::parse(r#"
        Window {
            property int count: 0
            Button {id: go; onClicked: {count += 1; invoke("missing")}}
        }
    "#).unwrap();
    assert!(ui.emit("go","clicked").is_err());
    assert_eq!(ui.get("count").unwrap(),Value::Number(0.0));
}
