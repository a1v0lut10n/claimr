//! General simplex with bounds (Dutertre & de Moura, CAV 2006), exact.
//!
//! State: every variable has a current value (a δ-rational) and optional
//! lower/upper bounds; basic variables are defined by a row `xᵢ = Σ aⱼ·xⱼ + c`
//! over non-basic variables. Invariants: values satisfy every row, and every
//! *non-basic* variable satisfies its bounds. `check` repairs basic
//! violations by Bland-rule pivoting or reports infeasibility. Backtracking
//! restores bounds only; rows and variables created since a mark persist
//! (unbounded, they constrain nothing).

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

/// A point in the solver's bound history.
#[derive(Debug, Clone, Copy)]
pub struct Mark(usize);

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
        self.vars.push(VarState { lower: None, upper: None, value: Delta::zero() });
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
        debug_assert!(!expr.terms.contains_key(&v), "definition must not be self-referential");
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
        Mark(self.trail.len())
    }

    /// Restore all bounds to those at `mark`. Values stay (they satisfy the
    /// rows; `check` repairs bound violations lazily).
    pub fn undo_to(&mut self, mark: Mark) {
        while self.trail.len() > mark.0 {
            let u = self.trail.pop().unwrap();
            let st = &mut self.vars[u.var.0];
            st.lower = u.lower;
            st.upper = u.upper;
        }
    }

    /// True if any bound changed since `mark`.
    pub fn changed_since(&self, mark: Mark) -> bool {
        self.trail.len() > mark.0
    }

    fn save(&mut self, v: SVar) {
        let st = &self.vars[v.0];
        self.trail.push(BoundUndo { var: v, lower: st.lower.clone(), upper: st.upper.clone() });
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
            let Some((xi, below)) = violated else { return true };
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
        self.rows[ri] = Row { basic: xj, expr: expr.clone() };
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
                return if l.is_exact() { Some(l.c.clone()) } else { None };
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
            (vec![(v[0], "0.25"), (v[1], "-8"), (v[2], "-1"), (v[3], "9"), (v[4], "1")], "0"),
            (vec![(v[0], "0.5"), (v[1], "-12"), (v[2], "-0.5"), (v[3], "3"), (v[5], "1")], "0"),
            (vec![(v[2], "1"), (v[6], "1")], "-1"),
        ];
        for (terms, c) in rows {
            let e = lin(&terms, c);
            assert!(s.assert_constraint(&e, RelOp::Eq));
        }
        // Push the objective-like combination around; must terminate.
        let obj = lin(&[(v[0], "-0.75"), (v[1], "20"), (v[2], "-0.5"), (v[3], "6")], "0");
        assert!(s.assert_constraint(&obj, RelOp::Le));
        let m = s.mark();
        let tight = lin(&[(v[0], "-0.75"), (v[1], "20"), (v[2], "-0.5"), (v[3], "6")], "0.5");
        let _ = s.assert_constraint(&tight, RelOp::Le); // either way, must return
        s.undo_to(m);
        assert!(s.check());
    }
}
