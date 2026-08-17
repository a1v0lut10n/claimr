//! Claimr — parser for the Claimr constraint logic language (Prolog III inspired).
//!
//! The grammar is documented in `docs/reference/grammar.md`. This module
//! exposes the AST types plus two entry points: [`all_consuming_parse_clause`]
//! for a single clause and [`parse_program`] for a whole source file.

use nom::{
    IResult,
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace0},
    combinator::{all_consuming, map, map_res, opt, recognize, verify},
    multi::{many0, separated_list0, separated_list1},
    sequence::{delimited, pair, terminated, tuple},
};

#[derive(Debug, Clone, PartialEq)]
pub enum Clause {
    Fact(Atom),
    Rule { head: Atom, body: Vec<Goal> },
    ConstraintFact(ConstraintExpr),
    ConstraintRule { head: Atom, body: Vec<Goal> },
    Implication { constraint: ConstraintExpr, head: Atom },
    Query(Vec<Goal>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Goal {
    Atom(Atom),
    Constraint(ConstraintExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Atom {
    pub name: String,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Atom(Box<Atom>),
    Var(String),
    Number(f64),
    Ident(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub left: Expr,
    pub op: RelOp,
    pub right: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintExpr {
    pub terms: Vec<Constraint>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelOp {
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
}

// Whitespace handling utility function
fn ws<'a, F, O>(f: F) -> impl FnMut(&'a str) -> IResult<&'a str, O>
where
    F: FnMut(&'a str) -> IResult<&'a str, O>,
{
    delimited(multispace0, f, multispace0)
}

// Basic parsers
fn parse_identifier(input: &str) -> IResult<&str, String> {
    map(
        recognize(pair(
            alpha1,
            many0(alt((alphanumeric1, tag("_"))))
        )),
        |s: &str| s.to_string()
    )(input)
}

fn parse_var(input: &str) -> IResult<&str, String> {
    verify(parse_identifier, |s: &String| s.chars().next().is_some_and(|c| c.is_uppercase()))(input)
}

fn parse_ident(input: &str) -> IResult<&str, String> {
    verify(parse_identifier, |s: &String| s.chars().next().is_some_and(|c| c.is_lowercase()))(input)
}

fn parse_number(input: &str) -> IResult<&str, f64> {
    map_res(
        recognize(tuple((digit1, opt(pair(char('.'), digit1))))),
        |s: &str| s.parse::<f64>()
    )(input)
}

// We need to break the circular reference between parse_expr and parse_atom
// First define parse_expr without the atom case
fn parse_expr_noatom(input: &str) -> IResult<&str, Expr> {
    ws(alt((
        map(parse_number, Expr::Number),
        map(parse_var, Expr::Var),
        map(parse_ident, Expr::Ident),
    )))(input)
}

// Then define atom using the simplified expr parser
fn parse_atom(input: &str) -> IResult<&str, Atom> {
    map(
        tuple((
            ws(parse_ident),
            ws(delimited(
                char('('),
                separated_list0(ws(char(',')), parse_expr_noatom),  // Use the simplified expr parser
                char(')')
            )),
        )),
        |(name, args)| Atom {
            name,
            args,
        }
    )(input)
}

// Now define the full expr parser that includes atoms
fn parse_expr(input: &str) -> IResult<&str, Expr> {
    ws(alt((
        map(parse_number, Expr::Number),
        map(parse_var, Expr::Var),
        map(parse_atom, |atom| Expr::Atom(Box::new(atom))), // Box to avoid infinite size issues
        map(parse_ident, Expr::Ident),
    )))(input)
}

// Relational operator parser
fn parse_relop(input: &str) -> IResult<&str, RelOp> {
    ws(alt((
        map(tag("<="), |_| RelOp::Le),
        map(tag(">="), |_| RelOp::Ge),
        map(tag("!="), |_| RelOp::Neq),
        map(tag("="), |_| RelOp::Eq),
        map(tag("<"), |_| RelOp::Lt),
        map(tag(">"), |_| RelOp::Gt),
    )))(input)
}

// Constraint parser (left op right)
fn parse_constraint(input: &str) -> IResult<&str, Constraint> {
    map(
        tuple((
            ws(parse_expr),
            ws(parse_relop),
            ws(parse_expr)
        )),
        |(left, op, right)| Constraint { left, op, right }
    )(input)
}

// Multiple constraints separated by commas
fn parse_constraint_expr(input: &str) -> IResult<&str, ConstraintExpr> {
    map(
        separated_list1(ws(char(',')), parse_constraint),
        |terms| ConstraintExpr { terms }
    )(input)
}

// Goals can be atoms or constraints in braces
fn parse_goal(input: &str) -> IResult<&str, Goal> {
    ws(alt((
        map(parse_atom, Goal::Atom),
        map(
            delimited(char('{'), ws(parse_constraint_expr), char('}')),
            Goal::Constraint
        ),
    )))(input)
}

// Body is a list of goals separated by commas
fn parse_body(input: &str) -> IResult<&str, Vec<Goal>> {
    separated_list0(ws(char(',')), parse_goal)(input)
}

// Fact: atom followed by period
fn parse_fact(input: &str) -> IResult<&str, Clause> {
    map(
        terminated(ws(parse_atom), char('.')),
        Clause::Fact
    )(input)
}

// Rule: head :- body.
fn parse_rule(input: &str) -> IResult<&str, Clause> {
    map(
        tuple((
            ws(parse_atom),
            ws(tag(":-")),
            ws(parse_body),
            char('.')
        )),
        |(head, _, body, _)| Clause::Rule { head, body }
    )(input)
}

// Constraint fact: {constraint}.
fn parse_constraint_fact(input: &str) -> IResult<&str, Clause> {
    map(
        terminated(
            delimited(char('{'), ws(parse_constraint_expr), char('}')),
            char('.')
        ),
        Clause::ConstraintFact
    )(input)
}

// Constraint rule: head :- body with constraints.
fn parse_constraint_rule(input: &str) -> IResult<&str, Clause> {
    map(
        tuple((
            ws(parse_atom),
            ws(tag(":-")),
            ws(parse_body),
            char('.')
        )),
        |(head, _, body, _)| {
            if body.iter().any(|goal| matches!(goal, Goal::Constraint(_))) {
                Clause::ConstraintRule { head, body }
            } else {
                Clause::Rule { head, body }
            }
        }
    )(input)
}

// Implication: {constraint} => head.
fn parse_implication(input: &str) -> IResult<&str, Clause> {
    map(
        tuple((
            delimited(char('{'), ws(parse_constraint_expr), char('}')),
            ws(tag("=>")),
            ws(parse_atom),
            char('.')
        )),
        |(constraint, _, head, _)| Clause::Implication { constraint, head }
    )(input)
}

// Query: ?- body.
fn parse_query(input: &str) -> IResult<&str, Clause> {
    map(
        tuple((
            ws(tag("?-")),
            ws(alt((
                map(delimited(char('{'), ws(parse_constraint_expr), char('}')), |constraint| {
                    vec![Goal::Constraint(constraint)]
                }),
                parse_body
            ))),
            char('.')
        )),
        |(_, body, _)| Clause::Query(body)
    )(input)
}

// A clause is any of the top-level elements
fn parse_clause(input: &str) -> IResult<&str, Clause> {
    ws(alt((
        parse_constraint_fact,
        parse_implication,
        parse_query,
        parse_constraint_rule,
        parse_rule,
        parse_fact
    )))(input)
}

// Entry point for parsing that ensures the entire input is consumed
pub fn all_consuming_parse_clause(input: &str) -> IResult<&str, Clause> {
    all_consuming(parse_clause)(input)
}

/// Parse a whole program (a sequence of clauses) and require that all input
/// is consumed. Whitespace between and around clauses is ignored.
pub fn parse_program(input: &str) -> IResult<&str, Vec<Clause>> {
    all_consuming(terminated(many0(parse_clause), multispace0))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fact() {
        let input = "human(socrates).";
        let result = all_consuming_parse_clause(input);
        println!("{:?}", result);
        assert!(matches!(result, Ok((_, Clause::Fact(_)))));
    }

    #[test]
    fn test_parse_rule() {
        let input = "mortal(X) :- human(X).";
        let result = all_consuming_parse_clause(input);
        println!("{:?}", result);
        assert!(matches!(result, Ok((_, Clause::Rule { .. }))));
    }

    #[test]
    fn test_parse_constraint_fact() {
        let input = "{ age(socrates) > 70 }.";
        let result = all_consuming_parse_clause(input);
        println!("{:?}", result);
        assert!(matches!(result, Ok((_, Clause::ConstraintFact(_)))));
    }

    #[test]
    fn test_parse_constraint_rule() {
        let input = "eligible(X) :- { age(X) >= 18 }.";
        let result = all_consuming_parse_clause(input);
        println!("{:?}", result);
        assert!(matches!(result, Ok((_, Clause::ConstraintRule { .. }))));
    }

    #[test]
    fn test_parse_implication() {
        let input = "{ age(X) >= 18 } => eligible(X).";
        let result = all_consuming_parse_clause(input);
        println!("{:?}", result);
        assert!(matches!(result, Ok((_, Clause::Implication { .. }))));
    }

    #[test]
    fn test_parse_program() {
        let input = "human(socrates).\nmortal(X) :- human(X).\n\n?- mortal(socrates).\n";
        let (rest, clauses) = parse_program(input).expect("program parses");
        assert!(rest.is_empty());
        assert_eq!(clauses.len(), 3);
    }

    #[test]
    fn test_parse_query() {
        let input = "?- eligible(alice), { age(alice) >= 18 }.";
        let result = all_consuming_parse_clause(input);
        println!("{:?}", result);
        assert!(matches!(result, Ok((_, Clause::Query(_)))));
    }
}
