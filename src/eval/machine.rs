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
//!
//! On request (CLM-0023, `Solutions::with_explanations`) the machine records
//! a proof arena: one record per successful step, each under the record of
//! the clause whose body it belongs to; choice points remember the arena's
//! length and backtracking truncates it. The answer's explanation is the
//! arena rendered as a tree.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::answer::{Answer, ProofNode, Resolution, render_answer, render_terms};
use super::compile::{
    ANONYMOUS, ArgKey, Builtin, CondCheck, PredKey, Program, Query, TGoal, TTerm, build_with,
};
use super::error::EvalError;
use super::store::{Addr, Cell, Mark, Store, TypeKind};
use super::symbol::LIST;
use super::table::TableState;
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

/// The proof record a goal's own record goes under (CLM-0023): `None` for a
/// query goal (a root) and whenever explanations are off.
type Under = Option<usize>;

/// A goal in the list: a body goal still to be built from its clause's
/// frame when it is reached (query goals too, since CLM-0022: the query's
/// variables are its frame), or a commit.
#[derive(Debug, Clone)]
enum Goal<'p> {
    Pending {
        frame: Frame,
        goal: &'p TGoal,
        under: Under,
    },
    /// CLM-0022: the condition of an if-then-else or a negation has just
    /// succeeded. Reaching it finalizes the store as for an answer (a
    /// condition's solution is one the answer stage would accept), drops
    /// the choice points above `height` — the condition's alternatives and
    /// the else branch — and continues with `then`.
    Commit {
        height: usize,
        then: Then<'p>,
        /// The construct as written (CLM-0027: named when a table under
        /// evaluation is consumed inside it).
        text: &'p str,
    },
    /// CLM-0025: a solution of a comprehension's body has been reached.
    /// Reaching it finalizes the store, copies the instance of the template
    /// into the accumulator (an error if it is not ground) and fails, so
    /// the search moves to the next solution; the finishing choice point
    /// below collects the copies.
    CollectOne {
        frame: Frame,
        template: &'p TTerm,
        construct: &'p str,
        acc: Rc<RefCell<Vec<TTerm>>>,
    },
    /// CLM-0027: an already-built call to resolve against its clauses — the
    /// abstract call of a table evaluation, which must not consult the
    /// table it is computing.
    Resolve { call: Addr, under: Under },
    /// CLM-0027: a solution of a table evaluation has been reached.
    /// Reaching it finalizes the store, records the call's instance as an
    /// answer of the table (an error if it is not a finite ground term) and
    /// fails, so the search moves to the next solution; the finishing
    /// choice point below decides completion.
    TableOne { key: Rc<TTerm>, call: Addr },
}

/// What follows a committed condition (CLM-0022).
#[derive(Debug, Clone)]
enum Then<'p> {
    Goals {
        frame: Frame,
        goals: &'p [TGoal],
        under: Under,
    },
    /// Negation: the condition succeeded, so `\+` fails.
    Fail,
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

/// Body goals in front of `rest`, pending on `frame`, recorded under `under`.
fn prepend<'p>(goals: &'p [TGoal], frame: &Frame, under: Under, rest: Goals<'p>) -> Goals<'p> {
    let mut out = rest;
    for g in goals.iter().rev() {
        out = cons(
            Goal::Pending {
                frame: frame.clone(),
                goal: g,
                under,
            },
            out,
        );
    }
    out
}

struct ChoicePoint<'p> {
    retry: Retry<'p>,
    mark: Mark,
    /// The proof arena's length when the choice point was made (CLM-0023):
    /// backtracking to it discards the records made since.
    proof_len: usize,
}

/// What a choice point resumes with.
enum Retry<'p> {
    /// The next clause of a call.
    Clauses {
        /// Goals remaining after the call being retried.
        rest: Goals<'p>,
        call: Addr,
        key: PredKey,
        /// The clauses that can match the call (CLM-0021), as positions in
        /// program order, and the next one to try.
        candidates: &'p [usize],
        next: usize,
        under: Under,
    },
    /// CLM-0022: a goal list — the second alternative of a disjunction, the
    /// else branch of an if-then-else, the continuation of a negation. The
    /// note, if any, is the `NoSolution` leaf to record when it is taken
    /// (CLM-0023): the condition as written, under its clause's record.
    Goals {
        goals: Goals<'p>,
        note: Option<(Under, &'p str)>,
    },
    /// CLM-0025: the body of a comprehension is exhausted — build the list
    /// from the accumulated copies, unify it with the comprehension's
    /// variable and continue.
    Finish {
        rest: Goals<'p>,
        frame: Frame,
        into: u32,
        acc: Rc<RefCell<Vec<TTerm>>>,
        under: Under,
        text: &'p str,
    },
    /// CLM-0027: the clauses of a table evaluation are exhausted — decide
    /// completion (another pass for a looping leader), swap the caller's
    /// store back and consume the table for the call that started it.
    TableFinish {
        key: Rc<TTerm>,
        rest: Goals<'p>,
        call: Addr,
        under: Under,
    },
    /// CLM-0027: the next answer of a table for a call.
    Answers {
        key: Rc<TTerm>,
        next: usize,
        rest: Goals<'p>,
        call: Addr,
        under: Under,
    },
}

/// CLM-0027: a table evaluation in progress. The machine runs it on a clone
/// of the program's world store, the caller's store parked here; everything
/// the evaluation pushes sits above its `TableFinish` choice point and is
/// gone before the stores are swapped back.
struct EvalFrame {
    parked: Store,
    /// The abstract call, built on the evaluation's store before its mark,
    /// and its variables (the key's, in index order).
    abstract_call: Addr,
    abstract_vars: Vec<Addr>,
    /// The lowest frame this evaluation's answers depend on — its own depth
    /// when none: a recursive variant call to a lower frame lowers it.
    leader_min: usize,
    /// A recursive variant call consumed this frame's table during the pass.
    looping: bool,
    /// `Tables::total` when the pass started.
    total_at_start: usize,
    /// Tables whose evaluation returned incomplete depending on this one.
    dependents: Vec<Rc<TTerm>>,
    /// The proof arena's length when the evaluation started.
    proof_start: usize,
}

/// One record of the proof arena (CLM-0023).
struct ProofRec<'p> {
    parent: Under,
    kind: ProofKind<'p>,
}

enum ProofKind<'p> {
    /// A call resolved by clause `idx` of `key`.
    Clause {
        key: PredKey,
        idx: usize,
        call: Addr,
    },
    /// A constraint goal, as posted.
    Constraint {
        a: Addr,
        op: &'static str,
        b: Addr,
    },
    Builtin {
        call: Addr,
    },
    /// An if-then-else whose condition succeeded; its proofs go under it.
    Committed {
        text: &'p str,
    },
    /// A condition that had no solution (an else branch or a negation taken).
    NoSolution {
        text: &'p str,
    },
    /// A comprehension (CLM-0025) and the list it produced.
    Collected {
        text: &'p str,
        into: Addr,
    },
    /// A tabled call (CLM-0027) answered by answer `idx` of its table.
    Tabled {
        key: Rc<TTerm>,
        idx: usize,
        call: Addr,
    },
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
    /// The proof arena (CLM-0023), present when explanations were asked for.
    proof: Option<Vec<ProofRec<'p>>>,
    /// CLM-0027: the table evaluations in progress, innermost last.
    eval_stack: Vec<EvalFrame>,
}

impl<'p> Solutions<'p> {
    pub(crate) fn new(program: &'p Program, query: &'p Query) -> Self {
        // The world store — constraint facts posted, baseline set — is built
        // once at compile time (validated satisfiable there).
        let mut store = program.world.clone();
        // The query's variables are its frame (allocated in order of first
        // occurrence, so definitions post in source order); its goals are
        // pending on it like a clause body's, so the control constructs of
        // CLM-0022 work the same in a query.
        let frame: Frame = (0..query.var_names.len())
            .map(|_| store.new_var())
            .collect();
        let goals = prepend(&query.goals, &frame, None, None);
        // An anonymous variable is not an answer variable (CLM-0020): what it
        // binds is visible only through named variables that reach it.
        let query_vars = query
            .var_names
            .iter()
            .cloned()
            .zip(frame.iter().copied())
            .filter(|(name, _)| name != ANONYMOUS)
            .collect();
        Solutions {
            program,
            store,
            goals,
            choice_points: Vec::new(),
            query_vars,
            started: false,
            exhausted: false,
            error: None,
            interrupt: None,
            interrupted: false,
            steps: 0,
            proof: None,
            eval_stack: Vec::new(),
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

    /// Record each answer's proof tree (CLM-0023): `Answer::explanation` is
    /// then present. Off by default, since the records of a derivation stay
    /// alive until backtracking discards them.
    pub fn with_explanations(mut self) -> Self {
        self.proof = Some(Vec::new());
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

    fn proof_len(&self) -> usize {
        self.proof.as_ref().map_or(0, Vec::len)
    }

    /// Append a proof record under `parent` (CLM-0023); its id, or `None`
    /// when explanations are off.
    fn record(&mut self, parent: Under, kind: ProofKind<'p>) -> Under {
        let arena = self.proof.as_mut()?;
        arena.push(ProofRec { parent, kind });
        Some(arena.len() - 1)
    }

    fn push_choice_point(&mut self, retry: Retry<'p>, mark: Mark) {
        let proof_len = self.proof_len();
        self.choice_points.push(ChoicePoint {
            retry,
            mark,
            proof_len,
        });
    }

    /// Try clauses `from..` of `key` against `call`; on success install the
    /// clause body, pending on the clause's frame, in front of `rest` (and a
    /// choice point if clauses remain).
    fn try_clauses(
        &mut self,
        key: PredKey,
        candidates: &'p [usize],
        from: usize,
        call: Addr,
        rest: Goals<'p>,
        under: Under,
    ) -> bool {
        let program: &'p Program = self.program;
        let clauses = program.clauses(&key);
        for j in from..candidates.len() {
            let mark = self.store.mark();
            let clause = &clauses[candidates[j]];
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
            if j + 1 < candidates.len() {
                self.push_choice_point(
                    Retry::Clauses {
                        rest: rest.clone(),
                        call,
                        key,
                        candidates,
                        next: j + 1,
                        under,
                    },
                    mark,
                );
            }
            let node = self.record(
                under,
                ProofKind::Clause {
                    key,
                    idx: candidates[j],
                    call,
                },
            );
            self.goals = prepend(&clause.body, &frame, node, rest);
            return true;
        }
        false
    }

    /// Return to the most recent choice point with clauses left. False if the
    /// search space is exhausted (or a store error stopped it).
    fn backtrack(&mut self) -> bool {
        while let Some(cp) = self.choice_points.pop() {
            self.store.undo_to(&cp.mark);
            if let Some(arena) = self.proof.as_mut() {
                arena.truncate(cp.proof_len);
            }
            match cp.retry {
                Retry::Clauses {
                    rest,
                    call,
                    key,
                    candidates,
                    next,
                    under,
                } => {
                    if self.try_clauses(key, candidates, next, call, rest, under) {
                        return true;
                    }
                    if self.store.error.is_some() {
                        return false;
                    }
                }
                Retry::Goals { goals, note } => {
                    if let Some((under, text)) = note {
                        self.record(under, ProofKind::NoSolution { text });
                    }
                    self.goals = goals;
                    return true;
                }
                Retry::Finish {
                    rest,
                    frame,
                    into,
                    acc,
                    under,
                    text,
                } => {
                    // The list of the copies, as a closed template rebuilt on
                    // the restored heap.
                    let mut list = TTerm::Const(LIST);
                    for item in acc.borrow().iter().rev() {
                        list = TTerm::Compound(LIST, vec![item.clone(), list]);
                    }
                    let Some(addr) = build_with(&mut self.store, &list, &mut |s, _| s.new_var())
                    else {
                        continue;
                    };
                    let target = frame[into as usize];
                    if !self.store.post_eq(target, addr) {
                        continue;
                    }
                    self.record(under, ProofKind::Collected { text, into: target });
                    self.goals = rest;
                    return true;
                }
                Retry::TableFinish {
                    key,
                    rest,
                    call,
                    under,
                } => {
                    if self.finish_evaluation(key, call, rest, under) {
                        return true;
                    }
                    if self.store.error.is_some() {
                        return false;
                    }
                }
                Retry::Answers {
                    key,
                    next,
                    rest,
                    call,
                    under,
                } => {
                    if self.consume(key, next, call, rest, under) {
                        return true;
                    }
                    if self.store.error.is_some() {
                        return false;
                    }
                }
            }
        }
        false
    }

    // --- tabling (CLM-0027) ------------------------------------------------

    /// The names the printer may use: the query's variables, except inside
    /// a table evaluation, whose store is not the query's.
    fn names(&self) -> &[(String, Addr)] {
        if self.eval_stack.is_empty() {
            &self.query_vars
        } else {
            &[]
        }
    }

    fn render_one(&self, a: Addr) -> String {
        render_terms(&self.store.symbols, &self.store, self.names(), &[a]).remove(0)
    }

    fn pred_name(&self, key: PredKey) -> String {
        format!(
            "{}/{}",
            crate::ast::display_name(self.store.symbols.name(key.0)),
            key.1
        )
    }

    /// A call to a tabled predicate: consume its table, or compute it first.
    fn tabled_call(&mut self, call: Addr, rest: Goals<'p>, under: Under) -> bool {
        let Some(vkey) = self.store.variant_key(call) else {
            self.store.error = Some(EvalError::CyclicTabledCall {
                call: self.render_one(call),
            });
            return false;
        };
        let vkey = Rc::new(vkey);
        let state = self.program.tables.borrow().state(&vkey);
        match state {
            Some(TableState::Complete) => self.consume(vkey, 0, call, rest, under),
            Some(TableState::Evaluating(d)) => {
                // A recursive variant call: every frame above `d` now depends
                // on `d`, which must iterate; the call consumes what is there.
                let top = self.eval_stack.len();
                for f in &mut self.eval_stack[d + 1..top] {
                    f.leader_min = f.leader_min.min(d);
                }
                self.eval_stack[d].looping = true;
                self.consume(vkey, 0, call, rest, under)
            }
            Some(TableState::Incomplete) | None => self.evaluate(vkey, call, rest, under),
        }
    }

    /// Start evaluating the table of `vkey`: park the caller's store, run on
    /// a clone of the world, resolve the abstract call against the clauses
    /// with a `TableOne` after it and a `TableFinish` choice point below.
    fn evaluate(&mut self, vkey: Rc<TTerm>, call: Addr, rest: Goals<'p>, under: Under) -> bool {
        let depth = self.eval_stack.len();
        let total = {
            let mut tables = self.program.tables.borrow_mut();
            tables.begin(&vkey, depth);
            tables.total
        };
        let world = self.program.world.clone();
        let parked = std::mem::replace(&mut self.store, world);
        let mut vars: Vec<Option<Addr>> = Vec::new();
        let built = build_with(&mut self.store, &vkey, &mut |s, i| {
            let i = i as usize;
            if vars.len() <= i {
                vars.resize(i + 1, None);
            }
            match vars[i] {
                Some(a) => a,
                None => {
                    let a = s.new_var();
                    vars[i] = Some(a);
                    a
                }
            }
        });
        let Some(abstract_call) = built else {
            self.store = parked;
            self.store.error = Some(EvalError::InternalInvariant {
                what: "a table key did not instantiate".to_string(),
            });
            return false;
        };
        let abstract_vars: Vec<Addr> = vars.into_iter().flatten().collect();
        self.eval_stack.push(EvalFrame {
            parked,
            abstract_call,
            abstract_vars,
            leader_min: depth,
            looping: false,
            total_at_start: total,
            dependents: Vec::new(),
            proof_start: self.proof_len(),
        });
        self.start_pass(vkey, call, rest, under, abstract_call);
        true
    }

    fn start_pass(
        &mut self,
        vkey: Rc<TTerm>,
        call: Addr,
        rest: Goals<'p>,
        under: Under,
        abstract_call: Addr,
    ) {
        let mark = self.store.mark();
        self.push_choice_point(
            Retry::TableFinish {
                key: vkey.clone(),
                rest,
                call,
                under,
            },
            mark,
        );
        // The evaluation's own goal list: it never empties, since `TableOne`
        // always fails (or stops the query).
        self.goals = cons(
            Goal::Resolve {
                call: abstract_call,
                under: None,
            },
            cons(
                Goal::TableOne {
                    key: vkey,
                    call: abstract_call,
                },
                None,
            ),
        );
    }

    /// `TableFinish`: the evaluation's clauses are exhausted (the store is
    /// undone to the pass's start). A looping leader whose pass added an
    /// answer anywhere runs another pass; otherwise the frame is popped, the
    /// table completes (or is left incomplete for its leader), the caller's
    /// store comes back and the caller consumes the table.
    fn finish_evaluation(
        &mut self,
        key: Rc<TTerm>,
        call: Addr,
        rest: Goals<'p>,
        under: Under,
    ) -> bool {
        let Some(frame) = self.eval_stack.pop() else {
            self.store.error = Some(EvalError::InternalInvariant {
                what: "a table evaluation finished with no frame".to_string(),
            });
            return false;
        };
        let depth = self.eval_stack.len();
        let total = self.program.tables.borrow().total;
        if frame.looping && frame.leader_min == depth && total > frame.total_at_start {
            let abstract_call = frame.abstract_call;
            self.eval_stack.push(EvalFrame {
                looping: false,
                total_at_start: total,
                ..frame
            });
            self.start_pass(key, call, rest, under, abstract_call);
            return true;
        }
        let world = std::mem::replace(&mut self.store, frame.parked);
        drop(world);
        {
            let mut tables = self.program.tables.borrow_mut();
            if frame.leader_min < depth {
                tables.set_state(&key, TableState::Incomplete);
                let leader = &mut self.eval_stack[frame.leader_min];
                leader.dependents.push(key.clone());
                leader.dependents.extend(frame.dependents);
            } else {
                tables.set_state(&key, TableState::Complete);
                for d in &frame.dependents {
                    tables.set_state(d, TableState::Complete);
                }
            }
        }
        self.consume(key, 0, call, rest, under)
    }

    /// Unify `call` with the answers of its table from `from` on, as with a
    /// list of facts: a choice point for the following answers (always one
    /// while the table may still grow). A table that is not complete may
    /// not be consumed inside a condition or a comprehension body.
    fn consume(
        &mut self,
        key: Rc<TTerm>,
        from: usize,
        call: Addr,
        rest: Goals<'p>,
        under: Under,
    ) -> bool {
        let (len, complete) = {
            let tables = self.program.tables.borrow();
            (
                tables.len(&key),
                tables.state(&key) == Some(TableState::Complete),
            )
        };
        if !complete {
            if let Some(construct) = enclosing_sub_search(&rest) {
                let predicate = self.pred_name(pred_key(&self.store, call));
                self.store.error = Some(EvalError::IncompleteTable {
                    predicate,
                    construct,
                });
                return false;
            }
        }
        for i in from..len {
            let answer = self
                .program
                .tables
                .borrow()
                .answer(&key, i)
                .map(|(a, _)| a.clone());
            let Some(answer) = answer else {
                break;
            };
            let mark = self.store.mark();
            // Ground: no clause variables to supply.
            let built = build_with(&mut self.store, &answer, &mut |s, _| s.new_var());
            let Some(built) = built else {
                self.store.undo_to(&mark);
                continue;
            };
            if !self.store.post_eq(call, built) {
                self.store.undo_to(&mark);
                continue;
            }
            if i + 1 < len || !complete {
                self.push_choice_point(
                    Retry::Answers {
                        key: key.clone(),
                        next: i + 1,
                        rest: rest.clone(),
                        call,
                        under,
                    },
                    mark,
                );
            }
            self.record(under, ProofKind::Tabled { key, idx: i, call });
            self.goals = rest;
            return true;
        }
        false
    }

    /// CLM-0027: the constraints a solution of the current evaluation holds
    /// under, beyond the world, projected onto the abstract call — rendered,
    /// or `None` when it is a fact. The store's cheap test answers most
    /// solutions; otherwise the answer printer's projection decides exactly.
    fn residual_of_solution(&self) -> Option<String> {
        if !self.store.has_residual() {
            return None;
        }
        let frame = self.eval_stack.last()?;
        let names: Vec<(String, Addr)> = frame
            .abstract_vars
            .iter()
            .enumerate()
            .map(|(i, a)| (format!("_T{i}"), *a))
            .collect();
        let (solution, _) = render_answer(&self.store.symbols, &self.store, &names, &[]);
        if solution.disequations.is_empty() && solution.constraints.is_empty() {
            return None;
        }
        // What the world alone says about the same unknowns is not residual:
        // a looked-up world fact prints with the solution as it would with
        // an answer.
        let mut world = self.program.world.clone();
        world.touch_like(&self.store);
        let (base, _) = render_answer(&world.symbols, &world, &[], &[]);
        let residual: Vec<String> = solution
            .disequations
            .iter()
            .chain(solution.constraints.iter())
            .filter(|c| !base.disequations.contains(c) && !base.constraints.contains(c))
            .cloned()
            .collect();
        if residual.is_empty() {
            None
        } else {
            Some(residual.join(", "))
        }
    }

    /// A query stopped inside an evaluation: the outermost parked store is
    /// the query's, and every table not complete is forgotten.
    fn abandon_evaluations(&mut self) {
        if self.eval_stack.is_empty() {
            return;
        }
        let outer = self.eval_stack.drain(..).next().map(|f| f.parked);
        if let Some(store) = outer {
            self.store = store;
        }
        self.program.tables.borrow_mut().abandon();
    }

    /// CLM-0022: the groundness rule. A condition's external variables must
    /// be ground when the construct is reached; otherwise the query stops
    /// with `EvalError::NonGroundCondition` (false, with the store's error
    /// set).
    fn check_condition(&mut self, frame: &Frame, check: &CondCheck) -> bool {
        for (i, name) in &check.external {
            if !self.store.is_ground(frame[*i as usize]) {
                self.store.error = Some(EvalError::NonGroundCondition {
                    construct: check.construct.clone(),
                    variable: name.clone(),
                    clause: check.clause.clone(),
                });
                return false;
            }
        }
        true
    }

    /// CLM-0022: open a construct — push its alternative (if any) and run its
    /// first part, with a commit after a condition.
    fn open_construct(
        &mut self,
        frame: &Frame,
        goal: &'p TGoal,
        rest: Goals<'p>,
        under: Under,
    ) -> bool {
        match goal {
            TGoal::Or(a, b) => {
                let mark = self.store.mark();
                self.push_choice_point(
                    Retry::Goals {
                        goals: prepend(b, frame, under, rest.clone()),
                        note: None,
                    },
                    mark,
                );
                self.goals = prepend(a, frame, under, rest);
            }
            TGoal::IfThenElse {
                cond,
                then,
                els,
                check,
            } => {
                if !self.check_condition(frame, check) {
                    return false;
                }
                let height = self.choice_points.len();
                if let Some(e) = els {
                    let mark = self.store.mark();
                    self.push_choice_point(
                        Retry::Goals {
                            goals: prepend(e, frame, under, rest.clone()),
                            note: Some((under, check.construct.as_str())),
                        },
                        mark,
                    );
                }
                // The condition's proofs go under the committed node; the
                // then branch's follow as siblings of the construct.
                let node = self.record(
                    under,
                    ProofKind::Committed {
                        text: check.construct.as_str(),
                    },
                );
                let commit = Goal::Commit {
                    height,
                    then: Then::Goals {
                        frame: frame.clone(),
                        goals: then,
                        under,
                    },
                    text: check.construct.as_str(),
                };
                self.goals = prepend(cond, frame, node, cons(commit, rest));
            }
            TGoal::Not { cond, check } => {
                if !self.check_condition(frame, check) {
                    return false;
                }
                let height = self.choice_points.len();
                let mark = self.store.mark();
                self.push_choice_point(
                    Retry::Goals {
                        goals: rest.clone(),
                        note: Some((under, check.construct.as_str())),
                    },
                    mark,
                );
                let commit = Goal::Commit {
                    height,
                    then: Then::Fail,
                    text: check.construct.as_str(),
                };
                // A succeeding condition fails the negation, so its records
                // never survive; they go under the clause's record meanwhile.
                self.goals = prepend(cond, frame, under, cons(commit, rest));
            }
            TGoal::Collect {
                template,
                body,
                into,
                check,
            } => {
                if !self.check_condition(frame, check) {
                    return false;
                }
                let acc: Rc<RefCell<Vec<TTerm>>> = Rc::new(RefCell::new(Vec::new()));
                let mark = self.store.mark();
                self.push_choice_point(
                    Retry::Finish {
                        rest: rest.clone(),
                        frame: frame.clone(),
                        into: *into,
                        acc: acc.clone(),
                        under,
                        text: check.construct.as_str(),
                    },
                    mark,
                );
                let one = Goal::CollectOne {
                    frame: frame.clone(),
                    template,
                    construct: check.construct.as_str(),
                    acc,
                };
                // The body's records are discarded with each solution.
                self.goals = prepend(body, frame, under, cons(one, rest));
            }
            TGoal::Call(_) | TGoal::Eq(..) | TGoal::Dif(..) | TGoal::Rel(..) => {
                unreachable!("not a control construct")
            }
        }
        true
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
            let (goal, under) = match &node.goal {
                Goal::Commit { height, then, .. } => {
                    if !self.store.finalize() {
                        // Not a solution after all: the condition's next
                        // alternative, if any.
                        if self.store.error.is_some()
                            || self.store.nonlinear.is_some()
                            || !self.backtrack()
                        {
                            return false;
                        }
                        continue;
                    }
                    self.choice_points.truncate(*height);
                    match then {
                        Then::Fail => {
                            if !self.backtrack() {
                                return false;
                            }
                        }
                        Then::Goals {
                            frame,
                            goals,
                            under,
                        } => self.goals = prepend(goals, frame, *under, rest),
                    }
                    continue;
                }
                Goal::CollectOne {
                    frame,
                    template,
                    construct,
                    acc,
                } => {
                    if !self.store.finalize() {
                        if self.store.error.is_some()
                            || self.store.nonlinear.is_some()
                            || !self.backtrack()
                        {
                            return false;
                        }
                        continue;
                    }
                    let frame = frame.clone();
                    let built =
                        build_with(&mut self.store, template, &mut |_, i| frame[i as usize]);
                    let Some(inst) = built else {
                        // The template's own arithmetic failed: not a solution.
                        if self.store.error.is_some() || !self.backtrack() {
                            return false;
                        }
                        continue;
                    };
                    match self.store.snapshot(inst) {
                        Some(copy) => {
                            acc.borrow_mut().push(copy);
                            if !self.backtrack() {
                                return false;
                            }
                            continue;
                        }
                        None => {
                            let instance = self.render_one(inst);
                            self.store.error = Some(EvalError::NonGroundSolution {
                                construct: construct.to_string(),
                                instance,
                            });
                            return false;
                        }
                    }
                }
                Goal::TableOne { key, call } => {
                    if !self.store.finalize() {
                        if self.store.error.is_some()
                            || self.store.nonlinear.is_some()
                            || !self.backtrack()
                        {
                            return false;
                        }
                        continue;
                    }
                    let (key, call) = (key.clone(), *call);
                    match self.store.snapshot(call) {
                        Some(answer) => {
                            if let Some(residual) = self.residual_of_solution() {
                                let predicate = self.pred_name(pred_key(&self.store, call));
                                let instance = self.render_one(call);
                                self.store.error = Some(EvalError::ConstrainedTabledAnswer {
                                    predicate,
                                    instance,
                                    residual,
                                });
                                return false;
                            }
                            let proof = if self.proof.is_some() {
                                let start = self.eval_stack.last().map_or(0, |f| f.proof_start);
                                Some(self.render_proof(start).1)
                            } else {
                                None
                            };
                            self.program.tables.borrow_mut().add(&key, answer, proof);
                            if !self.backtrack() {
                                return false;
                            }
                            continue;
                        }
                        None => {
                            let predicate = self.pred_name(pred_key(&self.store, call));
                            let instance = self.render_one(call);
                            self.store.error = Some(EvalError::NonGroundTabledAnswer {
                                predicate,
                                instance,
                            });
                            return false;
                        }
                    }
                }
                Goal::Resolve { call, under } => {
                    let (call, under) = (*call, *under);
                    let key = pred_key(&self.store, call);
                    let first = first_arg_key(&self.store, call);
                    let candidates = self.program.candidates(&key, first.as_ref());
                    let ok = self.try_clauses(key, candidates, 0, call, rest, under);
                    if !ok && (self.store.error.is_some() || !self.backtrack()) {
                        return false;
                    }
                    continue;
                }
                Goal::Pending {
                    frame,
                    goal:
                        goal @ (TGoal::Or(..)
                        | TGoal::IfThenElse { .. }
                        | TGoal::Not { .. }
                        | TGoal::Collect { .. }),
                    under,
                } => {
                    if !self.open_construct(frame, goal, rest, *under) {
                        return false;
                    }
                    continue;
                }
                Goal::Pending { frame, goal, under } => {
                    let frame = frame.clone();
                    let built =
                        instantiate_goal(&mut self.store, goal, &mut |_, i| frame[i as usize]);
                    match built {
                        Some(g) => (g, *under),
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
                            let ok = self.run_builtin(b, call);
                            if ok {
                                self.record(under, ProofKind::Builtin { call });
                            }
                            ok
                        }
                        None if self.program.tabled.contains(&key) => {
                            self.tabled_call(call, rest, under)
                        }
                        None => {
                            let first = first_arg_key(&self.store, call);
                            let candidates = self.program.candidates(&key, first.as_ref());
                            self.try_clauses(key, candidates, 0, call, rest, under)
                        }
                    }
                }
                RtGoal::Eq(a, b) => {
                    self.goals = rest;
                    self.constraint(under, a, "=", b, |s| s.post_eq_goal(a, b))
                }
                RtGoal::Dif(a, b) => {
                    self.goals = rest;
                    self.constraint(under, a, "!=", b, |s| s.post_dif_goal(a, b))
                }
                RtGoal::Rel(op, a, b) => {
                    self.goals = rest;
                    let text = match op {
                        RelOp::Eq => "=",
                        RelOp::Neq => "!=",
                        RelOp::Lt => "<",
                        RelOp::Gt => ">",
                        RelOp::Le => "<=",
                        RelOp::Ge => ">=",
                    };
                    self.constraint(under, a, text, b, |s| s.post_rel(op, a, b))
                }
            };
            if !ok && (self.store.error.is_some() || !self.backtrack()) {
                return false;
            }
        }
    }

    /// Post a constraint goal and, if it holds, record it (CLM-0023).
    fn constraint(
        &mut self,
        under: Under,
        a: Addr,
        op: &'static str,
        b: Addr,
        post: impl FnOnce(&mut Store) -> bool,
    ) -> bool {
        let ok = post(&mut self.store);
        if ok {
            self.record(under, ProofKind::Constraint { a, op, b });
        }
        ok
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
            Builtin::Sum => self.store.post_sum(args[0], args[1]),
        }
    }

    /// The answer, with its explanation when recording (CLM-0023): the
    /// arena's goal instances are rendered by the answer's own printer, then
    /// the tree is built bottom-up (children have higher ids than their
    /// parent, so a reverse pass sees every child before its parent).
    fn answer(&self) -> Answer {
        if self.proof.is_none() {
            return render_answer(&self.store.symbols, &self.store, &self.query_vars, &[]).0;
        }
        let (mut answer, roots) = self.render_proof(0);
        answer.explanation = Some(roots);
        answer
    }

    /// Render the arena's records from `from` on as proof trees (a record
    /// whose parent is below `from` is a root), with the answer of the
    /// current store. CLM-0027: a table evaluation renders its slice with
    /// its own store before the records are discarded.
    fn render_proof(&self, from: usize) -> (Answer, Vec<ProofNode>) {
        let arena: &[ProofRec<'p>] = match &self.proof {
            Some(a) => &a[from..],
            None => &[],
        };
        let mut addrs = Vec::new();
        for rec in arena {
            match &rec.kind {
                ProofKind::Clause { call, .. }
                | ProofKind::Builtin { call }
                | ProofKind::Tabled { call, .. } => addrs.push(*call),
                ProofKind::Constraint { a, b, .. } => {
                    addrs.push(*a);
                    addrs.push(*b);
                }
                ProofKind::Collected { into, .. } => addrs.push(*into),
                ProofKind::Committed { .. } | ProofKind::NoSolution { .. } => {}
            }
        }
        let (answer, rendered) =
            render_answer(&self.store.symbols, &self.store, self.names(), &addrs);
        let mut rendered = rendered.into_iter();
        // The clause each record cites: its own for a clause, the enclosing
        // clause's for a constraint goal.
        let cited = |i: usize| -> Option<(Option<crate::Span>, &str)> {
            let ProofKind::Clause { key, idx, .. } = &arena[i.checked_sub(from)?].kind else {
                return None;
            };
            let c = &self.program.clauses(key)[*idx];
            Some((c.span, c.text.as_str()))
        };
        let mut nodes: Vec<ProofNode> = Vec::with_capacity(arena.len());
        for rec in arena.iter() {
            let mut subgoals = Vec::new();
            let (goal, by, cite, clause) = match &rec.kind {
                ProofKind::Clause { key, idx, .. } => {
                    let c = &self.program.clauses(key)[*idx];
                    let by = if c.body.is_empty() {
                        Resolution::Fact
                    } else {
                        Resolution::Rule
                    };
                    (
                        rendered.next().unwrap_or_default(),
                        by,
                        c.span,
                        Some(c.text.clone()),
                    )
                }
                ProofKind::Constraint { op, .. } => {
                    let a = rendered.next().unwrap_or_default();
                    let b = rendered.next().unwrap_or_default();
                    let cite = rec.parent.and_then(cited).and_then(|(s, _)| s);
                    (
                        format!("{{ {a} {op} {b} }}"),
                        Resolution::Constraint,
                        cite,
                        None,
                    )
                }
                ProofKind::Builtin { .. } => (
                    rendered.next().unwrap_or_default(),
                    Resolution::Builtin,
                    None,
                    None,
                ),
                ProofKind::Committed { text } => {
                    (text.to_string(), Resolution::Committed, None, None)
                }
                ProofKind::NoSolution { text } => {
                    (text.to_string(), Resolution::NoSolution, None, None)
                }
                ProofKind::Collected { text, .. } => {
                    let list = rendered.next().unwrap_or_default();
                    (
                        format!("{text} = {list}"),
                        Resolution::Collected,
                        None,
                        None,
                    )
                }
                ProofKind::Tabled { key, idx, .. } => {
                    // The derivation that first produced the answer.
                    if let Some((_, Some(proof))) = self.program.tables.borrow().answer(key, *idx) {
                        subgoals = proof.clone();
                    }
                    (
                        rendered.next().unwrap_or_default(),
                        Resolution::Tabled,
                        None,
                        None,
                    )
                }
            };
            nodes.push(ProofNode {
                goal,
                by,
                line: cite.map(|s| s.line),
                column: cite.map(|s| s.column),
                clause,
                subgoals,
            });
        }
        // Attach children to parents, last to first, so each node is complete
        // before it is moved into its parent.
        let mut roots = Vec::new();
        let mut slots: Vec<Option<ProofNode>> = nodes.into_iter().map(Some).collect();
        for i in (0..arena.len()).rev() {
            let node = slots[i].take().expect("each node moves once");
            match arena[i].parent {
                Some(p) if p >= from => slots[p - from]
                    .as_mut()
                    .expect("parents precede children")
                    .subgoals
                    .insert(0, node),
                _ => roots.push(node),
            }
        }
        roots.reverse();
        (answer, roots)
    }

    /// Take the store's runtime error, rendering a non-linear residue with
    /// the query's variable names.
    fn take_error(&mut self) -> Option<EvalError> {
        if let Some(e) = self.store.error.take() {
            return Some(e);
        }
        let (a, b, op) = self.store.nonlinear.take()?;
        let rendered = render_terms(&self.store.symbols, &self.store, self.names(), &[a, b]);
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
            self.abandon_evaluations();
            return None;
        }
        self.started = true;
        if self.run() {
            Some(self.answer())
        } else {
            self.exhausted = true;
            self.error = self.take_error();
            self.abandon_evaluations();
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
        TGoal::Or(..) | TGoal::IfThenElse { .. } | TGoal::Not { .. } | TGoal::Collect { .. } => {
            unreachable!("control constructs are opened by the machine, not instantiated")
        }
    })
}

/// CLM-0027: the construct — a condition or a comprehension body — that the
/// goals still to run belong to, if any: a `Commit` or `CollectOne` later in
/// the list means the current goal is inside it. The list of a table
/// evaluation has an empty tail, so nothing outside the evaluation is seen.
fn enclosing_sub_search(rest: &Goals<'_>) -> Option<String> {
    let mut cur = rest.clone();
    while let Some(node) = cur {
        match &node.goal {
            Goal::Commit { text, .. } => return Some(text.to_string()),
            Goal::CollectOne { construct, .. } => return Some(construct.to_string()),
            _ => {}
        }
        cur = node.next.clone();
    }
    None
}

/// The index key of a call's first argument (CLM-0021): `None` when the
/// call is nullary or the argument is an unbound variable — numeric or not,
/// since a determined numeric variable is still a `Var` cell and may unify
/// with a number.
fn first_arg_key(store: &Store, call: Addr) -> Option<ArgKey> {
    let Cell::Struct(_, args) = store.cell(store.deref(call)) else {
        return None;
    };
    let a = store.deref(*args.first()?);
    match store.cell(a) {
        Cell::Const(c) => Some(ArgKey::Const(*c)),
        Cell::Num(n) => Some(ArgKey::Num(n.clone())),
        Cell::Struct(f, xs) => Some(ArgKey::Struct(*f, xs.len())),
        Cell::Var(_) => None,
    }
}

fn pred_key(store: &Store, call: Addr) -> PredKey {
    match store.cell(store.deref(call)) {
        Cell::Struct(f, args) => (*f, args.len()),
        Cell::Const(c) => (*c, 0),
        other => unreachable!("call goals are atoms, got {other:?}"),
    }
}
