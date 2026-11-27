//! `calc()` expressions, and a store that hands out [`CalcHandle`]s for them.
//!
//! [`Calc`] represents the CSS math functions that produce a `<length-percentage>`
//! (<https://drafts.csswg.org/css-values-4/#math>). [`CalcStore`] owns expressions and resolves their handles:
//! [`TaffyTree`](crate::TaffyTree) uses one, and a custom tree can use one from
//! [`LayoutPartialTree::resolve_calc_value`](crate::LayoutPartialTree::resolve_calc_value).
//!
//! Taffy only knows pixels and percentages, so other units must be converted to pixels before an expression is built.

use crate::style::CalcHandle;
use crate::util::sys::{Box, Vec};

/// A `calc()` expression over pixels, percentages and numbers. It must resolve to a length ([`Calc::validate`]).
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum Calc {
    /// A length in pixels
    Length(f32),
    /// A percentage of the basis, as a fraction (`0.5` is `50%`)
    Percent(f32),
    /// A number
    Number(f32),
    /// The sum of the terms (`a - b` is `Sum([a, Negate(b)])`)
    Sum(Vec<Calc>),
    /// The product of the factors (`a / b` is `Product([a, Invert(b)])`)
    Product(Vec<Calc>),
    /// The negation of a term
    Negate(Box<Calc>),
    /// The reciprocal of a factor
    Invert(Box<Calc>),
    /// A math function applied to its arguments
    Function(CalcFunction, Vec<Calc>),
}

/// A math function in a [`Calc`] expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[non_exhaustive]
pub enum CalcFunction {
    /// `min(a, …)`
    Min,
    /// `max(a, …)`
    Max,
    /// `clamp(min, value, max)`
    Clamp,
    /// `clamp(none, value, max)`, with arguments `value, max`
    ClampAbove,
    /// `clamp(min, value, none)`, with arguments `min, value`
    ClampBelow,
    /// `round(strategy, a, b)`; `b` may be omitted only when `a` is a number
    Round(RoundingStrategy),
    /// `mod(a, b)`: the remainder, with the sign of `b`
    Mod,
    /// `rem(a, b)`: the remainder, with the sign of `a`
    Rem,
    /// `abs(a)`
    Abs,
    /// `sign(a)`: -1, 0 or 1
    Sign,
}

/// How `round()` picks a multiple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RoundingStrategy {
    /// The nearer multiple; a value halfway between goes towards positive infinity
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
#[non_exhaustive]
pub enum CalcError {
    /// The expression doesn't resolve to a length
    NotALength,
    /// Values of different types are combined (`10px + 5`)
    TypeMismatch,
    /// A function has the wrong number of arguments, or a sum or product has none
    WrongArgumentCount,
}

impl core::fmt::Display for CalcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            CalcError::NotALength => "the calc() expression doesn't resolve to a length",
            CalcError::TypeMismatch => "the calc() expression combines values of different types",
            CalcError::WrongArgumentCount => "a calc() function has the wrong number of arguments",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CalcError {}

/// The largest length a `calc()` resolves to, in either direction: Chrome's layout range (`LayoutUnit`). Clamping
/// infinite results here rather than to `f32::MAX` keeps sums and products of them finite.
const CALC_MAX: f64 = 33_554_431.0;

/// The type of a value: the power of length in it (`px` is 1, `px / px` is 0, a number).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CalcType {
    /// The power of length (percentages are lengths)
    length: i8,
}

impl CalcType {
    /// A number
    const NUMBER: Self = Self { length: 0 };
    /// A length
    const LENGTH: Self = Self { length: 1 };
}

impl Calc {
    /// Checks that the expression is well-typed and resolves to a length, as CSS requires.
    pub fn validate(&self) -> Result<(), CalcError> {
        match self.type_of()? {
            CalcType::LENGTH => Ok(()),
            _ => Err(CalcError::NotALength),
        }
    }

    /// The expression's value with its percentages resolved against `basis`. A NaN result is 0 and an infinite one
    /// is clamped, as CSS specifies for a top-level calculation.
    pub fn resolve(&self, basis: f32) -> f32 {
        let v = self.eval(f64::from(basis));
        if v.is_nan() { 0.0 } else { v.clamp(-CALC_MAX, CALC_MAX) as f32 }
    }

    /// The expression's type, or why it has none
    fn type_of(&self) -> Result<CalcType, CalcError> {
        /// The type shared by all of `items`
        fn same(items: &[Calc]) -> Result<CalcType, CalcError> {
            let mut types = items.iter().map(Calc::type_of);
            let first = types.next().ok_or(CalcError::WrongArgumentCount)??;
            for t in types {
                if t? != first {
                    return Err(CalcError::TypeMismatch);
                }
            }
            Ok(first)
        }
        /// `items`'s type, if there are `count` of them
        fn exactly(items: &[Calc], count: usize) -> Result<CalcType, CalcError> {
            if items.len() == count { same(items) } else { Err(CalcError::WrongArgumentCount) }
        }
        Ok(match self {
            Calc::Length(_) | Calc::Percent(_) => CalcType::LENGTH,
            Calc::Number(_) => CalcType::NUMBER,
            Calc::Sum(items) => same(items)?,
            Calc::Product(items) => {
                if items.is_empty() {
                    return Err(CalcError::WrongArgumentCount);
                }
                let mut length = 0i8;
                for item in items {
                    length = length.saturating_add(item.type_of()?.length);
                }
                CalcType { length }
            }
            Calc::Negate(c) => c.type_of()?,
            Calc::Invert(c) => CalcType { length: -c.type_of()?.length },
            Calc::Function(f, items) => match f {
                CalcFunction::Min | CalcFunction::Max => same(items)?,
                CalcFunction::Clamp => exactly(items, 3)?,
                CalcFunction::ClampAbove | CalcFunction::ClampBelow | CalcFunction::Mod | CalcFunction::Rem => {
                    exactly(items, 2)?
                }
                CalcFunction::Round(_) => {
                    let t = same(items)?;
                    // Without a step, only a number can be rounded (to an integer)
                    if items.len() > 2 || (items.len() == 1 && t != CalcType::NUMBER) {
                        return Err(CalcError::WrongArgumentCount);
                    }
                    t
                }
                CalcFunction::Abs => exactly(items, 1)?,
                CalcFunction::Sign => {
                    exactly(items, 1)?;
                    CalcType::NUMBER
                }
            },
        })
    }

    /// The value in double precision, NaN and infinities kept
    fn eval(&self, basis: f64) -> f64 {
        match self {
            Calc::Length(v) | Calc::Number(v) => f64::from(*v),
            Calc::Percent(p) => f64::from(*p) * basis,
            Calc::Sum(items) => items.iter().map(|c| c.eval(basis)).sum(),
            Calc::Product(items) => items.iter().map(|c| c.eval(basis)).product(),
            Calc::Negate(c) => -c.eval(basis),
            Calc::Invert(c) => 1.0 / c.eval(basis),
            Calc::Function(f, items) => eval_function(*f, items, basis),
        }
    }

    /// True when the expression contains a percentage
    fn uses_percentage(&self) -> bool {
        match self {
            Calc::Percent(_) => true,
            Calc::Length(_) | Calc::Number(_) => false,
            Calc::Negate(c) | Calc::Invert(c) => c.uses_percentage(),
            Calc::Sum(items) | Calc::Product(items) | Calc::Function(_, items) => {
                items.iter().any(Calc::uses_percentage)
            }
        }
    }

    /// The plain length or percentage the expression simplifies to, as CSS simplifies `calc()` when it parses it: an
    /// expression without percentages is a length, and a sum and product of percentages and numbers alone is a
    /// percentage. Others (`calc(50% - 12px)`, and `calc(50% + 0px)`, whose two units CSS keeps) don't simplify.
    pub(crate) fn simplify(&self) -> Option<sealed::SimplifiedCalc> {
        self.validate().ok()?;
        if !self.uses_percentage() {
            return Some(sealed::SimplifiedCalc::Length(self.resolve(0.0)));
        }
        self.percent_only().map(|fraction| sealed::SimplifiedCalc::Percent(fraction as f32))
    }

    /// The fraction a sum and product of percentages and numbers comes to, if that's all the expression is
    fn percent_only(&self) -> Option<f64> {
        match self {
            Calc::Percent(p) => Some(f64::from(*p)),
            Calc::Negate(c) => c.percent_only().map(|f| -f),
            Calc::Sum(items) => items.iter().map(Calc::percent_only).sum(),
            Calc::Product(items) => {
                let mut fraction = None;
                let mut scale = 1.0;
                for item in items {
                    match item {
                        Calc::Number(n) => scale *= f64::from(*n),
                        Calc::Invert(inner) => match **inner {
                            Calc::Number(n) => scale /= f64::from(n),
                            _ => return None,
                        },
                        other if fraction.is_none() => fraction = Some(other.percent_only()?),
                        _ => return None,
                    }
                }
                Some(fraction? * scale)
            }
            _ => None,
        }
    }
}

/// `x` rounded towards zero (`%` is `fmod` in `core`)
fn trunc(x: f64) -> f64 {
    if x.is_finite() { x - x % 1.0 } else { x }
}

/// `x` rounded down
fn floor(x: f64) -> f64 {
    let t = trunc(x);
    if t > x { t - 1.0 } else { t }
}

/// `x` rounded up
fn ceil(x: f64) -> f64 {
    let t = trunc(x);
    if t < x { t + 1.0 } else { t }
}

/// A math function's value
fn eval_function(f: CalcFunction, items: &[Calc], basis: f64) -> f64 {
    let arg = |i: usize| items.get(i).map_or(f64::NAN, |c| c.eval(basis));
    match f {
        CalcFunction::Min => pick(items, basis, |v, best| v < best),
        CalcFunction::Max => pick(items, basis, |v, best| v > best),
        CalcFunction::Clamp => clamp(Some(arg(0)), arg(1), Some(arg(2))),
        CalcFunction::ClampAbove => clamp(None, arg(0), Some(arg(1))),
        CalcFunction::ClampBelow => clamp(Some(arg(0)), arg(1), None),
        CalcFunction::Round(strategy) => round(strategy, arg(0), if items.len() > 1 { arg(1) } else { 1.0 }),
        CalcFunction::Mod => modulo(arg(0), arg(1), true),
        CalcFunction::Rem => modulo(arg(0), arg(1), false),
        CalcFunction::Abs => arg(0).abs(),
        CalcFunction::Sign => {
            let v = arg(0);
            if v > 0.0 {
                1.0
            } else if v < 0.0 {
                -1.0
            } else {
                v
            }
        }
    }
}

/// `min()` / `max()`: a NaN argument makes the result NaN
fn pick(items: &[Calc], basis: f64, better: fn(f64, f64) -> bool) -> f64 {
    let mut best: Option<f64> = None;
    for v in items.iter().map(|c| c.eval(basis)) {
        if v.is_nan() {
            return f64::NAN;
        }
        if best.is_none_or(|b| better(v, b)) {
            best = Some(v);
        }
    }
    best.unwrap_or(f64::NAN)
}

/// `clamp()`: `max(min, min(value, max))`, so `min` wins when it's above `max`; NaN anywhere is NaN
fn clamp(min: Option<f64>, value: f64, max: Option<f64>) -> f64 {
    if min.is_some_and(f64::is_nan) || value.is_nan() || max.is_some_and(f64::is_nan) {
        return f64::NAN;
    }
    let v = max.map_or(value, |max| value.min(max));
    min.map_or(v, |min| v.max(min))
}

/// `round(strategy, a, b)` (<https://drafts.csswg.org/css-values-4/#round-func>)
fn round(strategy: RoundingStrategy, a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() || b == 0.0 || (a.is_infinite() && b.is_infinite()) {
        return f64::NAN;
    }
    if a.is_infinite() {
        return a;
    }
    let zero = if a.is_sign_negative() { -0.0 } else { 0.0 };
    if b.is_infinite() {
        return match strategy {
            RoundingStrategy::Up if a > 0.0 => f64::INFINITY,
            RoundingStrategy::Down if a < 0.0 => f64::NEG_INFINITY,
            _ => zero,
        };
    }
    let step = b.abs();
    let lower = floor(a / step) * step;
    let upper = ceil(a / step) * step;
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

/// `mod()` (`is_mod`: the quotient rounded down, so the result has `b`'s sign) and `rem()` (rounded towards zero, `a`'s)
fn modulo(a: f64, b: f64, is_mod: bool) -> f64 {
    if a.is_nan() || b.is_nan() || b == 0.0 || a.is_infinite() {
        return f64::NAN;
    }
    if b.is_infinite() {
        // `a` itself, unless mod() would have to cross zero to take `b`'s sign
        return if !is_mod || (a >= 0.0) == (b > 0.0) { a } else { f64::NAN };
    }
    a - b * if is_mod { floor(a / b) } else { trunc(a / b) }
}

/// The sealed half of [`FromCalc`]
mod sealed {
    use crate::style::CalcHandle;

    /// An expression simplified to a plain value
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum SimplifiedCalc {
        /// A length in pixels
        Length(f32),
        /// A percentage, as a fraction
        Percent(f32),
    }

    /// How a style value is built from an expression
    pub trait FromCalcSealed: Sized {
        /// The value for a simplified expression
        fn from_simplified(value: SimplifiedCalc) -> Self;
        /// The value for an expression behind a handle
        fn from_calc_handle(handle: CalcHandle) -> Self;
    }
}

/// A style value that can hold a `calc()`: what [`TaffyTree::new_calc`](crate::TaffyTree::new_calc) returns.
pub trait FromCalc: sealed::FromCalcSealed {}

/// Implements [`FromCalc`] for types with `length`, `percent` and `calc` constructors
macro_rules! impl_from_calc {
    ($($(#[$meta:meta])* $ty:ty),+ $(,)?) => {
        $(
            $(#[$meta])*
            impl sealed::FromCalcSealed for $ty {
                fn from_simplified(value: sealed::SimplifiedCalc) -> Self {
                    match value {
                        sealed::SimplifiedCalc::Length(v) => <$ty>::length(v),
                        sealed::SimplifiedCalc::Percent(v) => <$ty>::percent(v),
                    }
                }
                fn from_calc_handle(handle: CalcHandle) -> Self {
                    <$ty>::calc(handle)
                }
            }
            $(#[$meta])*
            impl FromCalc for $ty {}
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

/// The `calc()` expressions a tree's styles refer to, by [`CalcHandle`].
///
/// Handles are generation-checked: a handle kept after [`remove`](CalcStore::remove) resolves to 0, not to a later
/// expression in the same slot.
#[derive(Clone, Debug, Default)]
pub struct CalcStore {
    /// The slots, by index
    entries: Vec<CalcEntry>,
    /// The indices of empty slots
    free: Vec<u32>,
    /// How many slots hold an expression
    len: usize,
}

/// One slot of a [`CalcStore`]
#[derive(Clone, Debug)]
struct CalcEntry {
    /// Bumped each time the slot is emptied, so that old handles stop matching it
    generation: u32,
    /// The expression, if the slot holds one
    calc: Option<Calc>,
}

/// The bits of a generation a handle keeps (a handle has 61 bits: 32 for the slot and 28 for its generation)
const GENERATION_MASK: u32 = (1 << 28) - 1;

impl CalcStore {
    /// An empty store
    pub const fn new() -> Self {
        Self { entries: Vec::new(), free: Vec::new(), len: 0 }
    }

    /// Stores an expression and returns its handle. Errors when it doesn't resolve to a length.
    pub fn insert(&mut self, calc: Calc) -> Result<CalcHandle, CalcError> {
        calc.validate()?;
        Ok(self.insert_unchecked(calc))
    }

    /// Stores an expression that has been validated
    pub(crate) fn insert_unchecked(&mut self, calc: Calc) -> CalcHandle {
        self.len += 1;
        let index = match self.free.pop() {
            Some(index) => {
                self.entries[index as usize].calc = Some(calc);
                index
            }
            None => {
                self.entries.push(CalcEntry { generation: 1, calc: Some(calc) });
                (self.entries.len() - 1) as u32
            }
        };
        let key = (u64::from(self.entries[index as usize].generation) << 32) | u64::from(index);
        CalcHandle::from_bits(key << 3)
    }

    /// A style value for an expression: a plain length or percentage when it simplifies to one, else a handle
    pub(crate) fn value<T: FromCalc>(&mut self, calc: Calc) -> Result<T, CalcError> {
        calc.validate()?;
        Ok(match calc.simplify() {
            Some(simple) => T::from_simplified(simple),
            None => T::from_calc_handle(self.insert_unchecked(calc)),
        })
    }

    /// Removes an expression and returns it (`None` if the store doesn't hold it). Remove an expression only once no
    /// style refers to it: a style still holding the handle resolves it to 0.
    pub fn remove(&mut self, handle: CalcHandle) -> Option<Calc> {
        let index = self.index_of(handle)?;
        let entry = &mut self.entries[index];
        let calc = entry.calc.take()?;
        entry.generation = (entry.generation % GENERATION_MASK) + 1;
        self.free.push(index as u32);
        self.len -= 1;
        Some(calc)
    }

    /// The expression a handle refers to, if the store holds it
    pub fn get(&self, handle: CalcHandle) -> Option<&Calc> {
        self.entries.get(self.index_of(handle)?)?.calc.as_ref()
    }

    /// The value of a handle's expression with its percentages resolved against `basis` (0 if the store doesn't hold
    /// it)
    pub fn resolve(&self, handle: CalcHandle, basis: f32) -> f32 {
        self.get(handle).map_or(0.0, |calc| calc.resolve(basis))
    }

    /// How many expressions the store holds
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when the store holds no expressions
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes every expression
    pub fn clear(&mut self) {
        for (index, entry) in self.entries.iter_mut().enumerate() {
            if entry.calc.take().is_some() {
                entry.generation = (entry.generation % GENERATION_MASK) + 1;
                self.free.push(index as u32);
            }
        }
        self.len = 0;
    }

    /// The slot a handle refers to, if its generation still matches
    fn index_of(&self, handle: CalcHandle) -> Option<usize> {
        let key = handle.to_bits() >> 3;
        let index = (key & u64::from(u32::MAX)) as usize;
        (self.entries.get(index)?.generation == (key >> 32) as u32).then_some(index)
    }
}

#[cfg(test)]
mod tests {
    use super::sealed::SimplifiedCalc;
    use super::*;
    use crate::util::sys::Vec;

    fn px(v: f32) -> Calc {
        Calc::Length(v)
    }
    fn pc(v: f32) -> Calc {
        Calc::Percent(v)
    }
    fn n(v: f32) -> Calc {
        Calc::Number(v)
    }
    fn neg(c: Calc) -> Calc {
        Calc::Negate(Box::new(c))
    }
    fn inv(c: Calc) -> Calc {
        Calc::Invert(Box::new(c))
    }
    fn f(function: CalcFunction, items: Vec<Calc>) -> Calc {
        Calc::Function(function, items)
    }

    #[test]
    fn arithmetic() {
        // calc(50% - 12px), calc((50% + 10px) * 2), calc(100px / 4) at a basis of 400
        assert_eq!(Calc::Sum(vec![pc(0.5), neg(px(12.0))]).resolve(400.0), 188.0);
        assert_eq!(Calc::Product(vec![Calc::Sum(vec![pc(0.5), px(10.0)]), n(2.0)]).resolve(400.0), 420.0);
        assert_eq!(Calc::Product(vec![px(100.0), inv(n(4.0))]).resolve(0.0), 25.0);
        // Typed arithmetic: calc(100px * (10px / 5px)), calc(10% / 1px * 1px)
        let e = Calc::Product(vec![px(100.0), Calc::Product(vec![px(10.0), inv(px(5.0))])]);
        assert_eq!((e.validate(), e.resolve(0.0)), (Ok(()), 200.0));
        let e = Calc::Product(vec![pc(0.1), inv(px(1.0)), px(1.0)]);
        assert_eq!((e.validate(), e.resolve(400.0)), (Ok(()), 40.0));
    }

    #[test]
    fn validation() {
        assert_eq!(Calc::Sum(vec![px(10.0), n(5.0)]).validate(), Err(CalcError::TypeMismatch));
        assert_eq!(Calc::Product(vec![px(10.0), px(5.0)]).validate(), Err(CalcError::NotALength));
        assert_eq!(n(5.0).validate(), Err(CalcError::NotALength));
        assert_eq!(Calc::Sum(vec![]).validate(), Err(CalcError::WrongArgumentCount));
        assert_eq!(f(CalcFunction::Clamp, vec![px(1.0), px(2.0)]).validate(), Err(CalcError::WrongArgumentCount));
        let round = |items| f(CalcFunction::Round(RoundingStrategy::Nearest), items);
        assert_eq!(round(vec![px(17.0)]).validate(), Err(CalcError::WrongArgumentCount));
        assert_eq!(Calc::Product(vec![px(10.0), round(vec![n(2.4)])]).resolve(0.0), 20.0);
    }

    #[test]
    fn comparison_functions() {
        let min = f(CalcFunction::Min, vec![pc(0.5), px(150.0)]);
        assert_eq!((min.resolve(400.0), min.resolve(200.0)), (150.0, 100.0));
        let clamp = f(CalcFunction::Clamp, vec![px(10.0), pc(0.5), px(150.0)]);
        assert_eq!((clamp.resolve(400.0), clamp.resolve(100.0), clamp.resolve(10.0)), (150.0, 50.0, 10.0));
        assert_eq!(f(CalcFunction::Clamp, vec![px(30.0), px(20.0), px(10.0)]).resolve(0.0), 30.0);
        assert_eq!(f(CalcFunction::ClampAbove, vec![pc(0.5), px(150.0)]).resolve(400.0), 150.0);
        assert_eq!(f(CalcFunction::ClampBelow, vec![px(30.0), pc(0.05)]).resolve(400.0), 30.0);
    }

    #[test]
    fn stepped_value_functions() {
        // As Chrome 158 computes them
        use RoundingStrategy::*;
        let r = |s, a, b| f(CalcFunction::Round(s), vec![px(a), px(b)]).resolve(0.0);
        assert_eq!([r(Nearest, 17.0, 5.0), r(Up, 17.0, 5.0), r(Down, -17.0, 5.0)], [15.0, 20.0, -20.0]);
        assert_eq!([r(ToZero, -17.0, 5.0), r(Nearest, 15.0, 10.0), r(Nearest, -15.0, 10.0)], [-15.0, 20.0, -10.0]);
        let m = |func, a, b| f(func, vec![px(a), px(b)]).resolve(0.0);
        assert_eq!([m(CalcFunction::Mod, -17.0, 5.0), m(CalcFunction::Rem, -17.0, 5.0)], [3.0, -2.0]);
        assert_eq!([m(CalcFunction::Mod, 17.0, -5.0), m(CalcFunction::Rem, 17.0, -5.0)], [-3.0, 2.0]);
        assert_eq!(f(CalcFunction::Abs, vec![pc(-0.25)]).resolve(400.0), 100.0);
        assert_eq!(Calc::Product(vec![px(10.0), f(CalcFunction::Sign, vec![pc(-0.05)])]).resolve(400.0), -10.0);
    }

    #[test]
    fn non_finite_results() {
        let nan = Calc::Product(vec![px(1.0), n(0.0), inv(n(0.0))]);
        assert_eq!(nan.resolve(0.0), 0.0);
        assert_eq!(f(CalcFunction::Min, vec![nan, px(10.0)]).resolve(0.0), 0.0);
        assert_eq!(Calc::Product(vec![px(1.0), n(f32::INFINITY)]).resolve(0.0), CALC_MAX as f32);
        assert_eq!(Calc::Product(vec![px(1.0), n(f32::NEG_INFINITY)]).resolve(0.0), -CALC_MAX as f32);
        assert_eq!(f(CalcFunction::Round(RoundingStrategy::Nearest), vec![px(5.0), px(0.0)]).resolve(0.0), 0.0);
    }

    #[test]
    fn simplification() {
        use SimplifiedCalc::*;
        assert_eq!(Calc::Sum(vec![px(10.0), px(5.0)]).simplify(), Some(Length(15.0)));
        assert_eq!(f(CalcFunction::Min, vec![px(10.0), px(20.0)]).simplify(), Some(Length(10.0)));
        assert_eq!(Calc::Product(vec![pc(0.25), n(2.0)]).simplify(), Some(Percent(0.5)));
        assert_eq!(Calc::Product(vec![pc(0.5), inv(n(2.0))]).simplify(), Some(Percent(0.25)));
        // Kept as expressions: mixed units, including a zero length (CSS keeps both terms)
        assert_eq!(Calc::Sum(vec![pc(0.5), px(-12.0)]).simplify(), None);
        assert_eq!(Calc::Sum(vec![pc(0.5), px(0.0)]).simplify(), None);
        assert_eq!(f(CalcFunction::Min, vec![pc(0.5), px(20.0)]).simplify(), None);
    }

    #[test]
    fn store() {
        let mut store = CalcStore::new();
        let a = store.insert(Calc::Sum(vec![pc(0.5), px(-12.0)])).unwrap();
        let b = store.insert(px(3.0)).unwrap();
        assert_eq!((store.len(), store.resolve(a, 400.0)), (2, 188.0));
        assert!(store.remove(a).is_some());
        assert!(store.remove(a).is_none());
        // The slot is reused, but the old handle doesn't refer to it
        let c = store.insert(px(7.0)).unwrap();
        assert_eq!((store.resolve(a, 400.0), store.resolve(c, 400.0)), (0.0, 7.0));
        assert!(store.insert(n(1.0)).is_err());
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.resolve(b, 0.0), 0.0);
        // Handles are valid `CompactLength` payloads (non-zero, low three bits clear)
        let h = store.insert(px(1.0)).unwrap();
        assert_eq!(crate::style::Dimension::calc(h).calc_value(), h);
    }
}
