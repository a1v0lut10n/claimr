// SPDX-License-Identifier: Apache-2.0

//! Abstract syntax tree of a Claimr program — the output of parsing.

use std::fmt;

use crate::number::Number;

/// A top-level clause of a program.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Clause {
    Fact(Atom),
    Rule {
        head: Atom,
        body: Vec<Goal>,
    },
    ConstraintFact(ConstraintExpr),
    ConstraintRule {
        head: Atom,
        body: Vec<Goal>,
    },
    Implication {
        constraint: ConstraintExpr,
        head: Atom,
    },
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
/// Lists are terms too, spelled `<a, b> . T`; see [`Expr::List`].
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
    Binary {
        op: ArithOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// A list (CLM-0014): `<>`, `<a, b, c>`, or `<a, b> . T` when `tail` is
    /// present. It denotes the chain of `'<>'(H, T)` cells ending in `'<>'`
    /// or in the tail (docs/design/2026-11-09-lists-as-a-constraint-domain.md).
    List {
        items: Vec<Expr>,
        tail: Option<Box<Expr>>,
    },
    /// The length of a list (CLM-0015): `|L|`, a numeric term whose value
    /// lives in the constraint store.
    Len(Box<Expr>),
    /// The concatenation of two lists (CLM-0016): `X . Y`, a list whose
    /// length is `|X| + |Y|` and which is completed once `|X|` is known. A
    /// literal tuple on the left is [`Expr::List`] instead.
    Concat(Box<Expr>, Box<Expr>),
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
    /// Binding strength: `.` (a tailed list, a concatenation) is 0, below
    /// arithmetic.
    fn precedence(&self) -> u8 {
        match self {
            Expr::List { tail: Some(_), .. } | Expr::Concat(..) => 0,
            Expr::Binary { op, .. } => op.precedence(),
            Expr::Neg(_) => 3,
            _ => 4,
        }
    }

    /// Move the children out, leaving leaves behind (CLM-0019).
    fn take_children(&mut self, out: &mut Vec<Expr>) {
        match self {
            Expr::Var(_) | Expr::Number(_) | Expr::Ident(_) => {}
            Expr::Atom(a) => out.append(&mut a.args),
            Expr::Neg(x) | Expr::Len(x) => {
                out.push(std::mem::replace(x, Expr::Ident(String::new())))
            }
            Expr::Binary { left, right, .. } | Expr::Concat(left, right) => {
                out.push(std::mem::replace(left, Expr::Ident(String::new())));
                out.push(std::mem::replace(right, Expr::Ident(String::new())));
            }
            Expr::List { items, tail } => {
                out.append(items);
                if let Some(t) = tail.take() {
                    out.push(*t);
                }
            }
        }
    }
}

/// A term of any depth drops in constant stack (CLM-0019): the children are
/// moved into a worklist and dropped in a loop instead of by the recursive
/// drop glue — a long list literal is a chain as deep as it is long.
impl Drop for Expr {
    fn drop(&mut self) {
        let mut work: Vec<Expr> = Vec::new();
        self.take_children(&mut work);
        while let Some(mut e) = work.pop() {
            e.take_children(&mut work);
        }
    }
}

/// A unit of the iterative renderer's work (CLM-0019): an expression to
/// print at a minimum precedence, an atom, or text.
enum Piece<'a> {
    Expr(&'a Expr, u8),
    Atom(&'a Atom),
    Text(&'static str),
}

/// Render terms with an explicit stack, so the depth of a literal costs no
/// Rust stack. Items are pushed in reverse of their output order.
fn render(f: &mut fmt::Formatter<'_>, root: Piece<'_>) -> fmt::Result {
    let mut work = vec![root];
    while let Some(piece) = work.pop() {
        match piece {
            Piece::Text(t) => f.write_str(t)?,
            Piece::Atom(a) => {
                write!(f, "{}(", display_name(&a.name))?;
                work.push(Piece::Text(")"));
                for (i, arg) in a.args.iter().enumerate().rev() {
                    work.push(Piece::Expr(arg, 0));
                    if i > 0 {
                        work.push(Piece::Text(", "));
                    }
                }
            }
            Piece::Expr(e, min) => {
                let prec = e.precedence();
                if prec < min {
                    f.write_str("(")?;
                    work.push(Piece::Text(")"));
                }
                match e {
                    Expr::Atom(a) => work.push(Piece::Atom(a)),
                    Expr::Var(v) => f.write_str(v)?,
                    Expr::Number(n) => write!(f, "{n}")?,
                    Expr::Ident(i) => f.write_str(&display_name(i))?,
                    Expr::Neg(x) => {
                        f.write_str("-")?;
                        work.push(Piece::Expr(x, 3));
                    }
                    Expr::Binary { op, left, right } => {
                        // Left-associative: the right operand needs parentheses
                        // at equal precedence.
                        work.push(Piece::Expr(right, prec + 1));
                        work.push(Piece::Text(match op {
                            ArithOp::Add => " + ",
                            ArithOp::Sub => " - ",
                            ArithOp::Mul => " * ",
                            ArithOp::Div => " / ",
                        }));
                        work.push(Piece::Expr(left, prec));
                    }
                    Expr::Len(x) => {
                        f.write_str("|")?;
                        work.push(Piece::Text("|"));
                        work.push(Piece::Expr(x, 0));
                    }
                    Expr::Concat(l, r) => {
                        // Right-associative: a `.` on the left needs parentheses.
                        work.push(Piece::Expr(r, 0));
                        work.push(Piece::Text(" . "));
                        work.push(Piece::Expr(l, 1));
                    }
                    Expr::List { items, tail } => {
                        f.write_str("<")?;
                        if let Some(t) = tail {
                            // Right-associative and loosest: the tail never
                            // needs parentheses.
                            work.push(Piece::Expr(t, 0));
                            work.push(Piece::Text(" . "));
                        }
                        work.push(Piece::Text(">"));
                        for (i, item) in items.iter().enumerate().rev() {
                            work.push(Piece::Expr(item, 0));
                            if i > 0 {
                                work.push(Piece::Text(", "));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render(f, Piece::Expr(self, 0))
    }
}

/// CLM-0011: is this spelling a plain identifier (no quoting needed)?
pub fn name_is_plain(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// CLM-0011: an atom name as source — bare when the spelling is a
/// plain identifier, quoted with `'` and `\` escaped otherwise, so
/// printed answers round-trip as valid programs
/// (docs/design/2026-09-10-quoted-atoms.md).
pub fn display_name(name: &str) -> std::borrow::Cow<'_, str> {
    if name_is_plain(name) {
        return std::borrow::Cow::Borrowed(name);
    }
    let mut out = String::with_capacity(name.len() + 2);
    out.push('\'');
    for c in name.chars() {
        if c == '\'' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('\'');
    std::borrow::Cow::Owned(out)
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        render(f, Piece::Atom(self))
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
