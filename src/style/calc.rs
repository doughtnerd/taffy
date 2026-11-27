//! `calc()` expressions and a store that hands out [`CalcHandle`]s for them.
//!
//! Taffy's style types hold a `calc()` value as an opaque [`CalcHandle`] and ask the layout tree to resolve it
//! ([`LayoutPartialTree::resolve_calc_value`](crate::LayoutPartialTree::resolve_calc_value)). This module provides a
//! representation of the expressions themselves, [`Calc`], covering the CSS math functions that can produce a
//! `<length-percentage>` (CSS Values and Units Level 4 §10), and [`CalcStore`], which owns expressions and resolves their
//! handles. [`TaffyTree`](crate::TaffyTree) uses a `CalcStore` internally; custom trees can use one too.
//!
//! Taffy only knows pixels and percentages: other length units (`em`, `vw`, `rem`, …) must be converted to pixels before
//! an expression is built, as for any other length.
//!
//! ```
//! # use taffy::prelude::*;
//! # use taffy::{Calc, CalcStore};
//! let mut store = CalcStore::new();
//! // calc(50% - 12px)
//! let half_minus_gutter = Calc::Sum(vec![Calc::Percent(0.5), Calc::Length(-12.0)]);
//! let handle = store.insert(half_minus_gutter).unwrap();
//! let width = Dimension::calc(handle);
//! assert_eq!(store.resolve(handle, 400.0), 188.0);
//! # let _ = width;
//! ```

use crate::style::CalcHandle;
use crate::util::sys::{Box, Vec};

/// A `calc()` expression over pixels, percentages, numbers and angles.
///
/// An expression must resolve to a length (see [`Calc::validate`]); numbers and angles appear inside it, as in
/// `calc(100px * sin(30deg))` or `calc(10% / 1px * 1px)` (typed arithmetic). Percentages resolve against the basis
/// Taffy passes when it resolves the value: the containing block's width for `width`, margins and padding, its height
/// for `height`, and so on, exactly as for a plain percentage.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Calc {
    /// A length in pixels
    Length(f32),
    /// A percentage of the basis, as a fraction (`0.5` is `50%`)
    Percent(f32),
    /// A plain number (`2`, `pi`, `infinity`)
    Number(f32),
    /// An angle in radians (as an argument of a trigonometric function)
    Angle(f32),
    /// The sum of the terms (`a + b - c` is `Sum([a, b, Negate(c)])`)
    Sum(Vec<Calc>),
    /// The product of the factors (`a * b / c` is `Product([a, b, Invert(c)])`)
    Product(Vec<Calc>),
    /// The negation of a term
    Negate(Box<Calc>),
    /// The reciprocal of a factor
    Invert(Box<Calc>),
    /// A math function applied to its arguments
    Function(CalcFunction, Vec<Calc>),
}

/// A CSS math function that can appear in a [`Calc`] expression, with its arguments as [`Calc::Function`]'s list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CalcFunction {
    /// `min(a, b, …)`: one or more arguments of the same type
    Min,
    /// `max(a, b, …)`: one or more arguments of the same type
    Max,
    /// `clamp(min, value, max)`; an omitted bound (CSS's `none`) is [`CalcFunction::ClampAbove`] or
    /// [`CalcFunction::ClampBelow`]
    Clamp,
    /// `clamp(none, value, max)`: `value` and `max`
    ClampAbove,
    /// `clamp(min, value, none)`: `min` and `value`
    ClampBelow,
    /// `round(strategy, a, b)`: `a` rounded to a multiple of `b` (`b` may be omitted only when `a` is a number)
    Round(RoundingStrategy),
    /// `mod(a, b)`: the remainder with the sign of `b`
    Mod,
    /// `rem(a, b)`: the remainder with the sign of `a`
    Rem,
    /// `abs(a)`
    Abs,
    /// `sign(a)`: -1, 0 or 1, a number
    Sign,
    /// `hypot(a, b, …)`: the square root of the sum of squares
    #[cfg(feature = "std")]
    Hypot,
    /// `sin(a)` of a number (radians) or angle
    #[cfg(feature = "std")]
    Sin,
    /// `cos(a)` of a number (radians) or angle
    #[cfg(feature = "std")]
    Cos,
    /// `tan(a)` of a number (radians) or angle
    #[cfg(feature = "std")]
    Tan,
    /// `asin(a)` of a number: an angle
    #[cfg(feature = "std")]
    Asin,
    /// `acos(a)` of a number: an angle
    #[cfg(feature = "std")]
    Acos,
    /// `atan(a)` of a number: an angle
    #[cfg(feature = "std")]
    Atan,
    /// `atan2(y, x)` of two values of the same type: an angle
    #[cfg(feature = "std")]
    Atan2,
    /// `pow(base, exponent)` of numbers
    #[cfg(feature = "std")]
    Pow,
    /// `sqrt(a)` of a number
    #[cfg(feature = "std")]
    Sqrt,
    /// `log(a)` or `log(a, base)` of numbers
    #[cfg(feature = "std")]
    Log,
    /// `exp(a)` of a number
    #[cfg(feature = "std")]
    Exp,
}

/// How `round()` picks a multiple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RoundingStrategy {
    /// The nearer multiple; halfway values go towards positive infinity
    #[default]
    Nearest,
    /// The multiple towards positive infinity
    Up,
    /// The multiple towards negative infinity
    Down,
    /// The multiple towards zero
    ToZero,
}

/// A [`Calc`] expression that simplifies to a plain value, as CSS simplifies `calc()` when it parses it
/// ([`Calc::simplify`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SimplifiedCalc {
    /// A length in pixels (`calc(10px + 5px)`, `min(10px, 2em)` once `em` is converted)
    Length(f32),
    /// A percentage, as a fraction (`calc(50% * 2)`)
    Percent(f32),
}

/// A style value that can hold a `calc()` expression: built from a simplified value or from a handle.
/// [`TaffyTree::calc_value`](crate::TaffyTree::calc_value) and [`CalcStore::value`] return any of them.
pub trait FromCalc: Sized {
    /// The value for a simplified expression (a plain length or percentage)
    fn from_simplified(value: SimplifiedCalc) -> Self;
    /// The value for an expression stored behind a handle
    fn from_calc_handle(handle: CalcHandle) -> Self;
}

/// Implements [`FromCalc`] for types with `length`, `percent` and `calc` constructors.
macro_rules! impl_from_calc {
    ($($(#[$meta:meta])* $ty:ty),+ $(,)?) => {
        $(
            $(#[$meta])*
            impl FromCalc for $ty {
                fn from_simplified(value: SimplifiedCalc) -> Self {
                    match value {
                        SimplifiedCalc::Length(v) => <$ty>::length(v),
                        SimplifiedCalc::Percent(v) => <$ty>::percent(v),
                    }
                }
                fn from_calc_handle(handle: CalcHandle) -> Self {
                    <$ty>::calc(handle)
                }
            }
        )+
    };
}

impl_from_calc!(
    crate::style::LengthPercentage,
    crate::style::LengthPercentageAuto,
    crate::style::Dimension,
    #[cfg(feature = "grid")]
    crate::style::MinTrackSizingFunction,
    #[cfg(feature = "grid")]
    crate::style::MaxTrackSizingFunction,
);

/// Why a [`Calc`] expression is invalid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalcError {
    /// The expression doesn't resolve to a length (it resolves to a number, an angle, or a product such as `px * px`)
    NotALength,
    /// Values of different types are added, compared or passed to a function together (`10px + 5`)
    TypeMismatch,
    /// A function has the wrong number of arguments, or a sum or product has none
    WrongArgumentCount,
    /// A function's argument has a type it doesn't accept (`sqrt(4px)`)
    InvalidArgument,
}

impl core::fmt::Display for CalcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            CalcError::NotALength => "the calc() expression doesn't resolve to a length",
            CalcError::TypeMismatch => "the calc() expression combines values of different types",
            CalcError::WrongArgumentCount => "a calc() function has the wrong number of arguments",
            CalcError::InvalidArgument => "a calc() function argument has a type it doesn't accept",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CalcError {}

/// The type of a value in an expression: the powers of length and angle in it (`px` is length¹, `px / px` a number).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CalcType {
    /// The power of length (percentages count as lengths)
    length: i8,
    /// The power of angle
    angle: i8,
}

impl CalcType {
    /// A plain number
    const NUMBER: Self = Self { length: 0, angle: 0 };
    /// A length (or percentage)
    const LENGTH: Self = Self { length: 1, angle: 0 };
    /// An angle
    const ANGLE: Self = Self { length: 0, angle: 1 };

    /// True for a plain number
    fn is_number(self) -> bool {
        self == Self::NUMBER
    }
}

impl Calc {
    /// Checks that the expression is well-typed and resolves to a length (`<length-percentage>`), as CSS requires of a
    /// `calc()` in a length property. [`CalcStore::insert`] runs this check.
    pub fn validate(&self) -> Result<(), CalcError> {
        match self.type_of()? {
            CalcType::LENGTH => Ok(()),
            _ => Err(CalcError::NotALength),
        }
    }

    /// The plain length or percentage the expression simplifies to, if any: an expression without percentages is a
    /// length, and a linear one whose only term is a percentage is that percentage. CSS simplifies `calc()` this way
    /// when it parses it, and it matters to layout: Taffy treats every `calc()` value as percentage-dependent, so one
    /// resolves like a percentage of an indefinite size would (as `auto`, for a height in an auto-height parent) where a
    /// plain length wouldn't. [`TaffyTree::calc_value`](crate::TaffyTree::calc_value) applies this.
    pub fn simplify(&self) -> Option<SimplifiedCalc> {
        if self.validate().is_err() {
            return None;
        }
        if !self.uses_percentage() {
            return Some(SimplifiedCalc::Length(self.resolve(0.0)));
        }
        match self.linear_parts()? {
            (px, fraction) if px == 0.0 && fraction.is_finite() => Some(SimplifiedCalc::Percent(fraction as f32)),
            _ => None,
        }
    }

    /// `(px, fraction)` for an expression that is `px + fraction * basis` by its structure (sums, negations, products
    /// with at most one non-number factor), else `None`.
    fn linear_parts(&self) -> Option<(f64, f64)> {
        Some(match self {
            Calc::Length(v) => (f64::from(*v), 0.0),
            Calc::Percent(p) => (0.0, f64::from(*p)),
            Calc::Number(_) | Calc::Angle(_) | Calc::Function(..) | Calc::Invert(_) => return None,
            Calc::Negate(c) => {
                let (px, f) = c.linear_parts()?;
                (-px, -f)
            }
            Calc::Sum(items) => items.iter().try_fold((0.0, 0.0), |(px, f), c| {
                let (p2, f2) = c.linear_parts()?;
                Some((px + p2, f + f2))
            })?,
            Calc::Product(items) => {
                let mut scale = 1.0;
                let mut term = None;
                for item in items {
                    match item {
                        Calc::Number(n) => scale *= f64::from(*n),
                        Calc::Invert(inner) if matches!(**inner, Calc::Number(_)) => {
                            if let Calc::Number(n) = **inner {
                                scale /= f64::from(n);
                            }
                        }
                        other if term.is_none() => term = Some(other.linear_parts()?),
                        _ => return None,
                    }
                }
                let (px, f) = term?;
                (px * scale, f * scale)
            }
        })
    }

    /// True when the expression contains a percentage, so its value depends on the basis it's resolved against.
    pub fn uses_percentage(&self) -> bool {
        match self {
            Calc::Percent(_) => true,
            Calc::Length(_) | Calc::Number(_) | Calc::Angle(_) => false,
            Calc::Negate(c) | Calc::Invert(c) => c.uses_percentage(),
            Calc::Sum(items) | Calc::Product(items) | Calc::Function(_, items) => {
                items.iter().any(Calc::uses_percentage)
            }
        }
    }

    /// The expression's type, or why it has none
    fn type_of(&self) -> Result<CalcType, CalcError> {
        fn same(items: &[Calc]) -> Result<CalcType, CalcError> {
            let mut iter = items.iter();
            let first = iter.next().ok_or(CalcError::WrongArgumentCount)?.type_of()?;
            for item in iter {
                if item.type_of()? != first {
                    return Err(CalcError::TypeMismatch);
                }
            }
            Ok(first)
        }
        fn count(items: &[Calc], allowed: core::ops::RangeInclusive<usize>) -> Result<(), CalcError> {
            if allowed.contains(&items.len()) { Ok(()) } else { Err(CalcError::WrongArgumentCount) }
        }
        #[cfg(feature = "std")]
        fn number_args(items: &[Calc]) -> Result<CalcType, CalcError> {
            for item in items {
                if !item.type_of()?.is_number() {
                    return Err(CalcError::InvalidArgument);
                }
            }
            Ok(CalcType::NUMBER)
        }
        Ok(match self {
            Calc::Length(_) | Calc::Percent(_) => CalcType::LENGTH,
            Calc::Number(_) => CalcType::NUMBER,
            Calc::Angle(_) => CalcType::ANGLE,
            Calc::Sum(items) => same(items)?,
            Calc::Product(items) => {
                if items.is_empty() {
                    return Err(CalcError::WrongArgumentCount);
                }
                let mut t = CalcType::NUMBER;
                for item in items {
                    let it = item.type_of()?;
                    t = CalcType {
                        length: t.length.saturating_add(it.length),
                        angle: t.angle.saturating_add(it.angle),
                    };
                }
                t
            }
            Calc::Negate(c) => c.type_of()?,
            Calc::Invert(c) => {
                let t = c.type_of()?;
                CalcType { length: -t.length, angle: -t.angle }
            }
            Calc::Function(f, items) => match f {
                CalcFunction::Min | CalcFunction::Max => same(items)?,
                CalcFunction::Clamp => {
                    count(items, 3..=3)?;
                    same(items)?
                }
                CalcFunction::ClampAbove | CalcFunction::ClampBelow | CalcFunction::Mod | CalcFunction::Rem => {
                    count(items, 2..=2)?;
                    same(items)?
                }
                CalcFunction::Round(_) => {
                    count(items, 1..=2)?;
                    let t = same(items)?;
                    // Without a step only a number can be rounded (to an integer), as in CSS.
                    if items.len() == 1 && !t.is_number() {
                        return Err(CalcError::WrongArgumentCount);
                    }
                    t
                }
                CalcFunction::Abs => {
                    count(items, 1..=1)?;
                    same(items)?
                }
                CalcFunction::Sign => {
                    count(items, 1..=1)?;
                    same(items)?;
                    CalcType::NUMBER
                }
                #[cfg(feature = "std")]
                CalcFunction::Hypot => same(items)?,
                #[cfg(feature = "std")]
                CalcFunction::Sin | CalcFunction::Cos | CalcFunction::Tan => {
                    count(items, 1..=1)?;
                    match same(items)? {
                        CalcType::NUMBER | CalcType::ANGLE => CalcType::NUMBER,
                        _ => return Err(CalcError::InvalidArgument),
                    }
                }
                #[cfg(feature = "std")]
                CalcFunction::Asin | CalcFunction::Acos | CalcFunction::Atan => {
                    count(items, 1..=1)?;
                    number_args(items)?;
                    CalcType::ANGLE
                }
                #[cfg(feature = "std")]
                CalcFunction::Atan2 => {
                    count(items, 2..=2)?;
                    same(items)?;
                    CalcType::ANGLE
                }
                #[cfg(feature = "std")]
                CalcFunction::Pow => {
                    count(items, 2..=2)?;
                    number_args(items)?
                }
                #[cfg(feature = "std")]
                CalcFunction::Sqrt | CalcFunction::Exp => {
                    count(items, 1..=1)?;
                    number_args(items)?
                }
                #[cfg(feature = "std")]
                CalcFunction::Log => {
                    count(items, 1..=2)?;
                    number_args(items)?
                }
            },
        })
    }

    /// The expression's value with its percentages resolved against `basis`, as CSS defines it for a top-level
    /// calculation: NaN becomes 0 and an infinite value the largest finite one with its sign. The expression should
    /// have passed [`Calc::validate`]; an ill-typed one still produces a number, just not a meaningful one.
    pub fn resolve(&self, basis: f32) -> f32 {
        let v = self.eval(f64::from(basis));
        if v.is_nan() {
            0.0
        } else if v >= f64::from(f32::MAX) {
            f32::MAX
        } else if v <= f64::from(f32::MIN) {
            f32::MIN
        } else {
            v as f32
        }
    }

    /// The value in double precision (as browsers compute calc()), NaN and infinities kept.
    fn eval(&self, basis: f64) -> f64 {
        match self {
            Calc::Length(v) | Calc::Number(v) | Calc::Angle(v) => f64::from(*v),
            Calc::Percent(p) => f64::from(*p) * basis,
            Calc::Sum(items) => items.iter().map(|c| c.eval(basis)).sum(),
            Calc::Product(items) => items.iter().map(|c| c.eval(basis)).product(),
            Calc::Negate(c) => -c.eval(basis),
            Calc::Invert(c) => 1.0 / c.eval(basis),
            Calc::Function(f, items) => eval_function(*f, items, basis),
        }
    }
}

/// `x` rounded towards zero, without `std` (`%` is `fmod` in core).
fn trunc(x: f64) -> f64 {
    if x.is_finite() { x - x % 1.0 } else { x }
}

/// `x` rounded down, without `std`
fn floor(x: f64) -> f64 {
    let t = trunc(x);
    if t > x { t - 1.0 } else { t }
}

/// `x` rounded up, without `std`
fn ceil(x: f64) -> f64 {
    let t = trunc(x);
    if t < x { t + 1.0 } else { t }
}

/// CSS's `min()` / `max()`: NaN wins (it makes the whole calculation 0), unlike `f64::min`.
fn pick(items: &[Calc], basis: f64, better: fn(f64, f64) -> bool) -> f64 {
    let mut iter = items.iter().map(|c| c.eval(basis));
    let mut best = iter.next().unwrap_or(f64::NAN);
    for v in iter {
        if v.is_nan() || best.is_nan() {
            return f64::NAN;
        }
        if better(v, best) {
            best = v;
        }
    }
    best
}

/// The `i`th argument's value (NaN when it's missing)
fn arg(items: &[Calc], i: usize, basis: f64) -> f64 {
    items.get(i).map_or(f64::NAN, |c| c.eval(basis))
}

/// A math function's value
fn eval_function(f: CalcFunction, items: &[Calc], basis: f64) -> f64 {
    let a = |i| arg(items, i, basis);
    match f {
        CalcFunction::Min => pick(items, basis, |v, best| v < best),
        CalcFunction::Max => pick(items, basis, |v, best| v > best),
        // clamp(MIN, VAL, MAX) is max(MIN, min(VAL, MAX)): MIN wins when it's above MAX.
        CalcFunction::Clamp => clamp(Some(a(0)), a(1), Some(a(2))),
        CalcFunction::ClampAbove => clamp(None, a(0), Some(a(1))),
        CalcFunction::ClampBelow => clamp(Some(a(0)), a(1), None),
        CalcFunction::Round(strategy) => round(strategy, a(0), if items.len() > 1 { a(1) } else { 1.0 }),
        CalcFunction::Mod => modulo(a(0), a(1), true),
        CalcFunction::Rem => modulo(a(0), a(1), false),
        CalcFunction::Abs => {
            let v = a(0);
            if v < 0.0 { -v } else { v }
        }
        CalcFunction::Sign => {
            let v = a(0);
            if v > 0.0 {
                1.0
            } else if v < 0.0 {
                -1.0
            } else {
                v // 0, -0 or NaN
            }
        }
        #[cfg(feature = "std")]
        CalcFunction::Hypot => items.iter().map(|c| c.eval(basis).powi(2)).sum::<f64>().sqrt(),
        #[cfg(feature = "std")]
        CalcFunction::Sin => a(0).sin(),
        #[cfg(feature = "std")]
        CalcFunction::Cos => a(0).cos(),
        #[cfg(feature = "std")]
        CalcFunction::Tan => a(0).tan(),
        #[cfg(feature = "std")]
        CalcFunction::Asin => a(0).asin(),
        #[cfg(feature = "std")]
        CalcFunction::Acos => a(0).acos(),
        #[cfg(feature = "std")]
        CalcFunction::Atan => a(0).atan(),
        #[cfg(feature = "std")]
        CalcFunction::Atan2 => a(0).atan2(a(1)),
        #[cfg(feature = "std")]
        CalcFunction::Pow => a(0).powf(a(1)),
        #[cfg(feature = "std")]
        CalcFunction::Sqrt => a(0).sqrt(),
        #[cfg(feature = "std")]
        CalcFunction::Log => {
            if items.len() > 1 {
                a(0).ln() / a(1).ln()
            } else {
                a(0).ln()
            }
        }
        #[cfg(feature = "std")]
        CalcFunction::Exp => a(0).exp(),
    }
}

/// `clamp()`, with optional bounds; NaN in any argument is NaN
fn clamp(min: Option<f64>, value: f64, max: Option<f64>) -> f64 {
    if min.is_some_and(f64::is_nan) || value.is_nan() || max.is_some_and(f64::is_nan) {
        return f64::NAN;
    }
    let mut v = value;
    if let Some(max) = max {
        if v > max {
            v = max;
        }
    }
    if let Some(min) = min {
        if v < min {
            v = min;
        }
    }
    v
}

/// Zero with `a`'s sign (`f64::copysign` isn't in `core` at Taffy's MSRV)
fn signed_zero(a: f64) -> f64 {
    if a.is_sign_negative() { -0.0 } else { 0.0 }
}

/// `round(strategy, a, b)` (CSS Values 4 §10.4.1).
fn round(strategy: RoundingStrategy, a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() || b == 0.0 || (a.is_infinite() && b.is_infinite()) {
        return f64::NAN;
    }
    if a.is_infinite() {
        return a;
    }
    if b.is_infinite() {
        // The multiples are 0 and ±infinity.
        return match strategy {
            RoundingStrategy::Nearest | RoundingStrategy::ToZero => signed_zero(a),
            RoundingStrategy::Up => {
                if a > 0.0 {
                    f64::INFINITY
                } else {
                    signed_zero(a)
                }
            }
            RoundingStrategy::Down => {
                if a < 0.0 {
                    f64::NEG_INFINITY
                } else {
                    signed_zero(a)
                }
            }
        };
    }
    let step = if b < 0.0 { -b } else { b };
    let q = a / step;
    let lower = floor(q) * step;
    let upper = ceil(q) * step;
    match strategy {
        RoundingStrategy::Nearest => {
            if a - lower < upper - a {
                lower
            } else {
                upper
            }
        }
        RoundingStrategy::Up => upper,
        RoundingStrategy::Down => lower,
        RoundingStrategy::ToZero => {
            if a < 0.0 {
                upper
            } else {
                lower
            }
        }
    }
}

/// `mod()` (`is_mod`: the result takes `b`'s sign, rounding the quotient down) and `rem()` (`a`'s sign, rounding the
/// quotient towards zero).
fn modulo(a: f64, b: f64, is_mod: bool) -> f64 {
    if a.is_nan() || b.is_nan() || b == 0.0 || a.is_infinite() {
        return f64::NAN;
    }
    if b.is_infinite() {
        // An infinite divisor leaves `a`, unless mod() would have to cross zero to match `b`'s sign.
        let same_sign = (a >= 0.0) == (b > 0.0);
        return if same_sign || !is_mod { a } else { f64::NAN };
    }
    a - b * if is_mod { floor(a / b) } else { trunc(a / b) }
}

/// The `calc()` expressions a tree's styles refer to, by [`CalcHandle`].
///
/// [`insert`](CalcStore::insert) stores an expression and returns the handle a style holds
/// (`Dimension::calc(handle)`, `LengthPercentage::calc(handle)`, …); [`resolve`](CalcStore::resolve) is what a tree's
/// [`resolve_calc_value`](crate::LayoutPartialTree::resolve_calc_value) calls. A handle names its slot and the slot's
/// generation, so a handle kept after [`remove`](CalcStore::remove) resolves to 0 rather than to a later expression.
#[derive(Clone, Debug, Default)]
pub struct CalcStore {
    /// The slots, by index
    entries: Vec<CalcEntry>,
    /// The indices of empty slots
    free: Vec<u32>,
    /// How many slots hold an expression
    len: usize,
}

#[derive(Clone, Debug)]
/// One slot of a [`CalcStore`]
struct CalcEntry {
    /// Bumped each time the slot is emptied, so old handles stop matching it
    generation: u32,
    /// The expression, if the slot holds one
    calc: Option<Calc>,
}

/// The bits a handle's generation keeps (a handle has 61 usable bits: 32 for the slot, 28 for its generation).
const GENERATION_MASK: u32 = (1 << 28) - 1;

impl CalcStore {
    /// An empty store.
    pub const fn new() -> Self {
        Self { entries: Vec::new(), free: Vec::new(), len: 0 }
    }

    /// Stores a validated expression and returns its handle.
    pub fn insert(&mut self, calc: Calc) -> Result<CalcHandle, CalcError> {
        calc.validate()?;
        Ok(self.insert_unchecked(calc))
    }

    /// Stores an expression without [`Calc::validate`] (one already known to resolve to a length).
    pub fn insert_unchecked(&mut self, calc: Calc) -> CalcHandle {
        self.len += 1;
        let index = match self.free.pop() {
            Some(index) => {
                let entry = &mut self.entries[index as usize];
                entry.calc = Some(calc);
                index
            }
            None => {
                self.entries.push(CalcEntry { generation: 1, calc: Some(calc) });
                (self.entries.len() - 1) as u32
            }
        };
        Self::handle(index, self.entries[index as usize].generation)
    }

    /// A style value for an expression: a plain length or percentage when it simplifies to one ([`Calc::simplify`],
    /// as CSS does when it parses `calc()`), else a handle to the expression, stored here. Errors when the expression
    /// doesn't resolve to a length.
    pub fn value<T: FromCalc>(&mut self, calc: Calc) -> Result<T, CalcError> {
        calc.validate()?;
        Ok(match calc.simplify() {
            Some(simple) => T::from_simplified(simple),
            None => T::from_calc_handle(self.insert_unchecked(calc)),
        })
    }

    /// Parses a CSS value (`"calc(50% - 12px)"`, `"min(25%, 60px)"`, `"minmax(calc(10% + 5px), 1fr)"`, or any value
    /// the type's `FromStr` accepts), storing any `calc()` that doesn't simplify to a plain value here.
    #[cfg(feature = "parse")]
    pub fn parse_value<T: crate::util::parse::ParseWithCalc>(
        &mut self,
        input: &str,
    ) -> crate::util::parse::ParseResult<T> {
        T::parse_with_calc(input, self)
    }

    /// Removes an expression, returning it; `None` for a handle that isn't (or is no longer) in the store.
    ///
    /// Remove an expression only when no style refers to it any more: a style still holding the handle would resolve
    /// it to 0.
    pub fn remove(&mut self, handle: CalcHandle) -> Option<Calc> {
        let index = self.index_of(handle)?;
        let entry = &mut self.entries[index];
        let calc = entry.calc.take()?;
        entry.generation = (entry.generation % GENERATION_MASK) + 1;
        self.free.push(index as u32);
        self.len -= 1;
        Some(calc)
    }

    /// The expression a handle names, if it's in the store.
    pub fn get(&self, handle: CalcHandle) -> Option<&Calc> {
        self.entries.get(self.index_of(handle)?)?.calc.as_ref()
    }

    /// The handle's value against `basis` ([`Calc::resolve`]); 0 for a handle that isn't in the store.
    pub fn resolve(&self, handle: CalcHandle, basis: f32) -> f32 {
        self.get(handle).map_or(0.0, |calc| calc.resolve(basis))
    }

    /// How many expressions the store holds.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when the store holds no expressions.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes every expression (handles from before resolve to 0 afterwards).
    pub fn clear(&mut self) {
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if entry.calc.take().is_some() {
                entry.generation = (entry.generation % GENERATION_MASK) + 1;
                self.free.push(index as u32);
            }
        }
        self.len = 0;
    }

    /// The handle for a slot and generation
    fn handle(index: u32, generation: u32) -> CalcHandle {
        let key = (u64::from(generation & GENERATION_MASK) << 32) | u64::from(index);
        CalcHandle::from_bits(key << 3)
    }

    /// The slot a handle names, if its generation still matches
    fn index_of(&self, handle: CalcHandle) -> Option<usize> {
        let key = handle.to_bits() >> 3;
        let index = (key & u64::from(u32::MAX)) as usize;
        let generation = (key >> 32) as u32;
        (self.entries.get(index)?.generation == generation).then_some(index)
    }
}

/// Parsing `calc()` and the other CSS math functions (`parse` feature).
#[cfg(feature = "parse")]
pub(crate) mod parse {
    use super::{Calc, CalcFunction, RoundingStrategy, SimplifiedCalc};
    use crate::style::CalcHandle;
    use crate::util::parse::{CssParseError, CssParseResult, ParseContext, ParseError, Parser, Token};
    use crate::util::sys::{Box, Vec};
    use std::borrow::Cow;

    /// The math functions this parser accepts (CSS Values and Units Level 4 §10), by lowercase name.
    const FUNCTIONS: &[&str] = &[
        "calc", "min", "max", "clamp", "round", "mod", "rem", "abs", "sign", "hypot", "sin", "cos", "tan", "asin",
        "acos", "atan", "atan2", "pow", "sqrt", "log", "exp",
    ];

    /// True when `name` (as cssparser gives a function token's name) is a CSS math function.
    pub(crate) fn is_math_function(name: &str) -> bool {
        FUNCTIONS.iter().any(|f| f.eq_ignore_ascii_case(name))
    }

    /// A parsed math function: simplified to a plain value, or stored behind a handle.
    pub(crate) enum ParsedCalc {
        /// A plain length or percentage
        Simple(SimplifiedCalc),
        /// A handle in the parse context's store
        Handle(CalcHandle),
    }

    impl ParsedCalc {
        /// The style value
        pub(crate) fn into_value<T: super::FromCalc>(self) -> T {
            match self {
                ParsedCalc::Simple(simple) => T::from_simplified(simple),
                ParsedCalc::Handle(handle) => T::from_calc_handle(handle),
            }
        }
    }

    /// An error with a message
    fn error<'i>(_parser: &Parser<'i>, message: &'static str) -> CssParseError<'i> {
        CssParseError::custom(Cow::Borrowed(message))
    }

    /// Parses the math function `name` (its function token just read) to a value for a length property: simplified
    /// when it can be, else stored in the context's store; an error without one.
    pub(crate) fn parse_calc_value<'i>(
        name: &str,
        parser: &mut Parser<'i>,
        cx: &mut ParseContext<'_>,
    ) -> CssParseResult<'i, ParsedCalc> {
        let calc = parser.parse_nested_block(|parser| function(name, parser))?;
        if calc.validate().is_err() {
            return Err(error(parser, "the calc() expression doesn't resolve to a length"));
        }
        match calc.simplify() {
            Some(simple) => Ok(ParsedCalc::Simple(simple)),
            None => match cx.calcs.as_deref_mut() {
                Some(store) => Ok(ParsedCalc::Handle(store.insert_unchecked(calc))),
                None => Err(error(
                    parser,
                    "this calc() keeps a percentage, so it needs a CalcStore: parse with TaffyTree::parse_value or CalcStore::parse_value",
                )),
            },
        }
    }

    /// Parses a whole math function (`calc(50% - 12px)`) to a [`Calc`] expression, without validating it.
    pub(crate) fn parse_calc<'i>(parser: &mut Parser<'i>) -> CssParseResult<'i, Calc> {
        match parser.next()?.clone() {
            Token::Function(name) if is_math_function(&name) => {
                parser.parse_nested_block(|parser| function(&name, parser))
            }
            _ => Err(CssParseError::unexpected_token()),
        }
    }

    /// `+` / `-` separated products
    fn sum<'i>(parser: &mut Parser<'i>) -> CssParseResult<'i, Calc> {
        let mut terms = vec![product(parser)?];
        loop {
            let state = parser.state();
            match parser.next().cloned() {
                Ok(Token::Delim('+')) => terms.push(product(parser)?),
                Ok(Token::Delim('-')) => terms.push(Calc::Negate(Box::new(product(parser)?))),
                _ => {
                    parser.reset(&state);
                    break;
                }
            }
        }
        Ok(if terms.len() == 1 { terms.pop().unwrap() } else { Calc::Sum(terms) })
    }

    /// `*` / `/` separated values
    fn product<'i>(parser: &mut Parser<'i>) -> CssParseResult<'i, Calc> {
        let mut factors = vec![value(parser)?];
        loop {
            let state = parser.state();
            match parser.next().cloned() {
                Ok(Token::Delim('*')) => factors.push(value(parser)?),
                Ok(Token::Delim('/')) => factors.push(Calc::Invert(Box::new(value(parser)?))),
                _ => {
                    parser.reset(&state);
                    break;
                }
            }
        }
        Ok(if factors.len() == 1 { factors.pop().unwrap() } else { Calc::Product(factors) })
    }

    /// A number, dimension, percentage, constant, parenthesised sum or nested math function
    fn value<'i>(parser: &mut Parser<'i>) -> CssParseResult<'i, Calc> {
        use core::f32::consts::PI;
        match parser.next()?.clone() {
            Token::Number { value, .. } => Ok(Calc::Number(value)),
            Token::Percentage { unit_value, .. } => Ok(Calc::Percent(unit_value)),
            Token::Dimension { value, ref unit, .. } => {
                let unit: &str = unit;
                if unit.eq_ignore_ascii_case("px") {
                    Ok(Calc::Length(value))
                } else if unit.eq_ignore_ascii_case("deg") {
                    Ok(Calc::Angle(value * PI / 180.0))
                } else if unit.eq_ignore_ascii_case("rad") {
                    Ok(Calc::Angle(value))
                } else if unit.eq_ignore_ascii_case("grad") {
                    Ok(Calc::Angle(value * PI / 200.0))
                } else if unit.eq_ignore_ascii_case("turn") {
                    Ok(Calc::Angle(value * 2.0 * PI))
                } else {
                    Err(error(parser, "calc() supports px, %, numbers and angles: convert other units to px first"))
                }
            }
            Token::Ident(ref ident) => {
                let ident: &str = ident;
                let constant = |name: &str| ident.eq_ignore_ascii_case(name);
                if constant("e") {
                    Ok(Calc::Number(core::f32::consts::E))
                } else if constant("pi") {
                    Ok(Calc::Number(PI))
                } else if constant("infinity") {
                    Ok(Calc::Number(f32::INFINITY))
                } else if constant("-infinity") {
                    Ok(Calc::Number(f32::NEG_INFINITY))
                } else if constant("nan") {
                    Ok(Calc::Number(f32::NAN))
                } else {
                    Err(CssParseError::unexpected_token())
                }
            }
            Token::ParenthesisBlock => parser.parse_nested_block(sum),
            Token::Function(ref name) if is_math_function(name) => {
                let name = name.clone();
                parser.parse_nested_block(|parser| function(&name, parser))
            }
            _ => Err(CssParseError::unexpected_token()),
        }
    }

    /// Comma-separated sums, at least one
    fn list<'i>(parser: &mut Parser<'i>) -> CssParseResult<'i, Vec<Calc>> {
        let mut items = vec![sum(parser)?];
        while parser.try_parse(|parser| parser.expect_comma()).is_ok() {
            items.push(sum(parser)?);
        }
        Ok(items)
    }

    /// Exactly `count` comma-separated sums
    fn args<'i>(parser: &mut Parser<'i>, count: usize) -> CssParseResult<'i, Vec<Calc>> {
        let items = list(parser)?;
        if items.len() == count {
            Ok(items)
        } else {
            Err(error(parser, "a calc() function has the wrong number of arguments"))
        }
    }

    /// `clamp()`'s bound: a sum, or `none`
    fn bound<'i>(parser: &mut Parser<'i>) -> CssParseResult<'i, Option<Calc>> {
        if parser.try_parse(|parser| parser.expect_ident_matching("none")).is_ok() {
            Ok(None)
        } else {
            sum(parser).map(Some)
        }
    }

    /// The body of math function `name`
    fn function<'i>(name: &str, parser: &mut Parser<'i>) -> CssParseResult<'i, Calc> {
        let is = |f: &str| name.eq_ignore_ascii_case(f);
        let call = |f: CalcFunction, items: Vec<Calc>| Ok(Calc::Function(f, items));
        if is("calc") {
            return sum(parser);
        }
        if is("min") {
            return call(CalcFunction::Min, list(parser)?);
        }
        if is("max") {
            return call(CalcFunction::Max, list(parser)?);
        }
        if is("clamp") {
            let min = bound(parser)?;
            parser.expect_comma()?;
            let value = sum(parser)?;
            parser.expect_comma()?;
            let max = bound(parser)?;
            return match (min, max) {
                (Some(min), Some(max)) => call(CalcFunction::Clamp, vec![min, value, max]),
                (None, Some(max)) => call(CalcFunction::ClampAbove, vec![value, max]),
                (Some(min), None) => call(CalcFunction::ClampBelow, vec![min, value]),
                (None, None) => Ok(value),
            };
        }
        if is("round") {
            let strategy = parser
                .try_parse(|parser| -> CssParseResult<'i, RoundingStrategy> {
                    let ident = parser.expect_ident()?.clone();
                    let strategy = match () {
                        _ if ident.eq_ignore_ascii_case("nearest") => RoundingStrategy::Nearest,
                        _ if ident.eq_ignore_ascii_case("up") => RoundingStrategy::Up,
                        _ if ident.eq_ignore_ascii_case("down") => RoundingStrategy::Down,
                        _ if ident.eq_ignore_ascii_case("to-zero") => RoundingStrategy::ToZero,
                        _ => return Err(CssParseError::unexpected_token()),
                    };
                    parser.expect_comma()?;
                    Ok(strategy)
                })
                .unwrap_or_default();
            let items = list(parser)?;
            if items.len() > 2 {
                return Err(error(parser, "round() takes a value and an optional step"));
            }
            return call(CalcFunction::Round(strategy), items);
        }
        if is("mod") {
            return call(CalcFunction::Mod, args(parser, 2)?);
        }
        if is("rem") {
            return call(CalcFunction::Rem, args(parser, 2)?);
        }
        if is("abs") {
            return call(CalcFunction::Abs, args(parser, 1)?);
        }
        if is("sign") {
            return call(CalcFunction::Sign, args(parser, 1)?);
        }
        #[cfg(feature = "std")]
        {
            let one = |f: CalcFunction, parser: &mut Parser<'i>| -> CssParseResult<'i, Calc> {
                Ok(Calc::Function(f, args(parser, 1)?))
            };
            match () {
                _ if is("hypot") => return call(CalcFunction::Hypot, list(parser)?),
                _ if is("sin") => return one(CalcFunction::Sin, parser),
                _ if is("cos") => return one(CalcFunction::Cos, parser),
                _ if is("tan") => return one(CalcFunction::Tan, parser),
                _ if is("asin") => return one(CalcFunction::Asin, parser),
                _ if is("acos") => return one(CalcFunction::Acos, parser),
                _ if is("atan") => return one(CalcFunction::Atan, parser),
                _ if is("atan2") => return call(CalcFunction::Atan2, args(parser, 2)?),
                _ if is("pow") => return call(CalcFunction::Pow, args(parser, 2)?),
                _ if is("sqrt") => return one(CalcFunction::Sqrt, parser),
                _ if is("exp") => return one(CalcFunction::Exp, parser),
                _ if is("log") => {
                    let items = list(parser)?;
                    if items.len() > 2 {
                        return Err(error(parser, "log() takes a value and an optional base"));
                    }
                    return call(CalcFunction::Log, items);
                }
                _ => {}
            }
        }
        Err(error(parser, "this math function needs Taffy's std feature"))
    }

    /// `Calc` from a math function string, e.g. `"calc(50% - 12px)"` or `"min(25%, 60px)"` (not validated: see
    /// [`Calc::validate`]).
    impl core::str::FromStr for Calc {
        type Err = ParseError;
        fn from_str(input: &str) -> Result<Self, Self::Err> {
            let mut parser = Parser::new(input);
            parser.parse_entirely(parse_calc).map_err(ParseError::from)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::*;
        use core::str::FromStr;

        fn parse(input: &str) -> Calc {
            Calc::from_str(input).unwrap_or_else(|err| panic!("{input}: {err}"))
        }

        #[test]
        fn parses_the_math_functions_chrome_lays_out() {
            // The values Chrome 158 gives for `width: <expression>` in a 400px containing block (before negative
            // widths clamp to 0).
            let cases: &[(&str, f32)] = &[
                ("calc(50% - 12px)", 188.0),
                ("calc((50% + 10px) * 2)", 420.0),
                ("calc(100px * (10px / 5px))", 200.0),
                ("calc(10% / 1px * 1px)", 40.0),
                ("calc(10px - -5px)", 15.0),
                ("CALC(2 * 10PX)", 20.0),
                ("round(17px, 5px)", 15.0),
                ("round(up, 17px, 5px)", 20.0),
                ("round(down, -17px, 5px)", -20.0),
                ("round(to-zero, -17px, 5px)", -15.0),
                ("round(nearest, -15px, 10px)", -10.0),
                ("mod(-17px, 5px)", 3.0),
                ("rem(-17px, 5px)", -2.0),
                ("abs(-25%)", 100.0),
                ("clamp(10px, 50%, 150px)", 150.0),
                ("clamp(10px, 5%, 150px)", 20.0),
                ("clamp(none, 50%, 150px)", 150.0),
                ("clamp(30px, 5%, none)", 30.0),
                ("max(10px)", 10.0),
                ("min(50%, 150px)", 150.0),
                ("calc(sign(-5%) * 10px)", -10.0),
                ("calc(infinity * 1px)", f32::MAX),
                ("calc(NaN * 1px)", 0.0),
                ("calc(1px / 0)", f32::MAX),
            ];
            for (input, expected) in cases {
                let calc = parse(input);
                assert_eq!(calc.validate(), Ok(()), "{input}");
                assert_eq!(calc.resolve(400.0), *expected, "{input}");
            }
        }

        #[cfg(feature = "std")]
        #[test]
        fn parses_the_trigonometric_and_exponential_functions() {
            let cases: &[(&str, f32)] = &[
                ("calc(10px * pi)", 31.4159),
                ("calc(e * 10px)", 27.1828),
                ("hypot(30%, 40px)", 126.491),
                ("calc(100px * sin(30deg))", 50.0),
                ("calc(100px * sin(0.5turn))", 0.0),
                ("calc(100px * cos(100grad))", 0.0),
                ("calc(100px * tan(0.7853982rad))", 100.0),
                ("calc(atan2(1px, 1px) / 1deg * 1px)", 45.0),
                ("calc(atan(1) / 1deg * 1px)", 45.0),
                ("calc(pow(2, 3) * 10px)", 80.0),
                ("calc(log(8, 2) * 10px)", 30.0),
                ("calc(sqrt(16) * 10px)", 40.0),
                ("calc(exp(0) * 10px)", 10.0),
                ("calc(acos(2) / 1deg * 1px)", 0.0),
            ];
            for (input, expected) in cases {
                let calc = parse(input);
                assert_eq!(calc.validate(), Ok(()), "{input}");
                let v = calc.resolve(400.0);
                assert!((v - expected).abs() < 1e-3, "{input}: {v} != {expected}");
            }
        }

        #[test]
        fn refuses_what_chrome_refuses() {
            // Invalid syntax
            for input in [
                "calc(10px -5px)",
                "calc(10px+5px)",
                "calc(10px 5px)",
                "min()",
                "clamp(1px, 2px)",
                "calc(10em)",
                "round(a, 1px, 2px)",
            ] {
                assert!(Calc::from_str(input).is_err(), "{input}");
            }
            // Valid syntax, invalid types
            for input in ["calc(10px + 5)", "calc(10px * 5px)", "round(17px)", "calc(5)", "calc(sqrt(4px) * 1px)"] {
                assert!(Calc::from_str(input).map(|c| c.validate().is_err()).unwrap_or(true), "{input}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::sys::Vec;

    fn sum(items: Vec<Calc>) -> Calc {
        Calc::Sum(items)
    }
    fn product(items: Vec<Calc>) -> Calc {
        Calc::Product(items)
    }
    fn f(function: CalcFunction, items: Vec<Calc>) -> Calc {
        Calc::Function(function, items)
    }
    fn px(v: f32) -> Calc {
        Calc::Length(v)
    }
    fn pc(v: f32) -> Calc {
        Calc::Percent(v)
    }
    fn n(v: f32) -> Calc {
        Calc::Number(v)
    }

    #[test]
    fn linear_expressions_resolve_against_the_basis() {
        // calc(50% - 12px) at 400
        assert_eq!(sum(vec![pc(0.5), Calc::Negate(Box::new(px(12.0)))]).resolve(400.0), 188.0);
        // calc((50% + 10px) * 2)
        assert_eq!(product(vec![sum(vec![pc(0.5), px(10.0)]), n(2.0)]).resolve(100.0), 120.0);
        // calc(100px / 4)
        assert_eq!(product(vec![px(100.0), Calc::Invert(Box::new(n(4.0)))]).resolve(0.0), 25.0);
    }

    #[test]
    fn typed_arithmetic_divides_lengths_into_numbers() {
        // calc(100px * (10px / 5px)) is 200px; calc(10% / 1px * 1px) is 10%
        let ratio = product(vec![px(10.0), Calc::Invert(Box::new(px(5.0)))]);
        let e = product(vec![px(100.0), ratio]);
        assert_eq!(e.validate(), Ok(()));
        assert_eq!(e.resolve(0.0), 200.0);
        let e = product(vec![pc(0.1), Calc::Invert(Box::new(px(1.0))), px(1.0)]);
        assert_eq!(e.validate(), Ok(()));
        assert_eq!(e.resolve(400.0), 40.0);
    }

    #[test]
    fn validation_follows_css_types() {
        assert_eq!(sum(vec![px(10.0), n(5.0)]).validate(), Err(CalcError::TypeMismatch));
        assert_eq!(product(vec![px(10.0), px(5.0)]).validate(), Err(CalcError::NotALength));
        assert_eq!(n(5.0).validate(), Err(CalcError::NotALength));
        assert_eq!(sum(vec![]).validate(), Err(CalcError::WrongArgumentCount));
        assert_eq!(f(CalcFunction::Clamp, vec![px(1.0), px(2.0)]).validate(), Err(CalcError::WrongArgumentCount));
        // round() needs a step unless it rounds a number
        assert_eq!(
            f(CalcFunction::Round(RoundingStrategy::Nearest), vec![px(17.0)]).validate(),
            Err(CalcError::WrongArgumentCount)
        );
        assert_eq!(
            product(vec![px(10.0), f(CalcFunction::Round(RoundingStrategy::Nearest), vec![n(2.4)])]).resolve(0.0),
            20.0
        );
        assert_eq!(product(vec![px(10.0), f(CalcFunction::Sign, vec![pc(-0.05)])]).validate(), Ok(()));
    }

    #[test]
    fn comparison_functions() {
        let min = f(CalcFunction::Min, vec![pc(0.5), px(150.0)]);
        assert_eq!(min.resolve(400.0), 150.0);
        assert_eq!(min.resolve(200.0), 100.0);
        assert_eq!(f(CalcFunction::Max, vec![px(10.0)]).resolve(0.0), 10.0);
        let clamp = f(CalcFunction::Clamp, vec![px(10.0), pc(0.5), px(150.0)]);
        assert_eq!(clamp.resolve(400.0), 150.0);
        assert_eq!(clamp.resolve(100.0), 50.0);
        assert_eq!(clamp.resolve(10.0), 10.0);
        // clamp(min, value, max) with min > max is min
        assert_eq!(f(CalcFunction::Clamp, vec![px(30.0), px(20.0), px(10.0)]).resolve(0.0), 30.0);
        assert_eq!(f(CalcFunction::ClampAbove, vec![pc(0.5), px(150.0)]).resolve(400.0), 150.0);
        assert_eq!(f(CalcFunction::ClampBelow, vec![px(30.0), pc(0.05)]).resolve(400.0), 30.0);
    }

    #[test]
    fn stepped_value_functions_match_chrome() {
        use RoundingStrategy::*;
        // The values Chrome 158 gives (`width: round(...)`, before clamping negative widths to 0).
        let r = |s, a: f32, b: f32| f(CalcFunction::Round(s), vec![px(a), px(b)]).resolve(0.0);
        assert_eq!(r(Nearest, 17.0, 5.0), 15.0);
        assert_eq!(r(Up, 17.0, 5.0), 20.0);
        assert_eq!(r(Down, -17.0, 5.0), -20.0);
        assert_eq!(r(ToZero, -17.0, 5.0), -15.0);
        assert_eq!(r(Nearest, 15.0, 10.0), 20.0);
        assert_eq!(r(Nearest, -15.0, 10.0), -10.0);
        let m = |func, a: f32, b: f32| f(func, vec![px(a), px(b)]).resolve(0.0);
        assert_eq!(m(CalcFunction::Mod, -17.0, 5.0), 3.0);
        assert_eq!(m(CalcFunction::Rem, -17.0, 5.0), -2.0);
        assert_eq!(m(CalcFunction::Mod, 17.0, -5.0), -3.0);
        assert_eq!(m(CalcFunction::Rem, 17.0, -5.0), 2.0);
        assert_eq!(f(CalcFunction::Abs, vec![pc(-0.25)]).resolve(400.0), 100.0);
    }

    #[test]
    fn non_finite_results_follow_css() {
        // NaN becomes 0; infinities the largest finite value
        assert_eq!(product(vec![px(1.0), n(f32::NAN)]).resolve(0.0), 0.0);
        assert_eq!(product(vec![px(1.0), n(f32::INFINITY)]).resolve(0.0), f32::MAX);
        assert_eq!(product(vec![px(1.0), n(f32::NEG_INFINITY)]).resolve(0.0), f32::MIN);
        assert_eq!(product(vec![px(1.0), Calc::Invert(Box::new(n(0.0)))]).resolve(0.0), f32::MAX);
        // min() with NaN is NaN, so 0
        let nan = product(vec![px(1.0), n(0.0), Calc::Invert(Box::new(n(0.0)))]);
        assert_eq!(f(CalcFunction::Min, vec![nan, px(10.0)]).resolve(0.0), 0.0);
        assert_eq!(f(CalcFunction::Round(RoundingStrategy::Nearest), vec![px(5.0), px(0.0)]).resolve(0.0), 0.0);
    }

    #[cfg(feature = "std")]
    #[test]
    fn exponential_and_trigonometric_functions_match_chrome() {
        let close = |a: f32, b: f32| assert!((a - b).abs() < 1e-3, "{a} != {b}");
        close(
            product(vec![px(100.0), f(CalcFunction::Sin, vec![Calc::Angle(core::f32::consts::FRAC_PI_6)])])
                .resolve(0.0),
            50.0,
        );
        close(f(CalcFunction::Hypot, vec![pc(0.3), px(40.0)]).resolve(400.0), 126.491);
        let deg = Calc::Invert(Box::new(Calc::Angle(core::f32::consts::PI / 180.0)));
        close(product(vec![f(CalcFunction::Atan2, vec![px(1.0), px(1.0)]), deg.clone(), px(1.0)]).resolve(0.0), 45.0);
        close(product(vec![f(CalcFunction::Pow, vec![n(2.0), n(3.0)]), px(10.0)]).resolve(0.0), 80.0);
        close(product(vec![f(CalcFunction::Log, vec![n(8.0), n(2.0)]), px(10.0)]).resolve(0.0), 30.0);
        close(product(vec![f(CalcFunction::Sqrt, vec![n(16.0)]), px(10.0)]).resolve(0.0), 40.0);
        close(product(vec![f(CalcFunction::Exp, vec![n(0.0)]), px(10.0)]).resolve(0.0), 10.0);
        // acos(2) is NaN
        assert_eq!(product(vec![f(CalcFunction::Acos, vec![n(2.0)]), deg, px(1.0)]).resolve(0.0), 0.0);
        assert_eq!(f(CalcFunction::Sqrt, vec![px(4.0)]).validate(), Err(CalcError::InvalidArgument));
    }

    #[test]
    fn simplification_matches_css_parse_time_simplification() {
        use SimplifiedCalc::*;
        assert_eq!(sum(vec![px(10.0), px(5.0)]).simplify(), Some(Length(15.0)));
        assert_eq!(f(CalcFunction::Min, vec![px(10.0), px(20.0)]).simplify(), Some(Length(10.0)));
        assert_eq!(product(vec![pc(0.25), n(2.0)]).simplify(), Some(Percent(0.5)));
        assert_eq!(product(vec![pc(0.5), Calc::Invert(Box::new(n(2.0)))]).simplify(), Some(Percent(0.25)));
        // calc(50% - 50% + 3px) keeps its percentage term (CSS serialises it as calc(0% + 3px))
        assert_eq!(sum(vec![pc(0.5), Calc::Negate(Box::new(pc(0.5))), px(3.0)]).simplify(), None);
        assert_eq!(sum(vec![pc(0.5), px(-12.0)]).simplify(), None);
        assert_eq!(f(CalcFunction::Min, vec![pc(0.5), px(20.0)]).simplify(), None);
        assert_eq!(n(1.0).simplify(), None);
    }

    #[test]
    fn the_store_hands_out_handles_and_forgets_removed_ones() {
        let mut store = CalcStore::new();
        let a = store.insert(sum(vec![pc(0.5), px(-12.0)])).unwrap();
        let b = store.insert(px(3.0)).unwrap();
        assert_ne!(a, b);
        assert_eq!(store.len(), 2);
        assert_eq!(store.resolve(a, 400.0), 188.0);
        assert_eq!(store.remove(a), Some(sum(vec![pc(0.5), px(-12.0)])));
        assert_eq!(store.remove(a), None);
        // A new expression reuses the slot, but the old handle doesn't name it.
        let c = store.insert(px(7.0)).unwrap();
        assert_ne!(a, c);
        assert_eq!(store.resolve(a, 400.0), 0.0);
        assert_eq!(store.resolve(c, 400.0), 7.0);
        assert!(store.insert(n(1.0)).is_err());
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.resolve(b, 0.0), 0.0);
    }

    #[test]
    fn handles_are_valid_compact_length_payloads() {
        use crate::style::{Dimension, LengthPercentage};
        let mut store = CalcStore::new();
        let h = store.insert(px(1.0)).unwrap();
        // CompactLength::calc asserts a non-zero handle with its low three bits clear.
        let d = Dimension::calc(h);
        assert_eq!(d.calc_value(), h);
        let _ = LengthPercentage::calc(h);
    }
}
