use mivl::{Engine, Value, parse};

#[test]
fn parses_declarative_nested_components_and_comments() {
    let ast = parse(r#"
        // visual root
        Window {
            title: "Mata" /* comment */
            property int counter: 2
            Column {
                Text { id: status; text: "count " + counter }
                Button {
                    id: button
                    text: "Increment"
                    onClicked: { counter += 1 }
                }
            }
        }
    "#).unwrap();
    assert_eq!(ast.kind, "Window");
    assert_eq!(ast.children[0].children.len(), 2);
}

#[test]
fn reactive_property_binding_and_event_dispatch() {
    let mut app = Engine::parse(r#"
        Window {
            id: root
            property int count: 1
            Text { id: label; text: "Total: " + count }
            Button { id: button; onClicked: { count += 2 } }
        }
    "#).unwrap();
    assert_eq!(app.get("label.text").unwrap(), Value::from("Total: 1"));
    app.emit("button", "clicked").unwrap();
    assert_eq!(app.get("count").unwrap(), Value::Number(3.0));
    assert_eq!(app.get("label.text").unwrap(), Value::from("Total: 3"));
    app.set("root.count", 17).unwrap();
    assert_eq!(app.get("label.text").unwrap(), Value::from("Total: 17"));
}

#[test]
fn bindings_resolve_parent_window_and_ids() {
    let engine = Engine::parse(r#"
        Window {
            id: root
            width: 600
            height: 400
            Rectangle { id: card; x: 16; width: parent.width - 32 }
            Text { id: total; width: card.width / 2; text: str(window.width) }
        }
    "#).unwrap();
    assert_eq!(engine.get("card.width").unwrap(), Value::Number(568.0));
    assert_eq!(engine.get("total.width").unwrap(), Value::Number(284.0));
    assert_eq!(engine.get("total.text").unwrap(), Value::from("600"));
}

#[test]
fn expression_precedence_functions_and_conditionals() {
    let engine = Engine::parse(r#"
        Window {
            property real result: (2 + 3) * 4 / 2
            property bool passed: result == 10 && true
            property string message: passed ? "yes" : "no"
            property int limited: clamp(21, 0, 13)
        }
    "#).unwrap();
    assert_eq!(engine.get("result").unwrap(), Value::Number(10.0));
    assert_eq!(engine.get("message").unwrap(), Value::from("yes"));
    assert_eq!(engine.get("limited").unwrap(), Value::Number(13.0));
}

#[test]
fn compound_assignment_and_parent_scope() {
    let mut engine = Engine::parse(r#"
        Window {
            property int clicks: 5
            Column {
                Button { id: btn; onClicked: { clicks *= 2; clicks -= 3 } }
            }
        }
    "#).unwrap();
    engine.emit("btn","clicked").unwrap();
    assert_eq!(engine.get("clicks").unwrap(), Value::Number(7.0));
}

#[test]
fn catches_cyclic_bindings_and_unknown_variables() {
    let cyclic = Engine::parse("Window { property int a: b; property int b: a }");
    assert!(cyclic.unwrap_err().to_string().contains("cyclic binding"));
    let unknown = Engine::parse("Window { Text { text: missing } }");
    assert!(unknown.unwrap_err().to_string().contains("unknown property"));
}

#[test]
fn catches_invalid_syntax_and_duplicate_ids() {
    assert!(Engine::parse("Window { width: }").is_err());
    assert!(Engine::parse("Window { Text { id: foo } Text { id: foo } }").is_err());
    assert!(Engine::parse("Window { property int a: 7; property int a: 9 }").is_err());
}

#[test]
fn layouts_row_and_column_with_explicit_offsets() {
    let engine = Engine::parse(r#"
        Window {
            Column {
                x: 20; y: 30; spacing: 10
                Text { id: first; height: 40 }
                Text { id: second; height: 50 }
                Row {
                    id: row
                    Button { id: left; width: 80 }
                    Button { id: right; width: 90 }
                }
            }
        }
    "#).unwrap();
    let rects = engine.layout().unwrap();
    assert_eq!((rects[engine.lookup("first").unwrap()].x, rects[engine.lookup("first").unwrap()].y),(20,30));
    assert_eq!(rects[engine.lookup("second").unwrap()].y,80);
    assert_eq!(rects[engine.lookup("row").unwrap()].y,140);
    assert_eq!(rects[engine.lookup("right").unwrap()].x,108);
}

#[test]
fn type_coercion_and_divide_by_zero_are_checked() {
    let mut engine = Engine::parse("Window { property int count: 4.9 }").unwrap();
    assert_eq!(engine.get("count").unwrap(), Value::Number(4.0));
    engine.set("count",8.9).unwrap();
    assert_eq!(engine.get("count").unwrap(), Value::Number(8.0));
    assert!(Engine::parse("Window { property real count: 7 / 0 }").is_err());
}

#[test]
fn anchors_fill_and_center_layout() {
    let ui = Engine::parse(r#"
        Window {
            width: 400; height: 300
            Rectangle {
                id: canvas
                anchors.fill: parent
                anchors.margins: 12
            }
            Rectangle {
                id: badge
                anchors.centerIn: parent
                width: 70; height: 40
            }
        }
    "#).unwrap();
    let rects = ui.layout().unwrap();
    let canvas = rects[ui.lookup("canvas").unwrap()];
    assert_eq!((canvas.x,canvas.y,canvas.width,canvas.height),(12,12,376,276));
    let badge = rects[ui.lookup("badge").unwrap()];
    assert_eq!((badge.x,badge.y,badge.width,badge.height),(165,130,70,40));
}

#[test]
fn list_view_generates_live_delegates_and_binding_scopes() {
    let mut ui = Engine::parse(r#"
        Window {
            property int rows: 3
            ListView {
                id: catalog
                x: 10; y: 10; width: 200; height: 80
                spacing: 0
                model: rows
                delegate: Column {
                    height: 40
                    Text { text: "Entry " + index + " = " + modelData }
                }
            }
            Button { id: add; onClicked: { rows += 1 } }
        }
    "#).unwrap();
    let view = ui.lookup("catalog").unwrap();
    assert_eq!(ui.children(view).len(),3);
    let second = ui.children(view)[1];
    let label = ui.children(second)[0];
    assert_eq!(ui.property(label,"text").unwrap(),Value::from("Entry 1 = 1"));
    let generation = ui.generation();
    ui.emit("add","clicked").unwrap();
    assert_eq!(ui.children(view).len(),4);
    assert_ne!(ui.generation(),generation);
    assert_eq!(ui.get("rows").unwrap(),Value::Number(4.0));
    assert_eq!(ui.property(ui.children(ui.children(view)[3])[0],"text").unwrap(),Value::from("Entry 3 = 3"));
}

#[test]
fn list_array_delegates_and_typed_functions() {
    let mut ui = Engine::parse(r#"
        Window {
            property var data: ["alpha", "beta", "gamma"]
            ListView {
                id: list
                model: data
                delegate: Text { text: str(index) + ": " + modelData }
            }
        }
    "#).unwrap();
    let view = ui.lookup("list").unwrap();
    assert_eq!(ui.children(view).len(),3);
    assert_eq!(ui.property(ui.children(view)[2],"text").unwrap(),Value::from("2: gamma"));
    ui.set("data",Value::List(vec![Value::from("changed")])).unwrap();
    assert_eq!(ui.children(view).len(),1);
    assert_eq!(ui.property(ui.children(view)[0],"text").unwrap(),Value::from("0: changed"));
    assert_eq!(ui.get("data").unwrap(),Value::List(vec![Value::from("changed")]));
}

#[test]
fn scroll_view_clips_descendants_without_changing_logical_layout() {
    let mut ui = Engine::parse(r#"
        Window {
            width: 400; height: 300
            ListView {
                id: list
                x: 10; y: 10; width: 200; height: 80
                contentY: 30
                spacing: 0
                model: ["A", "B", "C"]
                delegate: Text { width: 180; height: 40; text: modelData }
            }
        }
    "#).unwrap();
    let view = ui.lookup("list").unwrap();
    let bounds = ui.layout().unwrap();
    let first = ui.children(view)[0];
    assert_eq!(bounds[first].y,-20);
    let clipped = ui.visible_layout(&bounds).unwrap();
    assert_eq!((clipped[first].unwrap().y,clipped[first].unwrap().height),(10,10));
    assert_eq!(ui.content_extent(view,&bounds).unwrap().1,120);
    ui.set("list.contentY",65).unwrap();
    let bounds = ui.layout().unwrap();
    assert!(ui.visible_layout(&bounds).unwrap()[first].is_none());
}

#[test]
fn grid_positions_use_columns_and_cell_sizes() {
    let ui = Engine::parse(r#"
        Window {
            GridView {
                id: grid
                x: 10; y: 20; width: 260; height: 120
                cellWidth: 70; cellHeight: 30; columns: 3
                spacing: 5; padding: 5
                model: ["a","b","c","d"]
                delegate: Rectangle { color: "blue" }
            }
        }
    "#).unwrap();
    let view = ui.lookup("grid").unwrap();
    let bounds = ui.layout().unwrap();
    assert_eq!((bounds[ui.children(view)[0]].x,bounds[ui.children(view)[0]].y),(15,25));
    assert_eq!((bounds[ui.children(view)[2]].x,bounds[ui.children(view)[2]].y),(165,25));
    assert_eq!((bounds[ui.children(view)[3]].x,bounds[ui.children(view)[3]].y),(15,60));
}

#[test]
fn stack_and_swipe_views_only_show_current_page() {
    let mut ui = Engine::parse(r#"
        Window {
            StackView {
                id: pages
                width: 300; height: 180
                currentIndex: 1
                Item { id: page1; Text { id: inside1; text: "first" } }
                Item { id: page2; Text { id: inside2; text: "second" } }
            }
        }
    "#).unwrap();
    let rects = ui.layout().unwrap();
    let visibility = ui.visible_layout(&rects).unwrap();
    assert!(visibility[ui.lookup("page1").unwrap()].is_none());
    assert!(visibility[ui.lookup("inside1").unwrap()].is_none());
    assert!(visibility[ui.lookup("inside2").unwrap()].is_some());
    ui.set("pages.currentIndex",0).unwrap();
    let visibility = ui.visible_layout(&ui.layout().unwrap()).unwrap();
    assert!(visibility[ui.lookup("inside1").unwrap()].is_some());
    assert!(visibility[ui.lookup("page2").unwrap()].is_none());
}

#[test]
fn rejects_unsupported_delegates_and_duplicate_item_ids() {
    assert!(Engine::parse("Window { Column { delegate: Text { text: 1 } } }").is_err());
    assert!(Engine::parse("Window { ListView { model: [1,2]; delegate: Text { id: duplicate } } }").is_err());
    assert!(Engine::parse("Window { ListView { model: [1,2] } }").is_err());
}

#[test]
fn view_metrics_are_computed_from_model_and_layout() {
    let ui = Engine::parse(r#"
        Window {
            property int n: 4
            ListView {
                id: list
                width: 180; height: 80
                spacing: 5; padding: 3
                model: n
                delegate: Text { height: 20; text: modelData }
            }
            GridView {
                id: grid
                width: 180; height: 100
                columns: 2; cellHeight: 30; cellWidth: 60
                spacing: 5; padding: 3
                model: ["A","B","C","D","E"]
                delegate: Text { text: modelData }
            }
        }
    "#).unwrap();
    assert_eq!(ui.get("list.count").unwrap(),Value::Number(4.0));
    assert_eq!(ui.get("list.contentHeight").unwrap(),Value::Number(101.0));
    assert_eq!(ui.get("grid.count").unwrap(),Value::Number(5.0));
    assert_eq!(ui.get("grid.contentHeight").unwrap(),Value::Number(106.0));
}

#[test]
fn scroll_api_clamps_to_content_and_model_changes_rollback_on_error() {
    let mut ui = Engine::parse(r#"
        Window {
            property int n: 5
            ListView {
                id: list
                width: 180; height: 60
                model: n
                spacing: 0
                delegate: Text { height: 30; text: modelData }
            }
        }
    "#).unwrap();
    ui.scroll("list",0.0,1000.0).unwrap();
    assert_eq!(ui.get("list.contentY").unwrap(),Value::Number(90.0));
    ui.scroll("list",0.0,-1000.0).unwrap();
    assert_eq!(ui.get("list.contentY").unwrap(),Value::Number(0.0));
    assert!(ui.set("n",5000).is_err());
    assert_eq!(ui.get("n").unwrap(),Value::Number(5.0));
    assert_eq!(ui.get("list.count").unwrap(),Value::Number(5.0));
}

#[test]
fn scroll_view_wraps_nested_content_and_hides_overflow() {
    let ui = Engine::parse(r#"
        Window {
            ScrollView {
                id: viewport
                x: 50; y: 50; width: 120; height: 80
                contentY: 60
                Column {
                    height: 180
                    Text { height: 40; text: "first" }
                    Text { height: 40; text: "second" }
                    Text { height: 40; text: "third" }
                }
            }
        }
    "#).unwrap();
    let view = ui.lookup("viewport").unwrap();
    let layout = ui.layout().unwrap();
    let content = ui.children(view)[0];
    assert_eq!(layout[content].y,-10);
    let bounds = ui.visible_layout(&layout).unwrap();
    assert!(bounds[ui.children(content)[0]].is_none());
    assert!(bounds[ui.children(content)[2]].is_some());
    assert!(ui.content_extent(view,&layout).unwrap().1 >= 180);
}

#[test]
fn virtualized_integer_model_is_bounded_and_preserves_absolute_indices() {
    let mut ui = Engine::parse(r#"
        Window {
            height: 400
            ListView {
                id: big
                width: 240; height: 200; spacing: 0
                itemHeight: 40
                cacheBuffer: 80
                model: 100000
                delegate: Button {height: 40; text: "row " + index + " data " + modelData }
            }
        }
    "#).unwrap();
    let view=ui.lookup("big").unwrap();
    assert_eq!(ui.get("big.count").unwrap(),Value::Number(100000.0));
    assert_eq!(ui.get("big.contentHeight").unwrap(),Value::Number(4_000_000.0));
    assert!(ui.children(view).len()<30,"only near-viewport delegates should exist");
    assert_eq!(ui.materialized_range(view).unwrap().0,0);
    ui.scroll("big",0.0,160_000.0).unwrap();
    let (begin,end)=ui.materialized_range(view).unwrap();
    assert!(begin>3900 && end<4100, "range should follow scroll: {begin}..{end}");
    assert!(ui.count()<35,"the full model must not be instantiated");
    let first=ui.children(view)[0];
    assert_eq!(ui.property(first,"index").unwrap(), Value::Number(begin as f64));
    assert_eq!(ui.property(first,"modelData").unwrap(), Value::Number(begin as f64));
    assert_eq!(ui.view_index_for_child(first),Some((view,begin)));
    assert_eq!(ui.layout().unwrap()[first].y, (begin*40) as i32-160000);
}

#[test]
fn virtualized_grid_absolute_row_and_column_positions() {
    let mut ui = Engine::parse(r#"
        Window {
            GridView {
                id: cells
                width: 270; height: 150
                columns: 3; cellWidth: 80; cellHeight: 40
                spacing: 0; cacheBuffer: 0
                model: 100000
                delegate: Text { text: index }
            }
        }
    "#).unwrap();
    let view=ui.lookup("cells").unwrap();
    assert_eq!(ui.get("cells.count").unwrap(),Value::Number(100000.0));
    ui.scroll("cells",0.0,40000.0).unwrap();
    let first=ui.children(view)[0];
    let index=ui.property(first,"index").unwrap().number().unwrap() as usize;
    assert_eq!(index,3000);
    let r=ui.layout().unwrap()[first];
    assert_eq!((r.x,r.y),(0,0));
    let next=ui.children(view)[1];
    assert_eq!(ui.layout().unwrap()[next].x,80);
    assert!(ui.children(view).len() < 30);
}

#[test]
fn virtualized_model_refresh_and_scroll_clamping() {
    let mut ui=Engine::parse(r#"
        Window {
            property int total: 100000
            ListView {
                id: list
                width: 240; height: 80
                itemHeight: 20; spacing: 0
                model: total
                delegate: Text {height: 20; text: index}
            }
        }
    "#).unwrap();
    ui.scroll("list",0.0,100000000.0).unwrap();
    assert_eq!(ui.get("list.contentY").unwrap(),Value::Number(1_999_920.0));
    let range=ui.materialized_range(ui.lookup("list").unwrap()).unwrap();
    assert!(range.0>=99980);
    ui.set("total",2).unwrap();
    assert_eq!(ui.get("list.count").unwrap(),Value::Number(2.0));
    // Shrinking the model automatically clamps an out-of-range viewport.
    assert_eq!(ui.get("list.contentY").unwrap(),Value::Number(0.0));
    ui.scroll("list",0.0,0.0).unwrap();
    assert_eq!(ui.children(ui.lookup("list").unwrap()).len(),2);
}

#[test]
fn dynamic_delegate_heights_require_explicit_item_height() {
    let err=Engine::parse(r#"
        Window {
            ListView {
                model: 1000
                delegate: Button {height: 20 + index; text: index}
            }
        }
    "#).unwrap_err();
    assert!(err.to_string().contains("itemHeight"));
}

#[test]
fn idle_layout_and_reads_never_invalidate_the_engine() {
    let app = Engine::parse(r#"
        Window {
            property string caption: "Static"
            Column { Text { text: caption }; Button { text: "Ready" } }
        }
    "#).unwrap();
    let revision=app.revision();
    let generation=app.generation();
    for _ in 0..100 {
        let _=app.layout().unwrap();
        let _=app.get("caption").unwrap();
    }
    assert_eq!(app.revision(),revision);
    assert_eq!(app.generation(),generation);
}

#[test]
fn revision_tracks_changes_and_ignores_repeated_identical_overrides() {
    let mut app = Engine::parse(r#"
        Window {
            property int counter: 0
            Text {text: "Count " + counter}
        }
    "#).unwrap();
    let start=app.revision();
    app.set("counter",7).unwrap();
    let changed=app.revision();
    assert_ne!(start,changed);
    app.set("counter",7).unwrap();
    assert_eq!(app.revision(),changed);
    app.set("counter",8).unwrap();
    assert_ne!(app.revision(),changed);
    assert_eq!(app.get("counter").unwrap(),Value::Number(8.0));
}

#[test]
fn frontend_messages_invalidate_idle_ui_without_native_events() {
    let mut app = Engine::parse(r#"
        Window {
            property string status: "waiting"
            EventListener {event: "update"; repeat: true;
                onTriggered: {status = eventData}}
            Text {text: status}
        }
    "#).unwrap();
    let old=app.revision();
    let frontend=app.frontend_handle();
    frontend.emit("update","arrived").unwrap();
    assert_eq!(app.pump_messages().unwrap(),1);
    assert_ne!(app.revision(),old);
    assert_eq!(app.get("status").unwrap(),Value::from("arrived"));
}
