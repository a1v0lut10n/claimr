/// This file is maintained by rustemo but can be modified manually.
/// All manual changes will be preserved except non-doc comments.
///
/// Semantic actions for `claimr.rustemo`. Rather than the default generated
/// tree types, every production builds the AST in [`crate::ast`] directly, so
/// the parser's public output is the `ast` module. Type aliases keep the names
/// rustemo's generated parser expects (one per grammar rule).
use rustemo::Token as RustemoToken;
use super::claimr::{TokenKind, Context};
use crate::ast;
pub type Input = str;
pub type Ctx<'i> = Context<'i, Input>;
#[allow(dead_code)]
pub type Token<'i> = RustemoToken<'i, Input, TokenKind>;
pub type Ident = String;
pub fn ident(_ctx: &Ctx, token: Token) -> Ident {
    token.value.into()
}
pub type Var = String;
pub fn var(_ctx: &Ctx, token: Token) -> Var {
    token.value.into()
}
pub type Number = crate::number::Number;
/// Decimal literals denote exact rationals: `18.5` is 37/2.
pub fn number(_ctx: &Ctx, token: Token) -> Number {
    Number::from_literal(token.value)
        .expect(
            "Number token text is a valid literal by construction of the terminal regex",
        )
}
pub type Program = Vec<ast::Clause>;
pub fn program_c1(_ctx: &Ctx, clauses: Clause0) -> Program {
    clauses.unwrap_or_default()
}
pub type Clause1 = Vec<Clause>;
pub fn clause1_c1(_ctx: &Ctx, mut clause1: Clause1, clause: Clause) -> Clause1 {
    clause1.push(clause);
    clause1
}
pub fn clause1_clause(_ctx: &Ctx, clause: Clause) -> Clause1 {
    vec![clause]
}
pub type Clause0 = Option<Clause1>;
pub fn clause0_clause1(_ctx: &Ctx, clause1: Clause1) -> Clause0 {
    Some(clause1)
}
pub fn clause0_empty(_ctx: &Ctx) -> Clause0 {
    None
}
pub type Clause = ast::Clause;
pub fn clause_fact(_ctx: &Ctx, fact: Fact) -> Clause {
    fact
}
pub fn clause_rule(_ctx: &Ctx, rule: Rule) -> Clause {
    rule
}
pub fn clause_constraint_fact(_ctx: &Ctx, constraint_fact: ConstraintFact) -> Clause {
    constraint_fact
}
pub fn clause_implication(_ctx: &Ctx, implication: Implication) -> Clause {
    implication
}
pub fn clause_query(_ctx: &Ctx, query: Query) -> Clause {
    query
}
pub type Fact = ast::Clause;
pub fn fact_c1(_ctx: &Ctx, head: Atom) -> Fact {
    ast::Clause::Fact(head)
}
pub type Rule = ast::Clause;
/// A rule whose body contains at least one constraint goal is a
/// `ConstraintRule` (the EBNF's `constraint_rule`); otherwise a plain `Rule`.
pub fn rule_c1(_ctx: &Ctx, head: Atom, body: Body) -> Rule {
    if body.iter().any(|goal| matches!(goal, ast::Goal::Constraint(_))) {
        ast::Clause::ConstraintRule {
            head,
            body,
        }
    } else {
        ast::Clause::Rule { head, body }
    }
}
pub type ConstraintFact = ast::Clause;
pub fn constraint_fact_c1(_ctx: &Ctx, constraints: ConstraintExpr) -> ConstraintFact {
    ast::Clause::ConstraintFact(constraints)
}
pub type Implication = ast::Clause;
pub fn implication_c1(
    _ctx: &Ctx,
    constraints: ConstraintExpr,
    head: Atom,
) -> Implication {
    ast::Clause::Implication {
        constraint: constraints,
        head,
    }
}
pub type Query = ast::Clause;
pub fn query_c1(_ctx: &Ctx, body: Body) -> Query {
    ast::Clause::Query(body)
}
pub type Body = Vec<ast::Goal>;
pub fn body_c1(_ctx: &Ctx, goals: Goal1) -> Body {
    goals
}
pub type Goal1 = Vec<Goal>;
pub fn goal1_c1(_ctx: &Ctx, mut goal1: Goal1, goal: Goal) -> Goal1 {
    goal1.push(goal);
    goal1
}
pub fn goal1_goal(_ctx: &Ctx, goal: Goal) -> Goal1 {
    vec![goal]
}
pub type Goal = ast::Goal;
pub fn goal_atom_goal(_ctx: &Ctx, atom: Atom) -> Goal {
    ast::Goal::Atom(atom)
}
pub fn goal_constraint_goal(_ctx: &Ctx, constraints: ConstraintExpr) -> Goal {
    ast::Goal::Constraint(constraints)
}
pub type ConstraintExpr = ast::ConstraintExpr;
pub fn constraint_expr_c1(_ctx: &Ctx, terms: Constraint1) -> ConstraintExpr {
    ast::ConstraintExpr { terms }
}
pub type Constraint1 = Vec<Constraint>;
pub fn constraint1_c1(
    _ctx: &Ctx,
    mut constraint1: Constraint1,
    constraint: Constraint,
) -> Constraint1 {
    constraint1.push(constraint);
    constraint1
}
pub fn constraint1_constraint(_ctx: &Ctx, constraint: Constraint) -> Constraint1 {
    vec![constraint]
}
pub type Constraint = ast::Constraint;
pub fn constraint_c1(_ctx: &Ctx, left: Expr, op: RelOp, right: Expr) -> Constraint {
    ast::Constraint { left, op, right }
}
pub type RelOp = ast::RelOp;
pub fn rel_op_eq(_ctx: &Ctx) -> RelOp {
    ast::RelOp::Eq
}
pub fn rel_op_neq(_ctx: &Ctx) -> RelOp {
    ast::RelOp::Neq
}
pub fn rel_op_lt(_ctx: &Ctx) -> RelOp {
    ast::RelOp::Lt
}
pub fn rel_op_gt(_ctx: &Ctx) -> RelOp {
    ast::RelOp::Gt
}
pub fn rel_op_le(_ctx: &Ctx) -> RelOp {
    ast::RelOp::Le
}
pub fn rel_op_ge(_ctx: &Ctx) -> RelOp {
    ast::RelOp::Ge
}
pub type Atom = ast::Atom;
pub fn atom_c1(_ctx: &Ctx, name: Ident, args: ArgsOpt) -> Atom {
    ast::Atom {
        name,
        args: args.unwrap_or_default(),
    }
}
pub type ArgsOpt = Option<Args>;
pub fn args_opt_args(_ctx: &Ctx, args: Args) -> ArgsOpt {
    Some(args)
}
pub fn args_opt_empty(_ctx: &Ctx) -> ArgsOpt {
    None
}
pub type Args = Vec<ast::Expr>;
pub fn args_c1(_ctx: &Ctx, exprs: Expr1) -> Args {
    exprs
}
pub type Expr1 = Vec<Expr>;
pub fn expr1_c1(_ctx: &Ctx, mut expr1: Expr1, expr: Expr) -> Expr1 {
    expr1.push(expr);
    expr1
}
pub fn expr1_expr(_ctx: &Ctx, expr: Expr) -> Expr1 {
    vec![expr]
}
pub type Expr = ast::Expr;
pub fn expr_atom_expr(_ctx: &Ctx, atom: Atom) -> Expr {
    ast::Expr::Atom(Box::new(atom))
}
pub fn expr_var_expr(_ctx: &Ctx, var: Var) -> Expr {
    ast::Expr::Var(var)
}
pub fn expr_number_expr(_ctx: &Ctx, number: Number) -> Expr {
    ast::Expr::Number(number)
}
pub fn expr_ident_expr(_ctx: &Ctx, name: Ident) -> Expr {
    ast::Expr::Ident(name)
}
