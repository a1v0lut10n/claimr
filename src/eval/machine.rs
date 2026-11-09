// SPDX-License-Identifier: Apache-2.0

//! The SLD machine: goals left to right, clauses in program order,
//! chronological backtracking through choice points and the store's trail.
//! Iterative — resolution depth never touches the Rust stack.
//!
//! A resolution step is taken only if head unification, and later each body
//! goal as it is reached, leave the store satisfiable; before an answer is
//! yielded the store is finalized (exact determination, disequation
//! re-checks) — `answer-soundness`.
//!
//! Body goals are instantiated **lazily**: a clause's variables are allocated
//! once into a frame at clause entry (the head is built from it for
//! unification), and each goal's terms are built from the frame when the goal
//! reaches the front of the list. By then the preceding goals have bound what
//! they bind, so arithmetic over values known at that point folds to numbers
//! instead of defining solver rows (CLM-0013).

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::answer::{Answer, render_answer, render_terms};
use super::compile::{ANONYMOUS, Builtin, PredKey, Program, Query, TGoal, build_with, lazy_var};
use super::error::EvalError;
use super::store::{Addr, Cell, Mark, Store, TypeKind};
use crate::ast::RelOp;

/// An instantiated runtime goal.
#[derive(Debug, Clone, Copy)]
enum RtGoal {
    Call(Addr),
    Eq(Addr, Addr),
    Dif(Addr, Addr),
    Rel(RelOp, Addr, Addr),
}

/// A clause's variables, allocated once at clause entry.
type Frame = Rc<[Addr]>;

/// A goal in the list: already instantiated (query goals), or a clause body
/// goal still to be built from its clause's frame when it is reached.
#[derive(Debug, Clone)]
enum Goal<'p> {
    Built(RtGoal),
    Pending { frame: Frame, goal: &'p TGoal },
}

/// Persistent goal list: cheap to snapshot into choice points.
type Goals<'p> = Option<Rc<GoalNode<'p>>>;

#[derive(Debug)]
struct GoalNode<'p> {
    goal: Goal<'p>,
    next: Goals<'p>,
}

fn cons<'p>(goal: Goal<'p>, next: Goals<'p>) -> Goals<'p> {
    Some(Rc::new(GoalNode { goal, next }))
}

struct ChoicePoint<'p> {
    /// Goals remaining after the call being retried.
    rest: Goals<'p>,
    call: Addr,
    key: PredKey,
    next_clause: usize,
    mark: Mark,
}

/// The solutions of one query, produced lazily; each `next()` resumes the
/// search where the previous answer left it. If the search stops on a
/// runtime error (a non-linear residue, a cyclic attribute term) the iterator
/// ends and [`Solutions::error`] reports it.
pub struct Solutions<'p> {
    program: &'p Program,
    store: Store,
    goals: Goals<'p>,
    choice_points: Vec<ChoicePoint<'p>>,
    query_vars: Vec<(String, Addr)>,
    started: bool,
    exhausted: bool,
    error: Option<EvalError>,
    /// Optional interrupt flag, polled between resolution steps.
    interrupt: Option<Arc<AtomicBool>>,
    interrupted: bool,
    steps: u64,
}

impl<'p> Solutions<'p> {
    pub(crate) fn new(program: &'p Program, query: &Query) -> Self {
        let mut store = Store::with_symbols(program.symbols.clone());
        // Constraint facts first (validated satisfiable at compile time).
        let initial_ok = program.post_initial(&mut store);
        debug_assert!(initial_ok, "initial store validated at compile time");
        store.set_baseline();
        let mut vars = vec![None; query.var_names.len()];
        let mut goals: Goals<'p> = None;
        let mut ok = initial_ok;
        // Query goals are built eagerly (their variables are the answer's), in
        // source order so definitions post in order; then link reversed.
        let mut built = Vec::with_capacity(query.goals.len());
        for g in &query.goals {
            match instantiate_goal(&mut store, g, &mut |store, i| lazy_var(store, &mut vars, i)) {
                Some(rg) => built.push(rg),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        for rg in built.into_iter().rev() {
            goals = cons(Goal::Built(rg), goals);
        }
        // An anonymous variable is not an answer variable (CLM-0020): what it
        // binds is visible only through named variables that reach it.
        let query_vars = query
            .var_names
            .iter()
            .cloned()
            .zip(vars.iter().map(|v| v.unwrap_or(usize::MAX)))
            .filter(|(name, a)| *a != usize::MAX && name != ANONYMOUS)
            .collect();
        let error = store.error.take();
        Solutions {
            program,
            store,
            goals,
            choice_points: Vec::new(),
            query_vars,
            started: false,
            exhausted: !ok,
            error,
            interrupt: None,
            interrupted: false,
            steps: 0,
        }
    }

    /// The runtime error that stopped the search, if any.
    pub fn error(&self) -> Option<&EvalError> {
        self.error.as_ref()
    }

    /// Poll `flag` between resolution steps; when it is set the search stops
    /// (no further answers, no error) and [`Solutions::interrupted`] is true.
    pub fn with_interrupt(mut self, flag: Arc<AtomicBool>) -> Self {
        self.interrupt = Some(flag);
        self
    }

    /// True if the search was stopped through the interrupt flag.
    pub fn interrupted(&self) -> bool {
        self.interrupted
    }

    /// True if asking for another answer could still find one: the search
    /// has untried alternatives. False after exhaustion or when the last
    /// answer left no choice point (it was final).
    pub fn may_continue(&self) -> bool {
        !self.exhausted && !self.choice_points.is_empty()
    }

    fn check_interrupt(&mut self) -> bool {
        self.steps += 1;
        if self.steps % 256 == 0 {
            if let Some(flag) = &self.interrupt {
                if flag.load(Ordering::Relaxed) {
                    self.interrupted = true;
                    return true;
                }
            }
        }
        false
    }

    /// Try clauses `from..` of `key` against `call`; on success install the
    /// clause body, pending on the clause's frame, in front of `rest` (and a
    /// choice point if clauses remain).
    fn try_clauses(&mut self, key: PredKey, from: usize, call: Addr, rest: Goals<'p>) -> bool {
        let program: &'p Program = self.program;
        let clauses = program.clauses(&key);
        for j in from..clauses.len() {
            let mark = self.store.mark();
            let clause = &clauses[j];
            let frame: Frame = (0..clause.nvars).map(|_| self.store.new_var()).collect();
            let head = build_with(&mut self.store, &clause.head, &mut |_, i| frame[i as usize]);
            let Some(head) = head else {
                self.store.undo_to(&mark);
                if self.store.error.is_some() {
                    return false;
                }
                continue;
            };
            if !self.store.post_eq(head, call) {
                self.store.undo_to(&mark);
                continue;
            }
            if j + 1 < clauses.len() {
                self.choice_points.push(ChoicePoint {
                    rest: rest.clone(),
                    call,
                    key,
                    next_clause: j + 1,
                    mark,
                });
            }
            let mut goals = rest;
            for g in clause.body.iter().rev() {
                goals = cons(
                    Goal::Pending {
                        frame: frame.clone(),
                        goal: g,
                    },
                    goals,
                );
            }
            self.goals = goals;
            return true;
        }
        false
    }

    /// Return to the most recent choice point with clauses left. False if the
    /// search space is exhausted (or a store error stopped it).
    fn backtrack(&mut self) -> bool {
        while let Some(cp) = self.choice_points.pop() {
            self.store.undo_to(&cp.mark);
            if self.try_clauses(cp.key, cp.next_clause, cp.call, cp.rest) {
                return true;
            }
            if self.store.error.is_some() {
                return false;
            }
        }
        false
    }

    /// Run until the goal list is empty and the store finalizes (an answer),
    /// or the search is exhausted / stopped.
    fn run(&mut self) -> bool {
        loop {
            if self.store.error.is_some() || self.check_interrupt() {
                return false;
            }
            let Some(node) = self.goals.clone() else {
                // All goals solved: finalize the store for this answer.
                if self.store.finalize() {
                    return true;
                }
                if self.store.error.is_some() || self.store.nonlinear.is_some() || !self.backtrack()
                {
                    return false;
                }
                continue;
            };
            let rest = node.next.clone();
            // A pending body goal is built now, from its clause's frame.
            let goal = match &node.goal {
                Goal::Built(g) => *g,
                Goal::Pending { frame, goal } => {
                    let frame = frame.clone();
                    let built =
                        instantiate_goal(&mut self.store, goal, &mut |_, i| frame[i as usize]);
                    match built {
                        Some(g) => g,
                        None => {
                            if self.store.error.is_some() || !self.backtrack() {
                                return false;
                            }
                            continue;
                        }
                    }
                }
            };
            let ok = match goal {
                RtGoal::Call(call) => {
                    let key = pred_key(&self.store, call);
                    match self.program.builtins.get(&key).copied() {
                        Some(b) => {
                            self.goals = rest;
                            self.run_builtin(b, call)
                        }
                        None => self.try_clauses(key, 0, call, rest),
                    }
                }
                RtGoal::Eq(a, b) => {
                    self.goals = rest;
                    self.store.post_eq_goal(a, b)
                }
                RtGoal::Dif(a, b) => {
                    self.goals = rest;
                    self.store.post_dif_goal(a, b)
                }
                RtGoal::Rel(op, a, b) => {
                    self.goals = rest;
                    self.store.post_rel(op, a, b)
                }
            };
            if !ok && (self.store.error.is_some() || !self.backtrack()) {
                return false;
            }
        }
    }

    /// A built-in predicate (CLM-0017): posts its constraint, deterministic.
    fn run_builtin(&mut self, b: Builtin, call: Addr) -> bool {
        let d = self.store.deref(call);
        let Cell::Struct(_, args) = self.store.cell(d).clone() else {
            return false;
        };
        match b {
            Builtin::AtomChars => self.store.post_atom_chars(args[0], args[1]),
            Builtin::Number => self.store.post_number(args[0]),
            Builtin::Atom => self.store.post_type(args[0], TypeKind::Atom),
            Builtin::List => self.store.post_type(args[0], TypeKind::List),
        }
    }

    fn answer(&self) -> Answer {
        render_answer(&self.store.symbols, &self.store, &self.query_vars)
    }

    /// Take the store's runtime error, rendering a non-linear residue with
    /// the query's variable names.
    fn take_error(&mut self) -> Option<EvalError> {
        if let Some(e) = self.store.error.take() {
            return Some(e);
        }
        let (a, b, op) = self.store.nonlinear.take()?;
        let rendered = render_terms(&self.store.symbols, &self.store, &self.query_vars, &[a, b]);
        Some(EvalError::NonLinear {
            constraint: format!("{} {op} {}", rendered[0], rendered[1]),
        })
    }
}

impl Iterator for Solutions<'_> {
    type Item = Answer;

    fn next(&mut self) -> Option<Answer> {
        if self.exhausted {
            return None;
        }
        if self.started && !self.backtrack() {
            self.exhausted = true;
            self.error = self.take_error();
            return None;
        }
        self.started = true;
        if self.run() {
            Some(self.answer())
        } else {
            self.exhausted = true;
            self.error = self.take_error();
            None
        }
    }
}

/// Build a goal's terms; `var` resolves clause variables (see `build_with`).
fn instantiate_goal(
    store: &mut Store,
    g: &TGoal,
    var: &mut dyn FnMut(&mut Store, u32) -> Addr,
) -> Option<RtGoal> {
    Some(match g {
        TGoal::Call(t) => RtGoal::Call(build_with(store, t, var)?),
        TGoal::Eq(a, b) => {
            let a = build_with(store, a, var)?;
            let b = build_with(store, b, var)?;
            RtGoal::Eq(a, b)
        }
        TGoal::Dif(a, b) => {
            let a = build_with(store, a, var)?;
            let b = build_with(store, b, var)?;
            RtGoal::Dif(a, b)
        }
        TGoal::Rel(op, a, b) => {
            let a = build_with(store, a, var)?;
            let b = build_with(store, b, var)?;
            RtGoal::Rel(*op, a, b)
        }
    })
}

fn pred_key(store: &Store, call: Addr) -> PredKey {
    match store.cell(store.deref(call)) {
        Cell::Struct(f, args) => (*f, args.len()),
        Cell::Const(c) => (*c, 0),
        other => unreachable!("call goals are atoms, got {other:?}"),
    }
}
