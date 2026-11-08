use mivl::{Engine, Value};

fn box_of(ui:&Engine,id:&str)->mivl::LayoutRect {
    ui.layout().unwrap()[ui.lookup(id).unwrap()]
}

#[test]
fn css_units_bind_to_window_parent_and_fonts() {
    let ui=Engine::parse(r#"
        Window {
            width: 800; height: 600; fontSize: 20
            Rectangle {
                id: panel
                width: 50vw; height: 25vh
                x: 10%; y: 10vh
                Rectangle {
                    id: inner
                    width: 50%; height: parent.height / 2
                    x: 2em; y: 1rem
                }
            }
        }
    "#).unwrap();
    let panel=box_of(&ui,"panel");
    assert_eq!((panel.x,panel.y,panel.width,panel.height),(80,60,400,150));
    let inner=box_of(&ui,"inner");
    assert_eq!((inner.x,inner.y,inner.width,inner.height),(112,80,200,75));
}

#[test]
fn rem_and_other_units_react_after_window_resize() {
    let mut ui=Engine::parse(r#"
        Window {
            width: 1000; height: 700; fontSize: 20
            Rectangle {
                id: sized
                width: 30vmin + 10px
                height: 2rem + 12pt
            }
        }
    "#).unwrap();
    assert_eq!(box_of(&ui,"sized").width,220);
    assert_eq!(box_of(&ui,"sized").height,56);
    ui.set("width",800.0).unwrap();
    ui.set("height",400.0).unwrap();
    assert_eq!(box_of(&ui,"sized").width,130);
}

#[test]
fn auto_text_and_intrinsic_container_sizing() {
    let mut ui=Engine::parse(r#"
        Window {
            Column {
                id: column
                width: auto; height: auto
                padding: 10
                spacing: 5
                Text { id: title; text: "hello"; fontSize: 20 }
                Text { id: subtitle; text: "tiny"; fontSize: 12 }
            }
        }
    "#).unwrap();
    let before=box_of(&ui,"column");
    assert!(before.width>20 && before.height>40);
    let title=box_of(&ui,"title");
    assert_eq!((title.x,title.y),(10,10));
    ui.set("title.text","a much longer title").unwrap();
    assert!(box_of(&ui,"column").width>before.width);
}

#[test]
fn padding_margins_positioning_and_limits() {
    let ui=Engine::parse(r#"
        Window {
            width: 600; height: 400
            Column {
                id: frame
                x: 15; y: 20; width: 300; height: 200
                border-padding: 12
                spacing: 6
                Text { id: first; text: "hi"; height: 20; margin-bottom: 4 }
                Button { id: next; width: 80; height: 35 }
                Rectangle {
                    id: close
                    position: "absolute"
                    width: 40; height: 20
                    right: 10; bottom: 5
                }
            }
        }
    "#).unwrap();
    assert_eq!((box_of(&ui,"first").x,box_of(&ui,"first").y),(27,32));
    assert_eq!(box_of(&ui,"next").y,62);
    assert_eq!((box_of(&ui,"close").x,box_of(&ui,"close").y),(253,183));
}

#[test]
fn overflow_hidden_and_scroll_clipping() {
    let mut ui=Engine::parse(r#"
        Window {
            Rectangle {
                id: viewport; x: 10; y: 20; width: 100; height: 70
                overflow: "scroll"
                Rectangle { id: tall; width: 90; height: 240 }
            }
        }
    "#).unwrap();
    let view=ui.lookup("viewport").unwrap();
    assert!(ui.is_scrollable(view).unwrap());
    assert_eq!(ui.content_extent(view,&ui.layout().unwrap()).unwrap().1,240);
    ui.scroll("viewport",0.0,50.0).unwrap();
    assert_eq!(ui.get("viewport.contentY").unwrap(),Value::Number(50.0));
    assert_eq!(box_of(&ui,"tall").y,-30);
    let visible=ui.visible_layout(&ui.layout().unwrap()).unwrap();
    assert_eq!(visible[ui.lookup("tall").unwrap()].unwrap().height,70);
}

#[test]
fn kebab_case_properties_are_normalized() {
    let ui=Engine::parse(r#"
        Window {
            Rectangle {
                id: a
                border-radius: 8px
                border-padding: 12px
                margin-left: 1rem
                overflow-x: "hidden"
                Text { text: "Hello"; text-align: "center" }
            }
        }
    "#).unwrap();
    let node=ui.lookup("a").unwrap();
    assert_eq!(ui.property(node,"borderRadius").unwrap(),Value::Number(8.0));
    assert_eq!(ui.property(node,"borderPadding").unwrap(),Value::Number(12.0));
    assert!(ui.is_clipping(node).unwrap());
}

#[test]
fn legacy_modulo_still_works() {
    let ui=Engine::parse("Window { property int answer: 11 % 4 }").unwrap();
    assert_eq!(ui.get("answer").unwrap(),Value::Number(3.0));
}

#[test]
fn percentages_of_font_size_use_parent_font() {
    let ui=Engine::parse(r#"
        Window {
            fontSize: 20px
            Text { id: enlarged; text: "hello"; fontSize: 150% }
        }
    "#).unwrap();
    assert_eq!(ui.get("enlarged.fontSize").unwrap(),Value::Number(30.0));
}


#[test]
fn em_ex_and_scientific_notation_coexist() {
    let ui = Engine::parse(r#"
        Window {
            width: 800; height: 600; fontSize: 20px
            Text { id: em_text; text: "Hello"; fontSize: 0.95em }
            Rectangle {
                id: units
                x: 2em
                y: 3ex
                width: 1e2px
                height: 2E-1rem
            }
        }
    "#).unwrap();
    assert_eq!(ui.get("em_text.fontSize").unwrap(), Value::Number(19.0));
    let units = box_of(&ui, "units");
    assert_eq!(units.width, 100);
    assert_eq!(units.height, 4);
}

#[test]
fn malformed_signed_exponents_remain_errors() {
    for number in ["1e+", "3E-"] {
        let source = format!("Window {{ property real value: {number} }}");
        let error = match Engine::parse(&source) { Ok(_) => panic!("unexpectedly accepted {number}"), Err(e) => e };
        assert!(error.to_string().contains("malformed exponent"));
    }
}

#[test]
fn responsive_example_parses_without_exponent_error() {
    let source = include_str!("../examples/responsive.mivl");
    Engine::parse(source).unwrap();
}

#[test]
fn long_text_auto_sizing_updates_reactively() {
    let mut ui = Engine::parse(r#"
        Window {
            width: 1200; height: 300
            Column {
                id: stack; width: auto; height: auto
                Text { id: title; text: "Responsive MIVL"; fontSize: 32 }
                Text { id: description; text: "Resize the window to exercise vw, vh, em, % and parent.width" }
            }
        }
    "#).unwrap();
    let width_before = box_of(&ui,"description").width;
    assert!(width_before > box_of(&ui,"title").width);
    assert!(box_of(&ui,"stack").width >= width_before);
    ui.set("description.text","A much longer string to test reactive intrinsic sizing and right-edge clipping").unwrap();
    assert!(box_of(&ui,"description").width > width_before);
    assert!(box_of(&ui,"stack").width >= box_of(&ui,"description").width);
}

#[test]
fn wrapped_text_reflows_height_when_width_changes() {
    let mut ui=Engine::parse(r#"
        Window {
            Column { height: auto; width: 320
                Text {
                    id: label
                    width: 160
                    height: auto
                    wrap: true
                    fontSize: 20
                    lineHeight: 1.2
                    text: "A line of many words that should wrap into separate lines"
                }
            }
        }
    "#).unwrap();
    let wide=box_of(&ui,"label");
    assert!(wide.height>24,"a constrained text should have multiple rows");
    ui.set("label.width",80.0).unwrap();
    let narrow=box_of(&ui,"label");
    assert!(narrow.height>wide.height,"smaller width must increase wrapped auto height");
}

#[test]
fn wrapped_text_autosizes_parent_column_and_supports_max_lines() {
    let ui=Engine::parse(r#"
        Window {
            Column { id: group; width: 140; height: auto; padding: 4
                Text { id: label; width: 120; height: auto; wrapMode: "word"
                    fontSize: 16; lineHeight: 1.25; maxLines: 2
                    textOverflow: "ellipsis"
                    text: "First second third fourth fifth sixth seventh eighth"
                }
            }
        }
    "#).unwrap();
    let label=box_of(&ui,"label");
    assert_eq!(label.height,40,"two lines at a 20px line height");
    assert_eq!(box_of(&ui,"group").height,48);
}

#[test]
fn scroll_clipping_preserves_original_text_geometry() {
    let mut ui=Engine::parse(r#"
        Window {
            Rectangle { id: scroll; width: 180; height: 50; overflow: "auto"
                Column { height: auto
                    Text { id: above; text: "Top"; width: 180; height: 30 }
                    Text { id: clipped; text: "Some text\nSecond line"; width: 180;
                        height: 64; wrap: true; lineHeight: 1.0 }
                    Text { text: "Bottom"; height: 45 }
                }
            }
        }
    "#).unwrap();
    ui.scroll("scroll",0.0,42.0).unwrap();
    let full=box_of(&ui,"clipped");
    let visible=ui.visible_layout(&ui.layout().unwrap()).unwrap()[ui.lookup("clipped").unwrap()].unwrap();
    assert_eq!(full.height,64);
    assert!(visible.height<full.height,"viewport should partially crop the label");
    assert!(visible.y>=0);
}
