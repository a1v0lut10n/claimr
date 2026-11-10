// SPDX-License-Identifier: Apache-2.0

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

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use crate::ast::ArithOp;
use crate::number::Number;
use crate::solver::{self, LinExpr, SVar, Simplex};

use super::compile::TTerm;
use super::error::EvalError;
use super::symbol::{LIST, Symbol, Symbols};

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
    /// The would-bind pairs `(variable, term)` of the last trial unification:
    /// the disequation is equivalent to "not all of these bindings". Empty if
    /// the trial also touched the numeric store (no faithful reduced form).
    reduced: Vec<(Addr, Addr)>,
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

/// A pending tree disequation for printing: `(left, right, reduced pairs)`.
pub type PendingDif = (Addr, Addr, Vec<(Addr, Addr)>);

#[derive(Debug, Clone)]
pub(crate) struct NumDif {
    pub d: SVar,
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

/// The length of a list variable (CLM-0015): `|list|` is the numeric heap
/// variable `n` (solver variable `svar`, bounded below by 0). The entry is
/// pending until the spine of `list` is known to end in `'<>'` or in another
/// list variable, whose entry then carries the unknown on.
#[derive(Debug, Clone)]
pub(crate) struct Length {
    pub list: Addr,
    pub n: Addr,
    pub svar: SVar,
    pub pending: bool,
}

/// A pending concatenation `z = x . y` (CLM-0016): `|z| = |x| + |y|` is in
/// the simplex from the start; the cells are unified once the spine of `x`
/// is complete.
#[derive(Debug, Clone)]
pub(crate) struct Concat {
    pub z: Addr,
    pub x: Addr,
    pub y: Addr,
    pub pending: bool,
}

/// The kind a type relation asserts (CLM-0017): Prolog III's `id` and
/// `list`, under Prolog-family names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Atom,
    List,
}

/// A pending type relation `atom(var)` / `list(var)` (CLM-0017): decided
/// when the variable is bound.
#[derive(Debug, Clone)]
pub(crate) struct TypeCon {
    pub var: Addr,
    pub kind: TypeKind,
    pub pending: bool,
}

/// A pending `atom_chars(a, l)` (CLM-0017): decided when `a` is a constant
/// or `l` a complete list of one-character constants.
#[derive(Debug, Clone)]
pub(crate) struct Chars {
    pub a: Addr,
    pub l: Addr,
    pub pending: bool,
}

/// A pending sum `sum(l, s)` (CLM-0025): decided as the linear equation over
/// the elements once the spine of `l` is complete; meanwhile suspended on
/// the open tail.
#[derive(Debug, Clone)]
pub(crate) struct Sum {
    pub l: Addr,
    pub s: Addr,
    pub pending: bool,
}

/// Where a `'<>'` spine ends.
enum Spine {
    /// A proper list of this many cells.
    Nil(usize),
    /// This many cells, then an unbound (non-numeric) variable.
    Open(usize, Addr),
    /// A cycle, or something that is not a list.
    Bad,
}

/// Something suspended on a variable, to be re-examined when it is bound (or,
/// for numeric variables, determined).
#[derive(Debug, Clone, Copy)]
enum Waker {
    Dif(DifId),
    Attr(usize),
    Len(usize),
    Concat(usize),
    Type(usize),
    Chars(usize),
    Sum(usize),
}

#[derive(Debug, Clone)]
enum Undo {
    Cell(Addr, Cell),
    DifPending(DifId),
    Wake(Addr),
    NumVar(Addr),
    AttrIndex(AttrKey),
    AttrKey(usize, AttrKey),
    NumDifPending(usize),
    ProductPending(usize),
    /// CLM-0024: a numeric variable left the candidate set by being bound.
    Undetermined(Addr),
    Alias(SVar),
    AttrTouched(usize),
    LengthPending(usize),
    LenIndex(Addr),
    ConcatPending(usize),
    TypePending(usize),
    CharsPending(usize),
    SumPending(usize),
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
    lengths: usize,
    concats: usize,
    types: usize,
    chars: usize,
    sums: usize,
    simplex: solver::Mark,
}

/// The store. `Clone` since CLM-0027: a table evaluation runs on a copy of
/// the program's world store.
#[derive(Debug, Default, Clone)]
pub struct Store {
    /// The program's symbol table, carried by the store since CLM-0017 so
    /// that `atom_chars` can intern the atom it spells at wake time.
    pub(crate) symbols: Rc<RefCell<Symbols>>,
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
    /// List lengths registered so far (CLM-0015).
    pub(crate) lengths: Vec<Length>,
    /// Unbound list variable → its length entry, so every `|T|` of one
    /// variable is one unknown.
    len_index: HashMap<Addr, usize>,
    /// Concatenations registered so far (CLM-0016).
    pub(crate) concats: Vec<Concat>,
    /// Type relations registered so far (CLM-0017).
    pub(crate) types: Vec<TypeCon>,
    /// `atom_chars` constraints registered so far (CLM-0017).
    pub(crate) chars: Vec<Chars>,
    /// Sums registered so far (CLM-0025).
    pub(crate) sums: Vec<Sum>,
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
    /// World attribute entries looked up during the current query: they are
    /// visible to the answer like entries the query created.
    pub(crate) attr_touched: HashSet<usize>,
    /// CLM-0024: the determination pass's candidates, kept incrementally —
    /// numeric heap variables that are still unbound (CLM-0026: a determined
    /// one is bound to its number, so it leaves the set by that binding), the
    /// pending numeric disequations and the pending products — so that the
    /// pass after every post walks only what is still open, not every entry
    /// ever registered (which made a long numeric derivation quadratic).
    undetermined: BTreeSet<Addr>,
    pending_numdifs: BTreeSet<usize>,
    pending_products: BTreeSet<usize>,
    /// Numeric heap variables touched during the current trial unification.
    touched: Vec<Addr>,
    /// A sticky runtime error (cyclic attribute term).
    pub(crate) error: Option<EvalError>,
    /// A non-linear product still pending at answer time: its operand terms
    /// and operator, rendered by the machine with the query's variable names.
    pub(crate) nonlinear: Option<(Addr, Addr, &'static str)>,
}

impl Store {
    /// A store over the given symbol table — the program's own, shared by
    /// every store a program runs (CLM-0028), so that a symbol interned at run
    /// time in one store means the same name in all of them.
    pub fn with_symbols(symbols: Rc<RefCell<Symbols>>) -> Self {
        Store {
            symbols,
            ..Self::default()
        }
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

    /// CLM-0022: is the term at `a` ground — no unbound variable anywhere in
    /// it, and every numeric variable in it determined by the solver
    /// (exactly, two-sided probes)? A cyclic term terminates through the
    /// visited set. The groundness rule of
    /// docs/design/2026-11-09-negation-disjunction-and-if-then-else.md.
    pub(crate) fn is_ground(&mut self, a: Addr) -> bool {
        let mut seen: HashSet<Addr> = HashSet::new();
        let mut work = vec![a];
        while let Some(x) = work.pop() {
            let d = self.deref(x);
            if !seen.insert(d) {
                continue;
            }
            match self.heap[d].clone() {
                Cell::Var(_) => match self.numvar.get(&d).copied() {
                    Some(sv) if self.simplex.is_determined(sv).is_some() => {}
                    _ => return false,
                },
                Cell::Const(_) | Cell::Num(_) => {}
                Cell::Struct(_, args) => work.extend(args),
            }
        }
        true
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
            lengths: self.lengths.len(),
            concats: self.concats.len(),
            types: self.types.len(),
            chars: self.chars.len(),
            sums: self.sums.len(),
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
                    self.undetermined.remove(&a);
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
                        self.pending_numdifs.insert(id);
                    }
                }
                Undo::ProductPending(id) => {
                    if id < self.products.len() {
                        self.products[id].pending = true;
                        self.pending_products.insert(id);
                    }
                }
                Undo::Undetermined(a) => {
                    self.undetermined.insert(a);
                }
                Undo::Alias(sv) => {
                    self.aliases.remove(&sv);
                }
                Undo::AttrTouched(id) => {
                    self.attr_touched.remove(&id);
                }
                Undo::LengthPending(id) => {
                    if id < self.lengths.len() {
                        self.lengths[id].pending = true;
                    }
                }
                Undo::LenIndex(v) => {
                    self.len_index.remove(&v);
                }
                Undo::ConcatPending(id) => {
                    if id < self.concats.len() {
                        self.concats[id].pending = true;
                    }
                }
                Undo::TypePending(id) => {
                    if id < self.types.len() {
                        self.types[id].pending = true;
                    }
                }
                Undo::CharsPending(id) => {
                    if id < self.chars.len() {
                        self.chars[id].pending = true;
                    }
                }
                Undo::SumPending(id) => {
                    if id < self.sums.len() {
                        self.sums[id].pending = true;
                    }
                }
            }
        }
        self.heap.truncate(mark.heap);
        self.difs.truncate(mark.difs);
        self.attrs.truncate(mark.attrs);
        self.numdifs.truncate(mark.numdifs);
        self.pending_numdifs.split_off(&mark.numdifs);
        self.products.truncate(mark.products);
        self.pending_products.split_off(&mark.products);
        self.lengths.truncate(mark.lengths);
        self.concats.truncate(mark.concats);
        self.types.truncate(mark.types);
        self.chars.truncate(mark.chars);
        self.sums.truncate(mark.sums);
        self.simplex.undo_to(mark.simplex);
        // Definitions of solver variables the simplex dropped (their indices
        // will be reused).
        let live = self.simplex.num_vars();
        self.defs.split_off(&SVar(live));
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
        // A numeric variable (docs/design/2026-11-09-numbers-numeric-
        // variables-and-attribute-terms.md): meeting a number fixes its
        // solver variable and binds it to the number, so from then on it is
        // the number (rule 1); meeting a variable merges; meeting a compound
        // or constant fails — unification never creates an attribute term
        // (rule 3, D4 as written).
        let d = self.deref(t);
        let fixed = self.class_value(sv);
        match self.heap[d].clone() {
            Cell::Num(c) => {
                if let Some(f) = fixed {
                    // Determined in the solver but not yet exhibited by the
                    // cheap pass: equal values bind, different values fail.
                    if f != c {
                        return false;
                    }
                } else if !self.simplex.fix(sv, c) {
                    return false;
                }
                self.touched.push(v);
                self.set(v, Cell::Var(d));
                self.leave_undetermined(v);
                self.fire(v);
                true
            }
            Cell::Var(_) => {
                if let (Some(f), Some(other)) = (&fixed, self.numvar.get(&d).copied()) {
                    if let Some(g) = self.class_value(other) {
                        // Both determined (neither yet exhibited): equal
                        // values are no new information, different values
                        // fail.
                        return *f == g;
                    }
                }
                self.touched.push(v);
                self.set(v, Cell::Var(t));
                self.leave_undetermined(v);
                self.fire(v);
                match self.numvar.get(&d).copied() {
                    Some(other) => self.equate(sv, other),
                    None => {
                        self.set_numvar(d, sv);
                        true
                    }
                }
            }
            Cell::Const(_) | Cell::Struct(..) => false,
        }
    }

    fn set_numvar(&mut self, v: Addr, sv: SVar) {
        self.numvar.insert(v, sv);
        self.undetermined.insert(v);
        self.trail.push(Undo::NumVar(v));
    }

    /// CLM-0024: `v`, a numeric variable, is being bound and leaves the
    /// determination candidates (trailed).
    fn leave_undetermined(&mut self, v: Addr) {
        if self.undetermined.remove(&v) {
            self.trail.push(Undo::Undetermined(v));
        }
    }

    fn suspend(&mut self, w: Waker, v: Addr) {
        let list = self.wake.entry(v).or_default();
        let dup = list.iter().any(|x| match (x, &w) {
            (Waker::Dif(a), Waker::Dif(b)) => a == b,
            (Waker::Attr(a), Waker::Attr(b)) => a == b,
            (Waker::Len(a), Waker::Len(b)) => a == b,
            (Waker::Concat(a), Waker::Concat(b)) => a == b,
            (Waker::Type(a), Waker::Type(b)) => a == b,
            (Waker::Chars(a), Waker::Chars(b)) => a == b,
            (Waker::Sum(a), Waker::Sum(b)) => a == b,
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

    /// A goal operand is numeric if it denotes a number (rule 2 of
    /// docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md):
    /// dereferenced, it is a number cell — a literal, an arithmetic result,
    /// or a variable bound to a number by whatever route — or an unbound
    /// numeric variable.
    fn is_numericish(&self, a: Addr) -> bool {
        let d = self.deref(a);
        match &self.heap[d] {
            Cell::Num(_) => true,
            Cell::Var(_) => self.numvar.contains_key(&d),
            Cell::Const(_) | Cell::Struct(..) => false,
        }
    }

    // --- tree disequations -------------------------------------------------

    /// Post the disequation `a != b`. On failure the store is restored.
    pub fn post_dif(&mut self, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        let id = self.difs.len();
        self.difs.push(Dif {
            a,
            b,
            pending: true,
            reduced: Vec::new(),
        });
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
        let (a, b, pending) = {
            let d = &self.difs[id];
            (d.a, d.b, d.pending)
        };
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
        // Would-bind pairs: variables the trial bound, with their targets.
        let pairs: Vec<(Addr, Addr)> = self.trail[mark.trail..]
            .iter()
            .filter_map(|u| match u {
                Undo::Cell(addr, Cell::Var(x)) if x == addr => match self.heap[*addr] {
                    Cell::Var(t) => Some((*addr, t)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        let numeric_change = self.simplex.changed_since(mark.simplex);
        let mut would_bind: Vec<Addr> = pairs.iter().map(|(v, _)| *v).collect();
        would_bind.extend(self.touched.iter().copied());
        self.undo_to(&mark);
        if would_bind.is_empty() && !numeric_change {
            return false; // already equal
        }
        self.difs[id].reduced = if numeric_change { Vec::new() } else { pairs };
        for v in would_bind {
            self.suspend(Waker::Dif(id), v);
        }
        true
    }

    /// The pending disequations: `(left, right, reduced pairs)`.
    pub fn pending_difs(&self) -> Vec<PendingDif> {
        self.difs
            .iter()
            .filter(|d| d.pending)
            .map(|d| (d.a, d.b, d.reduced.clone()))
            .collect()
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
                    Waker::Len(id) => self.recheck_len(id),
                    Waker::Concat(id) => self.recheck_concat(id),
                    Waker::Type(id) => self.recheck_type(id),
                    Waker::Chars(id) => self.recheck_chars(id),
                    Waker::Sum(id) => self.recheck_sum(id),
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
            // Determined heap variables: the open candidates (CLM-0024), in
            // address order. A determined variable is bound to its number
            // (CLM-0026, rule 1): from then on it is a tree leaf for
            // unification and a number for every goal, and it leaves the
            // candidates by that binding.
            let candidates: Vec<(Addr, SVar)> = self
                .undetermined
                .iter()
                .filter(|a| self.is_unbound_var(**a))
                .filter_map(|a| self.numvar.get(a).map(|s| (*a, *s)))
                .collect();
            for (a, sv) in candidates {
                let val = if exact {
                    self.simplex.is_determined(sv)
                } else {
                    self.cheap_value(sv)
                };
                if let Some(c) = val {
                    if !self.simplex.fix(sv, c.clone()) {
                        return false;
                    }
                    let n = self.new_num(c);
                    self.set(a, Cell::Var(n));
                    self.leave_undetermined(a);
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
            // Numeric disequations still pending (CLM-0024).
            for id in self.pending_numdifs.clone() {
                if !self.numdifs[id].pending {
                    continue;
                }
                let d = self.numdifs[id].d;
                let val = if exact {
                    self.simplex.is_determined(d)
                } else {
                    self.cheap_value(d)
                };
                if let Some(c) = val {
                    if c.is_zero() {
                        return false;
                    }
                    self.numdifs[id].pending = false;
                    self.pending_numdifs.remove(&id);
                    self.trail.push(Undo::NumDifPending(id));
                    progress = true;
                }
            }
            // Delayed products still pending (CLM-0024).
            for id in self.pending_products.clone() {
                if !self.products[id].pending {
                    continue;
                }
                let p = self.products[id].clone();
                let (av, bv) = if exact {
                    (
                        self.simplex.is_determined(p.a),
                        self.simplex.is_determined(p.b),
                    )
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
                    self.pending_products.remove(&id);
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
                    Waker::Len(id) => self.recheck_len(id),
                    Waker::Concat(id) => self.recheck_concat(id),
                    Waker::Type(id) => self.recheck_type(id),
                    Waker::Chars(id) => self.recheck_chars(id),
                    Waker::Sum(id) => self.recheck_sum(id),
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
    /// variable's solver variable, or an attribute term's unknown. A solver
    /// variable whose value the store has already determined is its value
    /// (CLM-0024): the constraint being built then folds as if the number
    /// had been written, so a value that flows through arithmetic defines no
    /// row and asserts no slack — in every solution of the store the variable
    /// has that value, so nothing posted changes, and the trail undoes the
    /// posted constraint with the determination it relied on.
    pub(crate) fn unknown_of(&mut self, a: Addr) -> Option<LinExpr> {
        let d = self.deref(a);
        let sv = match self.heap[d].clone() {
            Cell::Num(c) => return Some(LinExpr::constant(c)),
            Cell::Var(_) => match self.numvar.get(&d) {
                Some(sv) => *sv,
                None => {
                    // A variable under a pending `atom`/`list` relation
                    // or the atom side of `atom_chars` is not a number
                    // (CLM-0017).
                    if self.has_pending_type(d) {
                        return None;
                    }
                    let sv = self.simplex.new_var();
                    self.set_numvar(d, sv);
                    sv
                }
            },
            Cell::Const(_) | Cell::Struct(..) => self.attribute(d)?,
        };
        Some(match self.class_value(sv) {
            Some(c) => LinExpr::constant(c),
            None => LinExpr::var(sv),
        })
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
            self.numdifs.push(NumDif { d, pending: true });
            self.pending_numdifs.insert(id);
            match self.simplex.is_determined(d) {
                Some(c) if c.is_zero() => Some(false),
                Some(_) => {
                    self.numdifs[id].pending = false;
                    self.pending_numdifs.remove(&id);
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
            // Ground arithmetic folds to a number: no solver variable, no
            // row (design record 2026-11-09-fold-ground-arithmetic-before-
            // the-solver.md).
            if let Some(c) = e.as_constant() {
                let v = -c;
                return Some(self.new_num(v));
            }
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
            // Ground arithmetic folds to a number: no solver variable, no
            // row (design record 2026-11-09-fold-ground-arithmetic-before-
            // the-solver.md). Division by zero fails, as below.
            if let (Some(x), Some(y)) = (la.as_constant(), lb.as_constant()) {
                let v = match op {
                    ArithOp::Add => x + y,
                    ArithOp::Sub => x - y,
                    ArithOp::Mul => x * y,
                    ArithOp::Div => x.checked_div(y)?,
                };
                return Some(self.new_num(v));
            }
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

    fn delay_product(
        &mut self,
        kind: ProductKind,
        a: SVar,
        b: SVar,
        a_addr: Addr,
        b_addr: Addr,
    ) -> Option<Addr> {
        let n_addr = self.new_var();
        let n = self.simplex.new_var();
        self.set_numvar(n_addr, n);
        let id = self.products.len();
        self.products.push(Product {
            n,
            kind,
            a,
            b,
            a_addr,
            b_addr,
            pending: true,
        });
        self.pending_products.insert(id);
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
        self.attr_touched.clear();
    }

    /// True if attribute entry `id` is visible to the query's answers:
    /// created by the query, or a world entry the query looked up.
    pub(crate) fn attr_visible(&self, id: usize) -> bool {
        id >= self.baseline_attrs || self.attr_touched.contains(&id)
    }

    /// The fixed value of an alias class, if the solver exhibits one. The
    /// class is its root and the aliased variables whose root it is; only
    /// equated variables are in `aliases`, so this costs the number of
    /// aliases, not of solver variables (CLM-0024: `unknown_of` asks on
    /// every post).
    pub(crate) fn class_value(&self, sv: SVar) -> Option<Number> {
        let r = self.root(sv);
        if let Some(c) = self.cheap_value(r) {
            return Some(c);
        }
        if sv != r {
            if let Some(c) = self.cheap_value(sv) {
                return Some(c);
            }
        }
        self.aliases
            .keys()
            .filter(|v| **v != sv && self.root(**v) == r)
            .find_map(|v| self.cheap_value(*v))
    }

    // --- list lengths (CLM-0015) -------------------------------------------

    /// Follow the `'<>'` spine from `a`.
    fn walk_spine(&self, a: Addr) -> Spine {
        let mut seen: HashSet<Addr> = HashSet::new();
        let mut k = 0;
        let mut cur = a;
        loop {
            let d = self.deref(cur);
            match &self.heap[d] {
                Cell::Struct(f, args) if *f == LIST && args.len() == 2 => {
                    if !seen.insert(d) {
                        return Spine::Bad;
                    }
                    k += 1;
                    cur = args[1];
                }
                Cell::Const(c) if *c == LIST => return Spine::Nil(k),
                Cell::Var(_) if !self.numvar.contains_key(&d) => return Spine::Open(k, d),
                _ => return Spine::Bad,
            }
        }
    }

    /// The length entry of the unbound list variable `v`, created if new:
    /// a numeric heap variable bounded below by 0, suspended on `v` (its
    /// spine) and on itself (its determination).
    fn length_entry(&mut self, v: Addr) -> usize {
        if let Some(id) = self.len_index.get(&v) {
            return *id;
        }
        let n = self.new_var();
        let svar = self.simplex.new_var();
        self.set_numvar(n, svar);
        // A fresh variable's first bound cannot fail.
        self.simplex
            .assert_constraint(&LinExpr::var(svar), solver::RelOp::Ge);
        let id = self.lengths.len();
        self.lengths.push(Length {
            list: v,
            n,
            svar,
            pending: true,
        });
        self.len_index.insert(v, id);
        self.trail.push(Undo::LenIndex(v));
        self.suspend(Waker::Len(id), v);
        self.suspend(Waker::Len(id), n);
        id
    }

    /// `|a|` as a heap address: a number when the spine is complete (the
    /// ground length folds, like ground arithmetic), else a numeric variable
    /// equal to the cells counted plus the length of the open tail. `None`
    /// when `a` is cyclic or not a list.
    pub fn post_len(&mut self, a: Addr) -> Option<Addr> {
        match self.walk_spine(a) {
            Spine::Nil(k) => Some(self.new_num(Number::from_ratio(k, 1)?)),
            Spine::Open(k, v) => {
                let id = self.length_entry(v);
                let Length { n, svar, .. } = self.lengths[id].clone();
                if k == 0 {
                    return Some(n);
                }
                let mut e = LinExpr::var(svar);
                e.constant = Number::from_ratio(k, 1)?;
                Some(self.define_fresh(e))
            }
            Spine::Bad => None,
        }
    }

    /// A length entry's list variable was bound, or its length determined.
    /// Bound: the spine is followed and `|list| = k` or `|list| = k + |tail|`
    /// asserted; a cycle or a non-list fails. Determined while the list is
    /// still unbound: the value must be a non-negative integer `n`, and the
    /// list becomes `<_1, …, _n>` (Prolog III's solved form for a list of
    /// known length), which re-fires this waker to close the entry.
    fn recheck_len(&mut self, id: usize) -> bool {
        if id >= self.lengths.len() || !self.lengths[id].pending {
            return true;
        }
        let Length { list, svar, .. } = self.lengths[id].clone();
        let d = self.deref(list);
        // Still the same unbound variable: only its length can have changed.
        // (Bound to another variable, `d != list`: that is the spine case
        // below, `|list| = |d|` through `d`'s entry.)
        if d == list {
            let Some(c) = self.class_value(svar) else {
                return true;
            };
            let Some(k) = as_count(&c) else {
                return false;
            };
            let mut t = self.new_const(LIST);
            for _ in 0..k {
                let h = self.new_var();
                t = self.new_struct(LIST, vec![h, t]);
            }
            return self.unify(d, t);
        }
        let mut e = LinExpr::var(svar);
        match self.walk_spine(d) {
            Spine::Nil(k) => {
                let Some(k) = Number::from_ratio(k, 1) else {
                    return false;
                };
                e.constant = -k;
            }
            Spine::Open(k, v) => {
                let Some(k) = Number::from_ratio(k, 1) else {
                    return false;
                };
                let other = self.length_entry(v);
                e.sub(&LinExpr::var(self.lengths[other].svar));
                e.constant = -k;
            }
            Spine::Bad => return false,
        }
        self.lengths[id].pending = false;
        self.trail.push(Undo::LengthPending(id));
        self.assert_lin(e, solver::RelOp::Eq)
    }

    // --- list concatenation (CLM-0016) ---------------------------------------

    /// The heads along the complete spine of `a` (the caller has walked it).
    fn spine_heads(&self, a: Addr) -> Vec<Addr> {
        let mut heads = Vec::new();
        let mut cur = a;
        loop {
            let d = self.deref(cur);
            match &self.heap[d] {
                Cell::Struct(f, args) if *f == LIST && args.len() == 2 => {
                    heads.push(args[0]);
                    cur = args[1];
                }
                _ => return heads,
            }
        }
    }

    /// The chain of `heads` ending in `tail`, sharing the head cells.
    fn chain(&mut self, heads: &[Addr], tail: Addr) -> Addr {
        let mut t = tail;
        for &h in heads.iter().rev() {
            t = self.new_struct(LIST, vec![h, t]);
        }
        t
    }

    /// `x . y` as a heap address. A complete `x` gives the chain of its heads
    /// ending in `y` at once (the operation face). An open `x` gives a fresh
    /// list variable `z` with `|z| = |x| + |y|` posted and the concatenation
    /// pending until `x`'s spine is complete (the relation face; Prolog III's
    /// `=.`). A cyclic or non-list `x`, or a non-list `y`, fails.
    pub fn post_concat(&mut self, x: Addr, y: Addr) -> Option<Addr> {
        let mark = self.mark();
        let r = (|| match self.walk_spine(x) {
            Spine::Nil(_) => {
                let heads = self.spine_heads(x);
                Some(self.chain(&heads, y))
            }
            Spine::Open(_, t) => {
                let z = self.new_var();
                let lx = self.post_len(x)?;
                let ly = self.post_len(y)?;
                let lz = self.post_len(z)?;
                let mut e = self.unknown_of(lz)?;
                e.sub(&self.unknown_of(lx)?);
                e.sub(&self.unknown_of(ly)?);
                if !self.assert_lin(e, solver::RelOp::Eq) {
                    return None;
                }
                let id = self.concats.len();
                self.concats.push(Concat {
                    z,
                    x,
                    y,
                    pending: true,
                });
                self.suspend(Waker::Concat(id), t);
                if self.settle() { Some(z) } else { None }
            }
            Spine::Bad => None,
        })();
        if r.is_none() {
            self.undo_to(&mark);
        }
        r
    }

    /// The open tail of a pending concatenation's left operand was bound:
    /// complete the concatenation if the spine now ends, else wait on the
    /// new tail.
    fn recheck_concat(&mut self, id: usize) -> bool {
        if id >= self.concats.len() || !self.concats[id].pending {
            return true;
        }
        let Concat { z, x, y, .. } = self.concats[id].clone();
        match self.walk_spine(x) {
            Spine::Nil(_) => {
                self.concats[id].pending = false;
                self.trail.push(Undo::ConcatPending(id));
                let heads = self.spine_heads(x);
                let c = self.chain(&heads, y);
                self.unify(z, c)
            }
            Spine::Open(_, t) => {
                self.suspend(Waker::Concat(id), t);
                true
            }
            Spine::Bad => false,
        }
    }

    /// The length unknown of an unbound list variable, if it has one.
    pub(crate) fn length_svar(&self, a: Addr) -> Option<SVar> {
        let d = self.deref(a);
        self.len_index.get(&d).map(|id| self.lengths[*id].svar)
    }

    /// Pending concatenations, for printing: `(z, x, y)`.
    pub fn pending_concats(&self) -> Vec<(Addr, Addr, Addr)> {
        self.concats
            .iter()
            .filter(|c| c.pending)
            .map(|c| (c.z, c.x, c.y))
            .collect()
    }

    // --- type relations and atom_chars (CLM-0017) ------------------------------

    /// Is `d` held to be an atom or a list by a pending relation — a type
    /// relation, or the atom side of an `atom_chars`?
    fn has_pending_type(&self, d: Addr) -> bool {
        self.types
            .iter()
            .any(|t| t.pending && self.deref(t.var) == d)
            || self.chars.iter().any(|c| c.pending && self.deref(c.a) == d)
    }

    /// `number(a)`: a number or a numeric variable; an unbound plain
    /// variable becomes numeric (unless a type relation holds it); any
    /// tree fails.
    pub fn post_number(&mut self, a: Addr) -> bool {
        let d = self.deref(a);
        match self.heap[d].clone() {
            Cell::Num(_) => true,
            Cell::Var(_) => {
                if self.numvar.contains_key(&d) {
                    return true;
                }
                let mark = self.mark();
                if self.unknown_of(d).is_some() && self.settle() {
                    true
                } else {
                    self.undo_to(&mark);
                    false
                }
            }
            Cell::Const(_) | Cell::Struct(..) => false,
        }
    }

    /// `atom(a)` / `list(a)`: decided now when `a` is bound, else pending
    /// on the variable. A numeric variable is not an atom nor a list.
    pub fn post_type(&mut self, a: Addr, kind: TypeKind) -> bool {
        let mark = self.mark();
        let id = self.types.len();
        self.types.push(TypeCon {
            var: a,
            kind,
            pending: true,
        });
        if self.recheck_type(id) && self.settle() {
            true
        } else {
            self.undo_to(&mark);
            false
        }
    }

    fn recheck_type(&mut self, id: usize) -> bool {
        if id >= self.types.len() || !self.types[id].pending {
            return true;
        }
        let TypeCon { var, kind, .. } = self.types[id].clone();
        let d = self.deref(var);
        let decided = match self.heap[d].clone() {
            Cell::Var(_) => {
                // A numeric variable is neither; two relations of different
                // kinds on one variable cannot both hold.
                if self.numvar.contains_key(&d)
                    || self.types.iter().enumerate().any(|(i, t)| {
                        i != id && t.pending && t.kind != kind && self.deref(t.var) == d
                    })
                {
                    return false;
                }
                self.suspend(Waker::Type(id), d);
                return true;
            }
            Cell::Const(c) => match kind {
                TypeKind::Atom => c != LIST,
                TypeKind::List => c == LIST,
            },
            Cell::Struct(f, args) => match kind {
                TypeKind::Atom => false,
                TypeKind::List => f == LIST && args.len() == 2,
            },
            Cell::Num(_) => false,
        };
        self.types[id].pending = false;
        self.trail.push(Undo::TypePending(id));
        decided
    }

    /// `atom_chars(a, l)`: decided when `a` is a constant (other than `'<>'`)
    /// or `l` a complete list of one-character constants; fails on any
    /// other bound shape; otherwise pending on the variables involved.
    pub fn post_atom_chars(&mut self, a: Addr, l: Addr) -> bool {
        let mark = self.mark();
        let id = self.chars.len();
        self.chars.push(Chars {
            a,
            l,
            pending: true,
        });
        if self.recheck_chars(id) && self.settle() {
            true
        } else {
            self.undo_to(&mark);
            false
        }
    }

    fn close_chars(&mut self, id: usize) {
        self.chars[id].pending = false;
        self.trail.push(Undo::CharsPending(id));
    }

    fn recheck_chars(&mut self, id: usize) -> bool {
        if id >= self.chars.len() || !self.chars[id].pending {
            return true;
        }
        let Chars { a, l, .. } = self.chars[id].clone();
        let da = self.deref(a);
        match self.heap[da].clone() {
            Cell::Const(c) if c != LIST => {
                // The atom is known: `l` is the list of its characters.
                self.close_chars(id);
                let name = self.symbols.borrow().name(c).to_string();
                let mut t = self.new_const(LIST);
                for ch in name.chars().rev() {
                    let s = self.symbols.borrow_mut().intern(&ch.to_string());
                    let h = self.new_const(s);
                    t = self.new_struct(LIST, vec![h, t]);
                }
                return self.unify(l, t);
            }
            Cell::Const(_) | Cell::Num(_) | Cell::Struct(..) => {
                self.close_chars(id);
                return false;
            }
            Cell::Var(_) => {
                if self.numvar.contains_key(&da) {
                    self.close_chars(id);
                    return false;
                }
            }
        }
        // `a` is an unbound plain variable: spell it from `l` if possible.
        let open_tail = match self.walk_spine(l) {
            Spine::Bad => {
                self.close_chars(id);
                return false;
            }
            Spine::Nil(_) => None,
            Spine::Open(_, t) => Some(t),
        };
        let mut name = String::new();
        let mut waits = Vec::new();
        for h in self.spine_heads(l) {
            let dh = self.deref(h);
            match self.heap[dh].clone() {
                Cell::Const(c) if c != LIST => {
                    let n = self.symbols.borrow().name(c).to_string();
                    if n.chars().count() != 1 {
                        self.close_chars(id);
                        return false;
                    }
                    name.push_str(&n);
                }
                Cell::Var(_) if !self.numvar.contains_key(&dh) => waits.push(dh),
                _ => {
                    self.close_chars(id);
                    return false;
                }
            }
        }
        waits.extend(open_tail);
        if waits.is_empty() {
            self.close_chars(id);
            let s = self.symbols.borrow_mut().intern(&name);
            let c = self.new_const(s);
            return self.unify(a, c);
        }
        self.suspend(Waker::Chars(id), da);
        for v in waits {
            self.suspend(Waker::Chars(id), v);
        }
        true
    }

    /// Pending type relations, for printing: `(variable, kind)`.
    pub fn pending_types(&self) -> Vec<(Addr, TypeKind)> {
        self.types
            .iter()
            .filter(|t| t.pending)
            .map(|t| (t.var, t.kind))
            .collect()
    }

    /// Pending `atom_chars` constraints, for printing: `(a, l)`.
    pub fn pending_chars(&self) -> Vec<(Addr, Addr)> {
        self.chars
            .iter()
            .filter(|c| c.pending)
            .map(|c| (c.a, c.l))
            .collect()
    }

    // --- sums (CLM-0025) -----------------------------------------------------

    /// `sum(l, s)`: `s` is the sum of the elements of the list `l`. A
    /// complete spine posts the linear equation `s = e1 + … + en` through the
    /// solver's view of each element (a number, a numeric variable, or an
    /// attribute term, as in every numeric position; a variable held to be
    /// an atom or a list fails); an open spine registers a pending entry
    /// suspended on the tail; a cycle or a non-list fails.
    pub fn post_sum(&mut self, l: Addr, s: Addr) -> bool {
        let mark = self.mark();
        let ok = self.sum_step(l, s, None) && self.settle();
        if !ok {
            self.undo_to(&mark);
        }
        ok
    }

    /// Decide or (re)register the sum; `existing` is the entry being
    /// re-checked, if any.
    fn sum_step(&mut self, l: Addr, s: Addr, existing: Option<usize>) -> bool {
        match self.walk_spine(l) {
            Spine::Nil(_) => {
                let heads = self.spine_heads(l);
                let Some(mut e) = self.unknown_of(s) else {
                    return false;
                };
                for h in heads {
                    let Some(eh) = self.unknown_of(h) else {
                        return false;
                    };
                    e.sub(&eh);
                }
                if let Some(id) = existing {
                    self.sums[id].pending = false;
                    self.trail.push(Undo::SumPending(id));
                }
                self.assert_lin(e, solver::RelOp::Eq)
            }
            Spine::Open(_, v) => {
                let id = match existing {
                    Some(id) => id,
                    None => {
                        let id = self.sums.len();
                        self.sums.push(Sum {
                            l,
                            s,
                            pending: true,
                        });
                        id
                    }
                };
                self.suspend(Waker::Sum(id), v);
                true
            }
            Spine::Bad => false,
        }
    }

    /// A pending sum's list tail was bound: decide, or wait on the new tail.
    fn recheck_sum(&mut self, id: usize) -> bool {
        if id >= self.sums.len() || !self.sums[id].pending {
            return true;
        }
        let Sum { l, s, .. } = self.sums[id].clone();
        self.sum_step(l, s, Some(id))
    }

    /// Pending sums, for printing: `(l, s)`.
    pub fn pending_sums(&self) -> Vec<(Addr, Addr)> {
        self.sums
            .iter()
            .filter(|c| c.pending)
            .map(|c| (c.l, c.s))
            .collect()
    }

    /// CLM-0025: a closed copy of the ground term at `a` — constants,
    /// numbers (a determined numeric variable as its value) and compounds —
    /// which survives the undo of the solution it was taken from. `None` on
    /// an unbound or undetermined variable, or a cycle. Iterative.
    pub(crate) fn snapshot(&mut self, a: Addr) -> Option<TTerm> {
        enum Step {
            Enter(Addr),
            Build(Addr),
        }
        let mut on_path: HashSet<Addr> = HashSet::new();
        let mut work = vec![Step::Enter(a)];
        let mut out: Vec<TTerm> = Vec::new();
        while let Some(step) = work.pop() {
            match step {
                Step::Enter(x) => {
                    let d = self.deref(x);
                    match self.heap[d].clone() {
                        Cell::Var(_) => {
                            let sv = *self.numvar.get(&d)?;
                            let c = self.simplex.is_determined(sv)?;
                            out.push(TTerm::Number(c));
                        }
                        Cell::Const(c) => out.push(TTerm::Const(c)),
                        Cell::Num(n) => out.push(TTerm::Number(n)),
                        Cell::Struct(_, args) => {
                            if !on_path.insert(d) {
                                return None;
                            }
                            work.push(Step::Build(d));
                            work.extend(args.iter().rev().map(|a| Step::Enter(*a)));
                        }
                    }
                }
                Step::Build(d) => {
                    on_path.remove(&d);
                    let Cell::Struct(f, args) = self.heap[d].clone() else {
                        return None;
                    };
                    let kids = out.split_off(out.len() - args.len());
                    out.push(TTerm::Compound(f, kids));
                }
            }
        }
        out.pop()
    }

    /// CLM-0027: may this store hold a constraint beyond its world that a
    /// ground answer would drop? A conservative, cheap test on a store built
    /// from the world: any pending tree or numeric disequation, delayed list
    /// or type relation or sum, any attribute term created or looked up, or
    /// any numeric variable still undetermined. False means the solution is
    /// a fact relative to the world; true sends the caller to the exact test
    /// (the answer's projection).
    pub(crate) fn has_residual(&self) -> bool {
        !self.undetermined.is_empty()
            || self.attrs.len() > self.baseline_attrs
            || !self.attr_touched.is_empty()
            || self.difs.iter().any(|d| d.pending)
            || !self.pending_numdifs.is_empty()
            || !self.pending_products.is_empty()
            || self.lengths.iter().any(|l| l.pending)
            || self.concats.iter().any(|c| c.pending)
            || self.types.iter().any(|t| t.pending)
            || self.chars.iter().any(|c| c.pending)
            || self.sums.iter().any(|s| s.pending)
    }

    /// CLM-0027: make visible in this store (a clone of the world) the
    /// attribute entries that `other`, a store built from the same world,
    /// has created or looked up — by key, so only ground attribute terms
    /// match — so that the two stores project onto the same unknowns.
    pub(crate) fn touch_like(&mut self, other: &Store) {
        for (id, entry) in other.attrs.iter().enumerate() {
            if !other.attr_visible(id) {
                continue;
            }
            if let Some(sv) = self.attr_index.get(&entry.key).copied() {
                if let Some(mine) = self.attrs.iter().position(|e| e.svar == sv) {
                    self.attr_touched.insert(mine);
                }
            }
        }
    }

    /// CLM-0027: the *variant* of the call at `a` — the term as a template
    /// with every unbound variable (numeric or not: a caller's constraints on
    /// it are applied when the answers are consumed) replaced by a template
    /// variable numbered in order of first occurrence, so two calls that are
    /// renamings of each other give the same key. `None` on a cycle.
    /// Iterative.
    pub(crate) fn variant_key(&self, a: Addr) -> Option<TTerm> {
        enum Step {
            Enter(Addr),
            Build(Addr),
        }
        let mut vars: HashMap<Addr, u32> = HashMap::new();
        let mut on_path: HashSet<Addr> = HashSet::new();
        let mut work = vec![Step::Enter(a)];
        let mut out: Vec<TTerm> = Vec::new();
        while let Some(step) = work.pop() {
            match step {
                Step::Enter(x) => {
                    let d = self.deref(x);
                    match self.heap[d].clone() {
                        Cell::Var(_) => {
                            let n = vars.len() as u32;
                            let i = *vars.entry(d).or_insert(n);
                            out.push(TTerm::Var(i));
                        }
                        Cell::Const(c) => out.push(TTerm::Const(c)),
                        Cell::Num(n) => out.push(TTerm::Number(n)),
                        Cell::Struct(_, args) => {
                            if !on_path.insert(d) {
                                return None;
                            }
                            work.push(Step::Build(d));
                            work.extend(args.iter().rev().map(|a| Step::Enter(*a)));
                        }
                    }
                }
                Step::Build(d) => {
                    on_path.remove(&d);
                    let Cell::Struct(f, args) = self.heap[d].clone() else {
                        return None;
                    };
                    let kids = out.split_off(out.len() - args.len());
                    out.push(TTerm::Compound(f, kids));
                }
            }
        }
        out.pop()
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
            self.error = Some(EvalError::CyclicAttributeTerm {
                term: self.debug_term(d),
            });
            return None;
        };
        if let Some(sv) = self.attr_index.get(&key).copied() {
            // A world entry touched by the query becomes visible to answers.
            if let Some(id) = self.attrs.iter().position(|e| e.svar == sv) {
                if id < self.baseline_attrs && self.attr_touched.insert(id) {
                    self.trail.push(Undo::AttrTouched(id));
                }
            }
            return Some(sv);
        }
        let sv = self.simplex.new_var();
        self.attr_index.insert(key.clone(), sv);
        self.trail.push(Undo::AttrIndex(key.clone()));
        let id = self.attrs.len();
        let vars: Vec<Addr> = key
            .iter()
            .filter_map(|k| {
                if let KeyElem::Var(v) = k {
                    Some(*v)
                } else {
                    None
                }
            })
            .collect();
        self.attrs.push(AttrEntry {
            term: d,
            key,
            svar: sv,
        });
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
            self.error = Some(EvalError::CyclicAttributeTerm {
                term: self.debug_term(term),
            });
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

/// A non-negative integer as a count, if the number is one.
fn as_count(c: &Number) -> Option<usize> {
    if !c.is_integer() || c.is_negative() {
        return None;
    }
    usize::try_from(c.numer()).ok()
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
        let mut st = Store::default();
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
        let mut st = Store::default();
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
        let mut st = Store::default();
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
        let mut st = Store::default();
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
        let mut st = Store::default();
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
        let mut st2 = Store::default();
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
        let mut st = Store::default();
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
        let mut st = Store::default();
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
        let mut st2 = Store::default();
        let y = st2.new_var();
        let a3 = st2.new_const(sym(&mut syms, "a"));
        assert!(st2.post_eq(y, a3));
        assert!(!st2.post_dif(y, a3));
    }

    #[test]
    fn dif_over_structures_suspends_on_deep_variables() {
        let mut syms = Symbols::default();
        let mut st = Store::default();
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

    #[test]
    fn ground_arithmetic_folds_without_solver_variables() {
        let mut st = Store::default();
        let n = |k: i64| Number::from_ratio(k, 1).unwrap();
        let before = st.simplex.num_vars();
        let ten = st.new_num(n(10));
        let three = st.new_num(n(3));
        let zero = st.new_num(n(0));

        let prod = st.post_arith(ArithOp::Mul, ten, three).unwrap();
        assert!(matches!(&st.heap[st.deref(prod)], Cell::Num(v) if *v == n(30)));
        let quot = st.post_arith(ArithOp::Div, ten, three).unwrap();
        assert!(matches!(
            &st.heap[st.deref(quot)],
            Cell::Num(v) if *v == Number::from_ratio(10, 3).unwrap()
        ));
        let neg = st.post_neg(quot).unwrap();
        assert!(matches!(
            &st.heap[st.deref(neg)],
            Cell::Num(v) if *v == Number::from_ratio(-10, 3).unwrap()
        ));
        let sum = st.post_arith(ArithOp::Add, prod, neg).unwrap();
        assert!(matches!(
            &st.heap[st.deref(sum)],
            Cell::Num(v) if *v == Number::from_ratio(80, 3).unwrap()
        ));
        assert_eq!(
            st.simplex.num_vars(),
            before,
            "ground terms allocate nothing"
        );

        // Ground division by zero fails and allocates nothing.
        assert!(st.post_arith(ArithOp::Div, ten, zero).is_none());
        assert_eq!(st.simplex.num_vars(), before);

        // A non-ground operand still goes to the solver.
        let x = st.new_var();
        st.post_arith(ArithOp::Mul, ten, x).unwrap();
        assert!(st.simplex.num_vars() > before);
    }

    /// CLM-0024: the determination candidates kept incrementally agree with a
    /// recomputation from the registered entries, through posts, binds,
    /// determinations and undos.
    #[test]
    fn determination_candidates_agree_with_a_recomputation() {
        use crate::ast::RelOp;
        fn check(st: &Store) {
            let expect: BTreeSet<Addr> = st
                .numvar
                .keys()
                .copied()
                .filter(|a| st.is_unbound_var(*a))
                .collect();
            assert_eq!(st.undetermined, expect, "undetermined");
            let expect: BTreeSet<usize> = (0..st.numdifs.len())
                .filter(|i| st.numdifs[*i].pending)
                .collect();
            assert_eq!(st.pending_numdifs, expect, "pending numdifs");
            let expect: BTreeSet<usize> = (0..st.products.len())
                .filter(|i| st.products[*i].pending)
                .collect();
            assert_eq!(st.pending_products, expect, "pending products");
        }
        let mut st = Store::default();
        let n = |k: i64| st_num(k);
        fn st_num(k: i64) -> Number {
            Number::from_ratio(k, 1).unwrap()
        }
        let zero = st.new_num(n(0));
        let two = st.new_num(n(2));
        let (x, y, z, w) = (st.new_var(), st.new_var(), st.new_var(), st.new_var());
        assert!(st.post_rel(RelOp::Gt, x, zero)); // x numeric, open
        assert!(st.post_rel(RelOp::Gt, y, zero));
        assert!(st.post_numdif(x, two)); // pending: x != 2
        let prod = st.post_arith(crate::ast::ArithOp::Mul, x, y).unwrap(); // pending product
        assert!(st.post_rel(RelOp::Gt, prod, zero));
        check(&st);
        let mark = st.mark();
        assert!(st.post_eq_goal(y, two)); // y determined and bound; product linearised
        check(&st);
        assert!(st.post_eq(z, w)); // plain variables: no candidates involved
        assert!(st.post_eq(x, z)); // x bound to a plain variable: leaves the set
        check(&st);
        let _ = st.post_eq_goal(z, two); // either outcome keeps the sets consistent
        check(&st);
        st.undo_to(&mark);
        check(&st);
        assert!(!st.post_eq_goal(x, two)); // x != 2 pending: fails
        check(&st);
        let inner = st.mark();
        assert!(st.post_rel(RelOp::Lt, x, two));
        check(&st);
        st.undo_to(&inner);
        check(&st);
    }
}
