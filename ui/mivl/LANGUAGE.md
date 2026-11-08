# MIVL syntax reference

**File extension:** `.mivl`.

## Basic window

```qml
Window {
    id: main
    title: "App"
    width: 800
    height: 500
    color: #172030

    property int clicks: 0
    property string label: "Clicked " + clicks + " times"

    Text {
        id: status
        text: label
        x: 28
        y: 24
        width: parent.width - 56
        height: 48
        fontSize: 24
        color: "white"
    }

    Button {
        id: action
        x: 28
        y: 90
        text: "Click me"
        onClicked: { clicks += 1 }
    }
}
```

Colors can be `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`. Named colors are available to the native renderer if **quoted**: `color: "white"`, `color: "transparent"`, etc. The token `white` without quotes is interpreted as a property reference.

## Types

`property int`, `property real`, `property double`, `property bool`, `property string`, `property color`, and `property var` are supported. An ordinary `width: 300` assignment creates a bound widget property; declarations such as `property int counter: 1` are typed dynamic properties.

## Expressions

```qml
width: parent.width / 2 - 12
text: "Total: " + amount
color: count > 5 ? "#16a34a" : "#be123c"
visible: loaded && (progress >= 1)
fontSize: clamp(window.width / 30, 14, 32)
```

Operators, highest precedence first: unary `-` / `!`, `*` `/` `%`, `+` `-`, comparisons, equality, `&&`, `||`, then the conditional `condition ? yes : no`. Lazy branching applies to `&&`, `||`, and `?:`.

## Event handlers

```qml
Button {
    id: button
    onClicked: {
        clicks += 1
        status.text = "Updated " + clicks
    }
}
```

Handler statements assign to existing properties. Supported operators are `=`, `+=`, `-=`, `*=`, `/=`. The engine does not execute arbitrary native code or JavaScript expressions as statements. If you want a side effect (filesystem/network/sound), your Rust application can consume Mata events itself and call `Engine::set` or `Engine::emit` from its own trusted code.

## Layout

Row and Column place children sequentially using their children's widths/heights. Explicit `x:` and `y:` override the corresponding flow placement. `spacing:` controls the gap; `padding:` shifts the placement origin.

```qml
Column {
    x: 24
    y: 24
    spacing: 10
    Button { width: 160; text: "One" }
    Button { width: 160; text: "Two" }
}
Rectangle {
    anchors.fill: parent
    anchors.margins: 16
}
Text {
    anchors.centerIn: parent
    width: 220
    height: 50
    text: "Centered"
}
```

Anchor targets are currently restricted to `parent`, `root`, or `window`; arbitrary ID-based anchors and `anchors.left`/`right` are planned, not present. `anchors.fill` overrides width/height; `anchors.centerIn` uses the element's own width/height. A root `Window` cannot be anchored to a parent.

## Engine API

- `Engine::parse(&str)` parses and validates a new UI document.
- `Engine::get("id.property")` obtains the up-to-date value.
- `Engine::set("id.property", Value)` assigns an existing property, overriding any original binding.
- `Engine::emit("id", "clicked")` executes that element's `onClicked` block.
- `Engine::lookup("id")` finds an element index.
- `Engine::property(index, "name")`, `set_at`, `emit_at` support native integrations.
- `Engine::layout()` produces absolute rectangles in node order.
- `NativeApp::new(engine)`, `sync()`, `pump()`, and `run()` integrate with `mata-ui` and `mata-win` using built-in native UI.

## Model and view syntax

```qml
Window {
    width: 720; height: 500
    property var fruits: ["Apple", "Pear", "Orange"]
    ListView {
        id: rows
        width: 260; height: 160
        model: fruits
        spacing: 5
        delegate: Button {
            height: 40
            text: (index + 1) + ". " + modelData
            onClicked: { rows.currentIndex = index }
        }
    }
    GridView {
        x: 300; width: 380; height: 210
        model: fruits
        columns: 2
        cellWidth: 160
        cellHeight: 60
        delegate: Text { text: modelData }
    }
}
```

- List syntax: `[1, 2, 3]`, `["A", "B"]`, or `[]`, including trailing commas. `len(myList)` returns its length.
- `model: 5` repeats a delegate five times, with `index` and `modelData` both set to each item's zero-based index.
- `model: fruits` (list) repeats it for every value; `modelData` receives the corresponding value.
- `delegate: Text { ... }` may contain nested children; bindings within a delegate can refer to the closest item's `index` and `modelData`.
- Models are reactive: changing `fruits` or another property referenced by `model` rebuilds generated items; ids inside repeated delegates are forbidden.
- `ListView` defaults to rows that fill the view width and use the delegates' heights; `GridView` supports `columns: 0` to calculate its column count from the viewport width.
- `currentIndex` is writable and defaults to `-1` for ListView/GridView. `count` is computed and updates with the model.
- `contentY`/`contentX` control scroll offsets; `contentHeight`/`contentWidth` give computed sizes for ListView and GridView.
- `StackView` and `SwipeView` use `currentIndex` (default `0`) to select one visible child page. Native buttons provide previous/next navigation.
- `ScrollView`/`Flickable` provide scrollable, clipped viewports around explicitly declared child content.
- `Engine::scroll(id, dx, dy)` scrolls a named scrollable view with limits from the content extent; `Engine::visible_layout(&engine.layout()?)` returns clipped per-widget bounds.
- Native mouse-wheel routing and touch or desktop drag gestures are supported after applying the included Mata UI input patch. SwipeView reacts to a horizontal gesture; wheel scrolling follows the pointer into the deepest view.
- Virtualized models instantiate only visible+overscan delegates. `cacheBuffer` defaults to 120 pixels; `itemHeight` can specify a fixed ListView row height. `count` remains the full model size; delegate `index` is the original model index.
- Large count models (`model: 100000`) are compact and lazily indexed. Variable-height rows and native Wayland/X11 touch are not implemented.

## Rust command invocation in event handlers

You can call a Rust function registered by the host with `invoke("registered_name", arg1, arg2, ...)` in an event handler:

```qml
Window {
    property string greeting: "Ready"
    Button {
        onClicked: {
            greeting = invoke("greet", "Mata")
            invoke("log_message", greeting)
        }
    }
    Text {text: greeting}
}
```

The first argument is a string containing an explicitly registered command name; following arguments are dynamically evaluated. Functions can return `null`, booleans, numbers, strings or lists, and `#[command]` automatically handles common Rust owned argument and return types. Synchronous calls run in the event handler; slow commands block rendering.

`invoke()` is deliberately forbidden in reactive property expressions (`property var x: invoke(...)`), which can be evaluated repeatedly and must not cause side effects. There is no arbitrary Rust execution: registration is explicit, and unknown commands are errors.

## Responsive styling (v0.7)

Numeric lengths support `px`, `%`, `vw`, `vh`, `vmin`, `vmax`, `em`, `rem`,
`ch`, `ex`, and `pt`. For example, `width: 60vw - 2rem`,
`height: parent.height * 0.5` and `padding: 12px`. The box model adds
`margin`, directional padding, `borderRadius`, `borderWidth`, `textAlign`,
`minWidth`, `maxWidth`, `auto`, and `overflow`. General scrolling and scrollbar
appearance are configured with `overflow: "auto"`, `scrollbarPolicy`,
`scrollbarWidth`, `scrollbarColor`, and `scrollbarTrackColor`.

See [STYLING.md](STYLING.md) and [examples/responsive.mivl](examples/responsive.mivl)
for details and current limitations.
