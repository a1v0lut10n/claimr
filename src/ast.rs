// SPDX-License-Identifier: Apache-2.0

//! Abstract syntax tree of a Claimr program — the output of parsing.

use std::fmt;

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

// ---------------------------------------------------------------------------
// Display: renders a clause back to Claimr surface syntax (canonical spacing,
// minimal parentheses). Used to echo queries and to name clauses in errors.
// ---------------------------------------------------------------------------

impl fmt::Display for RelOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            RelOp::Eq => "=",
            RelOp::Neq => "!=",
            RelOp::Lt => "<",
            RelOp::Gt => ">",
            RelOp::Le => "<=",
            RelOp::Ge => ">=",
        })
    }
}

impl fmt::Display for ArithOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ArithOp::Add => "+",
            ArithOp::Sub => "-",
            ArithOp::Mul => "*",
            ArithOp::Div => "/",
        })
    }
}

impl ArithOp {
    /// Binding strength: `* /` above `+ -`; unary minus is 3.
    fn precedence(self) -> u8 {
        match self {
            ArithOp::Add | ArithOp::Sub => 1,
            ArithOp::Mul | ArithOp::Div => 2,
        }
    }
}

impl Expr {
    fn precedence(&self) -> u8 {
        match self {
            Expr::Binary { op, .. } => op.precedence(),
            Expr::Neg(_) => 3,
            _ => 4,
        }
    }

    fn fmt_prec(&self, f: &mut fmt::Formatter<'_>, min: u8) -> fmt::Result {
        let prec = self.precedence();
        let parens = prec < min;
        if parens {
            f.write_str("(")?;
        }
        match self {
            Expr::Atom(a) => write!(f, "{a}")?,
            Expr::Var(v) => f.write_str(v)?,
            Expr::Number(n) => write!(f, "{n}")?,
            Expr::Ident(i) => f.write_str(i)?,
            Expr::Neg(e) => {
                f.write_str("-")?;
                e.fmt_prec(f, 3)?;
            }
            Expr::Binary { op, left, right } => {
                // Left-associative: the right operand needs parentheses at equal precedence.
                left.fmt_prec(f, prec)?;
                write!(f, " {op} ")?;
                right.fmt_prec(f, prec + 1)?;
            }
        }
        if parens {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_prec(f, 0)
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.name)?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{arg}")?;
        }
        f.write_str(")")
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.left, self.op, self.right)
    }
}

impl fmt::Display for ConstraintExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("{ ")?;
        for (i, c) in self.terms.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{c}")?;
        }
        f.write_str(" }")
    }
}

impl fmt::Display for Goal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Goal::Atom(a) => write!(f, "{a}"),
            Goal::Constraint(c) => write!(f, "{c}"),
        }
    }
}

fn fmt_body(f: &mut fmt::Formatter<'_>, body: &[Goal]) -> fmt::Result {
    for (i, g) in body.iter().enumerate() {
        if i > 0 {
            f.write_str(", ")?;
        }
        write!(f, "{g}")?;
    }
    Ok(())
}

impl fmt::Display for Clause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Clause::Fact(a) => write!(f, "{a}."),
            Clause::Rule { head, body } | Clause::ConstraintRule { head, body } => {
                write!(f, "{head} :- ")?;
                fmt_body(f, body)?;
                f.write_str(".")
            }
            Clause::ConstraintFact(c) => write!(f, "{c}."),
            Clause::Implication { constraint, head } => write!(f, "{constraint} => {head}."),
            Clause::Query(body) => {
                f.write_str("?- ")?;
                fmt_body(f, body)?;
                f.write_str(".")
            }
        }
    }
}
