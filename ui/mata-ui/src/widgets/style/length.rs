//! CSS-like length units, arithmetic, parsing, interpolation, and resolution.
//!
//! The API implements `Copy`, all requested assignment operators, and total
//! structural ordering (`Eq`/`Ord`). Ordering between different units does
//! not reflect pixel size; use `cmp_resolved` for that.
//!
//! Multiplication, division, and remainder use **unitless numeric scalars**;
//! multiplying two CSS lengths would create a squared length and cannot be
//! represented by this linear data model. Mixed-unit remainder is taken per
//! coefficient, not after pixel resolution. `wrapping_*` follows IEEE-754
//! floats rather than integer-style wrapping. `saturating_*` maps infinities
//! to the largest finite values and NaNs to zero.
//!
//! `start..end` and `start..=end` automatically implement [`RangeBounds`] for
//! [`Length`]. Use [`Length::range_steps`] for stable, finite range iteration.
//! [`Sum`] is supported for both owned and borrowed length values. Standard
//! `Ord` methods such as `min`, `max`, and `clamp` work with structural ordering.
//! `Product` and `Length * Length` are deliberately omitted because CSS lengths
//! have dimensions and their products cannot be represented as lengths.
//!
//! All lengths implement [`Copy`]. Expressions containing multiple units are
//! stored as fixed-size coefficients instead of heap-allocated expression trees.
//!
//! # Examples
//!
//! ```rust,ignore
//! # use mata_ui::widgets::style::length::{Axis, Length, LengthContext, LengthExt};
//! let width = Length::Vw(100.0) - Length::Rem(2.0);
//! let ctx = LengthContext::new((1200, 800), (400, 300), 16.0, 16.0);
//! assert_eq!(width.resolve(Axis::Horizontal, ctx), Some(1168.0));
//!
//! let padding = 10.px() + 0.5.rem();
//! assert_eq!(padding.resolve(Axis::Horizontal, ctx), Some(18.0));
//! ```

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    iter::{FusedIterator, Sum},
    mem,
    ops::{
        Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Rem, RemAssign,
        Sub, SubAssign, Bound, RangeBounds,
    },
    str::FromStr,
};

/// Compares floats consistently with [`Eq`] and [`Ord`].
///
/// `+0.0` and `-0.0` compare equal. Identical NaN bit patterns compare
/// equal, including when compared with themselves; different NaN payloads
/// and NaN signs follow [`f32::total_cmp`] ordering.
#[inline]
fn compare_f32(left: f32, right: f32) -> Ordering {
    if left == 0.0 && right == 0.0 {
        Ordering::Equal
    } else {
        left.total_cmp(&right)
    }
}

/// Returns the bits used to hash float-backed lengths consistently with `Eq`.
///
/// Signed zeros are canonicalized because they compare equal.
#[inline]
fn hashable_f32_bits(value: f32) -> u32 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

/// An ordinary numeric scalar that can scale a CSS length.
///
/// Scalars are converted to `f32`, which may lose precision for wide integers
/// and `f64`. Length-to-length multiplication is intentionally unavailable:
/// it would require a squared length, not another CSS length.
pub trait LengthScalar: Copy {
    /// Converts this numeric scalar to the `f32` representation used by lengths.
    fn as_length_scalar(self) -> f32;
}

/// Implements conversion from primitive numeric scalars.
macro_rules! impl_length_scalar_trait {
    ($($ty:ty),* $(,)?) => {
        $(
            impl LengthScalar for $ty {
                /// Converts this primitive numeric value into an `f32` scalar.
                #[inline]
                fn as_length_scalar(self) -> f32 { self as f32 }
            }
        )*
    };
}

impl_length_scalar_trait!(
    f32, f64, i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
);

/// Maps IEEE-754 infinities to finite extrema and NaNs to zero.
///
/// This is the explicit, deterministic policy used by `saturating_*` methods.
#[inline]
fn saturate_f32(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else if value == f32::INFINITY {
        f32::MAX
    } else if value == f32::NEG_INFINITY {
        -f32::MAX
    } else {
        value
    }
}

/// A CSS-like length, including deferred calculations involving different units.
///
/// Each value is [`Copy`], including the [`Length::Calc`] variant. Mixed-unit
/// expressions retain their constituent units until [`Length::resolve`] is called.
/// Arithmetic involving `Auto` produces `Auto`, which cannot resolve to pixels.
#[derive(Debug, Clone, Copy)]
pub enum Length {
    /// An absolute number of CSS pixels.
    Px(f32),
    /// A percentage of the context's containing width or height.
    Percent(f32),
    /// A percentage of viewport width (1vw = 1% of viewport width).
    Vw(f32),
    /// A percentage of viewport height (1vh = 1% of viewport height).
    Vh(f32),
    /// A percentage of the smaller viewport dimension.
    Vmin(f32),
    /// A percentage of the larger viewport dimension.
    Vmax(f32),
    /// A multiple of the current element's font size.
    Em(f32),
    /// A multiple of the root element's font size.
    Rem(f32),
    /// A length determined by the layout algorithm rather than unit resolution.
    Auto,
    /// A fixed-size, potentially mixed-unit calculation.
    Calc(LengthCalc),
}

impl Default for Length {
    /// Returns a zero-pixel length.
    fn default() -> Self {
        Self::Px(0.0)
    }
}

/// Coefficients for a linear combination of CSS length units.
///
/// For example, `calc(100vw - 2rem + 5px)` is stored as `vw = 100.0`,
/// `rem = -2.0`, and `px = 5.0`.
///
/// This structure contains no pointers, so it is cheap to copy and doesn't
/// allocate when used for arithmetic. All coefficients use CSS unit values,
/// not resolved pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct LengthCalc {
    /// Coefficient of `px` units.
    pub px: f32,
    /// Coefficient of `%` units.
    pub percent: f32,
    /// Coefficient of `vw` units.
    pub vw: f32,
    /// Coefficient of `vh` units.
    pub vh: f32,
    /// Coefficient of `vmin` units.
    pub vmin: f32,
    /// Coefficient of `vmax` units.
    pub vmax: f32,
    /// Coefficient of `em` units.
    pub em: f32,
    /// Coefficient of `rem` units.
    pub rem: f32,
}

impl LengthCalc {
    /// Returns `true` if every coefficient is numerically zero.
    #[inline]
    pub fn is_zero(self) -> bool {
        self.px == 0.0
            && self.percent == 0.0
            && self.vw == 0.0
            && self.vh == 0.0
            && self.vmin == 0.0
            && self.vmax == 0.0
            && self.em == 0.0
            && self.rem == 0.0
    }

    /// Returns whether all eight coefficients are finite (`!NaN`, `!Infinity`).
    #[inline]
    pub fn is_finite(self) -> bool {
        [
            self.px, self.percent, self.vw, self.vh,
            self.vmin, self.vmax, self.em, self.rem,
        ]
        .iter()
        .all(|coefficient| coefficient.is_finite())
    }

    /// Returns whether any unit coefficient is NaN.
    #[inline]
    pub fn is_nan(self) -> bool {
        [self.px, self.percent, self.vw, self.vh, self.vmin, self.vmax, self.em, self.rem]
            .iter().any(|value| value.is_nan())
    }

    /// Returns whether any unit coefficient is positive or negative infinity.
    #[inline]
    pub fn is_infinite(self) -> bool {
        [self.px, self.percent, self.vw, self.vh, self.vmin, self.vmax, self.em, self.rem]
            .iter().any(|value| value.is_infinite())
    }

    /// Returns coefficient-wise absolute values, not a resolved pixel magnitude.
    #[inline]
    pub fn abs(self) -> Self {
        self.map(f32::abs)
    }

    /// Applies a numeric operation independently to each unit coefficient.
    ///
    /// This preserves the linear, fixed-size representation and never allocates.
    #[inline]
    fn map(self, operation: impl Fn(f32) -> f32) -> Self {
        Self {
            px: operation(self.px),
            percent: operation(self.percent),
            vw: operation(self.vw),
            vh: operation(self.vh),
            vmin: operation(self.vmin),
            vmax: operation(self.vmax),
            em: operation(self.em),
            rem: operation(self.rem),
        }
    }

    /// Converts non-finite coefficients to their saturating finite equivalents.
    #[inline]
    fn saturated(self) -> Self {
        self.map(saturate_f32)
    }

    /// Resolves all coefficients to a pixel length using `axis` and `ctx`.
    ///
    /// Returns `None` when the result is not finite.
    pub fn resolve(self, axis: Axis, ctx: LengthContext) -> Option<f32> {
        let containing = match axis {
            Axis::Horizontal => ctx.containing.0,
            Axis::Vertical => ctx.containing.1,
        };
        let vw = ctx.viewport.0 / 100.0;
        let vh = ctx.viewport.1 / 100.0;
        let result = self.px
            + self.percent * (containing / 100.0)
            + self.vw * vw
            + self.vh * vh
            + self.vmin * vw.min(vh)
            + self.vmax * vw.max(vh)
            + self.em * ctx.font_size
            + self.rem * ctx.root_font_size;

        result.is_finite().then_some(result)
    }
}

// Float-backed lengths cannot derive `Eq` or `Ord`: their ordering must
// explicitly specify what happens to NaNs and signed zero.
impl PartialEq for LengthCalc {
    /// Compares each coefficient with a reflexive float equality policy.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for LengthCalc {}

impl Hash for LengthCalc {
    /// Hashes all coefficients in the same order as structural comparisons.
    fn hash<H: Hasher>(&self, state: &mut H) {
        for value in [
            self.px, self.percent, self.vw, self.vh,
            self.vmin, self.vmax, self.em, self.rem,
        ] {
            hashable_f32_bits(value).hash(state);
        }
    }
}

impl PartialOrd for LengthCalc {
    /// Returns a deterministic lexicographic ordering of all coefficients.
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LengthCalc {
    /// Compares coefficients in `px`, `%`, `vw`, `vh`, `vmin`, `vmax`, `em`, `rem` order.
    ///
    /// This is a structural ordering, **not** an ordering of resolved pixel sizes.
    fn cmp(&self, other: &Self) -> Ordering {
        let left = [
            self.px, self.percent, self.vw, self.vh,
            self.vmin, self.vmax, self.em, self.rem,
        ];
        let right = [
            other.px, other.percent, other.vw, other.vh,
            other.vmin, other.vmax, other.em, other.rem,
        ];
        for (a, b) in left.iter().zip(right.iter()) {
            let ordering = compare_f32(*a, *b);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        Ordering::Equal
    }
}

impl Add for LengthCalc {
    type Output = Self;

    /// Adds corresponding unit coefficients without resolving them.
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            px: self.px + rhs.px,
            percent: self.percent + rhs.percent,
            vw: self.vw + rhs.vw,
            vh: self.vh + rhs.vh,
            vmin: self.vmin + rhs.vmin,
            vmax: self.vmax + rhs.vmax,
            em: self.em + rhs.em,
            rem: self.rem + rhs.rem,
        }
    }
}

impl Sub for LengthCalc {
    type Output = Self;

    /// Subtracts corresponding unit coefficients without resolving them.
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            px: self.px - rhs.px,
            percent: self.percent - rhs.percent,
            vw: self.vw - rhs.vw,
            vh: self.vh - rhs.vh,
            vmin: self.vmin - rhs.vmin,
            vmax: self.vmax - rhs.vmax,
            em: self.em - rhs.em,
            rem: self.rem - rhs.rem,
        }
    }
}

impl Neg for LengthCalc {
    type Output = Self;

    /// Negates all unit coefficients.
    fn neg(self) -> Self::Output {
        Self {
            px: -self.px,
            percent: -self.percent,
            vw: -self.vw,
            vh: -self.vh,
            vmin: -self.vmin,
            vmax: -self.vmax,
            em: -self.em,
            rem: -self.rem,
        }
    }
}

/// Generates checked, wrapping, saturating, and overflowing arithmetic.
///
/// Floating-point "wrapping" means ordinary IEEE-754 arithmetic; it does
/// **not** wrap at a finite bound like integer arithmetic does. Overflowing
/// reports non-finite inputs/results; checked arithmetic rejects them.
macro_rules! safety_binary_methods {
    ($checked:ident, $wrapping:ident, $saturating:ident, $overflowing:ident;
     $rhs:ty; $op:tt) => {
        #[doc = concat!("Performs checked `", stringify!($op), "` arithmetic.")]
        ///
        /// Returns `None` for `Auto`, non-finite inputs/results, or invalid
        /// division/remainder. All unit coefficients are checked.
        #[inline]
        pub fn $checked(self, rhs: $rhs) -> Option<Self> {
            if !self.is_finite() || !rhs.is_finite() {
                return None;
            }
            let result = self $op rhs;
            result.is_finite().then_some(result)
        }

        #[doc = concat!("Performs IEEE-754 `", stringify!($op), "` arithmetic.")]
        ///
        /// Unlike integer wrapping, floating-point overflow produces infinity;
        /// zero division or remainder may produce infinity or NaN.
        #[inline]
        pub fn $wrapping(self, rhs: $rhs) -> Self {
            self $op rhs
        }

        #[doc = concat!("Performs saturating `", stringify!($op), "` arithmetic.")]
        ///
        /// Infinities clamp to `+/-f32::MAX`, and NaNs become zero, on each
        /// coefficient. `Auto` remains `Auto`.
        #[inline]
        pub fn $saturating(self, rhs: $rhs) -> Self {
            (self $op rhs).saturated()
        }

        #[doc = concat!("Performs overflowing `", stringify!($op), "` arithmetic.")]
        ///
        /// Returns the IEEE-754 result and whether either input or the output
        /// contains a non-finite value (or is `Auto`).
        #[inline]
        pub fn $overflowing(self, rhs: $rhs) -> (Self, bool) {
            let valid_inputs = self.is_finite() && rhs.is_finite();
            let result = self $op rhs;
            let overflowed = !valid_inputs || !result.is_finite();
            (result, overflowed)
        }
    };
}

/// Generates primitive-like scalar safety methods accepting integer or float scalars.
///
/// Each input scalar is converted to `f32` before evaluation. Floating-point
/// arithmetic retains IEEE-754 overflow behavior rather than integer wrapping.
macro_rules! safety_scalar_methods {
    ($checked:ident, $wrapping:ident, $saturating:ident, $overflowing:ident; $op:tt) => {
        #[doc = concat!("Checks scalar `", stringify!($op), "` for finite operands and output.")]
        #[inline]
        pub fn $checked<S: LengthScalar>(self, rhs: S) -> Option<Self> {
            let rhs = rhs.as_length_scalar();
            if !self.is_finite() || !rhs.is_finite() {
                return None;
            }
            let result = self $op rhs;
            result.is_finite().then_some(result)
        }

        #[doc = concat!("Applies scalar `", stringify!($op), "` using IEEE-754 arithmetic.")]
        #[inline]
        pub fn $wrapping<S: LengthScalar>(self, rhs: S) -> Self {
            self $op rhs.as_length_scalar()
        }

        #[doc = concat!("Applies scalar `", stringify!($op), "`, clamping non-finite results.")]
        #[inline]
        pub fn $saturating<S: LengthScalar>(self, rhs: S) -> Self {
            (self $op rhs.as_length_scalar()).saturated()
        }

        #[doc = concat!("Applies scalar `", stringify!($op), "` and reports non-finite results.")]
        #[inline]
        pub fn $overflowing<S: LengthScalar>(self, rhs: S) -> (Self, bool) {
            let rhs = rhs.as_length_scalar();
            let result = self $op rhs;
            (result, !self.is_finite() || !rhs.is_finite() || !result.is_finite())
        }
    };
}

/// Generates checked and saturating methods for lengths and pixel-convertible numbers.
///
/// The left operand always retains its CSS-length semantics. Plain numeric
/// right operands are interpreted as pixels through `From<T> for Length`.
macro_rules! safety_length_methods {
    ($checked:ident, $wrapping:ident, $saturating:ident, $overflowing:ident; $op:tt) => {
        #[doc = concat!("Performs checked length `", stringify!($op), "` arithmetic.")]
        ///
        /// Accepts another `Length` or any numeric type convertible to pixels.
        #[inline]
        pub fn $checked<T: Into<Self>>(self, rhs: T) -> Option<Self> {
            let rhs = rhs.into();
            if !self.is_finite() || !rhs.is_finite() {
                return None;
            }
            let result = self $op rhs;
            result.is_finite().then_some(result)
        }

        #[doc = concat!("Performs IEEE-754 length `", stringify!($op), "` arithmetic.")]
        #[inline]
        pub fn $wrapping<T: Into<Self>>(self, rhs: T) -> Self {
            self $op rhs.into()
        }

        #[doc = concat!("Performs saturating length `", stringify!($op), "` arithmetic.")]
        #[inline]
        pub fn $saturating<T: Into<Self>>(self, rhs: T) -> Self {
            (self $op rhs.into()).saturated()
        }

        #[doc = concat!("Performs length `", stringify!($op), "` and reports non-finite results.")]
        #[inline]
        pub fn $overflowing<T: Into<Self>>(self, rhs: T) -> (Self, bool) {
            let rhs = rhs.into();
            let result = self $op rhs;
            (result, !self.is_finite() || !rhs.is_finite() || !result.is_finite())
        }
    };
}

impl LengthCalc {
    safety_binary_methods!(checked_add, wrapping_add, saturating_add, overflowing_add; Self; +);
    safety_binary_methods!(checked_sub, wrapping_sub, saturating_sub, overflowing_sub; Self; -);
    safety_scalar_methods!(checked_mul, wrapping_mul, saturating_mul, overflowing_mul; *);
    safety_scalar_methods!(checked_div, wrapping_div, saturating_div, overflowing_div; /);
    safety_scalar_methods!(checked_rem, wrapping_rem, saturating_rem, overflowing_rem; %);

    /// Returns `None` if any coefficient is non-finite, or its negation is.
    #[inline]
    pub fn checked_neg(self) -> Option<Self> {
        let result = -self;
        (self.is_finite() && result.is_finite()).then_some(result)
    }

    /// Negates all coefficients according to IEEE-754 float semantics.
    #[inline]
    pub fn wrapping_neg(self) -> Self {
        -self
    }

    /// Negates and saturates all coefficients into the finite `f32` range.
    #[inline]
    pub fn saturating_neg(self) -> Self {
        (-self).saturated()
    }

    /// Negates all coefficients and reports any non-finite input/result.
    #[inline]
    pub fn overflowing_neg(self) -> (Self, bool) {
        let result = -self;
        let overflowed = !self.is_finite() || !result.is_finite();
        (result, overflowed)
    }
}

impl Div<f32> for LengthCalc {
    type Output = Self;

    /// Divides each unit coefficient by a unitless scalar.
    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        self.map(|value| value / rhs)
    }
}

impl Rem<f32> for LengthCalc {
    type Output = Self;

    /// Takes a remainder independently for each unit coefficient.
    ///
    /// This is coefficient-wise, **not** the remainder of the resolved length.
    #[inline]
    fn rem(self, rhs: f32) -> Self::Output {
        self.map(|value| value % rhs)
    }
}

/// Implements primitive numeric scaling for the coefficient vector.
///
/// The `f32` forms are defined directly; these additional forms cast their
/// operands to `f32` so integer literals can be used without explicit casts.
macro_rules! impl_calc_numeric_scalars {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Mul<$ty> for LengthCalc {
                type Output = Self;

                /// Multiplies each coefficient by a unitless numeric scalar.
                #[inline]
                fn mul(self, rhs: $ty) -> Self { self * (rhs as f32) }
            }

            impl Div<$ty> for LengthCalc {
                type Output = Self;

                /// Divides each coefficient by a unitless numeric scalar.
                #[inline]
                fn div(self, rhs: $ty) -> Self { self / (rhs as f32) }
            }

            impl Rem<$ty> for LengthCalc {
                type Output = Self;

                /// Calculates the remainder of each coefficient by a scalar.
                #[inline]
                fn rem(self, rhs: $ty) -> Self { self % (rhs as f32) }
            }

            impl MulAssign<$ty> for LengthCalc {
                /// Multiplies each coefficient by a scalar in place.
                #[inline]
                fn mul_assign(&mut self, rhs: $ty) { *self = *self * rhs; }
            }

            impl DivAssign<$ty> for LengthCalc {
                /// Divides each coefficient by a scalar in place.
                #[inline]
                fn div_assign(&mut self, rhs: $ty) { *self = *self / rhs; }
            }

            impl RemAssign<$ty> for LengthCalc {
                /// Calculates the coefficient-wise remainder in place.
                #[inline]
                fn rem_assign(&mut self, rhs: $ty) { *self = *self % rhs; }
            }

            impl Mul<LengthCalc> for $ty {
                type Output = LengthCalc;

                /// Multiplies a coefficient vector by a unitless scalar.
                #[inline]
                fn mul(self, rhs: LengthCalc) -> LengthCalc { rhs * (self as f32) }
            }
        )*
    };
}

impl_calc_numeric_scalars!(
    f64, i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
);

impl Mul<LengthCalc> for f32 {
    type Output = LengthCalc;

    /// Multiplies a coefficient vector by a unitless floating-point scalar.
    #[inline]
    fn mul(self, rhs: LengthCalc) -> LengthCalc { rhs * self }
}

impl AddAssign for LengthCalc {
    /// Adds another coefficient vector in place.
    #[inline]
    fn add_assign(&mut self, rhs: Self) { *self = *self + rhs; }
}

impl SubAssign for LengthCalc {
    /// Subtracts another coefficient vector in place.
    #[inline]
    fn sub_assign(&mut self, rhs: Self) { *self = *self - rhs; }
}

impl MulAssign<f32> for LengthCalc {
    /// Multiplies each coefficient by a unitless scalar in place.
    #[inline]
    fn mul_assign(&mut self, rhs: f32) { *self = *self * rhs; }
}

impl DivAssign<f32> for LengthCalc {
    /// Divides each coefficient by a unitless scalar in place.
    #[inline]
    fn div_assign(&mut self, rhs: f32) { *self = *self / rhs; }
}

impl RemAssign<f32> for LengthCalc {
    /// Takes each coefficient's remainder by a scalar in place.
    #[inline]
    fn rem_assign(&mut self, rhs: f32) { *self = *self % rhs; }
}

/// The containing dimension selected for percentage-based resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    /// Use the containing block's width.
    Horizontal,
    /// Use the containing block's height.
    Vertical,
}

/// Layout dimensions and font sizes used to resolve a [`Length`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LengthContext {
    /// Viewport width and height in CSS pixels.
    pub viewport: (f32, f32),
    /// Containing block width and height in CSS pixels.
    pub containing: (f32, f32),
    /// Current element's font size in CSS pixels.
    pub font_size: f32,
    /// Root element's font size in CSS pixels.
    pub root_font_size: f32,
}

impl LengthContext {
    /// Creates a context from integer viewport and containing dimensions.
    ///
    /// Font sizes are specified in CSS pixels.
    #[inline]
    pub fn new(
        viewport: (u32, u32),
        containing: (u32, u32),
        font_size: f32,
        root_font_size: f32,
    ) -> Self {
        Self {
            viewport: (viewport.0 as f32, viewport.1 as f32),
            containing: (containing.0 as f32, containing.1 as f32),
            font_size,
            root_font_size,
        }
    }
}

/// Generates a documented constructor for each numeric CSS unit.
macro_rules! length_constructors {
    ($( $(#[$docs:meta])* $method:ident => $variant:ident ),* $(,)?) => {
        impl Length {
            $(
                $(#[$docs])*
                #[inline]
                pub const fn $method(value: f32) -> Self {
                    Self::$variant(value)
                }
            )*
        }
    };
}

length_constructors! {
    /// Creates a length measured in CSS pixels.
    px => Px,
    /// Creates a length measured in percentages of the containing dimension.
    percent => Percent,
    /// Creates a length measured in percentages of viewport width.
    vw => Vw,
    /// Creates a length measured in percentages of viewport height.
    vh => Vh,
    /// Creates a length measured in percentages of the smaller viewport dimension.
    vmin => Vmin,
    /// Creates a length measured in percentages of the larger viewport dimension.
    vmax => Vmax,
    /// Creates a length measured in multiples of the current font size.
    em => Em,
    /// Creates a length measured in multiples of the root font size.
    rem => Rem,
}

impl Length {
    /// Returns coefficients representing this length, or `None` for `Auto`.
    ///
    /// This method does not require a layout context and does not resolve units.
    #[inline]
    pub fn coefficients(self) -> Option<LengthCalc> {
        let mut out = LengthCalc::default();
        match self {
            Self::Px(value) => out.px = value,
            Self::Percent(value) => out.percent = value,
            Self::Vw(value) => out.vw = value,
            Self::Vh(value) => out.vh = value,
            Self::Vmin(value) => out.vmin = value,
            Self::Vmax(value) => out.vmax = value,
            Self::Em(value) => out.em = value,
            Self::Rem(value) => out.rem = value,
            Self::Calc(calc) => return Some(calc),
            Self::Auto => return None,
        }
        Some(out)
    }

    /// Creates the simplest length representation for the coefficients.
    ///
    /// When exactly one unit has a nonzero coefficient, returns the matching
    /// ordinary `Length` variant. When all coefficients are zero, returns `0px`.
    /// Otherwise returns a [`Length::Calc`] value.
    #[inline]
    pub fn from_coefficients(calc: LengthCalc) -> Self {
        let mut result = Self::Px(0.0);
        let mut count = 0usize;
        macro_rules! unit {
            ($field:ident, $variant:ident) => {
                if calc.$field != 0.0 {
                    result = Self::$variant(calc.$field);
                    count += 1;
                }
            };
        }
        unit!(px, Px);
        unit!(percent, Percent);
        unit!(vw, Vw);
        unit!(vh, Vh);
        unit!(vmin, Vmin);
        unit!(vmax, Vmax);
        unit!(em, Em);
        unit!(rem, Rem);
        if count > 1 { Self::Calc(calc) } else { result }
    }

    /// Returns whether this represents a finite measurable length.
    ///
    /// `Auto` is not measurable and therefore returns `false`.
    #[inline]
    pub fn is_finite(self) -> bool {
        self.coefficients().is_some_and(LengthCalc::is_finite)
    }

    /// Returns a new length with `operation` applied to each coefficient.
    ///
    /// The unit variant is preserved for ordinary lengths; `Auto` remains
    /// `Auto`. Mixed-unit calculations are simplified if only one unit remains.
    #[inline]
    fn map(self, operation: impl Fn(f32) -> f32) -> Self {
        match self {
            Self::Px(v) => Self::Px(operation(v)),
            Self::Percent(v) => Self::Percent(operation(v)),
            Self::Vw(v) => Self::Vw(operation(v)),
            Self::Vh(v) => Self::Vh(operation(v)),
            Self::Vmin(v) => Self::Vmin(operation(v)),
            Self::Vmax(v) => Self::Vmax(operation(v)),
            Self::Em(v) => Self::Em(operation(v)),
            Self::Rem(v) => Self::Rem(operation(v)),
            Self::Auto => Self::Auto,
            Self::Calc(calc) => Self::from_coefficients(calc.map(operation)),
        }
    }

    /// Saturates any non-finite unit coefficient to a finite value.
    ///
    /// Positive/negative infinity becomes positive/negative `f32::MAX`;
    /// NaN becomes zero. `Auto` stays `Auto`.
    #[inline]
    fn saturated(self) -> Self {
        self.map(saturate_f32)
    }

    /// Compares two resolved lengths, returning `None` for `Auto` or NaN/inf.
    ///
    /// Unlike [`Ord::cmp`], this compares actual pixel sizes in a given layout.
    #[inline]
    pub fn cmp_resolved(
        self, other: Self, axis: Axis, ctx: LengthContext,
    ) -> Option<Ordering> {
        let left = self.resolve(axis, ctx)?;
        let right = other.resolve(axis, ctx)?;
        Some(compare_f32(left, right))
    }

    /// Resolves the length to pixels for a specified axis and layout context.
    ///
    /// Returns `None` for `Auto` or a non-finite computed length. Values using
    /// percentages depend on the selected containing dimension.
    #[inline]
    pub fn resolve(self, axis: Axis, ctx: LengthContext) -> Option<f32> {
        self.coefficients()?.resolve(axis, ctx)
    }

    /// Returns a stable iterator over a bounded [`RangeBounds`] of lengths.
    ///
    /// Use an ordinary Rust range such as `10.px()..30.px()` or an inclusive
    /// range such as `10.px()..=30.px()`. Unlike primitive integer ranges,
    /// `Length` cannot implement the unstable [`std::iter::Step`] trait on stable
    /// Rust, so this explicit iterator uses a caller-specified increment.
    ///
    /// Returns `None` if the bounds are unbounded, the endpoints and step use
    /// different units, any value is non-finite, or the step is zero. Mixed-unit
    /// expressions require a layout context and cannot be stepped directly.
    /// Negative steps permit descending ranges. To keep floating-point loops
    /// finite, iteration ends if adding the step does not advance the value.
    ///
    /// ```rust,ignore
    /// let range = Length::range_steps(0.px()..=10.px(), 2.px()).unwrap();
    /// assert_eq!(range.collect::<Vec<_>>().len(), 6);
    /// ```
    pub fn range_steps<R: RangeBounds<Self>>(bounds: R, step: Self) -> Option<LengthSteps> {
        let (start, exclude_start) = match bounds.start_bound() {
            Bound::Included(value) => (*value, false),
            Bound::Excluded(value) => (*value, true),
            Bound::Unbounded => return None,
        };
        let (end, inclusive) = match bounds.end_bound() {
            Bound::Included(value) => (*value, true),
            Bound::Excluded(value) => (*value, false),
            Bound::Unbounded => return None,
        };
        if mem::discriminant(&start) != mem::discriminant(&end)
            || mem::discriminant(&start) != mem::discriminant(&step)
        {
            return None;
        }
        let increment = step.single_unit_value()?;
        if !start.is_finite() || !end.is_finite() || !increment.is_finite() || increment == 0.0 {
            return None;
        }
        let first = if exclude_start { start + step } else { start };
        if !first.is_finite() || (exclude_start && first == start) {
            return None;
        }
        Some(LengthSteps { next: Some(first), end, step, inclusive })
    }

    /// Extracts the `f32` coefficient from a single-unit length variant.
    ///
    /// Returns `None` for `Auto` and mixed-unit `Calc` values.
    #[inline]
    fn single_unit_value(self) -> Option<f32> {
        match self {
            Self::Px(value) | Self::Percent(value) | Self::Vw(value)
            | Self::Vh(value) | Self::Vmin(value) | Self::Vmax(value)
            | Self::Em(value) | Self::Rem(value) => Some(value),
            Self::Auto | Self::Calc(_) => None,
        }
    }

    /// Returns whether all coefficients are zero; `Auto` is not zero.
    #[inline]
    pub fn is_zero(self) -> bool {
        self.coefficients().is_some_and(LengthCalc::is_zero)
    }

    /// Returns whether any coefficient is NaN; `Auto` is not numeric.
    #[inline]
    pub fn is_nan(self) -> bool {
        self.coefficients().is_some_and(|coefficients| coefficients.is_nan())
    }

    /// Returns whether any coefficient is infinite; `Auto` is not numeric.
    #[inline]
    pub fn is_infinite(self) -> bool {
        self.coefficients().is_some_and(|coefficients| coefficients.is_infinite())
    }

    /// Returns the absolute value of each unit coefficient.
    ///
    /// For mixed units this operates on **individual coefficients**, not on the
    /// resolved pixel size. Use `resolve(...).map(f32::abs)` for pixel magnitude.
    #[inline]
    pub fn abs(self) -> Self {
        self.map(f32::abs)
    }

    /// Parses a length, including CSS-style additive `calc(...)` expressions.
    ///
    /// This is a convenience wrapper for [`FromStr`].
    #[inline]
    pub fn parse(value: &str) -> Result<Self, String> {
        value.parse()
    }

    /// Interpolates between two lengths by blending their unit coefficients.
    ///
    /// Unlike discrete interpolation, this smoothly interpolates mixed units
    /// without needing a layout context. If either endpoint is `Auto`, switches
    /// discretely at `t = 1.0`. The interpolation factor is not clamped.
    #[inline]
    pub(crate) fn interpolate(self, other: Self, t: f32) -> Self {
        if matches!(self, Self::Auto) || matches!(other, Self::Auto) {
            return if t >= 1.0 { other } else { self };
        }
        Self::from_coefficients(
            self.coefficients().unwrap_or_default() * (1.0 - t)
                + other.coefficients().unwrap_or_default() * t,
        )
    }
}

/// A stable iterator over a finite range of lengths that share one unit.
///
/// Construct with [`Length::range_steps`]. The iterator does not allocate and
/// preserves the original unit. A step that would stop changing the current
/// value because of `f32` precision terminates the iterator.
#[derive(Debug, Clone, Copy)]
pub struct LengthSteps {
    /// Next candidate to yield, or `None` when iteration is finished.
    next: Option<Length>,
    /// Exclusive or inclusive bound of the range.
    end: Length,
    /// Signed increment applied between successive values.
    step: Length,
    /// Whether the ending bound should be yielded when reached exactly.
    inclusive: bool,
}

impl Iterator for LengthSteps {
    /// The unit-preserving value produced on each iteration.
    type Item = Length;

    /// Returns the next length or `None` after the range has ended.
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        let value = current.single_unit_value()?;
        let end = self.end.single_unit_value()?;
        let step = self.step.single_unit_value()?;

        let beyond_end = if step > 0.0 {
            value > end || (!self.inclusive && value == end)
        } else {
            value < end || (!self.inclusive && value == end)
        };
        if beyond_end {
            self.next = None;
            return None;
        }

        let following = current + self.step;
        self.next = if following.is_finite() && following.single_unit_value() != Some(value) {
            Some(following)
        } else {
            None
        };
        Some(current)
    }
}

impl FusedIterator for LengthSteps {}

impl Mul<f32> for LengthCalc {
    type Output = Self;

    /// Scales all coefficients by a scalar.
    fn mul(self, scalar: f32) -> Self::Output {
        Self {
            px: self.px * scalar,
            percent: self.percent * scalar,
            vw: self.vw * scalar,
            vh: self.vh * scalar,
            vmin: self.vmin * scalar,
            vmax: self.vmax * scalar,
            em: self.em * scalar,
            rem: self.rem * scalar,
        }
    }
}

impl PartialEq for Length {
    /// Compares the unit variant and its value(s), using total float ordering.
    ///
    /// `+0.0` and `-0.0` are equal, as with regular float equality.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Length {}

impl Hash for Length {
    /// Hashes the variant discriminant and its canonicalized float values.
    ///
    /// Two values that compare equal always produce the same hash, including
    /// lengths containing signed zero and identically encoded NaNs.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.variant_rank().hash(state);
        match self {
            Self::Px(v) | Self::Percent(v) | Self::Vw(v)
            | Self::Vh(v) | Self::Vmin(v) | Self::Vmax(v)
            | Self::Em(v) | Self::Rem(v) => hashable_f32_bits(*v).hash(state),
            Self::Auto => {},
            Self::Calc(calc) => calc.hash(state),
        }
    }
}

impl PartialOrd for Length {
    /// Provides a deterministic structural ordering; not an ordering by pixels.
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Length {
    /// The stable variant order used by `Ord` for lengths of different units.
    #[inline]
    fn variant_rank(self) -> u8 {
        match self {
            Self::Px(_) => 0,
            Self::Percent(_) => 1,
            Self::Vw(_) => 2,
            Self::Vh(_) => 3,
            Self::Vmin(_) => 4,
            Self::Vmax(_) => 5,
            Self::Em(_) => 6,
            Self::Rem(_) => 7,
            Self::Auto => 8,
            Self::Calc(_) => 9,
        }
    }
}

impl Ord for Length {
    /// Orders values by unit variant, then numeric value/coefficients.
    ///
    /// This ordering is context-free and should be used for containers such
    /// as `BTreeSet`, **not** to compare physical sizes of mixed CSS units.
    fn cmp(&self, other: &Self) -> Ordering {
        let ranks = self.variant_rank().cmp(&other.variant_rank());
        if ranks != Ordering::Equal {
            return ranks;
        }
        match (self, other) {
            (Self::Px(a), Self::Px(b))
            | (Self::Percent(a), Self::Percent(b))
            | (Self::Vw(a), Self::Vw(b))
            | (Self::Vh(a), Self::Vh(b))
            | (Self::Vmin(a), Self::Vmin(b))
            | (Self::Vmax(a), Self::Vmax(b))
            | (Self::Em(a), Self::Em(b))
            | (Self::Rem(a), Self::Rem(b)) => compare_f32(*a, *b),
            (Self::Calc(a), Self::Calc(b)) => a.cmp(b),
            (Self::Auto, Self::Auto) => Ordering::Equal,
            _ => unreachable!("matching variant ranks must represent the same unit"),
        }
    }
}

/// Generates unit-aware addition and subtraction for lengths.
macro_rules! impl_length_op {
    ($trait:ident, $method:ident, $operator:tt) => {
        impl $trait for Length {
            type Output = Length;

            /// Combines lengths with the same unit directly, and different units
            /// by combining fixed-size coefficients. `Auto` remains unresolved.
            #[inline]
            fn $method(self, rhs: Self) -> Self::Output {
                match (self, rhs) {
                    (Self::Px(a), Self::Px(b)) => Self::Px(a $operator b),
                    (Self::Percent(a), Self::Percent(b)) => Self::Percent(a $operator b),
                    (Self::Vw(a), Self::Vw(b)) => Self::Vw(a $operator b),
                    (Self::Vh(a), Self::Vh(b)) => Self::Vh(a $operator b),
                    (Self::Vmin(a), Self::Vmin(b)) => Self::Vmin(a $operator b),
                    (Self::Vmax(a), Self::Vmax(b)) => Self::Vmax(a $operator b),
                    (Self::Em(a), Self::Em(b)) => Self::Em(a $operator b),
                    (Self::Rem(a), Self::Rem(b)) => Self::Rem(a $operator b),
                    (Self::Auto, _) | (_, Self::Auto) => Self::Auto,
                    (left, right) => Self::from_coefficients(
                        left.coefficients().unwrap_or_default()
                            $operator right.coefficients().unwrap_or_default(),
                    ),
                }
            }
        }
    };
}

impl_length_op!(Add, add, +);
impl_length_op!(Sub, sub, -);

impl AddAssign for Length {
    /// Adds another length to this one in place, preserving deferred units.
    #[inline]
    fn add_assign(&mut self, rhs: Self) { *self = *self + rhs; }
}

impl SubAssign for Length {
    /// Subtracts another length in place, preserving deferred units.
    #[inline]
    fn sub_assign(&mut self, rhs: Self) { *self = *self - rhs; }
}

impl Mul<f32> for Length {
    type Output = Self;

    /// Scales each unit by a unitless float (no length-squared unit is created).
    #[inline]
    fn mul(self, rhs: f32) -> Self::Output { self.map(|v| v * rhs) }
}

impl Div<f32> for Length {
    type Output = Self;

    /// Divides each unit coefficient by a unitless float.
    #[inline]
    fn div(self, rhs: f32) -> Self::Output { self.map(|v| v / rhs) }
}

impl Rem<f32> for Length {
    type Output = Self;

    /// Computes the remainder of each unit coefficient against a scalar.
    ///
    /// A mixed-unit expression's coefficient-wise remainder is NOT equivalent
    /// to the remainder of its resolved pixel value.
    #[inline]
    fn rem(self, rhs: f32) -> Self::Output { self.map(|v| v % rhs) }
}

impl MulAssign<f32> for Length {
    /// Multiplies all unit coefficients by a scalar in place.
    #[inline]
    fn mul_assign(&mut self, rhs: f32) { *self = *self * rhs; }
}

impl DivAssign<f32> for Length {
    /// Divides all unit coefficients by a scalar in place.
    #[inline]
    fn div_assign(&mut self, rhs: f32) { *self = *self / rhs; }
}

impl RemAssign<f32> for Length {
    /// Takes the coefficient-wise remainder in place.
    #[inline]
    fn rem_assign(&mut self, rhs: f32) { *self = *self % rhs; }
}

impl Length {
    safety_length_methods!(checked_add, wrapping_add, saturating_add, overflowing_add; +);
    safety_length_methods!(checked_sub, wrapping_sub, saturating_sub, overflowing_sub; -);
    safety_scalar_methods!(checked_mul, wrapping_mul, saturating_mul, overflowing_mul; *);
    safety_scalar_methods!(checked_div, wrapping_div, saturating_div, overflowing_div; /);
    safety_scalar_methods!(checked_rem, wrapping_rem, saturating_rem, overflowing_rem; %);

    /// Returns the negated length, or `None` for `Auto` or non-finite values.
    #[inline]
    pub fn checked_neg(self) -> Option<Self> {
        let result = -self;
        (self.is_finite() && result.is_finite()).then_some(result)
    }

    /// Negates the length with IEEE-754 floating-point semantics.
    #[inline]
    pub fn wrapping_neg(self) -> Self {
        -self
    }

    /// Negates the length, clamping infinities and mapping NaNs to zero.
    #[inline]
    pub fn saturating_neg(self) -> Self {
        (-self).saturated()
    }

    /// Negates the length and reports non-finite input/results or `Auto`.
    #[inline]
    pub fn overflowing_neg(self) -> (Self, bool) {
        let result = -self;
        (result, !self.is_finite() || !result.is_finite())
    }
}

// Rust does not allow a manual `Drop` implementation on a `Copy` type. Neither
// `Length` nor `LengthCalc` owns heap memory, so drop glue is unnecessary.

/// Supports scalar-friendly arithmetic without changing length-valued APIs.
///
/// `Length +/- number` treats the number as pixels, as does `From<number>`.
/// `Length * / % number` treats the number as a unitless scalar.
macro_rules! impl_length_scalar {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Add<$ty> for Length {
                type Output = Length;

                /// Adds a pixel length to this length.
                #[inline]
                fn add(self, rhs: $ty) -> Self::Output { self + Length::Px(rhs as f32) }
            }

            impl Sub<$ty> for Length {
                type Output = Length;

                /// Subtracts a pixel length from this length.
                #[inline]
                fn sub(self, rhs: $ty) -> Self::Output { self - Length::Px(rhs as f32) }
            }

            impl AddAssign<$ty> for Length {
                /// Adds a pixel value in place.
                #[inline]
                fn add_assign(&mut self, rhs: $ty) { *self = *self + rhs; }
            }

            impl SubAssign<$ty> for Length {
                /// Subtracts a pixel value in place.
                #[inline]
                fn sub_assign(&mut self, rhs: $ty) { *self = *self - rhs; }
            }

            impl Mul<$ty> for Length {
                type Output = Length;

                /// Multiplies all units by a numeric scalar.
                #[inline]
                fn mul(self, rhs: $ty) -> Self::Output { self * (rhs as f32) }
            }

            impl Div<$ty> for Length {
                type Output = Length;

                /// Divides all units by a numeric scalar.
                #[inline]
                fn div(self, rhs: $ty) -> Self::Output { self / (rhs as f32) }
            }

            impl Rem<$ty> for Length {
                type Output = Length;

                /// Takes the coefficient-wise remainder by a numeric scalar.
                #[inline]
                fn rem(self, rhs: $ty) -> Self::Output { self % (rhs as f32) }
            }

            impl MulAssign<$ty> for Length {
                /// Multiplies the length by a numeric scalar in place.
                #[inline]
                fn mul_assign(&mut self, rhs: $ty) { *self = *self * rhs; }
            }

            impl DivAssign<$ty> for Length {
                /// Divides the length by a numeric scalar in place.
                #[inline]
                fn div_assign(&mut self, rhs: $ty) { *self = *self / rhs; }
            }

            impl RemAssign<$ty> for Length {
                /// Takes the coefficient-wise remainder by a scalar in place.
                #[inline]
                fn rem_assign(&mut self, rhs: $ty) { *self = *self % rhs; }
            }
        )*
    };
}

impl_length_scalar!(f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

impl Add<f32> for Length {
    type Output = Self;

    /// Adds a pixel value to this length.
    #[inline]
    fn add(self, rhs: f32) -> Self::Output { self + Self::Px(rhs) }
}

impl Sub<f32> for Length {
    type Output = Self;

    /// Subtracts a pixel value from this length.
    #[inline]
    fn sub(self, rhs: f32) -> Self::Output { self - Self::Px(rhs) }
}

impl AddAssign<f32> for Length {
    /// Adds a pixel value in place.
    #[inline]
    fn add_assign(&mut self, rhs: f32) { *self = *self + rhs; }
}

impl SubAssign<f32> for Length {
    /// Subtracts a pixel value in place.
    #[inline]
    fn sub_assign(&mut self, rhs: f32) { *self = *self - rhs; }
}

/// Enables commutative scalar-left addition and multiplication.
macro_rules! impl_scalar_left_length {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Add<Length> for $ty {
                type Output = Length;

                /// Adds a pixel value to a CSS length.
                #[inline]
                fn add(self, rhs: Length) -> Self::Output { Length::Px(self as f32) + rhs }
            }

            impl Sub<Length> for $ty {
                type Output = Length;

                /// Subtracts a length from a pixel value.
                #[inline]
                fn sub(self, rhs: Length) -> Self::Output { Length::Px(self as f32) - rhs }
            }

            impl Mul<Length> for $ty {
                type Output = Length;

                /// Multiplies a length by a numeric scalar.
                #[inline]
                fn mul(self, rhs: Length) -> Self::Output { rhs * (self as f32) }
            }
        )*
    };
}

impl_scalar_left_length!(f32, f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

impl Neg for Length {
    type Output = Self;

    /// Negates the length without resolving relative units.
    #[inline]
    fn neg(self) -> Self::Output {
        match self {
            Self::Auto => Self::Auto,
            Self::Px(v) => Self::Px(-v),
            Self::Percent(v) => Self::Percent(-v),
            Self::Vw(v) => Self::Vw(-v),
            Self::Vh(v) => Self::Vh(-v),
            Self::Vmin(v) => Self::Vmin(-v),
            Self::Vmax(v) => Self::Vmax(-v),
            Self::Em(v) => Self::Em(-v),
            Self::Rem(v) => Self::Rem(-v),
            Self::Calc(calc) => Self::from_coefficients(-calc),
        }
    }
}

/// Generates conversions from primitive numeric types into pixel lengths.
macro_rules! impl_length_from {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for Length {
                /// Creates a pixel length by casting the number to `f32`.
                #[inline]
                fn from(value: $ty) -> Self {
                    Self::Px(value as f32)
                }
            }
        )*
    };
}

impl_length_from!(f32, f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

/// Methods that create CSS lengths directly from numeric values.
///
/// Import this trait to use syntax such as `20.px()` or `1.5.rem()`.
pub trait LengthExt {
    /// Creates a pixel length from the numeric value.
    fn px(self) -> Length;
    /// Creates a percentage length from the numeric value.
    fn percent(self) -> Length;
    /// Creates a viewport-width length from the numeric value.
    fn vw(self) -> Length;
    /// Creates a viewport-height length from the numeric value.
    fn vh(self) -> Length;
    /// Creates a viewport-minimum length from the numeric value.
    fn vmin(self) -> Length;
    /// Creates a viewport-maximum length from the numeric value.
    fn vmax(self) -> Length;
    /// Creates a current-font-size-relative length from the numeric value.
    fn em(self) -> Length;
    /// Creates a root-font-size-relative length from the numeric value.
    fn rem(self) -> Length;
}

/// Generates numeric extension-trait implementations.
macro_rules! impl_length_ext {
    ($($ty:ty),* $(,)?) => {
        $(
            impl LengthExt for $ty {
                /// Converts this number to pixels.
                #[inline]
                fn px(self) -> Length { Length::Px(self as f32) }

                /// Converts this number to a percentage.
                #[inline]
                fn percent(self) -> Length { Length::Percent(self as f32) }

                /// Converts this number to viewport-width units.
                #[inline]
                fn vw(self) -> Length { Length::Vw(self as f32) }

                /// Converts this number to viewport-height units.
                #[inline]
                fn vh(self) -> Length { Length::Vh(self as f32) }

                /// Converts this number to viewport-minimum units.
                #[inline]
                fn vmin(self) -> Length { Length::Vmin(self as f32) }

                /// Converts this number to viewport-maximum units.
                #[inline]
                fn vmax(self) -> Length { Length::Vmax(self as f32) }

                /// Converts this number to current-font-size-relative units.
                #[inline]
                fn em(self) -> Length { Length::Em(self as f32) }

                /// Converts this number to root-font-size-relative units.
                #[inline]
                fn rem(self) -> Length { Length::Rem(self as f32) }
            }
        )*
    };
}

impl_length_ext!(f32, f64, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

impl Sum for Length {
    /// Sums an iterator of CSS lengths without resolving relative units.
    ///
    /// An empty iterator returns `0px`, as with primitive numeric sums.
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |sum, value| sum + value)
    }
}

impl<'a> Sum<&'a Length> for Length {
    /// Sums borrowed lengths, taking advantage of the `Copy` representation.
    #[inline]
    fn sum<I: Iterator<Item = &'a Length>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

impl Sum for LengthCalc {
    /// Adds coefficient vectors from an iterator; empty iterators yield zero.
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |sum, value| sum + value)
    }
}

impl<'a> Sum<&'a LengthCalc> for LengthCalc {
    /// Adds borrowed coefficient vectors from an iterator.
    #[inline]
    fn sum<I: Iterator<Item = &'a LengthCalc>>(iter: I) -> Self {
        iter.copied().sum()
    }
}

impl TryFrom<&str> for Length {
    type Error = String;

    /// Parses a CSS-like length from a string slice.
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<String> for Length {
    type Error = String;

    /// Parses a CSS-like length from an owned string.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

/// Parser for additive CSS length expressions, including nested `calc(...)`.
struct LengthParser<'a> {
    /// Lowercase input text to parse.
    input: &'a str,
    /// Current byte position in `input`.
    pos: usize,
}

impl<'a> LengthParser<'a> {
    /// Creates a parser positioned at the beginning of `input`.
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Returns the next unconsumed ASCII byte, if any.
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    /// Consumes ASCII whitespace and returns whether any was present.
    fn skip_whitespace(&mut self) -> bool {
        let start = self.pos;
        while matches!(self.peek(), Some(b) if b.is_ascii_whitespace()) {
            self.pos += 1;
        }
        self.pos > start
    }

    /// Consumes `prefix` when it matches at the current location.
    fn consume(&mut self, prefix: &str) -> bool {
        if self.input[self.pos..].starts_with(prefix) {
            self.pos += prefix.len();
            true
        } else {
            false
        }
    }

    /// Parses an entire atomic length, rejecting any remaining text.
    fn parse_root(mut self) -> Result<Length, String> {
        self.skip_whitespace();
        let value = self.parse_atom()?;
        self.skip_whitespace();
        if self.pos != self.input.len() {
            Err(format!("unexpected input near {:?}", &self.input[self.pos..]))
        } else {
            Ok(value)
        }
    }

    /// Parses a `calc(...)` call, a parenthesized group, or a numeric unit.
    fn parse_atom(&mut self) -> Result<Length, String> {
        self.skip_whitespace();
        if self.consume("calc(") || self.consume("(") {
            let value = self.parse_expression()?;
            if !self.consume(")") {
                return Err("unclosed calc/group parenthesis".into());
            }
            return Ok(value);
        }
        self.parse_number_with_unit()
    }

    /// Parses a `+`/`-` expression until a matching `)` or end of input.
    ///
    /// Follows CSS syntax requiring whitespace around binary `+` and `-`.
    fn parse_expression(&mut self) -> Result<Length, String> {
        let mut result = self.parse_atom()?;
        loop {
            let had_whitespace = self.skip_whitespace();
            match self.peek() {
                None | Some(b')') => return Ok(result),
                Some(op @ (b'+' | b'-')) => {
                    if !had_whitespace {
                        return Err("calc operators require surrounding whitespace".into());
                    }
                    self.pos += 1;
                    if !matches!(self.peek(), Some(b) if b.is_ascii_whitespace()) {
                        return Err("calc operators require surrounding whitespace".into());
                    }
                    let rhs = self.parse_atom()?;
                    result = if op == b'+' { result + rhs } else { result - rhs };
                }
                _ => {
                    return Err(format!(
                        "unexpected input in calc near {:?}",
                        &self.input[self.pos..]
                    ));
                }
            }
        }
    }

    /// Parses a finite decimal number and an optional CSS length-unit suffix.
    ///
    /// A unitless number is only accepted when it is zero.
    fn parse_number_with_unit(&mut self) -> Result<Length, String> {
        let start = self.pos;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.pos += 1;
        }
        let digits_start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if self.input.as_bytes()[digits_start..self.pos]
            .iter()
            .all(|b| !b.is_ascii_digit())
        {
            return Err(format!("expected CSS length near {:?}", &self.input[start..]));
        }

        // 'e' introduces an exponent only if exponent digits follow;
        // otherwise it can begin the 'em' unit.
        if matches!(self.peek(), Some(b'e')) {
            let mut next = self.pos + 1;
            if matches!(self.input.as_bytes().get(next), Some(b'+' | b'-')) {
                next += 1;
            }
            if matches!(self.input.as_bytes().get(next), Some(b'0'..=b'9')) {
                self.pos = next + 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
        }

        let value = self.input[start..self.pos]
            .parse::<f32>()
            .map_err(|_| "invalid numeric length".to_string())?;
        if !value.is_finite() {
            return Err("length must be finite".into());
        }

        let unit_start = self.pos;
        while matches!(self.peek(), Some(b) if b.is_ascii_alphabetic() || b == b'%') {
            self.pos += 1;
        }
        let unit = &self.input[unit_start..self.pos];
        match unit {
            "px" => Ok(Length::Px(value)),
            "%" => Ok(Length::Percent(value)),
            "vw" => Ok(Length::Vw(value)),
            "vh" => Ok(Length::Vh(value)),
            "vmin" => Ok(Length::Vmin(value)),
            "vmax" => Ok(Length::Vmax(value)),
            "em" => Ok(Length::Em(value)),
            "rem" => Ok(Length::Rem(value)),
            "" if value == 0.0 => Ok(Length::Px(0.0)),
            _ => Err(format!("unsupported length unit: {unit:?}")),
        }
    }
}

impl FromStr for Length {
    type Err = String;

    /// Parses a CSS length or an additive, optionally nested `calc(...)`.
    ///
    /// Whitespace is required around binary `+` and `-` operators in `calc()`.
    /// Unsupported units and nonzero unitless values are rejected.
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let source = raw.trim().to_ascii_lowercase();
        if source == "auto" {
            return Ok(Self::Auto);
        }
        LengthParser::new(&source).parse_root()
    }
}

impl fmt::Display for Length {
    /// Formats a length using CSS-style notation.
    ///
    /// Mixed-unit calculations are rendered as an additive `calc(...)` with
    /// signed terms, and can be parsed back using [`FromStr`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Px(v) => write!(f, "{v}px"),
            Self::Percent(v) => write!(f, "{v}%"),
            Self::Vw(v) => write!(f, "{v}vw"),
            Self::Vh(v) => write!(f, "{v}vh"),
            Self::Vmin(v) => write!(f, "{v}vmin"),
            Self::Vmax(v) => write!(f, "{v}vmax"),
            Self::Em(v) => write!(f, "{v}em"),
            Self::Rem(v) => write!(f, "{v}rem"),
            Self::Auto => f.write_str("auto"),
            Self::Calc(calc) => {
                f.write_str("calc(")?;
                let mut wrote_term = false;
                for (value, unit) in [
                    (calc.px, "px"),
                    (calc.percent, "%"),
                    (calc.vw, "vw"),
                    (calc.vh, "vh"),
                    (calc.vmin, "vmin"),
                    (calc.vmax, "vmax"),
                    (calc.em, "em"),
                    (calc.rem, "rem"),
                ] {
                    if value == 0.0 {
                        continue;
                    }
                    if value.is_sign_negative() {
                        if wrote_term {
                            write!(f, " - {}{unit}", -value)?;
                        } else {
                            write!(f, "-{}{unit}", -value)?;
                        }
                    } else if wrote_term {
                        write!(f, " + {value}{unit}")?;
                    } else {
                        write!(f, "{value}{unit}")?;
                    }
                    wrote_term = true;
                }
                if !wrote_term {
                    f.write_str("0px")?;
                }
                f.write_str(")")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for direct and calculated length values.

    use super::{Axis, Length, LengthCalc, LengthContext, LengthExt};

    /// Checks that `Length` and its calculations are trivially copyable.
    #[test]
    fn length_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Length>();
        assert_copy::<LengthCalc>();
        let a = Length::Vw(18.0);
        let b = a;
        assert_eq!(a, b);
    }

    /// Checks automatic simplification for matching units and cancellation.
    #[test]
    fn simple_arithmetic() {
        assert_eq!(Length::Vw(100.0) - Length::Vw(18.0), Length::Vw(82.0));
        assert_eq!(20.px() + 5.px(), Length::Px(25.0));
        assert_eq!(20.px() - 20.px() + 3.rem(), Length::Rem(3.0));
        assert_eq!(10.vw() - 10.vw(), Length::Vw(0.0));
    }

    /// Checks that mixed units stay relative until layout resolution.
    #[test]
    fn mixed_units_resolve() {
        let ctx = LengthContext::new((1200, 800), (400, 200), 16.0, 16.0);
        let value = 100.vw() - 3.rem() + 10.px();
        assert_eq!(value.resolve(Axis::Horizontal, ctx), Some(1162.0));
        assert_eq!(value.resolve(Axis::Vertical, ctx), Some(1162.0));
        assert_eq!((50.percent()).resolve(Axis::Horizontal, ctx), Some(200.0));
        assert_eq!((50.percent()).resolve(Axis::Vertical, ctx), Some(100.0));
    }

    /// Checks parsing, canonical CSS formatting, and nested calculations.
    #[test]
    fn parse_and_display() {
        let samples = [
            "auto",
            "0",
            "20px",
            "-4rem",
            "2.5em",
            ".5vw",
            "5e-2rem",
            "calc(100vw - 20px + 3rem)",
            "calc(calc(100vw - 20px) + 3rem)",
            "calc(40% - -2px)",
            "calc((10vw + 2rem) - 4px)",
        ];
        for sample in samples {
            let value = Length::parse(sample).unwrap_or_else(|e| panic!("{sample}: {e}"));
            let printed = value.to_string();
            let reparsed = Length::parse(&printed).unwrap_or_else(|e| panic!("{printed}: {e}"));
            assert_eq!(value, reparsed, "{sample} -> {printed}");
        }
    }

    /// Checks invalid CSS lengths are reported as errors.
    #[test]
    fn invalid_input() {
        for sample in ["3", "20wat", "calc(1vw-2px)", "calc(1vw +2px)", "calc(5rem +)", "calc(2px", "NaNpx"] {
            assert!(Length::parse(sample).is_err(), "unexpectedly parsed {sample}");
        }
    }

    /// Checks `Auto` remains unresolved through arithmetic.
    #[test]
    fn auto_stays_unresolved() {
        let ctx = LengthContext::new((100, 100), (100, 100), 16.0, 16.0);
        assert_eq!((Length::Auto + 5.px()).resolve(Axis::Horizontal, ctx), None);
        assert_eq!((5.px() - Length::Auto).resolve(Axis::Horizontal, ctx), None);
    }

    /// Checks linear interpolation across different units.
    #[test]
    fn mixed_interpolation() {
        let ctx = LengthContext::new((1000, 800), (500, 300), 16.0, 16.0);
        let halfway = 10.px().interpolate(10.vw(), 0.5);
        assert_eq!(halfway.resolve(Axis::Horizontal, ctx), Some(55.0));
    }

    /// Tests assignment operators, scalar overloads, and preserved `Copy`.
    #[test]
    fn all_arithmetic_assignments() {
        let mut length = Length::Vw(100.0);
        length -= Length::Vw(18.0);
        length += 2.0_f32; // pixels
        assert_eq!(length, Length::Vw(82.0) + Length::Px(2.0));
        length *= 2;
        length /= 4.0_f32;
        assert_eq!(length, Length::Vw(41.0) + Length::Px(1.0));
        length %= 3.0_f32;
        assert_eq!(length, Length::Vw(2.0) + Length::Px(1.0));
        assert_eq!(-Length::Rem(3.0), Length::Rem(-3.0));
        assert_eq!(2.0_f32 * Length::Vw(3.0), Length::Vw(6.0));
        assert_eq!(2 + Length::Px(3.0), Length::Px(5.0));
    }

    /// Tests the finite, IEEE-754, saturation, and overflow policies.
    #[test]
    fn checked_saturated_wrapping_overflowing() {
        let max = Length::Px(f32::MAX);
        assert_eq!(max.checked_mul(2.0), None);
        assert_eq!(max.saturating_mul(2.0), max);
        assert_eq!(max.overflowing_mul(2.0).1, true);
        assert_eq!(max.wrapping_mul(2.0), Length::Px(f32::INFINITY));
        assert_eq!(Length::Px(8.0).checked_div(0.0), None);
        assert_eq!(Length::Px(8.0).saturating_div(0.0), max);
        assert_eq!(Length::Px(8.0).overflowing_div(0.0).1, true);
        assert_eq!(Length::Px(0.0).saturating_rem(0.0), Length::Px(0.0));
        assert_eq!(Length::Auto.checked_add(Length::Px(1.0)), None);
        assert_eq!(Length::Auto.saturating_add(Length::Px(1.0)), Length::Auto);
        assert_eq!(Length::Px(3.0).checked_rem(2.0), Some(Length::Px(1.0)));
        assert_eq!(Length::Px(3.0).checked_neg(), Some(Length::Px(-3.0)));
    }

    /// Checks total ordering, NaN reflexivity, signed zero, and pixel comparisons.
    #[test]
    fn total_order_and_context_comparison() {
        use std::cmp::Ordering;
        let nan = Length::Px(f32::NAN);
        assert_eq!(nan, nan);
        assert_eq!(Length::Px(-0.0), Length::Px(0.0));
        assert!(Length::Px(3.0) > Length::Px(2.0));
        let ctx = LengthContext::new((1000, 800), (500, 300), 16.0, 16.0);
        assert_eq!(Length::Vw(10.0).cmp_resolved(Length::Px(100.0), Axis::Horizontal, ctx), Some(Ordering::Equal));
        assert_eq!(Length::Auto.cmp_resolved(Length::Px(1.0), Axis::Horizontal, ctx), None);
        assert_ne!(Length::Vw(10.0), Length::Px(100.0));
    }

    /// Checks that standard Rust range types work as `RangeBounds<Length>`.
    #[test]
    fn range_bounds_and_steps() {
        use std::ops::{Bound, RangeBounds};
        fn check_bounds<R: RangeBounds<Length>>(r: R) -> bool {
            r.contains(&Length::Px(5.0))
        }
        assert!(check_bounds(Length::Px(0.0)..Length::Px(10.0)));
        assert!(check_bounds(Length::Px(0.0)..=Length::Px(10.0)));
        assert!(!check_bounds(Length::Px(6.0)..Length::Px(10.0)));

        let values: Vec<_> = Length::range_steps(0.px()..=10.px(), 2.px())
            .unwrap().collect();
        assert_eq!(values, vec![0.px(), 2.px(), 4.px(), 6.px(), 8.px(), 10.px()]);
        let desc: Vec<_> = Length::range_steps(6.vw()..0.vw(), (-2).vw())
            .unwrap().collect();
        assert_eq!(desc, vec![6.vw(), 4.vw(), 2.vw()]);
        let excluded: Vec<_> = Length::range_steps(
            (Bound::Excluded(1.px()), Bound::Included(3.px())), 1.px(),
        ).unwrap().collect();
        assert_eq!(excluded, vec![2.px(), 3.px()]);
        assert!(Length::range_steps(0.px()..10.vw(), 1.px()).is_none());
        assert!(Length::range_steps(0.px()..10.px(), 0.px()).is_none());
    }

    /// Checks primitive-style sums, hashing, and numeric convenience methods.
    #[test]
    fn sum_hash_and_numeric_conveniences() {
        use std::collections::{BTreeSet, HashSet};
        let lengths = [10.px(), 5.px(), 10.vw()];
        let total: Length = lengths.iter().sum();
        assert_eq!(total, 15.px() + 10.vw());
        assert_eq!(Length::Px(-3.0).abs(), Length::Px(3.0));
        assert!(Length::Px(0.0).is_zero());
        assert!(Length::Px(f32::NAN).is_nan());
        assert!(Length::Px(f32::INFINITY).is_infinite());
        assert_eq!(Length::Px(12.0).checked_div(3_i32), Some(Length::Px(4.0)));
        assert_eq!(Length::Px(12.0).checked_add(3_i32), Some(Length::Px(15.0)));
        let mut hashes = HashSet::new();
        hashes.insert(Length::Px(-0.0));
        assert!(hashes.contains(&Length::Px(0.0)));
        let mut sorted = BTreeSet::new();
        sorted.insert(Length::Px(2.0));
        sorted.insert(Length::Px(1.0));
        assert_eq!(sorted.into_iter().collect::<Vec<_>>(), vec![1.px(), 2.px()]);
        assert_eq!(Length::Px(15.0).clamp(Length::Px(0.0), Length::Px(10.0)), Length::Px(10.0));
    }

    /// Checks coefficient operations as well as arithmetic methods.
    #[test]
    fn coefficient_ops() {
        let mut calc = Length::Vw(10.0).coefficients().unwrap();
        calc += Length::Px(5.0).coefficients().unwrap();
        calc *= 2.0;
        calc -= Length::Px(2.0).coefficients().unwrap();
        calc /= 2.0;
        calc %= 4.0;
        assert_eq!(calc.vw, 2.0);
        assert_eq!(calc.px, 0.0);
        assert!(calc.is_finite());
        assert_eq!(calc.checked_div(0.0), None);
        assert!(calc.overflowing_div(0.0).1);
    }
}
