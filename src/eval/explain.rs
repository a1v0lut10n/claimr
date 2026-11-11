// SPDX-License-Identifier: Apache-2.0

//! Explanations of failure (CLM-0030, design record
//! `docs/design/2026-11-11-explanations-of-failure.md`): a derivation's
//! steps as closed, replayable records; the replay oracle that decides,
//! exactly, whether a set of steps has a solution; and the QuickXplain
//! minimiser that finds a minimal unsatisfiable subset of the steps before
//! a failing one.
//!
//! The oracle is the store itself: the steps are posted in order into a
//! fresh store over the program's symbol table and the store is finalized.
//! A set the minimiser reports is therefore unsatisfiable, and minimal, by
//! the very check that reports it — no origin is traced through the solver.

use std::collections::HashMap;
use std::rc::Rc;

use super::answer::render_terms;
use super::compile::{Builtin, Program, TTerm, build_with};
use super::error::EvalError;
use super::machine::run_builtin;
use super::store::{Addr, Store};
use crate::ast::RelOp;

/// One step of a derivation, closed over the derivation's variables: a
/// `TTerm::Var(v)` in a step is the heap address `v` of the frame variable
/// it stood for (see [`close`]), so steps taken under different clause
/// frames agree on which variable is which, and a replay allocates one
/// fresh variable per distinct `v`.
#[derive(Debug, Clone)]
pub(crate) enum Step {
    /// A clause's head unified with the call that resolved to it.
    Head { head: TTerm, call: TTerm },
    /// A constraint goal `{ a op b }`.
    Constraint {
        op: ConstraintOp,
        a: TTerm,
        b: TTerm,
    },
    /// A built-in predicate called.
    Builtin { builtin: Builtin, call: TTerm },
    /// A tabled call unified with a recorded answer of its table (a ground
    /// template).
    Tabled { call: TTerm, answer: TTerm },
    /// A comprehension's variable unified with the list it collected (a
    /// ground template).
    Collected { into: TTerm, list: TTerm },
    /// Constraint fact `idx` of the program, over its own variables.
    Fact(usize),
    /// The answer-time finalize (CLM-0031: the step at which a derivation
    /// whose goals all succeeded can still fail).
    Answer,
}

/// The operator of a constraint step: `=` and `!=` are decided numeric or
/// tree by what their operands denote when the step is replayed, as when it
/// was taken (CLM-0026); a relation is always numeric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConstraintOp {
    Eq,
    Dif,
    Rel(RelOp),
}

/// The closed variable id of a heap address. A heap of 2³² cells is out of
/// reach, so the fallback never conflates anything in practice.
pub(crate) fn var_id(a: Addr) -> u32 {
    u32::try_from(a).unwrap_or(u32::MAX)
}

/// Close a clause template over its frame: every `Var(i)` becomes
/// `Var(frame[i])`, the heap address of the variable it stood for.
/// Iterative, like the lowering it mirrors (CLM-0019).
pub(crate) fn close(tpl: &TTerm, frame: &[Addr]) -> TTerm {
    enum Walk<'a> {
        Enter(&'a TTerm),
        Build(&'a TTerm),
    }
    let mut work = vec![Walk::Enter(tpl)];
    let mut out: Vec<TTerm> = Vec::new();
    while let Some(step) = work.pop() {
        match step {
            Walk::Enter(t) => {
                work.push(Walk::Build(t));
                match t {
                    TTerm::Var(_) | TTerm::Number(_) | TTerm::Const(_) => {}
                    TTerm::Compound(_, args) => work.extend(args.iter().rev().map(Walk::Enter)),
                    TTerm::Neg(x) | TTerm::Len(x) => work.push(Walk::Enter(x)),
                    TTerm::Arith(_, l, r) | TTerm::Concat(l, r) => {
                        work.push(Walk::Enter(r));
                        work.push(Walk::Enter(l));
                    }
                }
            }
            Walk::Build(t) => {
                let built = match t {
                    TTerm::Var(i) => TTerm::Var(var_id(frame[*i as usize])),
                    TTerm::Number(n) => TTerm::Number(n.clone()),
                    TTerm::Const(c) => TTerm::Const(*c),
                    TTerm::Compound(f, args) => {
                        let kids = out.split_off(out.len() - args.len());
                        TTerm::Compound(*f, kids)
                    }
                    TTerm::Neg(_) => TTerm::Neg(Box::new(out.pop().unwrap_or(TTerm::Var(0)))),
                    TTerm::Len(_) => TTerm::Len(Box::new(out.pop().unwrap_or(TTerm::Var(0)))),
                    TTerm::Arith(op, ..) => {
                        let r = out.pop().unwrap_or(TTerm::Var(0));
                        let l = out.pop().unwrap_or(TTerm::Var(0));
                        TTerm::Arith(*op, Box::new(l), Box::new(r))
                    }
                    TTerm::Concat(..) => {
                        let r = out.pop().unwrap_or(TTerm::Var(0));
                        let l = out.pop().unwrap_or(TTerm::Var(0));
                        TTerm::Concat(Box::new(l), Box::new(r))
                    }
                };
                out.push(built);
            }
        }
    }
    out.pop().unwrap_or(TTerm::Var(0))
}

/// A replay in progress: the fresh store and the closed variables already
/// allocated in it.
struct Replay {
    store: Store,
    vars: HashMap<u32, Addr>,
}

impl Replay {
    fn new(program: &Program) -> Self {
        Replay {
            store: Store::with_symbols(Rc::clone(&program.symbols)),
            vars: HashMap::new(),
        }
    }

    /// Instantiate a closed template; `None` if its arithmetic makes the
    /// store unsatisfiable.
    fn build(&mut self, t: &TTerm) -> Option<Addr> {
        let vars = &mut self.vars;
        build_with(&mut self.store, t, &mut |s, i| match vars.get(&i) {
            Some(a) => *a,
            None => {
                let a = s.new_var();
                vars.insert(i, a);
                a
            }
        })
    }

    /// Post one step. `Ok(false)` when the store rejects it.
    fn post(&mut self, program: &Program, step: &Step) -> Result<bool, EvalError> {
        let ok = match step {
            Step::Head { head, call, .. } => match (self.build(head), self.build(call)) {
                (Some(h), Some(c)) => self.store.post_eq(h, c),
                _ => false,
            },
            Step::Constraint { op, a, b } => match (self.build(a), self.build(b)) {
                (Some(a), Some(b)) => match op {
                    ConstraintOp::Eq => self.store.post_eq_goal(a, b),
                    ConstraintOp::Dif => self.store.post_dif_goal(a, b),
                    ConstraintOp::Rel(op) => self.store.post_rel(*op, a, b),
                },
                _ => false,
            },
            Step::Builtin { builtin, call } => match self.build(call) {
                Some(c) => run_builtin(&mut self.store, *builtin, c),
                None => false,
            },
            Step::Tabled { call, answer } => match (self.build(call), self.build(answer)) {
                (Some(c), Some(a)) => self.store.post_eq(c, a),
                _ => false,
            },
            Step::Collected { into, list } => match (self.build(into), self.build(list)) {
                (Some(v), Some(l)) => self.store.post_eq(v, l),
                _ => false,
            },
            Step::Fact(idx) => program.post_fact(&mut self.store, *idx),
            Step::Answer => self.store.finalize(),
        };
        self.check_error()?;
        Ok(ok)
    }

    /// A store error — or a non-linear residue, which no exact verdict can
    /// cover — stops the explanation rather than deciding it.
    fn check_error(&mut self) -> Result<(), EvalError> {
        if let Some(e) = self.store.error.take() {
            return Err(e);
        }
        if let Some((a, b, op)) = self.store.nonlinear.take() {
            let rendered = render_terms(&self.store, &[], &[a, b]);
            return Err(EvalError::NonLinear {
                constraint: format!("{} {op} {}", rendered[0], rendered[1]),
            });
        }
        Ok(())
    }
}

/// Does the system of `steps`, posted in order into a fresh store and
/// finalized, have a solution? Exact: the same check an answer passes.
pub(crate) fn satisfiable(program: &Program, steps: &[&Step]) -> Result<bool, EvalError> {
    let mut replay = Replay::new(program);
    for step in steps {
        if !replay.post(program, step)? {
            return Ok(false);
        }
    }
    let ok = replay.store.finalize();
    replay.check_error()?;
    Ok(ok)
}

/// A minimal subset `C` of `candidates` (as indices, ascending) such that
/// `C ∪ {failing}` has no solution and every proper subset has one — given
/// that `candidates ∪ {failing}` has none, which the caller guarantees (the
/// steps before a failing one were satisfiable together, so the failing
/// step is in every such set and is kept implicit). QuickXplain (Junker
/// 2004) over the candidates in their order: the set reported is
/// deterministic when several minimal sets exist. The failing step alone
/// unsatisfiable gives the empty set.
pub(crate) fn minimal_conflict(
    program: &Program,
    candidates: &[Step],
    failing: &Step,
) -> Result<Vec<usize>, EvalError> {
    let mut qx = QuickXplain {
        program,
        candidates,
        failing,
    };
    let all: Vec<usize> = (0..candidates.len()).collect();
    let mut found = qx.explain(&[], true, &all)?;
    found.sort_unstable();
    Ok(found)
}

struct QuickXplain<'a> {
    program: &'a Program,
    candidates: &'a [Step],
    failing: &'a Step,
}

impl QuickXplain<'_> {
    /// `background ∪ {failing}` replayed, in candidate order.
    fn satisfiable(&self, background: &[usize]) -> Result<bool, EvalError> {
        let mut idx: Vec<usize> = background.to_vec();
        idx.sort_unstable();
        let mut steps: Vec<&Step> = idx.iter().map(|&i| &self.candidates[i]).collect();
        steps.push(self.failing);
        satisfiable(self.program, &steps)
    }

    /// Junker's QuickXplain′: a minimal subset of `c` that, with
    /// `background` (and the failing step), is unsatisfiable. `check` says
    /// the background grew since the caller's test, so it may already be
    /// unsatisfiable on its own. Recursion depth is logarithmic in `c`.
    fn explain(
        &mut self,
        background: &[usize],
        check: bool,
        c: &[usize],
    ) -> Result<Vec<usize>, EvalError> {
        if check && !self.satisfiable(background)? {
            return Ok(Vec::new());
        }
        if c.len() <= 1 {
            return Ok(c.to_vec());
        }
        let (c1, c2) = c.split_at(c.len() / 2);
        let mut b1 = background.to_vec();
        b1.extend_from_slice(c1);
        let d2 = self.explain(&b1, !c1.is_empty(), c2)?;
        let mut b2 = background.to_vec();
        b2.extend_from_slice(&d2);
        let mut d1 = self.explain(&b2, !d2.is_empty(), c1)?;
        d1.extend(d2);
        Ok(d1)
    }
}
