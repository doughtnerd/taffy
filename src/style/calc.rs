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
    length: i8,
    angle: i8,
}

impl CalcType {
    const NUMBER: Self = Self { length: 0, angle: 0 };
    const LENGTH: Self = Self { length: 1, angle: 0 };
    const ANGLE: Self = Self { length: 0, angle: 1 };

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

fn floor(x: f64) -> f64 {
    let t = trunc(x);
    if t > x { t - 1.0 } else { t }
}

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

fn arg(items: &[Calc], i: usize, basis: f64) -> f64 {
    items.get(i).map_or(f64::NAN, |c| c.eval(basis))
}

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
            RoundingStrategy::Nearest | RoundingStrategy::ToZero => 0.0_f64.copysign(a),
            RoundingStrategy::Up => {
                if a > 0.0 {
                    f64::INFINITY
                } else {
                    0.0_f64.copysign(a)
                }
            }
            RoundingStrategy::Down => {
                if a < 0.0 {
                    f64::NEG_INFINITY
                } else {
                    0.0_f64.copysign(a)
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
    entries: Vec<CalcEntry>,
    free: Vec<u32>,
    len: usize,
}

#[derive(Clone, Debug)]
struct CalcEntry {
    generation: u32,
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

    fn handle(index: u32, generation: u32) -> CalcHandle {
        let key = (u64::from(generation & GENERATION_MASK) << 32) | u64::from(index);
        CalcHandle::from_bits(key << 3)
    }

    fn index_of(&self, handle: CalcHandle) -> Option<usize> {
        let key = handle.to_bits() >> 3;
        let index = (key & u64::from(u32::MAX)) as usize;
        let generation = (key >> 32) as u32;
        (self.entries.get(index)?.generation == generation).then_some(index)
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
