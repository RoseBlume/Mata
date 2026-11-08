# mata-json

Dependency-free JSON parsing and serialization in Rust standard library.

Provides `Value`, `parse`, `stringify`, `to_string`, `to_vec_pretty`,
`from_str`, `from_value` and explicit `ToJson` / `FromJson` traits.

```rust
use mata_json::{Value, parse, stringify};
let value = parse(r#"{"score":10,"note":"hello"}"#)?;
assert_eq!(value.get("score").and_then(Value::as_u64), Some(10));
let output = stringify(&value, true)?;
# Ok::<(), mata_json::Error>(())
```

Supports Unicode surrogate pairs, strict JSON syntax, nested objects/arrays,
integers (preserving up to 64 bits), finite floating-point values, and
pretty/minified output. Nested input depth capped at 128.

Application structs implement typed codecs explicitly (see StudyForge
`src/model_json.rs`), avoiding proc-macro crates or Serde dependencies.

**Compatibility note:** StudyForge writes the same field names and enum tags
as its previous JSON files. Empty optional fields may be emitted where Serde
previously omitted them; both versions can read these fields.
