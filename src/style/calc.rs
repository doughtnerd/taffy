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
    /// An angle in radians, for the trigonometric functions
    Angle(f32),
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
    /// `hypot(a, …)`: the square root of the sum of the squares
    Hypot,
    /// `sin(a)` of an angle or a number of radians
    Sin,
    /// `cos(a)` of an angle or a number of radians
    Cos,
    /// `tan(a)` of an angle or a number of radians
    Tan,
    /// `asin(a)` of a number: an angle
    Asin,
    /// `acos(a)` of a number: an angle
    Acos,
    /// `atan(a)` of a number: an angle
    Atan,
    /// `atan2(y, x)`: an angle
    Atan2,
    /// `pow(base, exponent)` of numbers
    Pow,
    /// `sqrt(a)` of a number
    Sqrt,
    /// `log(a)` or `log(a, base)` of numbers
    Log,
    /// `exp(a)` of a number
    Exp,
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
    /// A function's argument has a type it doesn't accept (`sqrt(4px)`)
    InvalidArgument,
    /// The expression uses `hypot()`, a trigonometric or an exponential function, which need Taffy's `std` feature
    Unsupported,
}

impl core::fmt::Display for CalcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            CalcError::NotALength => "the calc() expression doesn't resolve to a length",
            CalcError::TypeMismatch => "the calc() expression combines values of different types",
            CalcError::WrongArgumentCount => "a calc() function has the wrong number of arguments",
            CalcError::InvalidArgument => "a calc() function's argument has a type it doesn't accept",
            CalcError::Unsupported => "hypot(), trigonometric and exponential functions need Taffy's std feature",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CalcError {}

/// The largest length a `calc()` resolves to, in either direction: Chrome's layout range (`LayoutUnit`). Clamping
/// infinite results here rather than to `f32::MAX` keeps sums and products of them finite.
const CALC_MAX: f64 = 33_554_431.0;

/// The type of a value: the powers of length and angle in it (`px` is length¹, `px / px` is a number).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CalcType {
    /// The power of length (percentages are lengths)
    length: i8,
    /// The power of angle
    angle: i8,
}

impl CalcType {
    /// A number
    const NUMBER: Self = Self { length: 0, angle: 0 };
    /// A length
    const LENGTH: Self = Self { length: 1, angle: 0 };
    /// An angle
    const ANGLE: Self = Self { length: 0, angle: 1 };
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
        /// A number, if `items` are numbers
        fn numbers(items: &[Calc]) -> Result<CalcType, CalcError> {
            match same(items)? {
                CalcType::NUMBER => Ok(CalcType::NUMBER),
                _ => Err(CalcError::InvalidArgument),
            }
        }
        /// The functions that need `std` (`f64`'s transcendental functions)
        fn needs_std() -> Result<(), CalcError> {
            if cfg!(feature = "std") { Ok(()) } else { Err(CalcError::Unsupported) }
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
                CalcFunction::Hypot => {
                    needs_std()?;
                    same(items)?
                }
                CalcFunction::Sin | CalcFunction::Cos | CalcFunction::Tan => {
                    needs_std()?;
                    match exactly(items, 1)? {
                        CalcType::NUMBER | CalcType::ANGLE => CalcType::NUMBER,
                        _ => return Err(CalcError::InvalidArgument),
                    }
                }
                CalcFunction::Asin | CalcFunction::Acos | CalcFunction::Atan => {
                    needs_std()?;
                    exactly(items, 1)?;
                    numbers(items)?;
                    CalcType::ANGLE
                }
                CalcFunction::Atan2 => {
                    needs_std()?;
                    exactly(items, 2)?;
                    CalcType::ANGLE
                }
                CalcFunction::Pow => {
                    needs_std()?;
                    exactly(items, 2)?;
                    numbers(items)?
                }
                CalcFunction::Sqrt | CalcFunction::Exp => {
                    needs_std()?;
                    exactly(items, 1)?;
                    numbers(items)?
                }
                CalcFunction::Log => {
                    needs_std()?;
                    if !(1..=2).contains(&items.len()) {
                        return Err(CalcError::WrongArgumentCount);
                    }
                    numbers(items)?
                }
            },
        })
    }

    /// The value in double precision, NaN and infinities kept
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

    /// True when the expression contains a percentage
    fn uses_percentage(&self) -> bool {
        match self {
            Calc::Percent(_) => true,
            Calc::Length(_) | Calc::Number(_) | Calc::Angle(_) => false,
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
        CalcFunction::Hypot => transcendental::hypot(items.iter().map(|c| c.eval(basis))),
        CalcFunction::Sin => transcendental::sin(arg(0)),
        CalcFunction::Cos => transcendental::cos(arg(0)),
        CalcFunction::Tan => transcendental::tan(arg(0)),
        CalcFunction::Asin => transcendental::asin(arg(0)),
        CalcFunction::Acos => transcendental::acos(arg(0)),
        CalcFunction::Atan => transcendental::atan(arg(0)),
        CalcFunction::Atan2 => transcendental::atan2(arg(0), arg(1)),
        CalcFunction::Pow => transcendental::pow(arg(0), arg(1)),
        CalcFunction::Sqrt => transcendental::sqrt(arg(0)),
        CalcFunction::Log => {
            let ln = transcendental::ln(arg(0));
            if items.len() > 1 { ln / transcendental::ln(arg(1)) } else { ln }
        }
        CalcFunction::Exp => transcendental::exp(arg(0)),
    }
}

/// `f64`'s transcendental functions, which need `std`. Without it, [`Calc::validate`] refuses the functions that use
/// them, and these return NaN.
mod transcendental {
    /// Defines functions that call the `f64` method of the same name with `std`, and return NaN without it
    macro_rules! transcendental {
        ($($name:ident($($arg:ident),+) => $body:expr;)+) => {
            $(
                #[doc = concat!("`", stringify!($name), "`")]
                #[allow(unused_variables)]
                pub(super) fn $name($($arg: f64),+) -> f64 {
                    #[cfg(feature = "std")]
                    return $body;
                    #[cfg(not(feature = "std"))]
                    f64::NAN
                }
            )+
        };
    }
    transcendental! {
        sin(a) => a.sin();
        cos(a) => a.cos();
        tan(a) => a.tan();
        asin(a) => a.asin();
        acos(a) => a.acos();
        atan(a) => a.atan();
        atan2(y, x) => y.atan2(x);
        pow(a, b) => a.powf(b);
        sqrt(a) => a.sqrt();
        ln(a) => a.ln();
        exp(a) => a.exp();
    }

    /// `hypot(a, …)`
    pub(super) fn hypot(values: impl Iterator<Item = f64>) -> f64 {
        sqrt(values.map(|v| v * v).sum())
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
pub(crate) mod sealed {
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

    /// Parses a CSS value (`"calc(50% - 12px)"`, `"min(25%, 60px)"`, `"minmax(calc(10% + 5px), 1fr)"`, or any value
    /// the type's `FromStr` accepts), storing any `calc()` that doesn't simplify to a plain value here. A parse that
    /// fails stores nothing.
    #[cfg(feature = "parse")]
    pub fn parse_value<T: crate::util::parse::ParseWithCalc>(
        &mut self,
        input: &str,
    ) -> crate::util::parse::ParseResult<T> {
        T::parse_with_calc(input, self)
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

/// Parsing `calc()` and the other CSS math functions (`parse` feature).
#[cfg(feature = "parse")]
pub(crate) mod parse {
    use super::{Calc, CalcFunction, RoundingStrategy, sealed::SimplifiedCalc};
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
            None => match cx.store(calc) {
                Some(handle) => Ok(ParsedCalc::Handle(handle)),
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
        match parser.next()?.clone() {
            Token::Number { value, .. } => Ok(Calc::Number(value)),
            Token::Percentage { unit_value, .. } => Ok(Calc::Percent(unit_value)),
            Token::Dimension { value, ref unit, .. } => {
                use core::f32::consts::PI;
                let unit: &str = unit;
                let is = |u: &str| unit.eq_ignore_ascii_case(u);
                if is("px") {
                    Ok(Calc::Length(value))
                } else if is("deg") {
                    Ok(Calc::Angle(value * PI / 180.0))
                } else if is("rad") {
                    Ok(Calc::Angle(value))
                } else if is("grad") {
                    Ok(Calc::Angle(value * PI / 200.0))
                } else if is("turn") {
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
                    Ok(Calc::Number(core::f32::consts::PI))
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
        if is("hypot") {
            return call(CalcFunction::Hypot, list(parser)?);
        }
        if is("atan2") {
            return call(CalcFunction::Atan2, args(parser, 2)?);
        }
        if is("pow") {
            return call(CalcFunction::Pow, args(parser, 2)?);
        }
        if is("log") {
            let items = list(parser)?;
            if items.len() > 2 {
                return Err(error(parser, "log() takes a value and an optional base"));
            }
            return call(CalcFunction::Log, items);
        }
        let one = [
            ("sin", CalcFunction::Sin),
            ("cos", CalcFunction::Cos),
            ("tan", CalcFunction::Tan),
            ("asin", CalcFunction::Asin),
            ("acos", CalcFunction::Acos),
            ("atan", CalcFunction::Atan),
            ("sqrt", CalcFunction::Sqrt),
            ("exp", CalcFunction::Exp),
        ];
        match one.iter().find(|(f, _)| is(f)) {
            Some(&(_, f)) => call(f, args(parser, 1)?),
            None => Err(CssParseError::unexpected_token()),
        }
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
                // Infinite and NaN results: Chrome clamps to its largest length (2^25 - 1 px), and NaN is 0
                ("calc(infinity * 1px)", 33_554_431.0),
                ("calc(-infinity * 1px)", -33_554_431.0),
                ("calc(NaN * 1px)", 0.0),
                ("calc(1px / 0)", 33_554_431.0),
            ];
            for (input, expected) in cases {
                let calc = parse(input);
                assert_eq!(calc.validate(), Ok(()), "{input}");
                assert_eq!(calc.resolve(400.0), *expected, "{input}");
            }
            for (input, expected) in [("calc(10px * pi)", 31.415_926), ("calc(e * 10px)", 27.182_818)] {
                assert!((parse(input).resolve(400.0) - expected).abs() < 1e-4, "{input}");
            }
        }

        #[cfg(feature = "std")]
        #[test]
        fn parses_the_trigonometric_and_exponential_functions() {
            // Chrome 158's values for `width: <expression>` in a 400px containing block
            let cases: &[(&str, f32)] = &[
                ("hypot(30%, 40px)", 126.491),
                ("calc(100px * sin(30deg))", 50.0),
                ("calc(100px * sin(0.5turn))", 0.0),
                ("calc(100px * cos(100grad))", 0.0),
                ("calc(100px * tan(0.7853982rad))", 100.0),
                ("calc(100px * sin(pi / 6))", 50.0),
                ("calc(atan2(1px, 1px) / 1deg * 1px)", 45.0),
                ("calc(atan(1) / 1deg * 1px)", 45.0),
                ("calc(pow(2, 3) * 10px)", 80.0),
                ("calc(log(8, 2) * 10px)", 30.0),
                ("calc(sqrt(16) * 10px)", 40.0),
                ("calc(exp(0) * 10px)", 10.0),
                // acos(2) is NaN, so the whole calculation is 0
                ("calc(acos(2) / 1deg * 1px)", 0.0),
            ];
            for (input, expected) in cases {
                let calc = parse(input);
                assert_eq!(calc.validate(), Ok(()), "{input}");
                let v = calc.resolve(400.0);
                assert!((v - expected).abs() < 1e-3, "{input}: {v} != {expected}");
            }
            for (input, error) in [
                ("calc(sqrt(4px) * 1px)", CalcError::InvalidArgument),
                ("calc(sin(10px) * 1px)", CalcError::InvalidArgument),
                ("calc(asin(30deg) * 1px)", CalcError::InvalidArgument),
                ("calc(atan2(1px, 1) * 1px)", CalcError::TypeMismatch),
                ("calc(10px * 1deg)", CalcError::NotALength),
                ("calc(atan(1) * 1px)", CalcError::NotALength),
            ] {
                assert_eq!(parse(input).validate(), Err(error), "{input}");
            }
        }

        #[cfg(not(feature = "std"))]
        #[test]
        fn the_transcendental_functions_need_std() {
            for input in ["hypot(30%, 40px)", "calc(100px * sin(30deg))", "calc(pow(2, 3) * 10px)"] {
                assert_eq!(parse(input).validate(), Err(CalcError::Unsupported), "{input}");
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
            for input in ["calc(10px + 5)", "calc(10px * 5px)", "round(17px)", "calc(5)", "calc(10deg)"] {
                assert!(Calc::from_str(input).map(|c| c.validate().is_err()).unwrap_or(true), "{input}");
            }
        }
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
