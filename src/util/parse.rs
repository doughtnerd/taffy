//! Helpers for parsing style types from strings

use cssparser::BasicParseError;
pub(crate) use cssparser::{Parser, Token};
use std::borrow::Cow;

/// Error type for parsing a type from `cssparser::Parser`
pub(crate) type CssParseError<'i> = cssparser::ParseError<Cow<'i, str>>;

/// Result type for parsing a type from `cssparser::Parser`
pub(crate) type CssParseResult<'i, T> = Result<T, CssParseError<'i>>;

/// Error type for parsing a type from string
#[derive(Clone, Debug)]
pub struct ParseError(String);

impl core::fmt::Display for ParseError {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(fmt, "{}", self.0)
    }
}
impl<'i> From<CssParseError<'i>> for ParseError {
    fn from(value: CssParseError<'i>) -> Self {
        Self(value.to_string())
    }
}
impl From<BasicParseError> for ParseError {
    fn from(value: BasicParseError) -> Self {
        Self(value.kind.to_string())
    }
}
#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

/// Result type for parsing a type from string
pub type ParseResult<T> = Result<T, ParseError>;

/// What parsing needs beyond its input: where `calc()` expressions that don't simplify to a plain value are stored.
/// Without a store (`FromStr`), such an expression is a parse error.
pub(crate) struct ParseContext<'a> {
    /// The store for `calc()` expressions, if any
    #[cfg(feature = "calc")]
    pub(crate) calcs: Option<&'a mut crate::style::CalcStore>,
    /// Ties the lifetime when `calc` is disabled
    pub(crate) _marker: core::marker::PhantomData<&'a mut ()>,
}

impl<'a> ParseContext<'a> {
    /// A context without a `calc()` store
    pub(crate) fn none() -> Self {
        Self {
            #[cfg(feature = "calc")]
            calcs: None,
            _marker: core::marker::PhantomData,
        }
    }

    /// A context that stores `calc()` expressions in `calcs`
    #[cfg(feature = "calc")]
    pub(crate) fn with_calcs(calcs: &'a mut crate::style::CalcStore) -> Self {
        Self { calcs: Some(calcs), _marker: core::marker::PhantomData }
    }
}

/// Trait for parsing a type from a `cssparser::Parser` input
pub(crate) trait FromCss: Sized {
    /// Parse type from a `cssparser::Parser` input
    fn from_css<'i>(parser: &mut Parser<'i>, cx: &mut ParseContext<'_>) -> CssParseResult<'i, Self>;
}

/// Parse a string into a type that implements `FromCss`, ensuring that the
/// entire input string is consumed.
pub(crate) fn parse_css_str_entirely<T: FromCss>(input: &str, cx: &mut ParseContext<'_>) -> Result<T, ParseError> {
    let mut parser = Parser::new(input);
    parser.parse_entirely(|parser| T::from_css(parser, cx)).map_err(|err| ParseError(err.to_string()))
}

/// A style value type that can be parsed with a [`CalcStore`](crate::CalcStore), so that it can hold a `calc()` that
/// doesn't simplify to a plain value: see [`CalcStore::parse_value`](crate::CalcStore::parse_value) and
/// [`TaffyTree::parse_value`](crate::TaffyTree::parse_value). (`FromStr` parses the same syntax without a store, and
/// fails on such a `calc()`.)
#[cfg(feature = "calc")]
pub trait ParseWithCalc: Sized {
    /// Parses `input`, storing any `calc()` expression that needs a handle in `calcs`
    fn parse_with_calc(input: &str, calcs: &mut crate::style::CalcStore) -> ParseResult<Self>;
}

/// Implements [`ParseWithCalc`] for types that implement `FromCss`
macro_rules! parse_with_calc_from_css {
    ($($(#[$meta:meta])* $ty:ty),+ $(,)?) => {
        $(
            $(#[$meta])*
            #[cfg(feature = "calc")]
            impl $crate::util::parse::ParseWithCalc for $ty {
                fn parse_with_calc(input: &str, calcs: &mut $crate::style::CalcStore) -> $crate::util::parse::ParseResult<Self> {
                    $crate::util::parse::parse_css_str_entirely(input, &mut $crate::util::parse::ParseContext::with_calcs(calcs))
                }
            }
        )+
    };
}
pub(crate) use parse_with_calc_from_css;

/// Automatically implement `FromStr` for a type that already implemented `FromCss`
macro_rules! from_str_from_css {
    ($ty:ident) => {
        #[cfg(feature = "parse")]
        impl core::str::FromStr for $ty {
            type Err = $crate::ParseError;
            fn from_str(input: &str) -> Result<Self, Self::Err> {
                $crate::util::parse::parse_css_str_entirely(input, &mut $crate::util::parse::ParseContext::none())
            }
        }
    };
}
pub(crate) use from_str_from_css;

/// Implements the `FromCss` and `FromStr` traits for a simple enum that consists of just keywords
macro_rules! impl_parse_for_keyword_enum {
    (
        // The type name (followed by a comma)
        $ty:ident,

        // Repeat 1-or-more times
        $(
            // 0-or-more metadata attributes (e.g. #[cfg] attributes for conditional compilation)
            $( #[$meta: meta] )*

            // keyword => enum_variant (e.g. "center" => AlignItems::Center)
            $keyword:literal => $enum_variant:ident,
        )+
    ) => {

        // Impl FromCss for the type
        impl $crate::util::parse::FromCss for $ty {
            fn from_css<'i>(input: &mut cssparser::Parser<'i>, _cx: &mut $crate::util::parse::ParseContext<'_>) -> $crate::util::parse::CssParseResult<'i, Self> {

                // Parse an ident (or return an error)
                let ident = input.expect_ident()?;

                // Match on the ident (case insensitive)
                cssparser::match_ignore_ascii_case! { &*ident,

                    // Define each item in the macro definition as a match case
                    $(
                        // Add the metadata attributes to the match case
                        $( #[$meta] )*
                        // If the keyword matches, return the corresponding variant of the enum
                        $keyword => return Ok(Self::$enum_variant),
                    )+

                    // If none of the cases match the input, return an "unexpected token" error
                    _ => Err($crate::util::parse::CssParseError::unexpected_token()),
                }
            }
        }

        // Impl FromStr for the type
        impl core::str::FromStr for $ty {
            type Err = $crate::ParseError;
            fn from_str(input: &str) -> Result<Self, Self::Err> {
                $crate::util::parse::parse_css_str_entirely(input, &mut $crate::util::parse::ParseContext::none())
            }
        }
    };
}
pub(crate) use impl_parse_for_keyword_enum;
