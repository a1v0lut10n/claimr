//! Exact numbers: arbitrary-precision rationals.
//!
//! Every number in Claimr is an exact rational; integers are the
//! denominator-1 case. There is deliberately no floating-point type and no
//! conversion to or from `f64` — see the `exact-arithmetic` aspect and
//! `docs/design/2026-08-17-exact-rational-arithmetic.md`. The bignum backend
//! (`num-rational` over `num-bigint`) is an implementation detail behind this
//! newtype.

use std::fmt;
use std::str::FromStr;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Zero};

/// An exact rational number of arbitrary precision.
///
/// Always kept in lowest terms with a positive denominator, so structural
/// equality is numeric equality: `0.5 == 1/2`.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Number(BigRational);

/// Error converting literal text into a [`Number`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid number literal {0:?}: expected digits with an optional fractional part")]
pub struct ParseNumberError(pub String);

impl Number {
    /// Zero.
    pub fn zero() -> Self {
        Number(BigRational::zero())
    }

    /// One.
    pub fn one() -> Self {
        Number(BigRational::one())
    }

    /// The rational `numer / denom`, or `None` if `denom` is zero.
    pub fn from_ratio(numer: impl Into<BigInt>, denom: impl Into<BigInt>) -> Option<Self> {
        let denom = denom.into();
        if denom.is_zero() {
            None
        } else {
            Some(Number(BigRational::new(numer.into(), denom)))
        }
    }

    /// Convert literal text to its exact value.
    ///
    /// Accepts what the grammar's `number` terminal produces — `digits`
    /// optionally followed by `.digits` — with an optional leading `-`.
    /// The decimal fraction is exact: `"18.5"` is 37/2, `"0.10"` is 1/10.
    pub fn from_literal(text: &str) -> Result<Self, ParseNumberError> {
        let err = || ParseNumberError(text.to_string());
        let (negative, body) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (int_part, frac_part) = match body.split_once('.') {
            Some((i, f)) => (i, f),
            None => (body, ""),
        };
        let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
        if !all_digits(int_part) || (body.contains('.') && !all_digits(frac_part)) {
            return Err(err());
        }
        // value = (int_part ++ frac_part) / 10^len(frac_part), reduced by BigRational::new.
        let digits = format!("{int_part}{frac_part}");
        let numer: BigInt = digits.parse().map_err(|_| err())?;
        let denom = BigInt::from(10u8).pow(frac_part.len() as u32);
        let value = BigRational::new(numer, denom);
        Ok(Number(if negative { -value } else { value }))
    }

    /// True if the denominator is one.
    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    /// True if the value is zero.
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Numerator in lowest terms (carries the sign).
    pub fn numer(&self) -> &BigInt {
        self.0.numer()
    }

    /// Denominator in lowest terms (always positive).
    pub fn denom(&self) -> &BigInt {
        self.0.denom()
    }
}

impl fmt::Display for Number {
    /// Integers print plainly (`7`); other values as `numer/denom` in lowest
    /// terms (`33/32`), as Prolog III printed them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer() {
            write!(f, "{}", self.numer())
        } else {
            write!(f, "{}/{}", self.numer(), self.denom())
        }
    }
}

impl fmt::Debug for Number {
    /// Same as `Display`, so `Expr::Number(37/2)` reads well in AST dumps.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl FromStr for Number {
    type Err = ParseNumberError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Number::from_literal(s)
    }
}

macro_rules! impl_from_int {
    ($($t:ty),*) => {$(
        impl From<$t> for Number {
            fn from(v: $t) -> Self {
                Number(BigRational::from_integer(BigInt::from(v)))
            }
        }
    )*};
}
impl_from_int!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize);

impl From<BigInt> for Number {
    fn from(v: BigInt) -> Self {
        Number(BigRational::from_integer(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> Number {
        Number::from_literal(s).unwrap()
    }

    #[test]
    fn integers() {
        assert_eq!(n("7"), Number::from(7));
        assert_eq!(n("007"), Number::from(7));
        assert_eq!(n("0"), Number::zero());
        assert!(n("42").is_integer());
        assert_eq!(n("42").to_string(), "42");
    }

    #[test]
    fn decimals_are_exact() {
        assert_eq!(n("18.5"), Number::from_ratio(37, 2).unwrap());
        assert_eq!(n("0.10"), Number::from_ratio(1, 10).unwrap());
        assert_eq!(n("0.5"), Number::from_ratio(1, 2).unwrap());
        assert_eq!(n("2.0"), Number::from(2));
        assert!(n("2.0").is_integer());
        assert!(!n("2.5").is_integer());
        // 0.1 + 0.2 == 0.3 exactly (this is the point).
        let sum = Number(n("0.1").0 + n("0.2").0);
        assert_eq!(sum, n("0.3"));
    }

    #[test]
    fn negative_literals() {
        assert_eq!(n("-3"), Number::from(-3));
        assert_eq!(n("-0.25"), Number::from_ratio(-1, 4).unwrap());
        assert_eq!(n("-0.25").to_string(), "-1/4");
    }

    #[test]
    fn display_and_debug() {
        assert_eq!(n("33").to_string(), "33");
        assert_eq!(Number::from_ratio(33, 32).unwrap().to_string(), "33/32");
        assert_eq!(Number::from_ratio(6, 4).unwrap().to_string(), "3/2");
        assert_eq!(format!("{:?}", n("18.5")), "37/2");
        assert_eq!(format!("{:?}", n("7")), "7");
    }

    #[test]
    fn ordering_and_equality() {
        assert!(n("1.5") < n("2"));
        assert!(n("0.333") < Number::from_ratio(1, 3).unwrap());
        assert_eq!(n("1.50"), n("1.5"));
        assert_ne!(n("1.5"), n("1.51"));
        assert_eq!(n("3.0"), Number::from_ratio(3, 1).unwrap());
    }

    #[test]
    fn arbitrary_precision() {
        let big = "123456789012345678901234567890.987654321098765432109876543210";
        let v = n(big);
        assert_eq!(v.numer().to_string(), "12345678901234567890123456789098765432109876543210987654321");
        assert_eq!(v.denom().to_string(), "100000000000000000000000000000");
    }

    #[test]
    fn rejects_malformed_literals() {
        for bad in ["", ".", "1.", ".5", "1.2.3", "abc", "1e5", "--1", "1/2", " 1"] {
            assert!(Number::from_literal(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(Number::from_ratio(1, 0).is_none());
    }

    #[test]
    fn from_str_roundtrip() {
        let v: Number = "2.75".parse().unwrap();
        assert_eq!(v, Number::from_ratio(11, 4).unwrap());
    }
}
