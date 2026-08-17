//! The tree part of the constraint store: a term heap, variable bindings,
//! the trail (Van Caneghem's *pile de restauration*), suspension of
//! disequations on variables, unification over rational trees, and `dif`.
//!
//! Every mutation is recorded on the trail so that [`Store::undo_to`] restores
//! an earlier [`Mark`] exactly — this is what chronological backtracking and
//! trial unification rely on.

use std::collections::HashMap;

use crate::number::Number;

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

#[derive(Debug)]
enum Undo {
    /// Restore `cell` at `addr`.
    Cell(Addr, Cell),
    /// The disequation was pending before it was resolved.
    DifPending(DifId),
    /// Pop the last suspension recorded for this variable.
    Wake(Addr),
}

/// A point in the store's history to return to.
#[derive(Debug, Clone, Copy)]
pub struct Mark {
    heap: usize,
    trail: usize,
    difs: usize,
}

/// The store.
#[derive(Debug, Default)]
pub struct Store {
    heap: Vec<Cell>,
    trail: Vec<Undo>,
    difs: Vec<Dif>,
    /// Variable → disequations suspended on it (waiting for it to be bound).
    wake: HashMap<Addr, Vec<DifId>>,
    /// Disequations woken by bindings since the last [`Store::settle`].
    woken: Vec<DifId>,
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

    // --- marks and undo ----------------------------------------------------

    pub fn mark(&self) -> Mark {
        Mark { heap: self.heap.len(), trail: self.trail.len(), difs: self.difs.len() }
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
            }
        }
        self.heap.truncate(mark.heap);
        self.difs.truncate(mark.difs);
        self.woken.clear();
    }

    // --- primitive mutations (all trailed) ---------------------------------

    fn set(&mut self, a: Addr, new: Cell) {
        let old = std::mem::replace(&mut self.heap[a], new);
        self.trail.push(Undo::Cell(a, old));
    }

    /// Bind the unbound variable `v` to `t`, waking suspended disequations.
    fn bind(&mut self, v: Addr, t: Addr) {
        debug_assert!(matches!(self.heap[v], Cell::Var(x) if x == v));
        self.set(v, Cell::Var(t));
        if let Some(ids) = self.wake.get(&v) {
            self.woken.extend(ids.iter().copied());
        }
    }

    fn suspend(&mut self, id: DifId, v: Addr) {
        let list = self.wake.entry(v).or_default();
        if list.contains(&id) {
            return;
        }
        list.push(id);
        self.trail.push(Undo::Wake(v));
    }

    // --- unification -------------------------------------------------------

    /// Unify `a` and `b` over rational trees (no occurs check). On failure the
    /// store is left as it was before the call. Does **not** settle woken
    /// disequations — see [`Store::post_eq`].
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
                (Cell::Var(_), _) => {
                    self.bind(a, b);
                    true
                }
                (_, Cell::Var(_)) => {
                    self.bind(b, a);
                    true
                }
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

    /// Post the equation `a = b`: unify, then re-check every disequation woken
    /// by the bindings. On failure the store is restored.
    pub fn post_eq(&mut self, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        if self.unify(a, b) && self.settle() {
            true
        } else {
            self.undo_to(&mark);
            false
        }
    }

    // --- disequations ------------------------------------------------------

    /// Post the disequation `a != b`. On failure the store is restored.
    pub fn post_dif(&mut self, a: Addr, b: Addr) -> bool {
        let mark = self.mark();
        let id = self.difs.len();
        self.difs.push(Dif { a, b, pending: true });
        if self.check_dif(id) {
            true
        } else {
            self.undo_to(&mark);
            false
        }
    }

    /// Re-check a disequation by trial unification (Colmerauer's criterion,
    /// Van Caneghem's implementation): if unifying the sides fails, the
    /// disequation holds and is dropped; if it succeeds without binding any
    /// variable, the sides are already equal and the store is unsatisfiable;
    /// otherwise the trial is undone and the disequation suspended on the
    /// variables that would have been bound.
    fn check_dif(&mut self, id: DifId) -> bool {
        let Dif { a, b, pending } = self.difs[id].clone();
        if !pending {
            return true;
        }
        let mark = self.mark();
        if !self.unify(a, b) {
            // Cannot ever be equal: satisfied.
            self.difs[id].pending = false;
            self.trail.push(Undo::DifPending(id));
            return true;
        }
        let would_bind: Vec<Addr> = self.trail[mark.trail..]
            .iter()
            .filter_map(|u| match u {
                Undo::Cell(addr, Cell::Var(x)) if x == addr => Some(*addr),
                _ => None,
            })
            .collect();
        self.undo_to(&mark);
        if would_bind.is_empty() {
            return false; // already equal
        }
        for v in would_bind {
            self.suspend(id, v);
        }
        true
    }

    /// Re-check every disequation woken since the last settle. Restores
    /// nothing on failure — callers hold a mark.
    fn settle(&mut self) -> bool {
        loop {
            let woken = std::mem::take(&mut self.woken);
            if woken.is_empty() {
                return true;
            }
            for id in woken {
                if self.difs[id].pending && !self.check_dif(id) {
                    return false;
                }
            }
        }
    }

    /// The pending disequations, as `(left, right)` addresses.
    pub fn pending_difs(&self) -> Vec<(Addr, Addr)> {
        self.difs.iter().filter(|d| d.pending).map(|d| (d.a, d.b)).collect()
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
