//! Lowering `ast::Clause`s to the runtime program: clause templates with
//! pre-numbered variables (structure copying at instantiation), predicate
//! index, the initial store's constraint facts, and the queries.
//!
//! Also the stage-2 boundary: numeric relations, arithmetic terms and
//! attribute terms are rejected here with [`EvalError::Unsupported`].

use std::collections::HashMap;

use crate::ast::{self, Clause, Expr, Goal, RelOp};
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
}

/// A body goal template.
#[derive(Debug, Clone)]
pub(crate) enum TGoal {
    Call(TTerm),
    Eq(TTerm, TTerm),
    Dif(TTerm, TTerm),
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
    clause_no: usize,
    clause: &'a Clause,
    span: Option<Span>,
}

impl Lowerer<'_> {
    fn unsupported(&self, what: &str) -> EvalError {
        EvalError::Unsupported {
            clause: self.clause_no,
            span: self.span,
            text: self.clause.to_string(),
            what: what.to_string(),
        }
    }

    fn term(&mut self, e: &Expr) -> Result<TTerm, EvalError> {
        Ok(match e {
            Expr::Var(v) => TTerm::Var(self.vars.get(v)),
            Expr::Number(n) => TTerm::Number(n.clone()),
            Expr::Ident(i) => TTerm::Const(self.symbols.intern(i)),
            Expr::Atom(a) => self.atom(a)?,
            Expr::Neg(_) | Expr::Binary { .. } => {
                return Err(self.unsupported(
                    "arithmetic terms are not supported yet (evaluator stage 3)",
                ));
            }
        })
    }

    fn atom(&mut self, a: &ast::Atom) -> Result<TTerm, EvalError> {
        let f = self.symbols.intern(&a.name);
        let args = a.args.iter().map(|e| self.term(e)).collect::<Result<Vec<_>, _>>()?;
        Ok(TTerm::Compound(f, args))
    }

    fn constraint(&mut self, c: &ast::Constraint) -> Result<TGoal, EvalError> {
        // Stage-2 boundary: only `=` and `!=` on trees. Numeric literals are
        // trees too, so `X = 3` is fine; `X < 3` and `X + 1 = 3` are not.
        match c.op {
            RelOp::Eq | RelOp::Neq => {}
            _ => {
                return Err(self.unsupported(&format!(
                    "numeric relation `{}` is not supported yet (evaluator stage 3)",
                    c.op
                )));
            }
        }
        let l = self.term(&c.left)?;
        let r = self.term(&c.right)?;
        Ok(match c.op {
            RelOp::Eq => TGoal::Eq(l, r),
            _ => TGoal::Dif(l, r),
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
    /// Compile clauses into a program. Positions in errors will be unknown;
    /// prefer [`Program::compile_spanned`] when spans are available.
    pub fn compile(clauses: &[Clause]) -> Result<Program, EvalError> {
        Self::compile_iter(clauses.iter().map(|c| (c, None)))
    }

    /// Compile clauses parsed with [`crate::parse_program_spanned`], so that
    /// errors carry the offending clause's line and column.
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

        for (i, (clause, span)) in clauses.enumerate() {
            let mut lw = Lowerer {
                symbols: &mut symbols,
                vars: VarMap::default(),
                clause_no: i + 1,
                clause,
                span,
            };
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
                    TGoal::Eq(a, b) => {
                        let (a, b) = (build(store, a, &mut vars), build(store, b, &mut vars));
                        store.post_eq(a, b)
                    }
                    TGoal::Dif(a, b) => {
                        let (a, b) = (build(store, a, &mut vars), build(store, b, &mut vars));
                        store.post_dif(a, b)
                    }
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
/// use (structure copying).
pub(crate) fn build(store: &mut Store, t: &TTerm, vars: &mut [Option<Addr>]) -> Addr {
    match t {
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
            let addrs: Vec<Addr> = args.iter().map(|a| build(store, a, vars)).collect();
            store.new_struct(*f, addrs)
        }
    }
}
