// SPDX-License-Identifier: Apache-2.0

//! Sparse linear expressions `Σ aᵢ·xᵢ + c` with exact rational coefficients.

use std::collections::BTreeMap;
use std::fmt;

use crate::Number;

use super::simplex::SVar;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LinExpr {
    /// Non-zero coefficients, keyed by variable.
    pub terms: BTreeMap<SVar, Number>,
    /// Constant part.
    pub constant: Number,
}

impl LinExpr {
    pub fn constant(c: Number) -> Self {
        LinExpr { terms: BTreeMap::new(), constant: c }
    }

    pub fn var(v: SVar) -> Self {
        let mut terms = BTreeMap::new();
        terms.insert(v, Number::one());
        LinExpr { terms, constant: Number::zero() }
    }

    pub fn is_constant(&self) -> bool {
        self.terms.is_empty()
    }

    /// The constant value if the expression has no variables.
    pub fn as_constant(&self) -> Option<&Number> {
        if self.terms.is_empty() { Some(&self.constant) } else { None }
    }

    /// If the expression is exactly `1·x + 0`, that variable.
    pub fn as_var(&self) -> Option<SVar> {
        if self.terms.len() == 1 && self.constant.is_zero() {
            let (v, a) = self.terms.iter().next().unwrap();
            if *a == Number::one() {
                return Some(*v);
            }
        }
        None
    }

    pub fn coeff(&self, v: SVar) -> Option<&Number> {
        self.terms.get(&v)
    }

    /// `self += a·x`, dropping the term if it cancels.
    pub fn add_term(&mut self, v: SVar, a: &Number) {
        if a.is_zero() {
            return;
        }
        let entry = self.terms.entry(v).or_insert_with(Number::zero);
        *entry += a;
        if entry.is_zero() {
            self.terms.remove(&v);
        }
    }

    /// `self += k·other`.
    pub fn add_scaled(&mut self, other: &LinExpr, k: &Number) {
        if k.is_zero() {
            return;
        }
        for (v, a) in &other.terms {
            self.add_term(*v, &(a * k));
        }
        self.constant += &(&other.constant * k);
    }

    pub fn add(&mut self, other: &LinExpr) {
        self.add_scaled(other, &Number::one());
    }

    pub fn sub(&mut self, other: &LinExpr) {
        self.add_scaled(other, &-Number::one());
    }

    pub fn scale(&mut self, k: &Number) {
        if k.is_zero() {
            self.terms.clear();
            self.constant = Number::zero();
            return;
        }
        for a in self.terms.values_mut() {
            *a *= k;
        }
        self.constant *= k;
    }

    pub fn negate(&mut self) {
        self.scale(&-Number::one());
    }

    /// Remove and return the coefficient of `v`.
    pub fn take(&mut self, v: SVar) -> Option<Number> {
        self.terms.remove(&v)
    }
}

impl fmt::Display for LinExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (v, a) in &self.terms {
            if first {
                if a.is_negative() {
                    f.write_str("-")?;
                }
            } else if a.is_negative() {
                f.write_str(" - ")?;
            } else {
                f.write_str(" + ")?;
            }
            first = false;
            let m = a.abs();
            if m != Number::one() {
                write!(f, "{m}*")?;
            }
            write!(f, "{v}")?;
        }
        if first {
            write!(f, "{}", self.constant)
        } else if self.constant.is_positive() {
            write!(f, " + {}", self.constant)
        } else if self.constant.is_negative() {
            write!(f, " - {}", self.constant.abs())
        } else {
            Ok(())
        }
    }
}
