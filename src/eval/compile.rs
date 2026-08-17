//! Lowering `ast::Clause`s to the runtime program: clause templates with
//! pre-numbered variables (structure copying at instantiation), predicate
//! index, the initial store's constraint facts, and the queries.
//!
//! Arithmetic terms stay in the templates and are lowered at instantiation:
//! each becomes a fresh numeric variable plus a linear constraint posted to
//! the store (design record `2026-08-17-arithmetic-in-terms.md`).

use std::collections::HashMap;

use crate::ast::{self, ArithOp, Clause, Expr, Goal, RelOp};
use crate::number::Number;
use crate::Span;

use super::error::EvalError;
use super::machine::Solutions;
use super::store::{Addr, Store};
use super::symbol::{Symbol, Symbols};

/// A term template: variables are clause-local indices.
#[derive(Debug, Clone)]
pub(crate) enum TTerm {
    Var(u32),
    Number(Number),
    Const(Symbol),
    Compound(Symbol, Vec<TTerm>),
    /// Unary minus: lowered to a fresh numeric variable at instantiation.
    Neg(Box<TTerm>),
    /// Binary arithmetic: lowered to a fresh numeric variable at instantiation.
    Arith(ArithOp, Box<TTerm>, Box<TTerm>),
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
}

/// A compiled clause: head, body, and the number of distinct variables.
#[derive(Debug, Clone)]
pub(crate) struct TClause {
    pub head: TTerm,
    pub body: Vec<TGoal>,
    pub nvars: u32,
}

/// Predicate key: functor and arity.
pub(crate) type PredKey = (Symbol, usize);

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
    pub(crate) symbols: Symbols,
    preds: HashMap<PredKey, Vec<TClause>>,
    /// Constraint facts, each with its variable count: the initial store.
    pub(crate) initial: Vec<(Vec<TGoal>, u32)>,
    queries: Vec<Query>,
}

/// Per-clause variable numbering.
#[derive(Default)]
struct VarMap {
    names: Vec<String>,
    index: HashMap<String, u32>,
}

impl VarMap {
    fn get(&mut self, name: &str) -> u32 {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.names.len() as u32;
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), i);
        i
    }
}

struct Lowerer<'a> {
    symbols: &'a mut Symbols,
    vars: VarMap,
}

impl Lowerer<'_> {
    fn term(&mut self, e: &Expr) -> Result<TTerm, EvalError> {
        Ok(match e {
            Expr::Var(v) => TTerm::Var(self.vars.get(v)),
            Expr::Number(n) => TTerm::Number(n.clone()),
            Expr::Ident(i) => TTerm::Const(self.symbols.intern(i)),
            Expr::Atom(a) => self.atom(a)?,
            Expr::Neg(e) => TTerm::Neg(Box::new(self.term(e)?)),
            Expr::Binary { op, left, right } => {
                TTerm::Arith(*op, Box::new(self.term(left)?), Box::new(self.term(right)?))
            }
        })
    }

    fn atom(&mut self, a: &ast::Atom) -> Result<TTerm, EvalError> {
        let f = self.symbols.intern(&a.name);
        let args = a.args.iter().map(|e| self.term(e)).collect::<Result<Vec<_>, _>>()?;
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

    fn body(&mut self, goals: &[Goal]) -> Result<Vec<TGoal>, EvalError> {
        let mut out = Vec::new();
        for g in goals {
            match g {
                Goal::Atom(a) => out.push(TGoal::Call(self.atom(a)?)),
                Goal::Constraint(ce) => {
                    for c in &ce.terms {
                        out.push(self.constraint(c)?);
                    }
                }
            }
        }
        Ok(out)
    }
}

impl Program {
    /// Compile clauses into a program.
    pub fn compile(clauses: &[Clause]) -> Result<Program, EvalError> {
        Self::compile_iter(clauses.iter().map(|c| (c, None)))
    }

    /// Compile clauses parsed with [`crate::parse_program_spanned`]. Spans
    /// are reserved for load-time diagnostics that point into the source;
    /// currently every construct compiles, so this behaves as
    /// [`Program::compile`].
    pub fn compile_spanned(clauses: &[(Clause, Span)]) -> Result<Program, EvalError> {
        Self::compile_iter(clauses.iter().map(|(c, s)| (c, Some(*s))))
    }

    fn compile_iter<'a>(
        clauses: impl Iterator<Item = (&'a Clause, Option<Span>)>,
    ) -> Result<Program, EvalError> {
        let mut symbols = Symbols::default();
        let mut preds: HashMap<PredKey, Vec<TClause>> = HashMap::new();
        let mut initial = Vec::new();
        let mut queries = Vec::new();

        for (clause, _span) in clauses {
            let mut lw = Lowerer { symbols: &mut symbols, vars: VarMap::default() };
            match clause {
                Clause::Fact(atom) => {
                    let head = lw.atom(atom)?;
                    let nvars = lw.vars.names.len() as u32;
                    push_clause(&mut preds, TClause { head, body: vec![], nvars });
                }
                Clause::Rule { head, body } | Clause::ConstraintRule { head, body } => {
                    let head = lw.atom(head)?;
                    let body = lw.body(body)?;
                    let nvars = lw.vars.names.len() as u32;
                    push_clause(&mut preds, TClause { head, body, nvars });
                }
                // `{c} => head.` is sugar for `head :- {c}.` (design D3).
                Clause::Implication { constraint, head } => {
                    let head = lw.atom(head)?;
                    let body = constraint
                        .terms
                        .iter()
                        .map(|c| lw.constraint(c))
                        .collect::<Result<Vec<_>, _>>()?;
                    let nvars = lw.vars.names.len() as u32;
                    push_clause(&mut preds, TClause { head, body, nvars });
                }
                // Global constraints: the initial store (design D3).
                Clause::ConstraintFact(ce) => {
                    let goals = ce
                        .terms
                        .iter()
                        .map(|c| lw.constraint(c))
                        .collect::<Result<Vec<_>, _>>()?;
                    initial.push((goals, lw.vars.names.len() as u32));
                }
                Clause::Query(body) => {
                    let goals = lw.body(body)?;
                    queries.push(Query {
                        goals,
                        var_names: lw.vars.names.clone(),
                        text: clause.to_string(),
                    });
                }
            }
        }

        let program = Program { symbols, preds, initial, queries };
        // The initial store must be satisfiable, or the program has no models.
        let mut store = Store::new();
        if !program.post_initial(&mut store) {
            return Err(EvalError::InitialStoreUnsatisfiable);
        }
        Ok(program)
    }

    /// The program's `?-` clauses, in source order.
    pub fn queries(&self) -> &[Query] {
        &self.queries
    }

    /// Answer a query: an iterator over its solutions, computed lazily.
    pub fn solve<'p>(&'p self, query: &Query) -> Solutions<'p> {
        Solutions::new(self, query)
    }

    /// Clauses defining `key`, in program order (empty if undefined).
    pub(crate) fn clauses(&self, key: &PredKey) -> &[TClause] {
        self.preds.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Post the constraint facts into a fresh store. False if unsatisfiable.
    pub(crate) fn post_initial(&self, store: &mut Store) -> bool {
        for (goals, nvars) in &self.initial {
            let mut vars = vec![None; *nvars as usize];
            for g in goals {
                let ok = match g {
                    TGoal::Eq(a, b) => match (build(store, a, &mut vars), build(store, b, &mut vars)) {
                        (Some(a), Some(b)) => store.post_eq_goal(a, b),
                        _ => false,
                    },
                    TGoal::Dif(a, b) => match (build(store, a, &mut vars), build(store, b, &mut vars)) {
                        (Some(a), Some(b)) => store.post_dif_goal(a, b),
                        _ => false,
                    },
                    TGoal::Rel(op, a, b) => match (build(store, a, &mut vars), build(store, b, &mut vars)) {
                        (Some(a), Some(b)) => store.post_rel(*op, a, b),
                        _ => false,
                    },
                    TGoal::Call(_) => unreachable!("constraint facts have no calls"),
                };
                if !ok {
                    return false;
                }
            }
        }
        true
    }
}

fn push_clause(preds: &mut HashMap<PredKey, Vec<TClause>>, clause: TClause) {
    let key = match &clause.head {
        TTerm::Compound(f, args) => (*f, args.len()),
        TTerm::Const(c) => (*c, 0),
        _ => unreachable!("clause heads are atoms"),
    };
    preds.entry(key).or_default().push(clause);
}

/// Instantiate a template on the heap, allocating fresh variables on first
/// use (structure copying). Arithmetic nodes become fresh numeric variables
/// with their defining constraint posted immediately; `None` if that makes
/// the store unsatisfiable (or raises a store error).
pub(crate) fn build(store: &mut Store, t: &TTerm, vars: &mut [Option<Addr>]) -> Option<Addr> {
    Some(match t {
        TTerm::Var(i) => {
            let slot = &mut vars[*i as usize];
            match slot {
                Some(a) => *a,
                None => {
                    let a = store.new_var();
                    *slot = Some(a);
                    a
                }
            }
        }
        TTerm::Number(n) => store.new_num(n.clone()),
        TTerm::Const(c) => store.new_const(*c),
        TTerm::Compound(f, args) => {
            let mut addrs = Vec::with_capacity(args.len());
            for a in args {
                addrs.push(build(store, a, vars)?);
            }
            store.new_struct(*f, addrs)
        }
        TTerm::Neg(e) => {
            let a = build(store, e, vars)?;
            store.post_neg(a)?
        }
        TTerm::Arith(op, l, r) => {
            let a = build(store, l, vars)?;
            let b = build(store, r, vars)?;
            store.post_arith(*op, a, b)?
        }
    })
}
