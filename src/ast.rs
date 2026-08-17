//! Abstract syntax tree of a Claimr program — the output of parsing.

/// A top-level clause of a program.
#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    Fact(Atom),
    Rule { head: Atom, body: Vec<Goal> },
    ConstraintFact(ConstraintExpr),
    ConstraintRule { head: Atom, body: Vec<Goal> },
    Implication { constraint: ConstraintExpr, head: Atom },
    Query(Vec<Goal>),
}

/// A goal in a rule or query body: an atom to prove, or a constraint block.
#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    Atom(Atom),
    Constraint(ConstraintExpr),
}

/// A compound term: `name(arg, ...)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Atom {
    pub name: String,
    pub args: Vec<Expr>,
}

/// A term expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Atom(Box<Atom>),
    Var(String),
    Number(f64),
    Ident(String),
}

/// A single relational constraint `left op right`.
#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub left: Expr,
    pub op: RelOp,
    pub right: Expr,
}

/// A conjunction of constraints, as written between `{` and `}`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintExpr {
    pub terms: Vec<Constraint>,
}

/// Relational operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOp {
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
}
