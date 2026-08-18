// SPDX-License-Identifier: Apache-2.0

//! Answer projection and simplification (evaluator stage 4, design D7).
//!
//! From the store after `finalize`, build the residual numeric system the
//! query can see, over alias-class roots; eliminate internal variables
//! (Gaussian substitution for equalities, Fourier–Motzkin for inequalities,
//! with a budget); remove redundant constraints exactly; orient equalities to
//! solved form. Everything is equivalence-preserving on the public variables,
//! so printing the result is sound (`answer-soundness`).

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::solver::{LinExpr, RelOp, SVar, Simplex};
use crate::Number;

use super::store::Store;

/// Above this many inequalities Fourier–Motzkin stops eliminating and the
/// remaining internal variables are named in the answer instead.
pub(crate) const FM_BUDGET: usize = 256;

/// `expr ⋈ 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
    Eq,
    /// `expr >= 0`
    Ge,
    /// `expr > 0`
    Gt,
    /// `expr != 0`
    Ne,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Constraint {
    pub expr: LinExpr,
    pub kind: Kind,
}

/// The simplified visible system.
#[derive(Debug, Default)]
pub(crate) struct Projected {
    /// Equalities in solved form: `subject = rhs` (rhs without the subject).
    pub eqs: Vec<(SVar, LinExpr)>,
    /// Inequalities `expr >= 0` / `expr > 0` (strict flag).
    pub ineqs: Vec<(LinExpr, bool)>,
    /// Disequations `expr != 0`.
    pub difs: Vec<LinExpr>,
    /// Internal variables that survive (budget fallback); to be named `_N`.
    pub survivors: BTreeSet<SVar>,
}

/// Collect, eliminate, simplify.
pub(crate) fn project(store: &Store, public: &BTreeSet<SVar>) -> Projected {
    let mut cs = collect(store, public);
    cs = connected(cs, public);
    let mut internal: BTreeSet<SVar> = BTreeSet::new();
    for c in &cs {
        for v in c.expr.terms.keys() {
            if !public.contains(v) {
                internal.insert(*v);
            }
        }
    }
    gaussian(&mut cs, &internal, public);
    let survivors = fourier_motzkin(&mut cs, &internal);
    // Disequations over internal variables that neither substitution nor
    // elimination connected to public ones are satisfiable independently.
    cs.retain(|c| {
        c.kind != Kind::Ne
            || c.expr.terms.keys().all(|v| public.contains(v) || survivors.contains(v))
    });
    normalise_all(&mut cs);
    dedupe(&mut cs);
    drop_redundant(&mut cs);
    orient(cs, public, survivors)
}

/// Every constraint of the store, over roots, with definitions of
/// non-public variables expanded and fixed classes substituted.
fn collect(store: &Store, public: &BTreeSet<SVar>) -> Vec<Constraint> {
    let n = store.simplex.num_vars();
    let mut out = Vec::new();
    // Bounds on every variable (slack definitions expand to their expressions).
    for i in 0..n {
        let v = SVar(i);
        let (lower, upper) = (store.simplex.lower(v), store.simplex.upper(v));
        if lower.is_none() && upper.is_none() {
            continue;
        }
        let mut e = LinExpr::default();
        expand(store, public, v, &Number::one(), &mut e, 0);
        if let (Some(l), Some(u)) = (lower, upper) {
            if l == u && l.is_exact() {
                // v = c
                let mut c = e.clone();
                c.constant -= &l.c;
                out.push(Constraint { expr: c, kind: Kind::Eq });
                continue;
            }
        }
        if let Some(l) = lower {
            // v - c (- kδ) >= 0
            let mut c = e.clone();
            c.constant -= &l.c;
            out.push(Constraint { expr: c, kind: if l.k.is_positive() { Kind::Gt } else { Kind::Ge } });
        }
        if let Some(u) = upper {
            // c - v >= 0
            let mut c = e.clone();
            c.negate();
            c.constant += &u.c;
            out.push(Constraint { expr: c, kind: if u.k.is_negative() { Kind::Gt } else { Kind::Ge } });
        }
    }
    // Definitions of public defined variables are genuine equalities.
    for (v, def) in &store.defs {
        let r = store.root(*v);
        if !public.contains(&r) {
            continue;
        }
        let mut e = LinExpr::default();
        expand(store, public, *v, &Number::one(), &mut e, 0);
        let mut d = LinExpr::default();
        for (x, a) in &def.terms {
            expand(store, public, *x, a, &mut d, 0);
        }
        d.constant += &def.constant;
        e.sub(&d);
        out.push(Constraint { expr: e, kind: Kind::Eq });
    }
    // Pending numeric disequations.
    for (i, nd) in store.numdifs.iter().enumerate() {
        if !nd.pending || i < store.baseline_numdifs {
            continue;
        }
        let mut e = LinExpr::default();
        expand(store, public, nd.d, &Number::one(), &mut e, 0);
        out.push(Constraint { expr: e, kind: Kind::Ne });
    }
    // Drop constant (trivially true) constraints.
    out.retain(|c| !c.expr.is_constant());
    out
}

/// `out += k · v`, expanding definitions of non-public variables and
/// substituting fixed classes.
fn expand(store: &Store, public: &BTreeSet<SVar>, v: SVar, k: &Number, out: &mut LinExpr, depth: usize) {
    let r = store.root(v);
    // A non-public defined variable *is* its definition (a fixed one still
    // carries a constraint through its bounds, so expand before substituting).
    if !public.contains(&r) && depth < 64 {
        if let Some(def) = store.defs.get(&v) {
            for (x, a) in &def.terms {
                expand(store, public, *x, &(a * k), out, depth + 1);
            }
            out.constant += &(&def.constant * k);
            return;
        }
    }
    if let Some(c) = store.class_value(r) {
        out.constant += &(&c * k);
        return;
    }
    out.add_term(r, k);
}

/// Keep only constraints connected (through shared variables) to a public one.
fn connected(cs: Vec<Constraint>, public: &BTreeSet<SVar>) -> Vec<Constraint> {
    let mut reached: HashSet<SVar> = public.iter().copied().collect();
    let mut kept = vec![false; cs.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for (i, c) in cs.iter().enumerate() {
            if kept[i] {
                continue;
            }
            if c.expr.terms.keys().any(|v| reached.contains(v)) {
                kept[i] = true;
                changed = true;
                reached.extend(c.expr.terms.keys().copied());
            }
        }
    }
    cs.into_iter().zip(kept).filter(|(_, k)| *k).map(|(c, _)| c).collect()
}

/// Substitute `v = rhs` (rhs without v) into `e`.
fn substitute(e: &mut LinExpr, v: SVar, rhs: &LinExpr) {
    if let Some(a) = e.take(v) {
        e.add_scaled(rhs, &a);
    }
}

/// Eliminate internal variables that occur in equalities by solving and
/// substituting; then row-reduce the remaining (public) equalities.
fn gaussian(cs: &mut Vec<Constraint>, internal: &BTreeSet<SVar>, public: &BTreeSet<SVar>) {
    loop {
        // Pick an equality and an internal variable in it (unit coefficient preferred).
        let mut pick: Option<(usize, SVar)> = None;
        'outer: for (i, c) in cs.iter().enumerate() {
            if c.kind != Kind::Eq {
                continue;
            }
            let mut fallback = None;
            for (v, a) in c.expr.terms.iter().rev() {
                if internal.contains(v) {
                    if a.abs() == Number::one() {
                        pick = Some((i, *v));
                        break 'outer;
                    }
                    fallback.get_or_insert(*v);
                }
            }
            if let Some(v) = fallback {
                pick = Some((i, v));
                break;
            }
        }
        let Some((i, v)) = pick else { break };
        let rhs = solve_for(&cs[i].expr, v);
        cs.remove(i);
        for c in cs.iter_mut() {
            substitute(&mut c.expr, v, &rhs);
        }
        cs.retain(|c| !(c.kind == Kind::Eq && c.expr.is_constant()));
    }
    // Public equalities: reduce so each subject appears once (dependent ones vanish).
    let mut done: BTreeSet<SVar> = BTreeSet::new();
    loop {
        let mut pick: Option<(usize, SVar)> = None;
        'outer: for (i, c) in cs.iter().enumerate() {
            if c.kind != Kind::Eq {
                continue;
            }
            if c.expr.terms.keys().any(|v| done.contains(v)) && c.expr.terms.keys().all(|v| done.contains(v)) {
                continue;
            }
            let mut fallback = None;
            for (v, a) in c.expr.terms.iter().rev() {
                if !done.contains(v) && public.contains(v) {
                    if a.abs() == Number::one() {
                        pick = Some((i, *v));
                        break 'outer;
                    }
                    fallback.get_or_insert(*v);
                }
            }
            if let Some(v) = fallback {
                pick = Some((i, v));
                break;
            }
        }
        let Some((i, v)) = pick else { break };
        let rhs = solve_for(&cs[i].expr, v);
        done.insert(v);
        // Substitute into the *other* equalities only (keep this one, oriented later).
        for (j, c) in cs.iter_mut().enumerate() {
            if j != i && c.kind == Kind::Eq {
                substitute(&mut c.expr, v, &rhs);
            }
        }
        cs.retain(|c| !(c.kind == Kind::Eq && c.expr.is_constant()));
    }
}

/// From `expr = 0` with `v` in it: `v = rhs`.
fn solve_for(expr: &LinExpr, v: SVar) -> LinExpr {
    let a = expr.coeff(v).expect("subject occurs").clone();
    let mut rhs = expr.clone();
    rhs.take(v);
    rhs.negate();
    rhs.scale(&a.recip().expect("non-zero coefficient"));
    rhs
}

/// Fourier–Motzkin over the inequalities for every internal variable still
/// present, cheapest first, within the budget. Returns the survivors.
fn fourier_motzkin(cs: &mut Vec<Constraint>, internal: &BTreeSet<SVar>) -> BTreeSet<SVar> {
    let mut remaining: BTreeSet<SVar> = internal
        .iter()
        .copied()
        .filter(|v| cs.iter().any(|c| c.expr.terms.contains_key(v)))
        .collect();
    let mut survivors = BTreeSet::new();
    while let Some(v) = cheapest(cs, &remaining) {
        remaining.remove(&v);
        let (lowers, uppers, others): (Vec<_>, Vec<_>, Vec<_>) = {
            let mut l = Vec::new();
            let mut u = Vec::new();
            let mut o = Vec::new();
            for c in cs.drain(..) {
                match c.expr.coeff(v) {
                    Some(a) if c.kind != Kind::Ne && a.is_positive() => l.push(c),
                    Some(a) if c.kind != Kind::Ne && a.is_negative() => u.push(c),
                    _ => o.push(c),
                }
            }
            (l, u, o)
        };
        let projected = lowers.len() * uppers.len();
        if others.len() + projected > FM_BUDGET {
            // Give up on this variable: keep everything, name it.
            *cs = others;
            cs.extend(lowers);
            cs.extend(uppers);
            survivors.insert(v);
            continue;
        }
        *cs = others;
        for l in &lowers {
            let a = l.expr.coeff(v).unwrap().clone(); // > 0
            for u in &uppers {
                let b = u.expr.coeff(v).unwrap().clone(); // < 0
                // (-b)·L + a·U eliminates v.
                let mut e = l.expr.clone();
                e.scale(&-&b);
                e.add_scaled(&u.expr, &a);
                debug_assert!(e.coeff(v).is_none());
                let strict = l.kind == Kind::Gt || u.kind == Kind::Gt;
                if e.is_constant() {
                    continue; // a consequence between constants; the store was feasible
                }
                cs.push(Constraint { expr: e, kind: if strict { Kind::Gt } else { Kind::Ge } });
            }
        }
    }
    // Disequations mentioning a variable eliminated by FM cannot be kept
    // faithfully without it; keep them named (rare) by marking as survivors.
    for c in cs.iter() {
        if c.kind == Kind::Ne {
            for v in c.expr.terms.keys() {
                if internal.contains(v) {
                    survivors.insert(*v);
                }
            }
        }
    }
    survivors
}

/// The remaining internal variable whose elimination grows the system least.
fn cheapest(cs: &[Constraint], remaining: &BTreeSet<SVar>) -> Option<SVar> {
    remaining
        .iter()
        .map(|v| {
            let l = cs.iter().filter(|c| c.kind != Kind::Ne && c.expr.coeff(*v).is_some_and(|a| a.is_positive())).count();
            let u = cs.iter().filter(|c| c.kind != Kind::Ne && c.expr.coeff(*v).is_some_and(|a| a.is_negative())).count();
            (l * u as isize as usize, *v, l + u)
        })
        .min_by_key(|(cost, v, n)| (*cost as isize - *n as isize, *v))
        .map(|(_, v, _)| v)
}

/// Scale so variable coefficients are integers with no common factor (the
/// constant may stay fractional), so equal constraints coincide and print
/// small: `2X <= 6` becomes `X <= 3`, `2*X - Y >= 1/3` stays as written.
fn normalise_all(cs: &mut [Constraint]) {
    for c in cs.iter_mut() {
        // Two passes: integralise, then divide out the gcd of the integers.
        if let Some(f) = Number::integralising_factor(c.expr.terms.values()) {
            c.expr.scale(&f);
        }
        if let Some(f) = Number::integralising_factor(c.expr.terms.values()) {
            c.expr.scale(&f);
        }
        // Equalities and disequations: make the leading coefficient positive.
        if matches!(c.kind, Kind::Eq | Kind::Ne) {
            if let Some((_, a)) = c.expr.terms.iter().next() {
                if a.is_negative() {
                    c.expr.negate();
                }
            }
        }
    }
}

fn dedupe(cs: &mut Vec<Constraint>) {
    let mut seen: Vec<Constraint> = Vec::new();
    cs.retain(|c| {
        if seen.contains(c) {
            false
        } else {
            seen.push(c.clone());
            true
        }
    });
}

/// Drop every inequality entailed by the others (exact, via a fresh simplex).
fn drop_redundant(cs: &mut Vec<Constraint>) {
    let mut i = 0;
    while i < cs.len() {
        if !matches!(cs[i].kind, Kind::Ge | Kind::Gt) {
            i += 1;
            continue;
        }
        let mut sx = Simplex::new();
        let mut map: BTreeMap<SVar, SVar> = BTreeMap::new();
        let mut var = |v: SVar, sx: &mut Simplex| *map.entry(v).or_insert_with(|| sx.new_var());
        let rewrite = |e: &LinExpr, sx: &mut Simplex, var: &mut dyn FnMut(SVar, &mut Simplex) -> SVar| {
            let mut out = LinExpr::constant(e.constant.clone());
            for (v, a) in &e.terms {
                let w = var(*v, sx);
                out.add_term(w, a);
            }
            out
        };
        let mut feasible = true;
        for (j, c) in cs.iter().enumerate() {
            if j == i || c.kind == Kind::Ne {
                continue;
            }
            let e = rewrite(&c.expr, &mut sx, &mut var);
            let op = match c.kind {
                Kind::Eq => RelOp::Eq,
                Kind::Ge => RelOp::Ge,
                Kind::Gt => RelOp::Gt,
                Kind::Ne => unreachable!(),
            };
            if !sx.assert_constraint(&e, op) {
                feasible = false;
                break;
            }
        }
        if feasible {
            let e = rewrite(&cs[i].expr, &mut sx, &mut var);
            let negated = match cs[i].kind {
                Kind::Ge => RelOp::Lt,
                Kind::Gt => RelOp::Le,
                _ => unreachable!(),
            };
            feasible = sx.assert_constraint(&e, negated);
        }
        if feasible {
            i += 1; // the candidate adds information
        } else {
            cs.remove(i); // entailed by the rest
        }
    }
}

/// Orient equalities to solved form and split by kind.
fn orient(cs: Vec<Constraint>, public: &BTreeSet<SVar>, survivors: BTreeSet<SVar>) -> Projected {
    let mut out = Projected { survivors, ..Default::default() };
    for c in cs {
        match c.kind {
            Kind::Eq => {
                // Subject: most recently introduced public unit-coefficient
                // variable, else most recent public, else any.
                let subject = c
                    .expr
                    .terms
                    .iter()
                    .rev()
                    .find(|(v, a)| public.contains(v) && a.abs() == Number::one())
                    .or_else(|| c.expr.terms.iter().rev().find(|(v, _)| public.contains(v)))
                    .or_else(|| c.expr.terms.iter().next_back())
                    .map(|(v, _)| *v);
                if let Some(s) = subject {
                    let rhs = solve_for(&c.expr, s);
                    out.eqs.push((s, rhs));
                }
            }
            Kind::Ge => out.ineqs.push((c.expr, false)),
            Kind::Gt => out.ineqs.push((c.expr, true)),
            Kind::Ne => out.difs.push(c.expr),
        }
    }
    out.eqs.sort_by_key(|(s, _)| *s);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(terms: &[(usize, &str)], c: &str) -> LinExpr {
        let mut e = LinExpr::constant(Number::from_literal(c).unwrap());
        for (v, a) in terms {
            let a = if let Some(rest) = a.strip_prefix('-') {
                -Number::from_literal(rest).unwrap()
            } else {
                Number::from_literal(a).unwrap()
            };
            e.add_term(SVar(*v), &a);
        }
        e
    }

    #[test]
    fn fm_eliminates_a_two_sided_variable() {
        // X = Y + Z (as Y = X - Z), Y > 0, Z > 0  =>  X > 0   with Y=1, Z=2 internal, X=0 public
        let mut cs = vec![
            Constraint { expr: lin(&[(0, "1"), (1, "-1"), (2, "-1")], "0"), kind: Kind::Eq },
            Constraint { expr: lin(&[(1, "1")], "0"), kind: Kind::Gt },
            Constraint { expr: lin(&[(2, "1")], "0"), kind: Kind::Gt },
        ];
        let internal: BTreeSet<SVar> = [SVar(1), SVar(2)].into();
        let public: BTreeSet<SVar> = [SVar(0)].into();
        gaussian(&mut cs, &internal, &public);
        let survivors = fourier_motzkin(&mut cs, &internal);
        assert!(survivors.is_empty());
        normalise_all(&mut cs);
        dedupe(&mut cs);
        drop_redundant(&mut cs);
        assert_eq!(cs, vec![Constraint { expr: lin(&[(0, "1")], "0"), kind: Kind::Gt }]);
    }

    #[test]
    fn redundancy_is_exact() {
        // X > 3, X > 2 => X > 3 ; X > Y, Y > Z, X > Z => drop X > Z
        let mut cs = vec![
            Constraint { expr: lin(&[(0, "1")], "-3"), kind: Kind::Gt },
            Constraint { expr: lin(&[(0, "1")], "-2"), kind: Kind::Gt },
        ];
        drop_redundant(&mut cs);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].expr.constant, -Number::from(3));
        let mut cs = vec![
            Constraint { expr: lin(&[(0, "1"), (1, "-1")], "0"), kind: Kind::Gt },
            Constraint { expr: lin(&[(1, "1"), (2, "-1")], "0"), kind: Kind::Gt },
            Constraint { expr: lin(&[(0, "1"), (2, "-1")], "0"), kind: Kind::Gt },
        ];
        drop_redundant(&mut cs);
        assert_eq!(cs.len(), 2);
    }

    #[test]
    fn budget_keeps_survivors() {
        // Many lowers and uppers on an internal variable: exceed the budget.
        let mut cs = Vec::new();
        for i in 0..20 {
            cs.push(Constraint { expr: lin(&[(0, "1"), (i + 1, "-1")], "0"), kind: Kind::Ge }); // v0 >= v_i
            cs.push(Constraint { expr: lin(&[(0, "-1"), (i + 21, "1")], "0"), kind: Kind::Ge }); // v0 <= v_j
        }
        let internal: BTreeSet<SVar> = [SVar(0)].into();
        let survivors = fourier_motzkin(&mut cs, &internal);
        assert_eq!(survivors, [SVar(0)].into());
        assert_eq!(cs.len(), 40);
    }
}
