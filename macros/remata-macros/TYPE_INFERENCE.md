# `FromPrimitive`: inferred canonical type with checked integer inputs

The derive selects a **canonical** integer type in this order:

1. `#[from_primitive(u16)]` explicitly selects the representation (recommended for fixed-width wire fields with an unknown-value catch-all).
2. A primitive `#[repr(u16)]`, if present, selects that type.
3. Otherwise, `#[value(...)]` literals determine the smallest signed or unsigned integer type that fits every code.

Unlike the previous version, the derived enum now accepts *all 12 standard signed and unsigned integer input types* (`u8` through `u128`, `i8` through `i128`, `usize`, and `isize`) through checked `TryFrom` implementations.

An inherent `Enum::try_from(...)` convenience method accepts any input implementing `TryInto<canonical_type>`. This avoids the ambiguity of `Enum::try_from(2)` when many integer `TryFrom` traits exist.

```rust
use remata_macros::FromPrimitive;

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
enum PhotoMetricInterpretation {
    #[value(0)] WhiteIsZero,
    #[value(2)] Rgb,
}

let from_tiff: u16 = 2;
assert_eq!(PhotoMetricInterpretation::try_from(from_tiff), Ok(PhotoMetricInterpretation::Rgb));
assert_eq!(PhotoMetricInterpretation::try_from(2), Ok(PhotoMetricInterpretation::Rgb));
assert!(PhotoMetricInterpretation::try_from(256u16).is_err());
```

For a closed enum, unknown codes return `Err(())`. For an enum with a catch-all tuple variant, `From<canonical_type>` is retained while the other integer types use checked `TryFrom`. A catch-all does *not* make conversions from larger types infallible: representability in the canonical type is still checked, preventing silent truncation.

```rust
#[derive(FromPrimitive)]
#[from_primitive(u16)]
enum Tag {
    #[value(1)] Known,
    Unknown(u16),
}
// `Tag::from(1000u16)` keeps the full unknown 16-bit value.
```

Supported variant syntax remains `#[value(42)]`, `#[value(0xff)]`, `#[value(-2)]`, `#[value(1..=5)]`, and `#[value(10..15)]`.

**Note:** The derive cannot discover types from future call sites. This update instead provides checked conversion implementations for multiple input widths. The inherent `try_from` method may conflict with a manually defined inherent method of the same name on an enum.

No FFI or extra dependencies were introduced. Tests are included but were not run.
