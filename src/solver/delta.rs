//! δ-rationals: values `c + kδ` for an infinitesimal positive δ, so that a
//! strict bound `x < c` is the non-strict bound `x <= c - δ`. Ordering is
//! lexicographic on `(c, k)`.

use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

use crate::Number;

/// `c + k·δ`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Delta {
    pub c: Number,
    pub k: Number,
}

impl Delta {
    pub fn new(c: Number, k: Number) -> Self {
        Delta { c, k }
    }

    /// A plain rational.
    pub fn exact(c: Number) -> Self {
        Delta { c, k: Number::zero() }
    }

    pub fn zero() -> Self {
        Delta::exact(Number::zero())
    }

    /// True if there is no infinitesimal part.
    pub fn is_exact(&self) -> bool {
        self.k.is_zero()
    }

    /// Scale by a rational.
    pub fn scale(&self, a: &Number) -> Delta {
        Delta { c: &self.c * a, k: &self.k * a }
    }
}

impl From<Number> for Delta {
    fn from(c: Number) -> Self {
        Delta::exact(c)
    }
}

impl<'a> Add<&'a Delta> for &'a Delta {
    type Output = Delta;
    fn add(self, rhs: &Delta) -> Delta {
        Delta { c: &self.c + &rhs.c, k: &self.k + &rhs.k }
    }
}
impl<'a> Sub<&'a Delta> for &'a Delta {
    type Output = Delta;
    fn sub(self, rhs: &Delta) -> Delta {
        Delta { c: &self.c - &rhs.c, k: &self.k - &rhs.k }
    }
}
impl<'a> Mul<&'a Number> for &'a Delta {
    type Output = Delta;
    fn mul(self, rhs: &Number) -> Delta {
        self.scale(rhs)
    }
}
impl Neg for &Delta {
    type Output = Delta;
    fn neg(self) -> Delta {
        Delta { c: -&self.c, k: -&self.k }
    }
}

impl fmt::Display for Delta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.k.is_zero() {
            write!(f, "{}", self.c)
        } else if self.k.is_positive() {
            write!(f, "{} + {}δ", self.c, self.k)
        } else {
            write!(f, "{} - {}δ", self.c, self.k.abs())
        }
    }
}
