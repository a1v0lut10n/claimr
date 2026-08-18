// SPDX-License-Identifier: Apache-2.0

//! The SLD machine: goals left to right, clauses in program order,
//! chronological backtracking through choice points and the store's trail.
//! Iterative — resolution depth never touches the Rust stack.
//!
//! A resolution step is taken only if head unification, the clause's
//! arithmetic definitions and any constraint goals reached leave the store
//! satisfiable; before an answer is yielded the store is finalized (exact
//! determination, disequation re-checks) — `answer-soundness`.

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::answer::{Answer, render_answer, render_terms};
use super::compile::{build, PredKey, Program, Query, TGoal};
use super::error::EvalError;
use super::store::{Addr, Cell, Mark, Store};
use crate::ast::RelOp;

/// A runtime goal.
#[derive(Debug, Clone, Copy)]
enum RtGoal {
    Call(Addr),
    Eq(Addr, Addr),
    Dif(Addr, Addr),
    Rel(RelOp, Addr, Addr),
}

/// Persistent goal list: cheap to snapshot into choice points.
type Goals = Option<Rc<GoalNode>>;

#[derive(Debug)]
struct GoalNode {
    goal: RtGoal,
    next: Goals,
}

fn cons(goal: RtGoal, next: Goals) -> Goals {
    Some(Rc::new(GoalNode { goal, next }))
}

struct ChoicePoint {
    /// Goals remaining after the call being retried.
    rest: Goals,
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
    goals: Goals,
    choice_points: Vec<ChoicePoint>,
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
        let mut store = Store::new();
        // Constraint facts first (validated satisfiable at compile time).
        let initial_ok = program.post_initial(&mut store);
        debug_assert!(initial_ok, "initial store validated at compile time");
        store.set_baseline();
        let mut vars = vec![None; query.var_names.len()];
        let mut goals: Goals = None;
        let mut ok = initial_ok;
        // Build in source order (definitions post in order), then link reversed.
        let mut built = Vec::with_capacity(query.goals.len());
        for g in &query.goals {
            match instantiate_goal(&mut store, g, &mut vars) {
                Some(rg) => built.push(rg),
                None => {
                    ok = false;
                    break;
                }
            }
        }
        for rg in built.into_iter().rev() {
            goals = cons(rg, goals);
        }
        let query_vars = query
            .var_names
            .iter()
            .cloned()
            .zip(vars.iter().map(|v| v.unwrap_or(usize::MAX)))
            .filter(|(_, a)| *a != usize::MAX)
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
    /// clause body in front of `rest` (and a choice point if clauses remain).
    fn try_clauses(&mut self, key: PredKey, from: usize, call: Addr, rest: Goals) -> bool {
        let clauses = self.program.clauses(&key);
        for j in from..clauses.len() {
            let mark = self.store.mark();
            let clause = &clauses[j];
            let mut vars = vec![None; clause.nvars as usize];
            let Some(head) = build(&mut self.store, &clause.head, &mut vars) else {
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
            // Instantiate the body (posting its arithmetic definitions).
            let mut body = Vec::with_capacity(clause.body.len());
            let mut ok = true;
            for g in &clause.body {
                match instantiate_goal(&mut self.store, g, &mut vars) {
                    Some(rg) => body.push(rg),
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                self.store.undo_to(&mark);
                if self.store.error.is_some() {
                    return false;
                }
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
            for rg in body.into_iter().rev() {
                goals = cons(rg, goals);
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
                if self.store.error.is_some() || self.store.nonlinear.is_some() || !self.backtrack() {
                    return false;
                }
                continue;
            };
            let rest = node.next.clone();
            let ok = match node.goal {
                RtGoal::Call(call) => {
                    let key = pred_key(&self.store, call);
                    self.try_clauses(key, 0, call, rest)
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

    fn answer(&self) -> Answer {
        render_answer(&self.program.symbols, &self.store, &self.query_vars)
    }

    /// Take the store's runtime error, rendering a non-linear residue with
    /// the query's variable names.
    fn take_error(&mut self) -> Option<EvalError> {
        if let Some(e) = self.store.error.take() {
            return Some(e);
        }
        let (a, b, op) = self.store.nonlinear.take()?;
        let rendered = render_terms(&self.program.symbols, &self.store, &self.query_vars, &[a, b]);
        Some(EvalError::NonLinear { constraint: format!("{} {op} {}", rendered[0], rendered[1]) })
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

fn instantiate_goal(store: &mut Store, g: &TGoal, vars: &mut [Option<Addr>]) -> Option<RtGoal> {
    Some(match g {
        TGoal::Call(t) => RtGoal::Call(build(store, t, vars)?),
        TGoal::Eq(a, b) => {
            let a = build(store, a, vars)?;
            let b = build(store, b, vars)?;
            RtGoal::Eq(a, b)
        }
        TGoal::Dif(a, b) => {
            let a = build(store, a, vars)?;
            let b = build(store, b, vars)?;
            RtGoal::Dif(a, b)
        }
        TGoal::Rel(op, a, b) => {
            let a = build(store, a, vars)?;
            let b = build(store, b, vars)?;
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

