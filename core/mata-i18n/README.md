# mata-i18n

An internationalization/localization library for the Mata Rust ecosystem. **No dependencies at all**: only the Rust standard library. There are no native runtime requirements, procedural macros, build scripts, or translation downloads.

## Included features

- UTF-8 `.lang`/`.properties` and nested-object `.json` translation catalogs, implemented internally without a JSON crate
- File, directory, or compile-time embedded resource loading (`include_str!`)
- BCP-47-style language tag normalization, language/script/region fallback, `Accept-Language` negotiation, and environment-based locale detection
- Shared, thread-safe runtime language switching (`I18n: Clone + Send + Sync`)
- Typed variables, message interpolation, `plural` with exact selectors and offsets, and `select` branches
- Cardinal plural rules for Arabic, East Slavic languages, Polish, Czech, Slovak, Slovenian, French, Portuguese, and representative one/other/other-only languages
- Locale-sensitive grouping/decimal marks and currency display for a **documented, limited subset** of locales
- Basic numeric/English/French date formatting and RTL/LTR direction hints
- Resource errors with line numbers and message parsing errors with byte offsets, with atomic per-bundle updates

## Build and test

```sh
cd mata-i18n
cargo test --offline
cargo run --offline --example basic
```

The crate has an empty `[dependencies]` section. No downloads are necessary for it.

## Usage

Add this dependency to your Mata application's `Cargo.toml`:

```toml
[dependencies]
mata-i18n = { path = "../mata-i18n" }
```

Or, if your app already imports Mata UI, enable the optional reexport:

```toml
[dependencies]
mata-ui = { path = "../mata-ui", features = ["i18n"] }
```

Then:

```rust
use mata_i18n::{args, I18n, format_currency, format_date, Date, DateStyle};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let i18n = I18n::new("en-US")?;
    i18n.load_directory("translations")?;
    i18n.set_locale("fr-CA")?;

    println!("{}", i18n.tr("actions.save"));
    println!("{}", i18n.tr_with("app.welcome", &args! { "name" => "Amélie" }));
    println!("{}", i18n.tr_with("app.file_count", &args! { "count" => 12 }));
    println!("{}", format_currency(&i18n.locale(), 1234.50, "CAD"));
    println!("{}", format_date(&i18n.locale(), Date::new(2026, 10, 20).unwrap(), DateStyle::Long));
    Ok(())
}
```

For compile-time embedded catalogs:

```rust
let i18n = mata_i18n::I18n::new("en-US")?;
i18n.add_resource("en-US", include_str!("../locales/en-US.lang"))?;
i18n.add_json("fr", include_str!("../locales/fr.json"))?;
```

## Resource syntax

Filename convention: `en-US.lang`, `fr.json`, `de-DE.properties`. `load_directory` loads only immediate children of its target folder. Additional files for the same locale are merged with later keys overriding earlier keys (sorted by path).

`.lang` / `.properties`:

```properties
# Optional comments
menu.save = Save
file.count = {count, plural, =0 {No files} one {# file} other {# files}}
message = {role, select, admin {Hello, administrator} other {Hello, {name}}}
welcome = Hello, {name}!
```

JSON (string values and nested objects only):

```json
{
  "menu": { "save": "Save" },
  "file": { "count": "{count, plural, one {# file} other {# files}}" }
}
```

Nested JSON keys flatten to `menu.save`, `file.count`. Keys are ASCII alphanumeric plus `-`, `_`, `.`; message text is fully UTF-8. `.properties` supports `\n`, `\r`, `\t`, `\\`, `\=`, `\#`, `\u{HEX}` and backslash-continued lines. The encoding is **not** the ISO-8859-1 convention of Java `.properties`.

The message formatter intentionally implements a useful **subset**, not all of ICU MessageFormat:

- `{name}`: substitute a typed argument
- `{count, plural, one {# item} other {# items}}`: locale-specific plural branch (`#` inserts localized count)
- `{count, plural, offset:1 =0 {None} one {# companion} other {# companions}}`: exact selector and offset
- `{gender, select, male {He} female {She} other {They}}`: selection
- `{{` / `}}`: literal braces in top-level text. Inside plural/select bodies, use `\{` or `\}` for literal braces (`\\{` / `\\}` in a `.properties` file).

Use `translate(key, &args)` when you want a `Result` and meaningful missing-resource errors. `tr` and `tr_with` return the key itself if translation fails (useful for UI prototypes). Inserted variable values are treated as plain text, never reparsed as templates.

## Mata UI integration

Enable `i18n` on `mata-ui`, then import `mata_ui::i18n::I18n`. The example `mata-ui/examples/i18n_switch.rs` demonstrates live language switching in a `WidgetWindow` using existing Button controls and `set_label`.

`mata-i18n` does **not** depend on `mata-ui`, `mata-win`, or any other Mata crate. Translation templates are independent from rendering. When the locale changes, reassign your widget labels with `set_label`/`set_text` and the appropriate text-direction and font settings.

## Limitations

- Not a complete Unicode, ICU, CLDR, RFC 5646, or ISO-4217 implementation. Some locales use the generic English one/other cardinal plural rule; there is no ordinal-plural grammar, native digit shaping, or time-zone database.
- The number/date/currency functions are small built-in convention tables, **not** full locale data. For sensitive monetary formatting, use an authoritative locale data set and decimal arithmetic rather than `f64`.
- RTL detection returns a direction hint but does not reorder, reshape, or lay out bidirectional text. The UI's font and text rendering code must support those scripts separately.
- Automatic locale detection reads environment variables (`LC_ALL`, `LC_MESSAGES`, `LANG`, `LANGUAGE`); on Windows/Android where these are missing, set the locale yourself. Default fallback is `en-US`.
- For frequently changing languages, the `I18n` instance is thread-safe; consumer widgets are responsible for refreshing their displayed text.
