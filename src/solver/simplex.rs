// SPDX-License-Identifier: Apache-2.0

//! General simplex with bounds (Dutertre & de Moura, CAV 2006), exact.
//!
//! State: every variable has a current value (a δ-rational) and optional
//! lower/upper bounds; basic variables are defined by a row `xᵢ = Σ aⱼ·xⱼ + c`
//! over non-basic variables. Invariants: values satisfy every row, and every
//! *non-basic* variable satisfies its bounds. `check` repairs basic
//! violations by Bland-rule pivoting or reports infeasibility. Backtracking
//! restores bounds **and** the variable and row set (CLM-0018,
//! `docs/design/2026-11-09-truncate-the-tableau-on-undo.md`): variables
//! created since a mark are pivoted out of the surviving rows and dropped
//! with their definitions, so their indices are reused.

use std::fmt;

use crate::Number;

use super::delta::Delta;
use super::linexpr::LinExpr;

/// A solver variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SVar(pub usize);

impl fmt::Display for SVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "_s{}", self.0)
    }
}

/// Numeric relations the solver accepts, all as `expr op 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOp {
    Lt,
    Le,
    Eq,
    Ge,
    Gt,
}

/// A bound to assert on a variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    Lower(Delta),
    Upper(Delta),
}

#[derive(Debug, Clone)]
struct VarState {
    lower: Option<Delta>,
    upper: Option<Delta>,
    value: Delta,
}

#[derive(Debug, Clone)]
struct Row {
    basic: SVar,
    expr: LinExpr, // over non-basic variables, may carry a constant
}

#[derive(Debug, Clone)]
struct BoundUndo {
    var: SVar,
    lower: Option<Delta>,
    upper: Option<Delta>,
}

/// A point in the solver's history: the bound trail and the variable count.
#[derive(Debug, Clone, Copy)]
pub struct Mark {
    trail: usize,
    vars: usize,
}

#[derive(Debug, Default, Clone)]
pub struct Simplex {
    vars: Vec<VarState>,
    rows: Vec<Row>,
    /// Basic variable → its row.
    row_of: Vec<Option<usize>>,
    trail: Vec<BoundUndo>,
}

impl Simplex {
    pub fn new() -> Self {
        Self::default()
    }

    // --- variables and rows -----------------------------------------------

    pub fn new_var(&mut self) -> SVar {
        self.vars.push(VarState {
            lower: None,
            upper: None,
            value: Delta::zero(),
        });
        self.row_of.push(None);
        SVar(self.vars.len() - 1)
    }

    pub fn is_basic(&self, v: SVar) -> bool {
        self.row_of[v.0].is_some()
    }

    /// Current value.
    pub fn value(&self, v: SVar) -> &Delta {
        &self.vars[v.0].value
    }

    pub fn lower(&self, v: SVar) -> Option<&Delta> {
        self.vars[v.0].lower.as_ref()
    }

    pub fn upper(&self, v: SVar) -> Option<&Delta> {
        self.vars[v.0].upper.as_ref()
    }

    /// The row defining a basic variable, if it is basic.
    pub fn row(&self, v: SVar) -> Option<&LinExpr> {
        self.row_of[v.0].map(|i| &self.rows[i].expr)
    }

    pub fn num_vars(&self) -> usize {
        self.vars.len()
    }

    /// Rewrite `expr` over non-basic variables only.
    fn substitute(&self, expr: &LinExpr) -> LinExpr {
        let mut out = LinExpr::constant(expr.constant.clone());
        for (v, a) in &expr.terms {
            match self.row_of[v.0] {
                Some(i) => out.add_scaled(&self.rows[i].expr, a),
                None => out.add_term(*v, a),
            }
        }
        out
    }

    fn eval(&self, expr: &LinExpr) -> Delta {
        let mut val = Delta::exact(expr.constant.clone());
        for (v, a) in &expr.terms {
            val = &val + &self.vars[v.0].value.scale(a);
        }
        val
    }

    /// Define a fresh variable `v` (no bounds, non-basic, in no row) as `expr`.
    pub fn define(&mut self, v: SVar, expr: &LinExpr) {
        debug_assert!(self.row_of[v.0].is_none());
        debug_assert!(self.vars[v.0].lower.is_none() && self.vars[v.0].upper.is_none());
        let expr = self.substitute(expr);
        debug_assert!(
            !expr.terms.contains_key(&v),
            "definition must not be self-referential"
        );
        // v must not occur in any other row (it is fresh), so no substitution needed.
        self.vars[v.0].value = self.eval(&expr);
        self.rows.push(Row { basic: v, expr });
        self.row_of[v.0] = Some(self.rows.len() - 1);
    }

    /// A slack variable defined as `expr`.
    pub fn slack(&mut self, expr: &LinExpr) -> SVar {
        let s = self.new_var();
        self.define(s, expr);
        s
    }

    // --- marks and undo ----------------------------------------------------

    pub fn mark(&self) -> Mark {
        Mark {
            trail: self.trail.len(),
            vars: self.vars.len(),
        }
    }

    /// Restore the solver to `mark`: bounds of the variables that existed
    /// then, and the tableau truncated to those variables (see
    /// [`Simplex::truncate`]). Values stay a solution of the rows.
    pub fn undo_to(&mut self, mark: Mark) {
        while self.trail.len() > mark.trail {
            let Some(u) = self.trail.pop() else { break };
            if u.var.0 < mark.vars {
                let st = &mut self.vars[u.var.0];
                st.lower = u.lower;
                st.upper = u.upper;
            }
        }
        if self.vars.len() > mark.vars {
            self.truncate(mark.vars);
        }
    }

    /// True if any bound changed, or any variable was created, since `mark`.
    pub fn changed_since(&self, mark: Mark) -> bool {
        self.trail.len() > mark.trail || self.vars.len() > mark.vars
    }

    /// Remove every variable `>= n` and every row that defines one.
    ///
    /// Rows are an equivalent form of the equations posted so far, and each
    /// equation posted since the mark defined a then-fresh variable, so the
    /// equations over the surviving variables are exactly the pre-mark
    /// system. Pivots since the mark may have written post-mark variables
    /// into pre-mark rows; pivoting each such variable back into basic
    /// position (into one of those rows, whose pre-mark basic becomes
    /// non-basic) leaves every surviving row over surviving variables, and
    /// the rows whose basic is a post-mark variable are then its definitions
    /// alone and are dropped. Pivoting does not touch values, so they remain
    /// a solution of the rows; a surviving variable turned non-basic may sit
    /// outside its restored bounds, which `update` and `check` repair as
    /// after any assertion.
    fn truncate(&mut self, n: usize) {
        loop {
            let found = self.rows.iter().enumerate().find_map(|(ri, row)| {
                if row.basic.0 >= n {
                    return None;
                }
                row.expr.terms.keys().find(|v| v.0 >= n).map(|v| (ri, *v))
            });
            let Some((ri, xj)) = found else { break };
            self.pivot(ri, xj);
        }
        self.rows.retain(|r| r.basic.0 < n);
        self.vars.truncate(n);
        self.row_of.truncate(n);
        for slot in self.row_of.iter_mut() {
            *slot = None;
        }
        for i in 0..self.rows.len() {
            let b = self.rows[i].basic.0;
            self.row_of[b] = Some(i);
        }
        for v in 0..n {
            if self.row_of[v].is_some() {
                continue;
            }
            let st = &self.vars[v];
            let target = match (&st.lower, &st.upper) {
                (Some(l), _) if st.value < *l => Some(l.clone()),
                (_, Some(u)) if st.value > *u => Some(u.clone()),
                _ => None,
            };
            if let Some(t) = target {
                self.update(SVar(v), t);
            }
        }
        let feasible = self.check();
        debug_assert!(feasible, "the pre-mark system was feasible");
    }

    fn save(&mut self, v: SVar) {
        let st = &self.vars[v.0];
        self.trail.push(BoundUndo {
            var: v,
            lower: st.lower.clone(),
            upper: st.upper.clone(),
        });
    }

    // --- assertions --------------------------------------------------------

    fn update(&mut self, x: SVar, v: Delta) {
        let diff = &v - &self.vars[x.0].value;
        for row in &self.rows {
            if let Some(a) = row.expr.coeff(x) {
                let b = row.basic.0;
                self.vars[b].value = &self.vars[b].value + &diff.scale(a);
            }
        }
        self.vars[x.0].value = v;
    }

    /// Assert `x >= l`. Returns feasibility; on `false` the caller undoes.
    pub fn assert_lower(&mut self, x: SVar, l: Delta) -> bool {
        if let Some(u) = &self.vars[x.0].upper {
            if l > *u {
                return false;
            }
        }
        if let Some(cur) = &self.vars[x.0].lower {
            if l <= *cur {
                return true;
            }
        }
        self.save(x);
        self.vars[x.0].lower = Some(l.clone());
        if !self.is_basic(x) && self.vars[x.0].value < l {
            self.update(x, l);
        }
        self.check()
    }

    /// Assert `x <= u`. Returns feasibility; on `false` the caller undoes.
    pub fn assert_upper(&mut self, x: SVar, u: Delta) -> bool {
        if let Some(l) = &self.vars[x.0].lower {
            if u < *l {
                return false;
            }
        }
        if let Some(cur) = &self.vars[x.0].upper {
            if u >= *cur {
                return true;
            }
        }
        self.save(x);
        self.vars[x.0].upper = Some(u.clone());
        if !self.is_basic(x) && self.vars[x.0].value > u {
            self.update(x, u);
        }
        self.check()
    }

    pub fn assert_bound(&mut self, x: SVar, b: Bound) -> bool {
        match b {
            Bound::Lower(l) => self.assert_lower(x, l),
            Bound::Upper(u) => self.assert_upper(x, u),
        }
    }

    /// Assert `x = c`.
    pub fn fix(&mut self, x: SVar, c: Number) -> bool {
        let d = Delta::exact(c);
        self.assert_lower(x, d.clone()) && self.assert_upper(x, d)
    }

    /// Assert `expr op 0`.
    pub fn assert_constraint(&mut self, expr: &LinExpr, op: RelOp) -> bool {
        // Σ a·x + c op 0  ⇔  Σ a·x op −c
        let rhs = -&expr.constant;
        let mut lhs = expr.clone();
        lhs.constant = Number::zero();
        if lhs.is_constant() {
            let z = Number::zero();
            return match op {
                RelOp::Lt => z < rhs,
                RelOp::Le => z <= rhs,
                RelOp::Eq => z == rhs,
                RelOp::Ge => z >= rhs,
                RelOp::Gt => z > rhs,
            };
        }
        // Single term a·x: bound on x directly (flip for negative a).
        let (var, op, rhs) = if lhs.terms.len() == 1 {
            let (v, a) = lhs.terms.iter().next().unwrap();
            let (v, a) = (*v, a.clone());
            let rhs = &rhs / &a;
            let op = if a.is_negative() { op.flip() } else { op };
            (v, op, rhs)
        } else {
            (self.slack(&lhs), op, rhs)
        };
        match op {
            RelOp::Lt => self.assert_upper(var, Delta::new(rhs, -Number::one())),
            RelOp::Le => self.assert_upper(var, Delta::exact(rhs)),
            RelOp::Eq => self.fix(var, rhs),
            RelOp::Ge => self.assert_lower(var, Delta::exact(rhs)),
            RelOp::Gt => self.assert_lower(var, Delta::new(rhs, Number::one())),
        }
    }

    // --- the core ----------------------------------------------------------

    /// Repair basic-variable bound violations by pivoting (Bland's rule).
    /// True if a feasible assignment was reached.
    pub fn check(&mut self) -> bool {
        loop {
            // Smallest-index basic variable violating a bound.
            let mut violated: Option<(SVar, bool)> = None; // (var, below_lower)
            for row in &self.rows {
                let st = &self.vars[row.basic.0];
                let below = st.lower.as_ref().is_some_and(|l| st.value < *l);
                let above = st.upper.as_ref().is_some_and(|u| st.value > *u);
                if below || above {
                    match violated {
                        Some((v, _)) if v <= row.basic => {}
                        _ => violated = Some((row.basic, below)),
                    }
                }
            }
            let Some((xi, below)) = violated else {
                return true;
            };
            let ri = self.row_of[xi.0].unwrap();
            // Smallest-index non-basic variable that can move xi toward its bound.
            let mut pivot: Option<SVar> = None;
            for (xj, a) in &self.rows[ri].expr.terms {
                let st = &self.vars[xj.0];
                let can_increase = st.upper.as_ref().is_none_or(|u| st.value < *u);
                let can_decrease = st.lower.as_ref().is_none_or(|l| st.value > *l);
                let ok = if below {
                    (a.is_positive() && can_increase) || (a.is_negative() && can_decrease)
                } else {
                    (a.is_positive() && can_decrease) || (a.is_negative() && can_increase)
                };
                if ok {
                    pivot = Some(*xj);
                    break; // BTreeMap iterates in increasing SVar order
                }
            }
            let Some(xj) = pivot else { return false };
            let target = if below {
                self.vars[xi.0].lower.clone().unwrap()
            } else {
                self.vars[xi.0].upper.clone().unwrap()
            };
            self.pivot_and_update(xi, xj, target);
        }
    }

    fn pivot_and_update(&mut self, xi: SVar, xj: SVar, v: Delta) {
        let ri = self.row_of[xi.0].unwrap();
        let a = self.rows[ri].expr.coeff(xj).unwrap().clone();
        let theta = (&v - &self.vars[xi.0].value).scale(&a.recip().unwrap());
        self.vars[xi.0].value = v;
        self.vars[xj.0].value = &self.vars[xj.0].value + &theta;
        for (k, row) in self.rows.iter().enumerate() {
            if k == ri {
                continue;
            }
            if let Some(akj) = row.expr.coeff(xj) {
                let b = row.basic.0;
                self.vars[b].value = &self.vars[b].value + &theta.scale(akj);
            }
        }
        self.pivot(ri, xj);
    }

    /// Make `xj` basic in row `ri` (whose basic variable becomes non-basic).
    fn pivot(&mut self, ri: usize, xj: SVar) {
        let xi = self.rows[ri].basic;
        let mut expr = self.rows[ri].expr.clone();
        let a = expr.take(xj).unwrap();
        // xi = a·xj + rest  ⇒  xj = (xi − rest) / a
        let inv = a.recip().unwrap();
        expr.negate();
        expr.add_term(xi, &Number::one());
        expr.scale(&inv);
        self.rows[ri] = Row {
            basic: xj,
            expr: expr.clone(),
        };
        self.row_of[xi.0] = None;
        self.row_of[xj.0] = Some(ri);
        for (k, row) in self.rows.iter_mut().enumerate() {
            if k == ri {
                continue;
            }
            if let Some(akj) = row.expr.take(xj) {
                row.expr.add_scaled(&expr, &akj);
            }
        }
    }

    // --- queries -----------------------------------------------------------

    /// If `x` can take only one value in the current store, that value.
    /// Exact: probes both strict sides. Leaves the store as it was.
    pub fn is_determined(&mut self, x: SVar) -> Option<Number> {
        if !self.check() {
            return None;
        }
        // Cheap case: coinciding bounds.
        if let (Some(l), Some(u)) = (&self.vars[x.0].lower, &self.vars[x.0].upper) {
            if l == u {
                return if l.is_exact() {
                    Some(l.c.clone())
                } else {
                    None
                };
            }
        }
        let v = self.vars[x.0].value.clone();
        let mark = self.mark();
        let below = self.assert_upper(x, Delta::new(v.c.clone(), &v.k - &Number::one()));
        self.undo_to(mark);
        if below {
            self.check();
            return None;
        }
        let above = self.assert_lower(x, Delta::new(v.c.clone(), &v.k + &Number::one()));
        self.undo_to(mark);
        self.check();
        if above {
            return None;
        }
        // Fixed. A fixed value is a genuine rational.
        debug_assert!(v.is_exact(), "fixed value with infinitesimal part: {v}");
        Some(v.c)
    }
}

impl RelOp {
    /// The relation with both sides negated (`a < b` ⇔ `-a > -b`).
    pub fn flip(self) -> RelOp {
        match self {
            RelOp::Lt => RelOp::Gt,
            RelOp::Le => RelOp::Ge,
            RelOp::Eq => RelOp::Eq,
            RelOp::Ge => RelOp::Le,
            RelOp::Gt => RelOp::Lt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> Number {
        Number::from_literal(s).unwrap()
    }
    fn lin(terms: &[(SVar, &str)], c: &str) -> LinExpr {
        let mut e = LinExpr::constant(n(c));
        for (v, a) in terms {
            e.add_term(*v, &n(a));
        }
        e
    }
    fn neg(s: &str) -> Number {
        -n(s)
    }

    #[test]
    fn two_equations_determine_both_variables() {
        // X + Y = 10, X - Y = 2  =>  X = 6, Y = 4
        let mut s = Simplex::new();
        let x = s.new_var();
        let y = s.new_var();
        let mut e1 = lin(&[(x, "1"), (y, "1")], "0");
        e1.constant = neg("10");
        assert!(s.assert_constraint(&e1, RelOp::Eq));
        let mut e2 = lin(&[(x, "1")], "0");
        e2.add_term(y, &neg("1"));
        e2.constant = neg("2");
        assert!(s.assert_constraint(&e2, RelOp::Eq));
        assert_eq!(s.is_determined(x), Some(n("6")));
        assert_eq!(s.is_determined(y), Some(n("4")));
    }

    #[test]
    fn coinciding_bounds_and_infeasibility() {
        let mut s = Simplex::new();
        let x = s.new_var();
        assert!(s.assert_lower(x, Delta::exact(n("3"))));
        assert_eq!(s.is_determined(x), None);
        assert!(s.assert_upper(x, Delta::exact(n("3"))));
        assert_eq!(s.is_determined(x), Some(n("3")));
        let m = s.mark();
        // X > 3 with X = 3 is infeasible; strict via delta.
        assert!(!s.assert_lower(x, Delta::new(n("3"), n("1"))));
        s.undo_to(m);
        assert_eq!(s.is_determined(x), Some(n("3")));
    }

    #[test]
    fn strict_inequalities() {
        let mut s = Simplex::new();
        let x = s.new_var();
        let e = lin(&[(x, "1")], "-3"); // x - 3
        assert!(s.assert_constraint(&e, RelOp::Gt)); // x > 3
        let m = s.mark();
        assert!(!s.assert_constraint(&e, RelOp::Lt)); // x < 3
        s.undo_to(m);
        assert!(!s.assert_constraint(&e, RelOp::Le)); // x <= 3
        s.undo_to(m);
        assert!(s.assert_constraint(&e, RelOp::Ge)); // x >= 3 fine
        assert_eq!(s.is_determined(x), None);
    }

    #[test]
    fn implied_equality_through_rows() {
        // X - Y = 0, Y = 3  =>  X determined 3 though its own bounds are free.
        let mut s = Simplex::new();
        let x = s.new_var();
        let y = s.new_var();
        let mut e = lin(&[(x, "1")], "0");
        e.add_term(y, &neg("1"));
        assert!(s.assert_constraint(&e, RelOp::Eq));
        assert!(s.fix(y, n("3")));
        assert_eq!(s.is_determined(x), Some(n("3")));
        // And a difference variable D = X - Y is fixed at 0 without either fixed.
        let mut s2 = Simplex::new();
        let x = s2.new_var();
        let y = s2.new_var();
        let d = s2.new_var();
        let mut e = lin(&[(x, "1")], "0");
        e.add_term(y, &neg("1"));
        s2.define(d, &e);
        assert!(s2.fix(d, n("0")));
        assert_eq!(s2.is_determined(x), None);
        assert_eq!(s2.is_determined(d), Some(n("0")));
    }

    #[test]
    fn undo_restores_bounds_and_feasibility() {
        let mut s = Simplex::new();
        let x = s.new_var();
        let y = s.new_var();
        let sum = lin(&[(x, "1"), (y, "1")], "-10"); // x + y - 10
        assert!(s.assert_constraint(&sum, RelOp::Le)); // x + y <= 10
        let m = s.mark();
        assert!(s.assert_lower(x, Delta::exact(n("8"))));
        assert!(!s.assert_lower(y, Delta::exact(n("5")))); // 8 + 5 > 10
        s.undo_to(m);
        assert!(s.assert_lower(y, Delta::exact(n("5"))));
        assert!(s.assert_lower(x, Delta::exact(n("5"))));
        assert_eq!(s.is_determined(x), Some(n("5")));
        assert_eq!(s.is_determined(y), Some(n("5")));
    }

    #[test]
    fn exact_rationals_no_drift() {
        let mut s = Simplex::new();
        let x = s.new_var();
        // 3x = 1  =>  x = 1/3; then 3x - 1 = 0 consistent, x != 0.333 detectable
        let e = lin(&[(x, "3")], "-1");
        assert!(s.assert_constraint(&e, RelOp::Eq));
        assert_eq!(s.is_determined(x), Number::from_ratio(1, 3));
        let mut acc = LinExpr::constant(neg("0.3"));
        for _ in 0..3 {
            acc.add_term(x, &n("0.1"));
        }
        // 0.1x + 0.1x + 0.1x - 0.3 = 0.3x - 0.3 = 0.1 - 0.3 != 0 -> infeasible as Eq
        let m = s.mark();
        assert!(!s.assert_constraint(&acc, RelOp::Eq));
        s.undo_to(m);
    }

    #[test]
    fn bland_terminates_on_a_cycling_prone_instance() {
        // Beale's example (classic cycling for naive rules) as feasibility.
        let mut s = Simplex::new();
        let v: Vec<SVar> = (0..7).map(|_| s.new_var()).collect();
        for x in &v {
            assert!(s.assert_lower(*x, Delta::zero()));
        }
        let rows = [
            (
                vec![
                    (v[0], "0.25"),
                    (v[1], "-8"),
                    (v[2], "-1"),
                    (v[3], "9"),
                    (v[4], "1"),
                ],
                "0",
            ),
            (
                vec![
                    (v[0], "0.5"),
                    (v[1], "-12"),
                    (v[2], "-0.5"),
                    (v[3], "3"),
                    (v[5], "1"),
                ],
                "0",
            ),
            (vec![(v[2], "1"), (v[6], "1")], "-1"),
        ];
        for (terms, c) in rows {
            let e = lin(&terms, c);
            assert!(s.assert_constraint(&e, RelOp::Eq));
        }
        // Push the objective-like combination around; must terminate.
        let obj = lin(
            &[(v[0], "-0.75"), (v[1], "20"), (v[2], "-0.5"), (v[3], "6")],
            "0",
        );
        assert!(s.assert_constraint(&obj, RelOp::Le));
        let m = s.mark();
        let tight = lin(
            &[(v[0], "-0.75"), (v[1], "20"), (v[2], "-0.5"), (v[3], "6")],
            "0.5",
        );
        let _ = s.assert_constraint(&tight, RelOp::Le); // either way, must return
        s.undo_to(m);
        assert!(s.check());
    }

    // --- undo truncation (CLM-0018) ---------------------------------------

    fn d(i: i64) -> Delta {
        Delta::exact(Number::from_ratio(i, 1).unwrap())
    }

    /// Values satisfy every row, every basic variable has its row and no
    /// other, and no row mentions a variable that is not there.
    fn tableau_is_consistent(s: &Simplex) {
        assert_eq!(s.row_of.len(), s.vars.len());
        for (i, row) in s.rows.iter().enumerate() {
            assert_eq!(
                s.row_of[row.basic.0],
                Some(i),
                "basic {} owns row {i}",
                row.basic
            );
            for v in row.expr.terms.keys() {
                assert!(
                    v.0 < s.vars.len(),
                    "row {i} mentions a dropped variable {v}"
                );
                assert!(!s.is_basic(*v), "row {i} mentions a basic variable {v}");
            }
            assert_eq!(
                s.eval(&row.expr),
                s.vars[row.basic.0].value,
                "row {i} holds"
            );
        }
        let basics = s.row_of.iter().filter(|r| r.is_some()).count();
        assert_eq!(basics, s.rows.len());
    }

    #[test]
    fn undo_drops_the_variables_and_rows_made_since_the_mark() {
        let mut s = Simplex::new();
        let x = s.new_var();
        let y = s.new_var();
        // y = 2x, x >= 1: one row, pre-mark.
        let mut e = lin(&[(y, "1"), (x, "-2")], "0");
        assert!(s.assert_constraint(&e, RelOp::Eq));
        assert!(s.assert_lower(x, d(1)));
        let mark = s.mark();
        let (nv, nr) = (s.num_vars(), s.rows.len());
        // Post-mark: z = x + y, z <= 2 forces pivots that write z's row into
        // the old rows; then w defined over z.
        let z = s.new_var();
        e = lin(&[(z, "1"), (x, "-1"), (y, "-1")], "0");
        assert!(s.assert_constraint(&e, RelOp::Eq));
        assert!(s.assert_upper(z, d(3)));
        let w = s.slack(&lin(&[(z, "1"), (x, "1")], "1"));
        assert!(s.assert_lower(w, d(0)));
        assert!(s.num_vars() > nv && s.rows.len() > nr);
        s.undo_to(mark);
        assert_eq!(s.num_vars(), nv);
        assert_eq!(s.rows.len(), nr);
        tableau_is_consistent(&s);
        // The pre-mark system is intact: y = 2x, x >= 1.
        assert!(s.fix(x, n("5")));
        assert_eq!(s.is_determined(y), Some(n("10")));
        // Indices are reused: the next variable is the old z's index.
        assert_eq!(s.new_var(), z);
    }

    /// A small deterministic generator.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
        fn small(&mut self) -> i64 {
            self.below(9) as i64 - 4
        }
    }

    #[derive(Clone, Debug)]
    enum Op {
        NewVar,
        Define(LinExpr),
        Slack(LinExpr),
        Lower(SVar, Delta),
        Upper(SVar, Delta),
    }

    fn random_expr(rng: &mut Rng, nvars: usize) -> LinExpr {
        let mut e = LinExpr::constant(Number::from_ratio(rng.small(), 1).unwrap());
        let terms = 1 + rng.below(3.min(nvars));
        for _ in 0..terms {
            let v = SVar(rng.below(nvars));
            let a = match rng.below(5) {
                0 => n("-2"),
                1 => n("-1"),
                2 => n("1"),
                3 => n("2"),
                _ => Number::from_ratio(1, 2).unwrap(),
            };
            e.add_term(v, &a);
        }
        if e.terms.is_empty() {
            e.add_term(SVar(0), &n("1"));
        }
        e
    }

    /// A bound as the store makes them: a strict lower bound is `c + δ`, a
    /// strict upper bound `c − δ` (an upper bound `c + δ` has no source
    /// constraint and would break `is_determined`'s invariant that a fixed
    /// value is a plain rational).
    fn random_bound(rng: &mut Rng, lower: bool) -> Delta {
        let c = Number::from_ratio(rng.small(), 1).unwrap();
        match rng.below(3) {
            0 if lower => Delta::new(c, n("1")),
            0 => Delta::new(c, n("-1")),
            _ => Delta::exact(c),
        }
    }

    /// Apply `op` to `s`; false if an assertion failed.
    fn apply(s: &mut Simplex, op: &Op) -> bool {
        match op {
            Op::NewVar => {
                s.new_var();
                true
            }
            Op::Define(e) => {
                let v = s.new_var();
                s.define(v, e);
                true
            }
            Op::Slack(e) => {
                s.slack(e);
                true
            }
            Op::Lower(v, b) => s.assert_lower(*v, b.clone()),
            Op::Upper(v, b) => s.assert_upper(*v, b.clone()),
        }
    }

    fn replay(ops: &[Op]) -> Simplex {
        let mut s = Simplex::new();
        for op in ops {
            assert!(
                apply(&mut s, op),
                "a surviving operation must succeed on replay"
            );
        }
        s
    }

    /// The live solver after undos must be indistinguishable from a fresh
    /// solver that only ever saw the surviving operations: same variables,
    /// bounds, determined values, and the same verdict on further assertions.
    fn agree(live: &mut Simplex, fresh: &mut Simplex, rng: &mut Rng) {
        assert_eq!(live.num_vars(), fresh.num_vars());
        tableau_is_consistent(live);
        for v in 0..live.num_vars() {
            let v = SVar(v);
            assert_eq!(live.lower(v), fresh.lower(v), "lower bound of {v}");
            assert_eq!(live.upper(v), fresh.upper(v), "upper bound of {v}");
            assert_eq!(
                live.is_determined(v),
                fresh.is_determined(v),
                "determination of {v}"
            );
        }
        for _ in 0..4 {
            let probe = if rng.below(2) == 0 {
                Op::Lower(SVar(rng.below(live.num_vars())), random_bound(rng, true))
            } else {
                Op::Upper(SVar(rng.below(live.num_vars())), random_bound(rng, false))
            };
            let (mut a, mut b) = (live.clone(), fresh.clone());
            assert_eq!(
                apply(&mut a, &probe),
                apply(&mut b, &probe),
                "verdict on {probe:?}"
            );
        }
    }

    #[test]
    fn undo_is_equivalent_to_replaying_the_surviving_operations() {
        for seed in 1..=160u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let mut live = Simplex::new();
            // One variable always survives, so every draw has a range.
            live.new_var();
            let mut ops: Vec<Op> = vec![Op::NewVar];
            // (mark, number of surviving ops at the mark)
            let mut marks: Vec<(Mark, usize)> = vec![(live.mark(), 1)];
            for _ in 0..40 {
                match rng.below(10) {
                    0 | 1 => marks.push((live.mark(), ops.len())),
                    2 if marks.len() > 1 => {
                        let k = 1 + rng.below(marks.len() - 1);
                        let (mark, len) = marks[k];
                        marks.truncate(k);
                        live.undo_to(mark);
                        ops.truncate(len);
                        let mut fresh = replay(&ops);
                        agree(&mut live, &mut fresh, &mut rng);
                    }
                    _ => {
                        let nv = live.num_vars();
                        let op = match rng.below(6) {
                            0 => Op::NewVar,
                            1 => Op::Define(random_expr(&mut rng, nv)),
                            2 => Op::Slack(random_expr(&mut rng, nv)),
                            3 | 4 => Op::Lower(SVar(rng.below(nv)), random_bound(&mut rng, true)),
                            _ => Op::Upper(SVar(rng.below(nv)), random_bound(&mut rng, false)),
                        };
                        if apply(&mut live, &op) {
                            ops.push(op);
                        } else {
                            // As the store does: a failed assertion is undone
                            // to the innermost mark.
                            let (mark, len) = *marks.last().unwrap();
                            live.undo_to(mark);
                            ops.truncate(len);
                            let mut fresh = replay(&ops);
                            agree(&mut live, &mut fresh, &mut rng);
                        }
                    }
                }
            }
        }
    }
}
