// SPDX-License-Identifier: Apache-2.0

//! Lowering `ast::Clause`s to the runtime program: clause templates with
//! pre-numbered variables (structure copying at instantiation), predicate
//! index, the initial store's constraint facts, and the queries.
//!
//! Arithmetic terms stay in the templates and are lowered at instantiation:
//! each becomes a fresh numeric variable plus a linear constraint posted to
//! the store (design record `2026-08-17-arithmetic-in-terms.md`).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use crate::Span;
use crate::ast::{self, ArithOp, Clause, Expr, Goal, RelOp};
use crate::number::Number;

use super::error::EvalError;
use super::explain::{self, Step};
use super::machine::Solutions;
use super::store::{Addr, Store};
use super::symbol::{LIST_LABEL, Symbol, Symbols};
use super::table::Tables;

/// A term template: variables are clause-local indices. `Eq`/`Hash` since
/// CLM-0027: a table's variant key and its ground answers are templates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum TTerm {
    Var(u32),
    Number(Number),
    Const(Symbol),
    Compound(Symbol, Vec<TTerm>),
    /// Unary minus: lowered to a fresh numeric variable at instantiation.
    Neg(Box<TTerm>),
    /// Binary arithmetic: lowered to a fresh numeric variable at instantiation.
    Arith(ArithOp, Box<TTerm>, Box<TTerm>),
    /// List length `|L|` (CLM-0015): lowered to the list's length unknown.
    Len(Box<TTerm>),
    /// List concatenation `X . Y` (CLM-0016): lowered to the chain when the
    /// left spine is complete, else to a fresh list variable with a pending
    /// concatenation.
    Concat(Box<TTerm>, Box<TTerm>),
}

/// A term template's children, moved out so the template can be dropped in
/// a loop (CLM-0019): a chain as deep as a long list is long would
/// otherwise recurse once per cell in the drop glue.
impl Drop for TTerm {
    fn drop(&mut self) {
        let mut work: Vec<TTerm> = Vec::new();
        self.take_children(&mut work);
        while let Some(mut t) = work.pop() {
            t.take_children(&mut work);
        }
    }
}

impl TTerm {
    fn take_children(&mut self, out: &mut Vec<TTerm>) {
        match self {
            TTerm::Var(_) | TTerm::Number(_) | TTerm::Const(_) => {}
            TTerm::Compound(_, args) => out.append(args),
            TTerm::Neg(x) | TTerm::Len(x) => out.push(std::mem::replace(x, TTerm::Var(0))),
            TTerm::Arith(_, l, r) | TTerm::Concat(l, r) => {
                out.push(std::mem::replace(l, TTerm::Var(0)));
                out.push(std::mem::replace(r, TTerm::Var(0)));
            }
        }
    }
}

/// The lowering's result stack ran dry: a walk entered and built a node
/// out of step. Never happens for a well-formed tree.
fn unbalanced() -> EvalError {
    EvalError::InternalInvariant {
        what: "term lowering walk out of step".to_string(),
    }
}

/// A body goal template.
#[derive(Debug, Clone)]
pub(crate) enum TGoal {
    Call(TTerm),
    /// `=`: numeric equation if either side is numeric, else unification.
    Eq(TTerm, TTerm),
    /// `!=`: numeric disequation if either side is numeric, else `dif`.
    Dif(TTerm, TTerm),
    /// `< > <= >=`: always numeric.
    Rel(RelOp, TTerm, TTerm),
    /// `A ; B` (CLM-0022): both alternatives, in order.
    Or(Vec<TGoal>, Vec<TGoal>),
    /// `C -> T ; E` or `C -> T` (CLM-0022): commit to the condition's first
    /// solution, else the else branch (or failure).
    IfThenElse {
        cond: Vec<TGoal>,
        then: Vec<TGoal>,
        els: Option<Vec<TGoal>>,
        check: CondCheck,
    },
    /// `\+ G` (CLM-0022): `(G -> fail ; true)`.
    Not {
        cond: Vec<TGoal>,
        check: CondCheck,
    },
    /// A list comprehension (CLM-0025): collect the instances of `template`
    /// for every solution of `body` into the fresh clause variable `into`,
    /// which the goal that followed the comprehension then uses.
    Collect {
        template: TTerm,
        body: Vec<TGoal>,
        into: u32,
        check: CondCheck,
    },
}

/// What the machine checks when it reaches a condition (CLM-0022,
/// docs/design/2026-11-09-negation-disjunction-and-if-then-else.md): the
/// condition's *external* variables — those also occurring in the clause
/// outside the whole construct — must be ground, or the query stops. Kept
/// with the construct's and the clause's source text for the diagnostic.
#[derive(Debug, Clone, Default)]
pub(crate) struct CondCheck {
    /// Clause-variable index and source name, in index order.
    pub external: Vec<(u32, String)>,
    pub construct: String,
    pub clause: String,
}

/// Does a body contain a control construct (CLM-0022) or a collection
/// (CLM-0025)?
fn has_control(goals: &[TGoal]) -> bool {
    goals.iter().any(|g| {
        matches!(
            g,
            TGoal::Or(..) | TGoal::IfThenElse { .. } | TGoal::Not { .. } | TGoal::Collect { .. }
        )
    })
}

/// The clause variables a term mentions. Iterative: a literal may be deep
/// (CLM-0019).
pub(crate) fn term_vars(root: &TTerm, out: &mut BTreeSet<u32>) {
    let mut work = vec![root];
    while let Some(t) = work.pop() {
        match t {
            TTerm::Var(i) => {
                out.insert(*i);
            }
            TTerm::Number(_) | TTerm::Const(_) => {}
            TTerm::Compound(_, args) => work.extend(args.iter()),
            TTerm::Neg(x) | TTerm::Len(x) => work.push(x),
            TTerm::Arith(_, l, r) | TTerm::Concat(l, r) => {
                work.push(l);
                work.push(r);
            }
        }
    }
}

/// The clause variables a body mentions, constructs included. Recursion
/// follows the source nesting of constructs only.
fn goals_vars(goals: &[TGoal], out: &mut BTreeSet<u32>) {
    for g in goals {
        match g {
            TGoal::Call(t) => term_vars(t, out),
            TGoal::Eq(a, b) | TGoal::Dif(a, b) | TGoal::Rel(_, a, b) => {
                term_vars(a, out);
                term_vars(b, out);
            }
            TGoal::Or(a, b) => {
                goals_vars(a, out);
                goals_vars(b, out);
            }
            TGoal::IfThenElse {
                cond, then, els, ..
            } => {
                goals_vars(cond, out);
                goals_vars(then, out);
                if let Some(e) = els {
                    goals_vars(e, out);
                }
            }
            TGoal::Not { cond, .. } => goals_vars(cond, out),
            TGoal::Collect {
                template,
                body,
                into,
                ..
            } => {
                term_vars(template, out);
                goals_vars(body, out);
                out.insert(*into);
            }
        }
    }
}

/// Resolve every condition's external variables in `goals` (CLM-0022):
/// `outside` holds the variables occurring in the clause outside this goal
/// sequence (the head, enclosing constructs' other parts); a construct's
/// surroundings are those plus its siblings in the sequence.
fn resolve_conditions(
    goals: &mut [TGoal],
    outside: &BTreeSet<u32>,
    names: &[String],
    clause: &str,
) {
    // Variables of each goal, so that sibling sets are cheap to form.
    let per_goal: Vec<BTreeSet<u32>> = goals
        .iter()
        .map(|g| {
            let mut s = BTreeSet::new();
            goals_vars(std::slice::from_ref(g), &mut s);
            s
        })
        .collect();
    for (i, g) in goals.iter_mut().enumerate() {
        if matches!(
            g,
            TGoal::Call(_) | TGoal::Eq(..) | TGoal::Dif(..) | TGoal::Rel(..)
        ) {
            continue;
        }
        let mut around: BTreeSet<u32> = outside.clone();
        for (j, s) in per_goal.iter().enumerate() {
            if j != i {
                around.extend(s.iter().copied());
            }
        }
        match g {
            TGoal::Or(a, b) => {
                let mut va = BTreeSet::new();
                goals_vars(a, &mut va);
                let mut vb = BTreeSet::new();
                goals_vars(b, &mut vb);
                let mut oa = around.clone();
                oa.extend(vb.iter().copied());
                resolve_conditions(a, &oa, names, clause);
                let mut ob = around;
                ob.extend(va.iter().copied());
                resolve_conditions(b, &ob, names, clause);
            }
            TGoal::IfThenElse {
                cond,
                then,
                els,
                check,
            } => {
                let mut vc = BTreeSet::new();
                goals_vars(cond, &mut vc);
                let mut vt = BTreeSet::new();
                goals_vars(then, &mut vt);
                let mut ve = BTreeSet::new();
                if let Some(e) = els {
                    goals_vars(e, &mut ve);
                }
                check.external = vc
                    .iter()
                    .filter(|v| around.contains(v))
                    .map(|v| (*v, names[*v as usize].clone()))
                    .collect();
                check.clause = clause.to_string();
                let mut oc = around.clone();
                oc.extend(vt.iter().copied());
                oc.extend(ve.iter().copied());
                resolve_conditions(cond, &oc, names, clause);
                let mut ot = around.clone();
                ot.extend(vc.iter().copied());
                ot.extend(ve.iter().copied());
                resolve_conditions(then, &ot, names, clause);
                if let Some(e) = els {
                    let mut oe = around;
                    oe.extend(vc.iter().copied());
                    oe.extend(vt.iter().copied());
                    resolve_conditions(e, &oe, names, clause);
                }
            }
            TGoal::Not { cond, check } => {
                let mut vc = BTreeSet::new();
                goals_vars(cond, &mut vc);
                check.external = vc
                    .iter()
                    .filter(|v| around.contains(v))
                    .map(|v| (*v, names[*v as usize].clone()))
                    .collect();
                check.clause = clause.to_string();
                resolve_conditions(cond, &around, names, clause);
            }
            // CLM-0025: the comprehension's variables are the template's and
            // the body's; `into` is the result, outside the construct.
            TGoal::Collect {
                template,
                body,
                check,
                ..
            } => {
                let mut vt = BTreeSet::new();
                term_vars(template, &mut vt);
                let mut vb = BTreeSet::new();
                goals_vars(body, &mut vb);
                check.external = vt
                    .union(&vb)
                    .filter(|v| around.contains(v))
                    .map(|v| (*v, names[*v as usize].clone()))
                    .collect();
                check.clause = clause.to_string();
                let mut ob = around;
                ob.extend(vt.iter().copied());
                resolve_conditions(body, &ob, names, clause);
            }
            TGoal::Call(_) | TGoal::Eq(..) | TGoal::Dif(..) | TGoal::Rel(..) => {}
        }
    }
}

/// A compiled clause: head, body, the number of distinct variables, and
/// (CLM-0023) its source position and text as written, for explanations.
#[derive(Debug, Clone)]
pub(crate) struct TClause {
    pub head: TTerm,
    pub body: Vec<TGoal>,
    pub nvars: u32,
    pub span: Option<Span>,
    pub text: String,
}

/// A constraint fact `{ … }.` of the program: the initial store's content,
/// cited by position and text when the facts conflict (CLM-0030).
#[derive(Debug, Clone)]
pub(crate) struct Fact {
    pub goals: Vec<TGoal>,
    pub nvars: u32,
    pub span: Option<Span>,
    pub text: String,
}

/// Predicate key: functor and arity.
pub(crate) type PredKey = (Symbol, usize);

/// The principal functor of a first argument (CLM-0021): what a call's
/// first argument must have for a clause to be able to match. Two terms
/// with different keys never unify.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ArgKey {
    Const(Symbol),
    Num(Number),
    Struct(Symbol, usize),
}

/// A predicate's clauses with their first-argument index (CLM-0021). Every
/// list holds clause positions in program order, so clause order — the
/// language's semantics — is untouched by the index.
#[derive(Debug, Clone, Default)]
pub(crate) struct Pred {
    pub clauses: Vec<TClause>,
    /// Every clause: for a call whose first argument is unbound, or a
    /// nullary predicate.
    all: Vec<usize>,
    /// Per key: the clauses with that key merged with the variable-first
    /// clauses.
    by_key: HashMap<ArgKey, Vec<usize>>,
    /// The clauses whose first argument is a variable (or an arithmetic
    /// term, whose value is not known statically): the candidates for a key
    /// no clause has.
    var_only: Vec<usize>,
}

impl Pred {
    fn push(&mut self, clause: TClause) {
        let pos = self.clauses.len();
        let key = match &clause.head {
            TTerm::Compound(_, args) => args.first().and_then(arg_key),
            _ => None,
        };
        self.all.push(pos);
        match key {
            Some(k) => self.by_key.entry(k).or_default().push(pos),
            None => {
                // A variable-first clause is a candidate for every key.
                self.var_only.push(pos);
                for list in self.by_key.values_mut() {
                    list.push(pos);
                }
            }
        }
        self.clauses.push(clause);
    }

    /// Finish the index: a key list must also hold the variable-first
    /// clauses that came *before* the key's first clause (they were not in
    /// the map yet when pushed).
    fn finish(&mut self) {
        for list in self.by_key.values_mut() {
            list.extend(self.var_only.iter().copied());
            list.sort_unstable();
            list.dedup();
        }
    }

    /// The clauses that can match a call whose first argument has `key`
    /// (`None`: unbound), as positions in program order.
    fn candidates(&self, key: Option<&ArgKey>) -> &[usize] {
        match key {
            None => &self.all,
            Some(k) => self
                .by_key
                .get(k)
                .map(Vec::as_slice)
                .unwrap_or(&self.var_only),
        }
    }
}

/// The index key of a head's first argument; `None` when it can match
/// anything (a variable) or its value is unknown at compile time (an
/// arithmetic term).
fn arg_key(t: &TTerm) -> Option<ArgKey> {
    match t {
        TTerm::Const(c) => Some(ArgKey::Const(*c)),
        TTerm::Number(n) => Some(ArgKey::Num(n.clone())),
        TTerm::Compound(f, args) => Some(ArgKey::Struct(*f, args.len())),
        TTerm::Var(_) | TTerm::Neg(_) | TTerm::Arith(..) | TTerm::Len(_) | TTerm::Concat(..) => {
            None
        }
    }
}

/// The built-in predicates (CLM-0017): the only reserved names besides
/// `'<>'`. Each posts a constraint; none creates a choice point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Builtin {
    /// `atom_chars(A, L)`: `L` is the list of `A`'s characters, delayed.
    AtomChars,
    /// `number(X)`.
    Number,
    /// `atom(X)`, delayed.
    Atom,
    /// `list(X)`, delayed.
    List,
    /// `sum(L, S)` (CLM-0025): `S` is the sum of the list's elements, a
    /// linear equation once the spine is complete; delayed on the tail.
    Sum,
}

const BUILTINS: [(&str, usize, Builtin); 5] = [
    ("atom_chars", 2, Builtin::AtomChars),
    ("number", 1, Builtin::Number),
    ("atom", 1, Builtin::Atom),
    ("list", 1, Builtin::List),
    ("sum", 2, Builtin::Sum),
];

/// A compiled query: its goals, its variable names in order of first
/// occurrence, and its source rendering.
#[derive(Debug, Clone)]
pub struct Query {
    pub(crate) goals: Vec<TGoal>,
    pub(crate) var_names: Vec<String>,
    text: String,
}

impl Query {
    /// The query as written (canonically re-rendered), e.g. `?- mortal(X).`
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// A compiled program, ready to answer queries.
#[derive(Debug, Clone)]
pub struct Program {
    /// The symbol table, shared with every store the program runs (CLM-0028).
    pub(crate) symbols: Rc<RefCell<Symbols>>,
    preds: HashMap<PredKey, Pred>,
    /// The built-in predicates by key (CLM-0017).
    pub(crate) builtins: HashMap<PredKey, Builtin>,
    /// Constraint facts — the initial store — with position and text for
    /// the load-time conflict set (CLM-0030).
    pub(crate) initial: Vec<Fact>,
    queries: Vec<Query>,
    /// CLM-0027: the predicates declared `:- table`.
    pub(crate) tabled: HashSet<PredKey>,
    /// The world store — the constraint facts posted, baseline set — built
    /// once here; every query, and every table evaluation (CLM-0027), runs
    /// on a clone.
    pub(crate) world: Store,
    /// CLM-0027: the tables, a function of the program; behind a `RefCell`
    /// because queries borrow the program shared.
    pub(crate) tables: RefCell<Tables>,
}

/// Per-clause variable numbering.
#[derive(Default)]
struct VarMap {
    names: Vec<String>,
    index: HashMap<String, u32>,
}

/// The anonymous variable (CLM-0020): every occurrence is a fresh variable.
pub(crate) const ANONYMOUS: &str = "_";

impl VarMap {
    /// A fresh, unnamed variable (CLM-0025: the result of a comprehension).
    fn fresh(&mut self) -> u32 {
        self.get(ANONYMOUS)
    }

    fn get(&mut self, name: &str) -> u32 {
        if name != ANONYMOUS {
            if let Some(&i) = self.index.get(name) {
                return i;
            }
        }
        let i = self.names.len() as u32;
        self.names.push(name.to_string());
        if name != ANONYMOUS {
            self.index.insert(name.to_string(), i);
        }
        i
    }
}

struct Lowerer<'a> {
    symbols: &'a mut Symbols,
    vars: VarMap,
    /// CLM-0025: the collections the terms lowered so far asked for, to be
    /// emitted before the goal they belong to.
    collects: Vec<TGoal>,
}

/// A step of an explicit post-order walk (CLM-0019): `Enter` schedules a
/// node's children, `Build` makes the node from their results.
enum Walk<'a, N> {
    Enter(&'a N),
    Build(&'a N),
}

/// The last `k` results, in order.
fn take_last<T>(out: &mut Vec<T>, k: usize) -> Vec<T> {
    out.split_off(out.len() - k)
}

fn take_two<T>(out: &mut Vec<T>) -> Option<(T, T)> {
    let r = out.pop()?;
    let l = out.pop()?;
    Some((l, r))
}

impl Lowerer<'_> {
    /// Lower a term template. Iterative (CLM-0019): an explicit stack walks
    /// the expression post-order, so the depth of a literal — a nested term
    /// or a long list, which lowers to a chain as deep as it is long — costs
    /// no Rust stack.
    fn term(&mut self, root: &Expr) -> Result<TTerm, EvalError> {
        let mut work = vec![Walk::Enter(root)];
        let mut out: Vec<TTerm> = Vec::new();
        while let Some(step) = work.pop() {
            match step {
                Walk::Enter(e) => {
                    work.push(Walk::Build(e));
                    // Children are entered in reverse so that they are built
                    // left to right, in source order.
                    match e {
                        Expr::Var(_) | Expr::Number(_) | Expr::Ident(_) => {}
                        Expr::Atom(a) => work.extend(a.args.iter().rev().map(Walk::Enter)),
                        Expr::Neg(x) | Expr::Len(x) => work.push(Walk::Enter(x)),
                        Expr::Binary { left, right, .. } | Expr::Concat(left, right) => {
                            work.push(Walk::Enter(right));
                            work.push(Walk::Enter(left));
                        }
                        Expr::List { items, tail } => {
                            work.extend(tail.iter().map(|t| Walk::Enter(t.as_ref())));
                            work.extend(items.iter().rev().map(Walk::Enter));
                        }
                        // A comprehension is lowered as a unit at Build time.
                        Expr::Comprehension { .. } => {}
                    }
                }
                Walk::Build(e) => {
                    let built = match e {
                        Expr::Var(v) => TTerm::Var(self.vars.get(v)),
                        Expr::Number(n) => TTerm::Number(n.clone()),
                        Expr::Ident(i) => TTerm::Const(self.symbols.intern(i)),
                        Expr::Atom(a) => {
                            let args = take_last(&mut out, a.args.len());
                            TTerm::Compound(self.symbols.intern(&a.name), args)
                        }
                        Expr::Neg(_) => TTerm::Neg(Box::new(out.pop().ok_or_else(unbalanced)?)),
                        Expr::Len(_) => TTerm::Len(Box::new(out.pop().ok_or_else(unbalanced)?)),
                        Expr::Binary { op, .. } => {
                            let (l, r) = take_two(&mut out).ok_or_else(unbalanced)?;
                            TTerm::Arith(*op, Box::new(l), Box::new(r))
                        }
                        Expr::Concat(..) => {
                            let (l, r) = take_two(&mut out).ok_or_else(unbalanced)?;
                            TTerm::Concat(Box::new(l), Box::new(r))
                        }
                        // CLM-0025: a comprehension becomes a fresh variable
                        // here and a collection step before the goal.
                        Expr::Comprehension { template, body } => {
                            // The template is evaluated once per solution, so
                            // a comprehension inside it is collected at the
                            // end of the body, not before this one.
                            let outer = std::mem::take(&mut self.collects);
                            let template = self.term(template)?;
                            let in_template = std::mem::take(&mut self.collects);
                            self.collects = outer;
                            let mut body = self.body(body)?;
                            body.extend(in_template);
                            let into = self.vars.fresh();
                            self.collects.push(TGoal::Collect {
                                template,
                                body,
                                into,
                                check: CondCheck {
                                    construct: e.to_string(),
                                    ..CondCheck::default()
                                },
                            });
                            TTerm::Var(into)
                        }
                        // CLM-0014: `<a, b> . T` is the chain '<>'(a, '<>'(b, T));
                        // a list without a tail ends in the constant '<>'.
                        Expr::List { items, tail } => {
                            let nil = self.symbols.intern(LIST_LABEL);
                            let mut t = match tail {
                                Some(_) => out.pop().ok_or_else(unbalanced)?,
                                None => TTerm::Const(nil),
                            };
                            for h in take_last(&mut out, items.len()).into_iter().rev() {
                                t = TTerm::Compound(nil, vec![h, t]);
                            }
                            t
                        }
                    };
                    out.push(built);
                }
            }
        }
        out.pop().ok_or_else(unbalanced)
    }

    fn atom(&mut self, a: &ast::Atom) -> Result<TTerm, EvalError> {
        let f = self.symbols.intern(&a.name);
        let mut args = Vec::with_capacity(a.args.len());
        for e in &a.args {
            args.push(self.term(e)?);
        }
        Ok(TTerm::Compound(f, args))
    }

    fn constraint(&mut self, c: &ast::Constraint) -> Result<TGoal, EvalError> {
        let l = self.term(&c.left)?;
        let r = self.term(&c.right)?;
        Ok(match c.op {
            RelOp::Eq => TGoal::Eq(l, r),
            RelOp::Neq => TGoal::Dif(l, r),
            op => TGoal::Rel(op, l, r),
        })
    }

    /// Lower a body. Recursion follows the source nesting of the control
    /// constructs (CLM-0022); a construct's external variables are resolved
    /// afterwards by `resolve_conditions`, once the whole clause is known.
    fn body(&mut self, goals: &[Goal]) -> Result<Vec<TGoal>, EvalError> {
        // Collections queued by an enclosing goal's earlier terms belong to
        // that goal, not to this body (CLM-0025).
        let saved = std::mem::take(&mut self.collects);
        let result = self.body_goals(goals);
        self.collects = saved;
        result
    }

    fn body_goals(&mut self, goals: &[Goal]) -> Result<Vec<TGoal>, EvalError> {
        let mut out = Vec::new();
        for g in goals {
            match g {
                Goal::Atom(a) => {
                    let call = TGoal::Call(self.atom(a)?);
                    out.append(&mut self.collects);
                    out.push(call);
                }
                Goal::Constraint(ce) => {
                    for c in &ce.terms {
                        let goal = self.constraint(c)?;
                        out.append(&mut self.collects);
                        out.push(goal);
                    }
                }
                Goal::Or(a, b) => out.push(TGoal::Or(self.body(a)?, self.body(b)?)),
                Goal::IfThenElse { cond, then, els } => out.push(TGoal::IfThenElse {
                    cond: self.body(cond)?,
                    then: self.body(then)?,
                    els: match els {
                        Some(e) => Some(self.body(e)?),
                        None => None,
                    },
                    check: CondCheck {
                        construct: g.to_string(),
                        ..CondCheck::default()
                    },
                }),
                Goal::Not(inner) => out.push(TGoal::Not {
                    cond: self.body(inner)?,
                    check: CondCheck {
                        construct: g.to_string(),
                        ..CondCheck::default()
                    },
                }),
            }
        }
        Ok(out)
    }

    /// CLM-0025: a head or a constraint fact admits no comprehension.
    fn no_collects(&self, span: Option<Span>) -> Result<(), EvalError> {
        if self.collects.is_empty() {
            Ok(())
        } else {
            Err(EvalError::ComprehensionInHead { span })
        }
    }

    /// Finish a lowered body (CLM-0022): resolve the conditions' external
    /// variables against the clause's head variables. Only bodies with a
    /// construct pay for the clause text.
    fn finish_body(&self, body: &mut [TGoal], head: Option<&TTerm>, clause: &Clause) {
        if !has_control(body) {
            return;
        }
        let mut outside = BTreeSet::new();
        if let Some(h) = head {
            term_vars(h, &mut outside);
        }
        let text = clause.to_string();
        resolve_conditions(body, &outside, &self.vars.names, &text);
    }
}

impl Program {
    /// Compile clauses into a program.
    pub fn compile(clauses: &[Clause]) -> Result<Program, EvalError> {
        Self::compile_iter(clauses.iter().map(|c| (c, None)))
    }

    /// Compile clauses parsed with [`crate::parse_program_spanned`]: a
    /// load-time diagnostic that points into the source
    /// ([`EvalError::ReservedName`]) then carries the clause's position.
    pub fn compile_spanned(clauses: &[(Clause, Span)]) -> Result<Program, EvalError> {
        Self::compile_iter(clauses.iter().map(|(c, s)| (c, Some(*s))))
    }

    fn compile_iter<'a>(
        clauses: impl Iterator<Item = (&'a Clause, Option<Span>)>,
    ) -> Result<Program, EvalError> {
        let mut symbols = Symbols::default();
        let builtins: HashMap<PredKey, Builtin> = BUILTINS
            .iter()
            .map(|(name, arity, b)| ((symbols.intern(name), *arity), *b))
            .collect();
        let mut preds: HashMap<PredKey, Pred> = HashMap::new();
        let mut initial = Vec::new();
        let mut queries = Vec::new();
        let mut tabled: HashSet<PredKey> = HashSet::new();

        for (clause, span) in clauses {
            // CLM-0014: `'<>'` is the list constructor; a program cannot define
            // a predicate of that name.
            if let Clause::Fact(head)
            | Clause::Rule { head, .. }
            | Clause::ConstraintRule { head, .. }
            | Clause::Implication { head, .. } = clause
            {
                if head.name == LIST_LABEL {
                    return Err(EvalError::ReservedName {
                        name: head.name.clone(),
                        span,
                    });
                }
                if BUILTINS
                    .iter()
                    .any(|(n, a, _)| *n == head.name && *a == head.args.len())
                {
                    return Err(EvalError::ReservedBuiltin {
                        name: head.name.clone(),
                        arity: head.args.len(),
                        span,
                    });
                }
            }
            let mut lw = Lowerer {
                symbols: &mut symbols,
                vars: VarMap::default(),
                collects: Vec::new(),
            };
            match clause {
                Clause::Fact(atom) => {
                    let head = lw.atom(atom)?;
                    lw.no_collects(span)?;
                    let nvars = lw.vars.names.len() as u32;
                    push_clause(
                        &mut preds,
                        TClause {
                            head,
                            body: vec![],
                            nvars,
                            span,
                            text: clause.to_string(),
                        },
                    );
                }
                Clause::Rule { head, body } | Clause::ConstraintRule { head, body } => {
                    let head = lw.atom(head)?;
                    lw.no_collects(span)?;
                    let mut body = lw.body(body)?;
                    lw.finish_body(&mut body, Some(&head), clause);
                    let nvars = lw.vars.names.len() as u32;
                    push_clause(
                        &mut preds,
                        TClause {
                            head,
                            body,
                            nvars,
                            span,
                            text: clause.to_string(),
                        },
                    );
                }
                // `{c} => head.` is sugar for `head :- {c}.` (design D3).
                Clause::Implication { constraint, head } => {
                    let head = lw.atom(head)?;
                    lw.no_collects(span)?;
                    // The constraints as a body, so a comprehension among them
                    // is collected before its constraint (CLM-0025).
                    let as_body = vec![Goal::Constraint(constraint.clone())];
                    let mut body = lw.body(&as_body)?;
                    lw.finish_body(&mut body, Some(&head), clause);
                    let nvars = lw.vars.names.len() as u32;
                    push_clause(
                        &mut preds,
                        TClause {
                            head,
                            body,
                            nvars,
                            span,
                            text: clause.to_string(),
                        },
                    );
                }
                // Global constraints: the initial store (design D3).
                Clause::ConstraintFact(ce) => {
                    let goals = ce
                        .terms
                        .iter()
                        .map(|c| lw.constraint(c))
                        .collect::<Result<Vec<_>, _>>()?;
                    lw.no_collects(span)?;
                    initial.push(Fact {
                        goals,
                        nvars: lw.vars.names.len() as u32,
                        span,
                        text: clause.to_string(),
                    });
                }
                Clause::Query(body) => {
                    let mut goals = lw.body(body)?;
                    lw.finish_body(&mut goals, None, clause);
                    queries.push(Query {
                        goals,
                        var_names: lw.vars.names.clone(),
                        text: clause.to_string(),
                    });
                }
                // CLM-0027: `:- table name/arity, ….`
                Clause::Directive { name, indicators } => {
                    let bad = |message: String| EvalError::BadDirective { message, span };
                    if name != "table" {
                        return Err(bad(format!("unknown directive `{name}`")));
                    }
                    for ind in indicators {
                        let arity = if ind.arity.is_integer() && !ind.arity.is_negative() {
                            ind.arity.numer().to_string().parse::<usize>().ok()
                        } else {
                            None
                        };
                        let Some(arity) = arity else {
                            return Err(bad(format!(
                                "the arity of `{ind}` must be a non-negative integer"
                            )));
                        };
                        if ind.name == LIST_LABEL
                            || BUILTINS
                                .iter()
                                .any(|(n, a, _)| *n == ind.name && *a == arity)
                        {
                            return Err(bad(format!("`{ind}` is a built-in and cannot be tabled")));
                        }
                        tabled.insert((lw.symbols.intern(&ind.name), arity));
                    }
                }
            }
        }

        for pred in preds.values_mut() {
            pred.finish();
        }
        let mut program = Program {
            symbols: Rc::new(RefCell::new(symbols)),
            preds,
            builtins,
            initial,
            queries,
            tabled,
            world: Store::default(),
            tables: RefCell::new(Tables::default()),
        };
        // The initial store must be satisfiable, or the program has no models.
        let mut store = Store::with_symbols(Rc::clone(&program.symbols));
        if let Some(k) = program.post_initial(&mut store) {
            // CLM-0030: a minimal conflicting set among the facts up to the
            // first the store rejects, cited by position and text. The
            // replay cannot fail where the facts themselves loaded; should
            // it, the error stays the bare one.
            let candidates: Vec<Step> = (0..k).map(Step::Fact).collect();
            let conflict = match explain::minimal_conflict(&program, &candidates, &Step::Fact(k)) {
                Ok(mut set) => {
                    set.push(k);
                    set.iter()
                        .map(|&i| {
                            let f = &program.initial[i];
                            (f.span, f.text.clone())
                        })
                        .collect()
                }
                Err(_) => Vec::new(),
            };
            return Err(EvalError::InitialStoreUnsatisfiable { conflict });
        }
        store.set_baseline();
        program.world = store;
        Ok(program)
    }

    /// The program's `?-` clauses, in source order.
    pub fn queries(&self) -> &[Query] {
        &self.queries
    }

    /// Answer a query: an iterator over its solutions, computed lazily.
    pub fn solve<'p>(&'p self, query: &'p Query) -> Solutions<'p> {
        Solutions::new(self, query)
    }

    /// Clauses defining `key`, in program order (empty if undefined).
    pub(crate) fn clauses(&self, key: &PredKey) -> &[TClause] {
        self.preds
            .get(key)
            .map(|p| p.clauses.as_slice())
            .unwrap_or(&[])
    }

    /// The positions of the clauses of `key` that can match a call whose
    /// first argument has `first` (`None`: unbound), in program order
    /// (CLM-0021).
    pub(crate) fn candidates(&self, key: &PredKey, first: Option<&ArgKey>) -> &[usize] {
        self.preds
            .get(key)
            .map(|p| p.candidates(first))
            .unwrap_or(&[])
    }

    /// Post the constraint facts into a fresh store: the index of the first
    /// fact the store rejects, `None` when all hold.
    pub(crate) fn post_initial(&self, store: &mut Store) -> Option<usize> {
        (0..self.initial.len()).find(|&i| !self.post_fact(store, i))
    }

    /// Post constraint fact `idx` on a fresh frame of its own (CLM-0030: one
    /// replay step). False if the store rejects it.
    pub(crate) fn post_fact(&self, store: &mut Store, idx: usize) -> bool {
        let fact = &self.initial[idx];
        let mut vars = vec![None; fact.nvars as usize];
        for g in &fact.goals {
            let ok = match g {
                TGoal::Eq(a, b) => match (build(store, a, &mut vars), build(store, b, &mut vars)) {
                    (Some(a), Some(b)) => store.post_eq_goal(a, b),
                    _ => false,
                },
                TGoal::Dif(a, b) => {
                    match (build(store, a, &mut vars), build(store, b, &mut vars)) {
                        (Some(a), Some(b)) => store.post_dif_goal(a, b),
                        _ => false,
                    }
                }
                TGoal::Rel(op, a, b) => {
                    match (build(store, a, &mut vars), build(store, b, &mut vars)) {
                        (Some(a), Some(b)) => store.post_rel(*op, a, b),
                        _ => false,
                    }
                }
                TGoal::Call(_)
                | TGoal::Or(..)
                | TGoal::IfThenElse { .. }
                | TGoal::Not { .. }
                | TGoal::Collect { .. } => {
                    unreachable!("constraint facts hold constraints only")
                }
            };
            if !ok {
                return false;
            }
        }
        true
    }
}

fn push_clause(preds: &mut HashMap<PredKey, Pred>, clause: TClause) {
    let key = match &clause.head {
        TTerm::Compound(f, args) => (*f, args.len()),
        TTerm::Const(c) => (*c, 0),
        _ => unreachable!("clause heads are atoms"),
    };
    preds.entry(key).or_default().push(clause);
}

/// Instantiate a template on the heap. `var(store, i)` supplies the heap
/// address of clause variable `i`. Arithmetic nodes over numbers fold to
/// numbers; others become fresh numeric variables with their defining
/// constraint posted immediately; `None` if that makes the store
/// unsatisfiable (or raises a store error). Iterative (CLM-0019): an
/// explicit post-order walk, so constraints are posted in the same order
/// as before — children left to right, then the parent — at any depth.
pub(crate) fn build_with(
    store: &mut Store,
    root: &TTerm,
    var: &mut dyn FnMut(&mut Store, u32) -> Addr,
) -> Option<Addr> {
    let mut work = vec![Walk::Enter(root)];
    let mut out: Vec<Addr> = Vec::new();
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
                    TTerm::Var(i) => var(store, *i),
                    TTerm::Number(n) => store.new_num(n.clone()),
                    TTerm::Const(c) => store.new_const(*c),
                    TTerm::Compound(f, args) => {
                        let addrs = take_last(&mut out, args.len());
                        store.new_struct(*f, addrs)
                    }
                    TTerm::Neg(_) => store.post_neg(out.pop()?)?,
                    TTerm::Len(_) => store.post_len(out.pop()?)?,
                    TTerm::Arith(op, ..) => {
                        let (a, b) = take_two(&mut out)?;
                        store.post_arith(*op, a, b)?
                    }
                    TTerm::Concat(..) => {
                        let (x, y) = take_two(&mut out)?;
                        store.post_concat(x, y)?
                    }
                };
                out.push(built);
            }
        }
    }
    out.pop()
}

/// The heap address of clause variable `i` in a lazily allocated frame: a
/// fresh variable on first use (structure copying).
pub(crate) fn lazy_var(store: &mut Store, vars: &mut [Option<Addr>], i: u32) -> Addr {
    let slot = &mut vars[i as usize];
    match slot {
        Some(a) => *a,
        None => {
            let a = store.new_var();
            *slot = Some(a);
            a
        }
    }
}

/// [`build_with`] over a lazily allocated frame.
pub(crate) fn build(store: &mut Store, t: &TTerm, vars: &mut [Option<Addr>]) -> Option<Addr> {
    build_with(store, t, &mut |store, i| lazy_var(store, vars, i))
}
