//! Regression cases for type inference and explicit wire widths.
//! Intentionally not executed as part of this delivery.
use remata_macros::FromPrimitive;

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
enum PngColor { #[value(0)] Gray, #[value(6)] Rgba }

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
enum Compression { #[value(1)] None, #[value(32773)] PackBits }

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
#[from_primitive(u16)]
enum Photo { #[value(0)] White, #[value(2)] Rgb }

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
#[from_primitive(u16)]
enum Tag { #[value(1..=3)] Common(u16), Other(u16) }

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
enum Offset { #[value(-1)] Left, #[value(10)] Right }

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
#[repr(u32)]
enum Code { #[value(17)] Known }

#[test]
fn narrow_values_choose_u8() {
    fn requires_u8<T: TryFrom<u8>>() {}
    requires_u8::<PngColor>();
    assert_eq!(PngColor::try_from(6), Ok(PngColor::Rgba));
    assert!(PngColor::try_from(5).is_err());
}

#[test]
fn wider_values_choose_u16() {
    fn requires_u16<T: TryFrom<u16>>() {}
    requires_u16::<Compression>();
    assert_eq!(Compression::try_from(32773), Ok(Compression::PackBits));
}

#[test]
fn explicit_type_can_be_wider_than_variants() {
    let value: u16 = 2;
    assert_eq!(Photo::try_from(value), Ok(Photo::Rgb));
    assert_eq!(Tag::from(2u16), Tag::Common(2));
    assert_eq!(Tag::from(500u16), Tag::Other(500));
}

#[test]
fn sign_and_repr() {
    assert_eq!(Offset::try_from(-1i8), Ok(Offset::Left));
    assert_eq!(Code::try_from(17u32), Ok(Code::Known));
}

// Regression: TIFF stores these tags in u16 fields even though all enum
// discriminants fit in u8. No #[from_primitive(u16)] should be needed.
#[derive(Debug, PartialEq, Eq, FromPrimitive)]
enum PhotoMetricInterpretation {
    #[value(0)] WhiteIsZero,
    #[value(1)] BlackIsZero,
    #[value(2)] Rgb,
    #[value(6)] YCbCr,
}

#[derive(Debug, PartialEq, Eq, FromPrimitive)]
enum FillOrder {
    #[value(1)] LsbToMsb,
    #[value(2)] MsbToLsb,
}

#[test]
fn tiff_u16_tag_inputs_are_checked_automatically() {
    let photometric: u16 = 2;
    let fill_order: u16 = 1;
    assert_eq!(PhotoMetricInterpretation::try_from(photometric), Ok(PhotoMetricInterpretation::Rgb));
    assert_eq!(FillOrder::try_from(fill_order), Ok(FillOrder::LsbToMsb));

    // Both the ergonomic method and the actual TryFrom<u16> trait impl work.
    assert_eq!(<PhotoMetricInterpretation as TryFrom<u16>>::try_from(6u16), Ok(PhotoMetricInterpretation::YCbCr));
    assert_eq!(<FillOrder as TryFrom<u16>>::try_from(2u16), Ok(FillOrder::MsbToLsb));

    assert_eq!(PhotoMetricInterpretation::try_from(256u16), Err(()));
    assert_eq!(FillOrder::try_from(257u16), Err(()));
    assert_eq!(FillOrder::try_from(-1i32), Err(()));
    assert_eq!(FillOrder::try_from(2), Ok(FillOrder::MsbToLsb));
}

#[test]
fn other_widths_and_overflow() {
    assert_eq!(PhotoMetricInterpretation::try_from(2u32), Ok(PhotoMetricInterpretation::Rgb));
    assert_eq!(PhotoMetricInterpretation::try_from(2u64), Ok(PhotoMetricInterpretation::Rgb));
    assert_eq!(PhotoMetricInterpretation::try_from(2usize), Ok(PhotoMetricInterpretation::Rgb));
    assert_eq!(PhotoMetricInterpretation::try_from(2i16), Ok(PhotoMetricInterpretation::Rgb));
    assert_eq!(PhotoMetricInterpretation::try_from(2i128), Ok(PhotoMetricInterpretation::Rgb));
    assert_eq!(PhotoMetricInterpretation::try_from(u128::MAX), Err(()));
    assert_eq!(PhotoMetricInterpretation::try_from(255u16), Err(()));
}

#[test]
fn canonical_catchall_retains_checked_wider_inputs() {
    assert_eq!(Tag::try_from(2u32), Ok(Tag::Common(2)));
    assert_eq!(Tag::try_from(500u32), Ok(Tag::Other(500)));
    assert!(Tag::try_from(65536u32).is_err());
}
