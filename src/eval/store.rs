//! The constraint store: a term heap, variable bindings, the trail (Van
//! Caneghem's *pile de restauration*), suspension of wakers on variables,
//! unification over rational trees, `dif` — and the numeric part: solver
//! variables for numeric heap variables and attribute terms, linear
//! constraints in the simplex, numeric disequations, delayed products.
//!
//! Every mutation is recorded on the trail (or in the simplex's own bound
//! trail, marked together) so that [`Store::undo_to`] restores an earlier
//! [`Mark`] exactly — chronological backtracking and trial unification rely
//! on that.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ast::ArithOp;
use crate::number::Number;
use crate::solver::{self, LinExpr, SVar, Simplex};

use super::error::EvalError;
use super::symbol::Symbol;

/// Address of a cell in the heap.
pub type Addr = usize;

/// Index of a disequation in the store.
pub type DifId = usize;

/// A heap cell. A variable is `Var(a)` at address `a` when unbound; binding it
/// makes it `Var(target)`. Compound cells may be overwritten by a `Var`
/// forwarding to an equal compound (bind-before-descend), which is what makes
/// unification of rational trees terminate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cell {
    Var(Addr),
    Const(Symbol),
    Num(Number),
    Struct(Symbol, Vec<Addr>),
}

#[derive(Debug, Clone)]
struct Dif {
    a: Addr,
    b: Addr,
    pending: bool,
}

/// Structural key of an attribute term modulo current bindings.
pub(crate) type AttrKey = Vec<KeyElem>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum KeyElem {
    Sym(Symbol, usize),
    Num(Number),
    Var(Addr),
}

#[derive(Debug, Clone)]
pub(crate) struct AttrEntry {
    pub term: Addr,
    key: AttrKey,
    pub svar: SVar,
}

#[derive(Debug, Clone)]
pub(crate) struct NumDif {
    pub d: SVar,
    pub a: Addr,
    pub b: Addr,
    pub pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProductKind {
    Mul,
    Div,
}

#[derive(Debug, Clone)]
pub(crate) struct Product {
    pub n: SVar,
    pub kind: ProductKind,
    pub a: SVar,
    pub b: SVar,
    pub a_addr: Addr,
    pub b_addr: Addr,
    pub pending: bool,
}

/// Something suspended on a variable, to be re-examined when it is bound (or,
/// for numeric variables, determined).
#[derive(Debug, Clone, Copy)]
enum Waker {
    Dif(DifId),
    Attr(usize),
}

#[derive(Debug)]
enum Undo {
    Cell(Addr, Cell),
    DifPending(DifId),
    Wake(Addr),
    NumVar(Addr),
    AttrIndex(AttrKey),
    AttrKey(usize, AttrKey),
    NumDifPending(usize),
    ProductPending(usize),
    Fired(Addr),
    Alias(SVar),
}

/// A point in the store's history to return to.
#[derive(Debug, Clone, Copy)]
pub struct Mark {
    heap: usize,
    trail: usize,
    difs: usize,
    attrs: usize,
    numdifs: usize,
    products: usize,
    simplex: solver::Mark,
}

/// The store.
#[derive(Debug, Default)]
pub struct Store {
    heap: Vec<Cell>,
    trail: Vec<Undo>,
    difs: Vec<Dif>,
    /// Variable → wakers suspended on it.
    wake: HashMap<Addr, Vec<Waker>>,
    /// Wakers woken since the last settle.
    woken: Vec<Waker>,

    // --- numeric part ---
    pub(crate) simplex: Simplex,
    /// Unbound heap variable → its solver variable (the variable is numeric).
    pub(crate) numvar: HashMap<Addr, SVar>,
    /// Attribute terms registered so far.
    pub(crate) attrs: Vec<AttrEntry>,
    attr_index: HashMap<AttrKey, SVar>,
    pub(crate) numdifs: Vec<NumDif>,
    pub(crate) products: Vec<Product>,
    /// Original definition of each slack/defined solver variable, for printing.
    pub(crate) defs: BTreeMap<SVar, LinExpr>,
    /// Solver variables equated by `equate`: child → parent (union-find, no
    /// path compression so it can be trailed).
    aliases: HashMap<SVar, SVar>,
    /// Counts at the start of the current query (after the initial store):
    /// attribute entries and numeric disequations created before it belong to
    /// the world, not to the answer.
    pub(crate) baseline_attrs: usize,
    pub(crate) baseline_numdifs: usize,
    /// Heap variables whose "determined" event has fired.
    fired: HashSet<Addr>,
    /// Numeric heap variables touched during the current trial unification.
    touched: Vec<Addr>,
    /// A sticky runtime error (cyclic attribute term).
    pub(crate) error: Option<EvalError>,
    /// A non-linear product still pending at answer time: its operand terms
    /// and operator, rendered by the machine with the query's variable names.
    pub(crate) nonlinear: Option<(Addr, Addr, &'static str)>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    // --- construction ------------------------------------------------------

    pub fn new_var(&mut self) -> Addr {
        let a = self.heap.len();
        self.heap.push(Cell::Var(a));
        a
    }

    pub fn new_const(&mut self, s: Symbol) -> Addr {
        self.heap.push(Cell::Const(s));
        self.heap.len() - 1
    }

    pub fn new_num(&mut self, n: Number) -> Addr {
        self.heap.push(Cell::Num(n));
        self.heap.len() - 1
    }

    pub fn new_struct(&mut self, f: Symbol, args: Vec<Addr>) -> Addr {
        self.heap.push(Cell::Struct(f, args));
        self.heap.len() - 1
    }

    // --- inspection --------------------------------------------------------

    /// Follow variable bindings and forwarding to the representative cell.
    pub fn deref(&self, mut a: Addr) -> Addr {
        loop {
            match &self.heap[a] {
                Cell::Var(t) if *t != a => a = *t,
                _ => return a,
            }
        }
    }

    /// The cell at `a` (not dereferenced).
    pub fn cell(&self, a: Addr) -> &Cell {
        &self.heap[a]
    }

    pub(crate) fn is_unbound_var(&self, a: Addr) -> bool {
        matches!(self.heap[a], Cell::Var(t) if t == a)
    }

    // --- marks and undo ----------------------------------------------------

    pub fn mark(&self) -> Mark {
        Mark {
            heap: self.heap.len(),
            trail: self.trail.len(),
            difs: self.difs.len(),
            attrs: self.attrs.len(),
            numdifs: self.numdifs.len(),
            products: self.products.len(),
            simplex: self.simplex.mark(),
        }
    }

    /// Restore the store to `mark`, undoing every change made since.
    pub fn undo_to(&mut self, mark: &Mark) {
        while self.trail.len() > mark.trail {
            match self.trail.pop().unwrap() {
                Undo::Cell(a, old) => self.heap[a] = old,
                Undo::DifPending(id) => self.difs[id].pending = true,
                Undo::Wake(v) => {
                    if let Some(list) = self.wake.get_mut(&v) {
                        list.pop();
                        if list.is_empty() {
                            self.wake.remove(&v);
                        }
                    }
                }
                Undo::NumVar(a) => {
                    self.numvar.remove(&a);
                }
                Undo::AttrIndex(key) => {
                    self.attr_index.remove(&key);
                }
                Undo::AttrKey(id, old) => {
                    if id < self.attrs.len() {
                        self.attrs[id].key = old;
                    }
                }
                Undo::NumDifPending(id) => {
                    if id < self.numdifs.len() {
                        self.numdifs[id].pending = true;
                    }
                }
                Undo::ProductPending(id) => {
                    if id < self.products.len() {
                        self.products[id].pending = true;
                    }
                }
                Undo::Fired(a) => {
                    self.fired.remove(&a);
                }
                Undo::Alias(sv) => {
                    self.aliases.remove(&sv);
                }
            }
        }
        self.heap.truncate(mark.heap);
        self.difs.truncate(mark.difs);
        self.attrs.truncate(mark.attrs);
        self.numdifs.truncate(mark.numdifs);
        self.products.truncate(mark.products);
        self.simplex.undo_to(mark.simplex);
        self.woken.clear();
        self.touched.clear();
    }

    // --- primitive mutations (all trailed) ---------------------------------

    fn set(&mut self, a: Addr, new: Cell) {
        let old = std::mem::replace(&mut self.heap[a], new);
        self.trail.push(Undo::Cell(a, old));
    }

    fn fire(&mut self, v: Addr) {
        if let Some(ids) = self.wake.get(&v) {
            self.woken.extend(ids.iter().copied());
        }
    }

    /// Bind the unbound variable `v` to `t`, waking suspended wakers and
    /// propagating numeric typing. False if the numeric store becomes
    /// unsatisfiable (caller undoes).
    fn bind(&mut self, v: Addr, t: Addr) -> bool {
        debug_assert!(self.is_unbound_var(v));
        let Some(sv) = self.numvar.get(&v).copied() else {
            self.set(v, Cell::Var(t));
            self.fire(v);
            return true;
        };
        // A numeric variable stays a variable of the solver: meeting a
        // number is an equation (no heap binding), meeting a variable merges,
        // meeting a compound or constant makes that term an attribute term.
        // A determined numeric variable meeting a number or another
        // determined variable just compares values — no store change, so a
        // trial unification sees "already equal" (dif) rather than a binding.
        let d = self.deref(t);
        let fixed = self.class_value(sv);
        match self.heap[d].clone() {
            Cell::Num(c) => {
                if let Some(f) = fixed {
                    return f == c;
                }
                self.touched.push(v);
                self.simplex.fix(sv, c)
            }
            Cell::Var(_) => {
                if let (Some(f), Some(other)) = (&fixed, self.numvar.get(&d).copied()) {
                    if let Some(g) = self.class_value(other) {
                        // Both determined: equal values are no new information
                        // (nothing changes), different values fail.
                        return *f == g;
                    }
                }
                self.touched.push(v);
                self.set(v, Cell::Var(t));
                self.fire(v);
                match self.numvar.get(&d).copied() {
                    Some(other) => self.equate(sv, other),
                    None => {
                        self.set_numvar(d, sv);
                        true
                    }
                }
            }
            Cell::Const(_) | Cell::Struct(..) => {
                self.touched.push(v);
                self.set(v, Cell::Var(t));
                self.fire(v);
                match self.attribute(d) {
                    Some(other) => self.equate(sv, other),
                    None => false,
                }
            }
        }
    }

    fn set_numvar(&mut self, v: Addr, sv: SVar) {
        self.numvar.insert(v, sv);
        self.trail.push(Undo::NumVar(v));
    }

    fn suspend(&mut self, w: Waker, v: Addr) {
        let list = self.wake.entry(v).or_default();
        let dup = list.iter().any(|x| match (x, &w) {
            (Waker::Dif(a), Waker::Dif(b)) => a == b,
            (Waker::Attr(a), Waker::Attr(b)) => a == b,
            _ => false,
        });
        if dup {
            return;
        }
        list.push(w);
        self.trail.push(Undo::Wake(v));
    }

    // --- unification -------------------------------------------------------

    /// Unify `a` and `b` over rational trees (no occurs check). On failure the
    /// store is left as it was before the call. Does **not** settle woken
    /// wakers — see [`Store::post_eq`].
    fn unify(&mut self, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        let mut stack = vec![(a, b)];
        while let Some((a, b)) = stack.pop() {
            let a = self.deref(a);
            let b = self.deref(b);
            if a == b {
                continue;
            }
            let ok = match (&self.heap[a], &self.heap[b]) {
                // Two variables: keep a numeric one as the representative.
                (Cell::Var(_), Cell::Var(_)) => {
                    if self.numvar.contains_key(&a) && !self.numvar.contains_key(&b) {
                        self.bind(b, a)
                    } else {
                        self.bind(a, b)
                    }
                }
                (Cell::Var(_), _) => self.bind(a, b),
                (_, Cell::Var(_)) => self.bind(b, a),
                (Cell::Const(x), Cell::Const(y)) => x == y,
                (Cell::Num(x), Cell::Num(y)) => x == y,
                (Cell::Struct(f, xs), Cell::Struct(g, ys)) => {
                    if f != g || xs.len() != ys.len() {
                        false
                    } else {
                        // Bind before descend: forward `a` to `b` so a
                        // revisit of this pair terminates on cyclic terms.
                        let pairs: Vec<(Addr, Addr)> =
                            xs.iter().copied().zip(ys.iter().copied()).collect();
                        self.set(a, Cell::Var(b));
                        stack.extend(pairs.into_iter().rev());
                        true
                    }
                }
                _ => false,
            };
            if !ok {
                self.undo_to(&mark);
                return false;
            }
        }
        true
    }

    /// Post the equation `a = b` as tree unification, then settle. On failure
    /// the store is restored.
    pub fn post_eq(&mut self, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        if self.unify(a, b) && self.settle() {
            true
        } else {
            self.undo_to(&mark);
            false
        }
    }

    /// The `=` of a constraint goal: a numeric equation if either side is
    /// numeric (a number, a numeric variable), else unification.
    pub fn post_eq_goal(&mut self, a: Addr, b: Addr) -> bool {
        if self.is_numericish(a) || self.is_numericish(b) {
            self.post_rel(crate::ast::RelOp::Eq, a, b)
        } else {
            self.post_eq(a, b)
        }
    }

    /// The `!=` of a constraint goal: numeric disequation or `dif`.
    pub fn post_dif_goal(&mut self, a: Addr, b: Addr) -> bool {
        if self.is_numericish(a) || self.is_numericish(b) {
            self.post_numdif(a, b)
        } else {
            self.post_dif(a, b)
        }
    }

    /// A goal operand is numeric if it is itself a number (a literal or an
    /// arithmetic result) or an unbound numeric variable — not if it is a
    /// plain variable that happens to be bound to a number tree.
    fn is_numericish(&self, a: Addr) -> bool {
        if matches!(self.heap[a], Cell::Num(_)) {
            return true;
        }
        let d = self.deref(a);
        matches!(self.heap[d], Cell::Var(_)) && self.numvar.contains_key(&d)
    }

    // --- tree disequations -------------------------------------------------

    /// Post the disequation `a != b`. On failure the store is restored.
    pub fn post_dif(&mut self, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        let id = self.difs.len();
        self.difs.push(Dif { a, b, pending: true });
        if self.check_dif(id) && self.settle() {
            true
        } else {
            self.undo_to(&mark);
            false
        }
    }

    /// Re-check a disequation by trial unification (Colmerauer's criterion,
    /// Van Caneghem's implementation): if unifying the sides fails, the
    /// disequation holds and is dropped; if it succeeds without binding any
    /// variable (and without changing the numeric store), the sides are
    /// already equal and the store is unsatisfiable; otherwise the trial is
    /// undone and the disequation suspended on the variables that would have
    /// been bound (or numerically touched), re-checked when any is bound or
    /// determined.
    fn check_dif(&mut self, id: DifId) -> bool {
        let Dif { a, b, pending } = self.difs[id].clone();
        if !pending {
            return true;
        }
        let mark = self.mark();
        self.touched.clear();
        if !self.unify(a, b) {
            self.difs[id].pending = false;
            self.trail.push(Undo::DifPending(id));
            return true;
        }
        let mut would_bind: Vec<Addr> = self.trail[mark.trail..]
            .iter()
            .filter_map(|u| match u {
                Undo::Cell(addr, Cell::Var(x)) if x == addr => Some(*addr),
                _ => None,
            })
            .collect();
        let numeric_change = self.simplex.changed_since(mark.simplex);
        would_bind.extend(self.touched.iter().copied());
        self.undo_to(&mark);
        if would_bind.is_empty() && !numeric_change {
            return false; // already equal
        }
        for v in would_bind {
            self.suspend(Waker::Dif(id), v);
        }
        true
    }

    /// The pending disequations, as `(left, right)` addresses.
    pub fn pending_difs(&self) -> Vec<(Addr, Addr)> {
        self.difs.iter().filter(|d| d.pending).map(|d| (d.a, d.b)).collect()
    }

    // --- settle: wakers, determinations, products --------------------------

    /// Process everything woken since the last settle, then numeric
    /// consequences (newly determined variables, decidable numeric
    /// disequations, linearisable products), until quiescent. Restores
    /// nothing on failure — callers hold a mark.
    fn settle(&mut self) -> bool {
        loop {
            let woken = std::mem::take(&mut self.woken);
            for w in woken {
                let ok = match w {
                    Waker::Dif(id) => !self.difs[id].pending || self.check_dif(id),
                    Waker::Attr(id) => self.recanonicalize(id),
                };
                if !ok {
                    return false;
                }
            }
            if !self.numeric_consequences(false) {
                return false;
            }
            if self.woken.is_empty() {
                return true;
            }
        }
    }

    /// Cheap (or, with `exact`, probe-based) detection of determined numeric
    /// variables, decision of numeric disequations, and linearisation of
    /// delayed products. Fires "determined" wakers. False on inconsistency.
    fn numeric_consequences(&mut self, exact: bool) -> bool {
        loop {
            let mut progress = false;
            // Determined heap variables.
            let mut candidates: Vec<(Addr, SVar)> = self
                .numvar
                .iter()
                .filter(|(a, _)| !self.fired.contains(*a) && self.is_unbound_var(**a))
                .map(|(a, s)| (*a, *s))
                .collect();
            candidates.sort();
            for (a, sv) in candidates {
                let val = if exact { self.simplex.is_determined(sv) } else { self.cheap_value(sv) };
                if let Some(c) = val {
                    if !self.simplex.fix(sv, c) {
                        return false;
                    }
                    self.fired.insert(a);
                    self.trail.push(Undo::Fired(a));
                    self.fire(a);
                    progress = true;
                }
            }
            // Attribute svars determined (only matters with `exact`, for tight bounds).
            if exact {
                let attr_svars: Vec<SVar> = self.attrs.iter().map(|e| e.svar).collect();
                for sv in attr_svars {
                    if let Some(c) = self.simplex.is_determined(sv) {
                        if !self.simplex.fix(sv, c) {
                            return false;
                        }
                    }
                }
            }
            // Numeric disequations.
            for id in 0..self.numdifs.len() {
                if !self.numdifs[id].pending {
                    continue;
                }
                let d = self.numdifs[id].d;
                let val = if exact { self.simplex.is_determined(d) } else { self.cheap_value(d) };
                if let Some(c) = val {
                    if c.is_zero() {
                        return false;
                    }
                    self.numdifs[id].pending = false;
                    self.trail.push(Undo::NumDifPending(id));
                    progress = true;
                }
            }
            // Delayed products.
            for id in 0..self.products.len() {
                if !self.products[id].pending {
                    continue;
                }
                let p = self.products[id].clone();
                let (av, bv) = if exact {
                    (self.simplex.is_determined(p.a), self.simplex.is_determined(p.b))
                } else {
                    (self.cheap_value(p.a), self.cheap_value(p.b))
                };
                let linear: Option<LinExpr> = match p.kind {
                    ProductKind::Mul => match (av, bv) {
                        (Some(c), _) => {
                            let mut e = LinExpr::var(p.b);
                            e.scale(&c);
                            Some(e)
                        }
                        (_, Some(c)) => {
                            let mut e = LinExpr::var(p.a);
                            e.scale(&c);
                            Some(e)
                        }
                        _ => None,
                    },
                    ProductKind::Div => match bv {
                        Some(c) => {
                            let Some(inv) = c.recip() else { return false }; // division by zero
                            let mut e = LinExpr::var(p.a);
                            e.scale(&inv);
                            Some(e)
                        }
                        None => None,
                    },
                };
                if let Some(e) = linear {
                    let mut eq = LinExpr::var(p.n);
                    eq.sub(&e);
                    if !self.simplex.assert_constraint(&eq, solver::RelOp::Eq) {
                        return false;
                    }
                    self.products[id].pending = false;
                    self.trail.push(Undo::ProductPending(id));
                    progress = true;
                }
            }
            if !progress {
                return true;
            }
            // Progress may have woken wakers; the caller's loop handles them,
            // but determinations can cascade, so iterate here as well.
            let woken = std::mem::take(&mut self.woken);
            for w in woken {
                let ok = match w {
                    Waker::Dif(id) => !self.difs[id].pending || self.check_dif(id),
                    Waker::Attr(id) => self.recanonicalize(id),
                };
                if !ok {
                    return false;
                }
            }
        }
    }

    /// A value the solver already exhibits as fixed without probing:
    /// coinciding bounds, or a basic row whose variables all have such values.
    pub(crate) fn cheap_value(&self, sv: SVar) -> Option<Number> {
        if let (Some(l), Some(u)) = (self.simplex.lower(sv), self.simplex.upper(sv)) {
            if l == u && l.is_exact() {
                return Some(l.c.clone());
            }
        }
        let row = self.simplex.row(sv)?;
        let mut acc = row.constant.clone();
        for (v, a) in &row.terms {
            let (Some(l), Some(u)) = (self.simplex.lower(*v), self.simplex.upper(*v)) else {
                return None;
            };
            if l != u || !l.is_exact() {
                return None;
            }
            acc += &(&l.c * a);
        }
        Some(acc)
    }

    /// Answer time: exact determination of every numeric variable and
    /// disequation, congruence and `dif` re-checks, and the non-linear residue
    /// check. False if the store turns out unsatisfiable (or on a store
    /// error, which is then set).
    pub fn finalize(&mut self) -> bool {
        if !self.numeric_consequences(true) {
            return false;
        }
        // Re-check every pending tree disequation exactly (numeric variables
        // in them may have been determined without a heap binding).
        for id in 0..self.difs.len() {
            if self.difs[id].pending && !self.check_dif(id) {
                return false;
            }
        }
        if !self.settle() {
            return false;
        }
        if let Some(p) = self.products.iter().find(|p| p.pending) {
            let op = match p.kind {
                ProductKind::Mul => "*",
                ProductKind::Div => "/",
            };
            self.nonlinear = Some((p.a_addr, p.b_addr, op));
            return false;
        }
        true
    }

    // --- numeric constraints ----------------------------------------------

    /// The solver's view of a term in numeric position: a constant, a numeric
    /// variable's solver variable, or an attribute term's unknown.
    pub(crate) fn unknown_of(&mut self, a: Addr) -> Option<LinExpr> {
        let d = self.deref(a);
        match self.heap[d].clone() {
            Cell::Num(c) => Some(LinExpr::constant(c)),
            Cell::Var(_) => {
                let sv = match self.numvar.get(&d) {
                    Some(sv) => *sv,
                    None => {
                        let sv = self.simplex.new_var();
                        self.set_numvar(d, sv);
                        sv
                    }
                };
                Some(LinExpr::var(sv))
            }
            Cell::Const(_) | Cell::Struct(..) => self.attribute(d).map(LinExpr::var),
        }
    }

    /// Post `a op b` for a numeric relation.
    pub fn post_rel(&mut self, op: crate::ast::RelOp, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        let ok = (|| {
            let mut e = self.unknown_of(a)?;
            let rb = self.unknown_of(b)?;
            e.sub(&rb);
            let sop = match op {
                crate::ast::RelOp::Eq => solver::RelOp::Eq,
                crate::ast::RelOp::Lt => solver::RelOp::Lt,
                crate::ast::RelOp::Gt => solver::RelOp::Gt,
                crate::ast::RelOp::Le => solver::RelOp::Le,
                crate::ast::RelOp::Ge => solver::RelOp::Ge,
                crate::ast::RelOp::Neq => unreachable!("!= goes through post_numdif"),
            };
            Some(self.assert_lin(e, sop) && self.settle())
        })()
        .unwrap_or(false);
        if !ok {
            self.undo_to(&mark);
        }
        ok
    }

    /// Post the numeric disequation `a != b`.
    pub fn post_numdif(&mut self, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        let ok = (|| {
            let mut e = self.unknown_of(a)?;
            let rb = self.unknown_of(b)?;
            e.sub(&rb);
            if let Some(c) = e.as_constant() {
                return Some(!c.is_zero());
            }
            let d = self.simplex.slack(&e);
            self.defs.insert(d, e);
            let id = self.numdifs.len();
            self.numdifs.push(NumDif { d, a, b, pending: true });
            match self.simplex.is_determined(d) {
                Some(c) if c.is_zero() => Some(false),
                Some(_) => {
                    self.numdifs[id].pending = false;
                    self.trail.push(Undo::NumDifPending(id));
                    Some(true)
                }
                None => Some(true),
            }
        })()
        .unwrap_or(false);
        if !ok {
            self.undo_to(&mark);
        }
        ok
    }

    /// A fresh numeric heap variable `n` with `n = -a` posted. `None` on failure.
    pub fn post_neg(&mut self, a: Addr) -> Option<Addr> {
        let mark = self.mark();
        let r = (|| {
            let mut e = self.unknown_of(a)?;
            e.negate();
            Some(self.define_fresh(e))
        })();
        if r.is_none() {
            self.undo_to(&mark);
        }
        r
    }

    /// A fresh numeric heap variable `n` with `n = a op b` posted (or delayed
    /// when non-linear). `None` on failure.
    pub fn post_arith(&mut self, op: ArithOp, a: Addr, b: Addr) -> Option<Addr> {
        let mark = self.mark();
        let r = (|| {
            let la = self.unknown_of(a)?;
            let lb = self.unknown_of(b)?;
            match op {
                ArithOp::Add => {
                    let mut e = la;
                    e.add(&lb);
                    Some(self.define_fresh(e))
                }
                ArithOp::Sub => {
                    let mut e = la;
                    e.sub(&lb);
                    Some(self.define_fresh(e))
                }
                ArithOp::Mul => {
                    if let Some(c) = la.as_constant() {
                        let mut e = lb.clone();
                        e.scale(c);
                        return Some(self.define_fresh(e));
                    }
                    if let Some(c) = lb.as_constant() {
                        let mut e = la.clone();
                        e.scale(c);
                        return Some(self.define_fresh(e));
                    }
                    let (av, bv) = (self.linear_var(la), self.linear_var(lb));
                    self.delay_product(ProductKind::Mul, av, bv, a, b)
                }
                ArithOp::Div => {
                    if let Some(c) = lb.as_constant() {
                        let inv = c.recip()?; // division by zero fails
                        let mut e = la.clone();
                        e.scale(&inv);
                        return Some(self.define_fresh(e));
                    }
                    let (av, bv) = (self.linear_var(la), self.linear_var(lb));
                    self.delay_product(ProductKind::Div, av, bv, a, b)
                }
            }
        })();
        if r.is_none() {
            self.undo_to(&mark);
        }
        r
    }

    /// A solver variable standing for a linear expression (itself if it is a
    /// bare variable, else a defined slack).
    fn linear_var(&mut self, e: LinExpr) -> SVar {
        if let Some(v) = e.as_var() {
            return v;
        }
        let s = self.simplex.slack(&e);
        self.defs.insert(s, e);
        s
    }

    fn delay_product(&mut self, kind: ProductKind, a: SVar, b: SVar, a_addr: Addr, b_addr: Addr) -> Option<Addr> {
        let n_addr = self.new_var();
        let n = self.simplex.new_var();
        self.set_numvar(n_addr, n);
        self.products.push(Product { n, kind, a, b, a_addr, b_addr, pending: true });
        // Maybe already linear (a factor determined): settle decides.
        if self.settle() { Some(n_addr) } else { None }
    }

    /// A fresh numeric heap variable defined as `e`.
    fn define_fresh(&mut self, e: LinExpr) -> Addr {
        let n_addr = self.new_var();
        let n = self.simplex.new_var();
        self.set_numvar(n_addr, n);
        self.simplex.define(n, &e);
        self.defs.insert(n, e);
        n_addr
    }

    /// Assert `e op 0`, registering any slack's definition for printing.
    fn assert_lin(&mut self, e: LinExpr, op: solver::RelOp) -> bool {
        if e.terms.len() <= 1 {
            return self.simplex.assert_constraint(&e, op);
        }
        let rhs = -&e.constant;
        let mut lhs = e;
        lhs.constant = Number::zero();
        let s = self.simplex.slack(&lhs);
        self.defs.insert(s, lhs);
        let mut bound = LinExpr::var(s);
        bound.constant = -rhs;
        self.simplex.assert_constraint(&bound, op)
    }

    fn equate(&mut self, a: SVar, b: SVar) -> bool {
        let (ra, rb) = (self.root(a), self.root(b));
        if ra == rb {
            return true;
        }
        let mut e = LinExpr::var(a);
        e.sub(&LinExpr::var(b));
        if !self.assert_lin(e, solver::RelOp::Eq) {
            return false;
        }
        self.aliases.insert(rb, ra);
        self.trail.push(Undo::Alias(rb));
        true
    }

    /// Representative of a solver variable's alias class.
    pub(crate) fn root(&self, mut sv: SVar) -> SVar {
        while let Some(p) = self.aliases.get(&sv) {
            sv = *p;
        }
        sv
    }

    /// Mark the start of a query: what exists now is the world.
    pub(crate) fn set_baseline(&mut self) {
        self.baseline_attrs = self.attrs.len();
        self.baseline_numdifs = self.numdifs.len();
    }

    /// Combined bounds of an alias class (tightest of its members).
    pub(crate) fn class_bounds(&self, sv: SVar) -> (Option<solver::Delta>, Option<solver::Delta>) {
        let r = self.root(sv);
        let mut lower: Option<solver::Delta> = None;
        let mut upper: Option<solver::Delta> = None;
        for v in 0..self.simplex.num_vars() {
            let v = SVar(v);
            if self.root(v) != r {
                continue;
            }
            if let Some(l) = self.simplex.lower(v) {
                if lower.as_ref().is_none_or(|cur| l > cur) {
                    lower = Some(l.clone());
                }
            }
            if let Some(u) = self.simplex.upper(v) {
                if upper.as_ref().is_none_or(|cur| u < cur) {
                    upper = Some(u.clone());
                }
            }
        }
        (lower, upper)
    }

    /// A row defining some member of `sv`'s alias class, if any is basic.
    pub(crate) fn class_row(&self, sv: SVar) -> Option<&LinExpr> {
        let r = self.root(sv);
        (0..self.simplex.num_vars())
            .map(SVar)
            .filter(|v| self.root(*v) == r)
            .find_map(|v| self.simplex.row(v))
    }

    /// The fixed value of an alias class, if the solver exhibits one.
    pub(crate) fn class_value(&self, sv: SVar) -> Option<Number> {
        let r = self.root(sv);
        (0..self.simplex.num_vars())
            .map(SVar)
            .filter(|v| self.root(*v) == r)
            .find_map(|v| self.cheap_value(v))
    }

    // --- attribute terms ---------------------------------------------------

    /// Structural key of the term at `a` modulo bindings; `None` if cyclic.
    fn attr_key(&self, a: Addr) -> Option<AttrKey> {
        let mut key = Vec::new();
        let mut path: Vec<Addr> = Vec::new();
        enum Step {
            Enter(Addr),
            Leave,
        }
        let mut stack = vec![Step::Enter(a)];
        while let Some(step) = stack.pop() {
            match step {
                Step::Leave => {
                    path.pop();
                }
                Step::Enter(x) => {
                    let d = self.deref(x);
                    match &self.heap[d] {
                        Cell::Var(_) => {
                            // A determined numeric variable keys by its value.
                            let val = self.numvar.get(&d).and_then(|sv| self.cheap_value(*sv));
                            key.push(match val {
                                Some(c) => KeyElem::Num(c),
                                None => KeyElem::Var(d),
                            });
                        }
                        Cell::Num(c) => key.push(KeyElem::Num(c.clone())),
                        Cell::Const(s) => key.push(KeyElem::Sym(*s, 0)),
                        Cell::Struct(f, args) => {
                            if path.contains(&d) {
                                return None;
                            }
                            key.push(KeyElem::Sym(*f, args.len()));
                            path.push(d);
                            stack.push(Step::Leave);
                            for &arg in args.iter().rev() {
                                stack.push(Step::Enter(arg));
                            }
                        }
                    }
                }
            }
        }
        Some(key)
    }

    /// The unknown denoted by the attribute term at `a`, registering it (and
    /// suspending on its variables) if new. `None` (with the error set) if
    /// the term is cyclic.
    fn attribute(&mut self, a: Addr) -> Option<SVar> {
        let d = self.deref(a);
        let Some(key) = self.attr_key(d) else {
            self.error = Some(EvalError::CyclicAttributeTerm { term: self.debug_term(d) });
            return None;
        };
        if let Some(sv) = self.attr_index.get(&key) {
            return Some(*sv);
        }
        let sv = self.simplex.new_var();
        self.attr_index.insert(key.clone(), sv);
        self.trail.push(Undo::AttrIndex(key.clone()));
        let id = self.attrs.len();
        let vars: Vec<Addr> = key
            .iter()
            .filter_map(|k| if let KeyElem::Var(v) = k { Some(*v) } else { None })
            .collect();
        self.attrs.push(AttrEntry { term: d, key, svar: sv });
        for v in vars {
            self.suspend(Waker::Attr(id), v);
        }
        Some(sv)
    }

    /// A variable inside an attribute term was bound or determined: recompute
    /// its key and merge with any term that is now equal (congruence).
    fn recanonicalize(&mut self, id: usize) -> bool {
        if id >= self.attrs.len() {
            return true;
        }
        let term = self.attrs[id].term;
        let sv = self.attrs[id].svar;
        let Some(key) = self.attr_key(term) else {
            self.error = Some(EvalError::CyclicAttributeTerm { term: self.debug_term(term) });
            return false;
        };
        if key == self.attrs[id].key {
            return true;
        }
        let old = std::mem::replace(&mut self.attrs[id].key, key.clone());
        self.trail.push(Undo::AttrKey(id, old));
        match self.attr_index.get(&key).copied() {
            Some(other) => {
                if !self.equate(sv, other) {
                    return false;
                }
            }
            None => {
                self.attr_index.insert(key.clone(), sv);
                self.trail.push(Undo::AttrIndex(key.clone()));
            }
        }
        for k in &key {
            if let KeyElem::Var(v) = k {
                self.suspend(Waker::Attr(id), *v);
            }
        }
        true
    }

    /// Minimal rendering for error messages (symbols by index).
    fn debug_term(&self, a: Addr) -> String {
        let d = self.deref(a);
        match &self.heap[d] {
            Cell::Var(_) => format!("_{d}"),
            Cell::Num(n) => n.to_string(),
            Cell::Const(s) => format!("{s:?}"),
            Cell::Struct(f, args) => {
                let inner: Vec<String> = args.iter().map(|x| self.debug_term(*x)).collect();
                format!("{f:?}({})", inner.join(", "))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::symbol::Symbols;

    fn sym(s: &mut Symbols, n: &str) -> Symbol {
        s.intern(n)
    }

    #[test]
    fn unify_constants_and_numbers() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let a1 = st.new_const(sym(&mut syms, "a"));
        let a2 = st.new_const(sym(&mut syms, "a"));
        let b = st.new_const(sym(&mut syms, "b"));
        assert!(st.post_eq(a1, a2));
        assert!(!st.post_eq(a1, b));
        let half = st.new_num(Number::from_ratio(1, 2).unwrap());
        let half2 = st.new_num(Number::from_literal("0.50").unwrap());
        let third = st.new_num(Number::from_ratio(1, 3).unwrap());
        assert!(st.post_eq(half, half2));
        assert!(!st.post_eq(half, third));
        assert!(!st.post_eq(half, a1));
    }

    #[test]
    fn unify_binds_variables_and_aliases() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let x = st.new_var();
        let y = st.new_var();
        let a = st.new_const(sym(&mut syms, "a"));
        assert!(st.post_eq(x, y));
        assert_eq!(st.deref(x), st.deref(y));
        assert!(st.post_eq(y, a));
        assert_eq!(st.deref(x), a);
        assert_eq!(st.deref(y), a);
    }

    #[test]
    fn unify_structures_and_fail_on_mismatch() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let f = sym(&mut syms, "f");
        let g = sym(&mut syms, "g");
        let a = st.new_const(sym(&mut syms, "a"));
        let b = st.new_const(sym(&mut syms, "b"));
        let x = st.new_var();
        let y = st.new_var();
        let fxa = st.new_struct(f, vec![x, a]);
        let fby = st.new_struct(f, vec![b, y]);
        assert!(st.post_eq(fxa, fby));
        assert_eq!(st.deref(x), b);
        assert_eq!(st.deref(y), a);
        let ga = st.new_struct(g, vec![a]);
        let fa = st.new_struct(f, vec![a]);
        assert!(!st.post_eq(ga, fa)); // functor
        let faa = st.new_struct(f, vec![a, a]);
        assert!(!st.post_eq(fa, faa)); // arity
    }

    #[test]
    fn undo_restores_exactly() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let f = sym(&mut syms, "f");
        let x = st.new_var();
        let a = st.new_const(sym(&mut syms, "a"));
        let mark = st.mark();
        let fx = st.new_struct(f, vec![x]);
        let fa = st.new_struct(f, vec![a]);
        assert!(st.post_eq(fx, fa));
        assert_eq!(st.deref(x), a);
        st.undo_to(&mark);
        assert_eq!(st.deref(x), x);
        assert_eq!(st.heap.len(), mark.heap);
        assert_eq!(st.trail.len(), mark.trail);
    }

    #[test]
    fn cyclic_terms_unify_and_terminate() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let f = sym(&mut syms, "f");
        // X = f(X)
        let x = st.new_var();
        let fx = st.new_struct(f, vec![x]);
        assert!(st.post_eq(x, fx));
        // then X = f(f(X)) succeeds (rational trees)
        let ffx = {
            let inner = st.new_struct(f, vec![x]);
            st.new_struct(f, vec![inner])
        };
        assert!(st.post_eq(x, ffx));
        // Y = f(Y), X = Y succeeds
        let y = st.new_var();
        let fy = st.new_struct(f, vec![y]);
        assert!(st.post_eq(y, fy));
        assert!(st.post_eq(x, y));
        // X = f(X, a) then X = f(X, b) fails
        let mut st2 = Store::new();
        let a = st2.new_const(sym(&mut syms, "a"));
        let b = st2.new_const(sym(&mut syms, "b"));
        let z = st2.new_var();
        let fza = st2.new_struct(f, vec![z, a]);
        assert!(st2.post_eq(z, fza));
        let fzb = st2.new_struct(f, vec![z, b]);
        assert!(!st2.post_eq(z, fzb));
    }

    #[test]
    fn dif_immediate_cases() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let a = st.new_const(sym(&mut syms, "a"));
        let a2 = st.new_const(sym(&mut syms, "a"));
        let b = st.new_const(sym(&mut syms, "b"));
        assert!(st.post_dif(a, b)); // satisfied, dropped
        assert!(st.pending_difs().is_empty());
        assert!(!st.post_dif(a, a2)); // already equal
    }

    #[test]
    fn dif_suspends_then_wakes() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let a = st.new_const(sym(&mut syms, "a"));
        let b = st.new_const(sym(&mut syms, "b"));
        let x = st.new_var();
        assert!(st.post_dif(x, a));
        assert_eq!(st.pending_difs().len(), 1);
        let mark = st.mark();
        assert!(!st.post_eq(x, a)); // violates
        st.undo_to(&mark);
        assert!(st.post_eq(x, b)); // satisfies and drops
        assert!(st.pending_difs().is_empty());
        // Order-independent: bind first, then dif.
        let mut st2 = Store::new();
        let y = st2.new_var();
        let a3 = st2.new_const(sym(&mut syms, "a"));
        assert!(st2.post_eq(y, a3));
        assert!(!st2.post_dif(y, a3));
    }

    #[test]
    fn dif_over_structures_suspends_on_deep_variables() {
        let mut syms = Symbols::default();
        let mut st = Store::new();
        let f = sym(&mut syms, "f");
        let x = st.new_var();
        let y = st.new_var();
        assert!(st.post_dif(x, y));
        let z = st.new_var();
        let w = st.new_var();
        let fz = st.new_struct(f, vec![z]);
        let fw = st.new_struct(f, vec![w]);
        assert!(st.post_eq(x, fz));
        assert!(st.post_eq(y, fw)); // still undecided: suspended on Z, W
        assert_eq!(st.pending_difs().len(), 1);
        let mark = st.mark();
        assert!(!st.post_eq(z, w)); // Z = W makes X = Y: violated
        st.undo_to(&mark);
        let one = st.new_num(Number::from(1));
        let two = st.new_num(Number::from(2));
        assert!(st.post_eq(z, one));
        assert!(st.post_eq(w, two)); // now decidably different: dropped
        assert!(st.pending_difs().is_empty());
    }
}
