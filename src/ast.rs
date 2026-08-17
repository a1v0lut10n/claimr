//! Abstract syntax tree of a Claimr program — the output of parsing.

use crate::number::Number;

/// A top-level clause of a program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Clause {
    Fact(Atom),
    Rule { head: Atom, body: Vec<Goal> },
    ConstraintFact(ConstraintExpr),
    ConstraintRule { head: Atom, body: Vec<Goal> },
    Implication { constraint: ConstraintExpr, head: Atom },
    Query(Vec<Goal>),
}

/// A goal in a rule or query body: an atom to prove, or a constraint block.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Goal {
    Atom(Atom),
    Constraint(ConstraintExpr),
}

/// A compound term: `name(arg, ...)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Atom {
    pub name: String,
    pub args: Vec<Expr>,
}

/// A term expression.
///
/// Arithmetic operators are term constructors admissible anywhere a term
/// goes (Prolog III); an arithmetic term denotes a number at evaluation
/// time. The parser does no constant folding: `1/3` is `Binary { Div, 1, 3 }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    Atom(Box<Atom>),
    Var(String),
    /// An exact rational literal (see [`Number`]).
    Number(Number),
    Ident(String),
    /// Unary minus.
    Neg(Box<Expr>),
    /// A binary arithmetic operation.
    Binary { op: ArithOp, left: Box<Expr>, right: Box<Expr> },
}

/// Binary arithmetic operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
}

/// A single relational constraint `left op right`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Constraint {
    pub left: Expr,
    pub op: RelOp,
    pub right: Expr,
}

/// A conjunction of constraints, as written between `{` and `}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstraintExpr {
    pub terms: Vec<Constraint>,
}

/// Relational operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelOp {
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
}
