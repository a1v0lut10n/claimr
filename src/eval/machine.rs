//! The SLD machine: goals left to right, clauses in program order,
//! chronological backtracking through choice points and the store's trail.
//! Iterative — resolution depth never touches the Rust stack.
//!
//! A resolution step is taken only if head unification and any constraint
//! goals reached leave the store satisfiable (`answer-soundness`).

use std::rc::Rc;

use super::answer::{Answer, render_answer};
use super::compile::{build, PredKey, Program, Query, TGoal};
use super::store::{Addr, Cell, Mark, Store};

/// A runtime goal.
#[derive(Debug, Clone, Copy)]
enum RtGoal {
    Call(Addr),
    Eq(Addr, Addr),
    Dif(Addr, Addr),
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
/// search where the previous answer left it.
pub struct Solutions<'p> {
    program: &'p Program,
    store: Store,
    goals: Goals,
    choice_points: Vec<ChoicePoint>,
    query_vars: Vec<(String, Addr)>,
    started: bool,
    exhausted: bool,
}

impl<'p> Solutions<'p> {
    pub(crate) fn new(program: &'p Program, query: &Query) -> Self {
        let mut store = Store::new();
        // Constraint facts first (validated satisfiable at compile time).
        let initial_ok = program.post_initial(&mut store);
        debug_assert!(initial_ok, "initial store validated at compile time");
        let mut vars = vec![None; query.var_names.len()];
        let mut goals: Goals = None;
        for g in query.goals.iter().rev() {
            goals = cons(instantiate_goal(&mut store, g, &mut vars), goals);
        }
        let query_vars = query
            .var_names
            .iter()
            .cloned()
            .zip(vars.iter().map(|v| v.expect("every query variable occurs in a goal")))
            .collect();
        Solutions {
            program,
            store,
            goals,
            choice_points: Vec::new(),
            query_vars,
            started: false,
            exhausted: !initial_ok,
        }
    }

    /// Try clauses `from..` of `key` against `call`; on success install the
    /// clause body in front of `rest` (and a choice point if clauses remain).
    fn try_clauses(&mut self, key: PredKey, from: usize, call: Addr, rest: Goals) -> bool {
        let clauses = self.program.clauses(&key);
        for j in from..clauses.len() {
            let mark = self.store.mark();
            let clause = &clauses[j];
            let mut vars = vec![None; clause.nvars as usize];
            let head = build(&mut self.store, &clause.head, &mut vars);
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
                goals = cons(instantiate_goal(&mut self.store, g, &mut vars), goals);
            }
            self.goals = goals;
            return true;
        }
        false
    }

    /// Return to the most recent choice point with clauses left. False if the
    /// search space is exhausted.
    fn backtrack(&mut self) -> bool {
        while let Some(cp) = self.choice_points.pop() {
            self.store.undo_to(&cp.mark);
            if self.try_clauses(cp.key, cp.next_clause, cp.call, cp.rest) {
                return true;
            }
        }
        false
    }

    /// Run until the goal list is empty (an answer) or the search is exhausted.
    fn run(&mut self) -> bool {
        loop {
            let Some(node) = self.goals.clone() else {
                return true;
            };
            let rest = node.next.clone();
            let ok = match node.goal {
                RtGoal::Call(call) => {
                    let key = pred_key(&self.store, call);
                    self.try_clauses(key, 0, call, rest)
                }
                RtGoal::Eq(a, b) => {
                    self.goals = rest;
                    self.store.post_eq(a, b)
                }
                RtGoal::Dif(a, b) => {
                    self.goals = rest;
                    self.store.post_dif(a, b)
                }
            };
            if !ok && !self.backtrack() {
                return false;
            }
        }
    }

    fn answer(&self) -> Answer {
        render_answer(&self.program.symbols, &self.store, &self.query_vars)
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
            return None;
        }
        self.started = true;
        if self.run() {
            Some(self.answer())
        } else {
            self.exhausted = true;
            None
        }
    }
}

fn instantiate_goal(store: &mut Store, g: &TGoal, vars: &mut [Option<Addr>]) -> RtGoal {
    match g {
        TGoal::Call(t) => RtGoal::Call(build(store, t, vars)),
        TGoal::Eq(a, b) => {
            let a = build(store, a, vars);
            let b = build(store, b, vars);
            RtGoal::Eq(a, b)
        }
        TGoal::Dif(a, b) => {
            let a = build(store, a, vars);
            let b = build(store, b, vars);
            RtGoal::Dif(a, b)
        }
    }
}

fn pred_key(store: &Store, call: Addr) -> PredKey {
    match store.cell(store.deref(call)) {
        Cell::Struct(f, args) => (*f, args.len()),
        Cell::Const(c) => (*c, 0),
        other => unreachable!("call goals are atoms, got {other:?}"),
    }
}
