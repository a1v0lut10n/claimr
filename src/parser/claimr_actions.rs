/// This file is maintained by rustemo but can be modified manually.
/// All manual changes will be preserved except non-doc comments.
///
/// Semantic actions for `claimr.rustemo`. Rather than the default generated
/// tree types, every production builds the AST in [`crate::ast`] directly, so
/// the parser's public output is the `ast` module. Type aliases keep the names
/// rustemo's generated parser expects (one per grammar rule).
use rustemo::Token as RustemoToken;
use rustemo::Context as _;
use super::claimr::{TokenKind, Context};
use crate::ast;
use crate::Span;
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
pub type Program = Vec<(ast::Clause, Span)>;
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
/// A clause with the source position where it starts.
pub type Clause = (ast::Clause, Span);
fn span_of(ctx: &Ctx) -> Span {
    Span::from(ctx.span().start)
}
pub fn clause_fact(ctx: &Ctx, fact: Fact) -> Clause {
    (fact, span_of(ctx))
}
pub fn clause_rule(ctx: &Ctx, rule: Rule) -> Clause {
    (rule, span_of(ctx))
}
pub fn clause_constraint_fact(ctx: &Ctx, constraint_fact: ConstraintFact) -> Clause {
    (constraint_fact, span_of(ctx))
}
pub fn clause_implication(ctx: &Ctx, implication: Implication) -> Clause {
    (implication, span_of(ctx))
}
pub fn clause_query(ctx: &Ctx, query: Query) -> Clause {
    (query, span_of(ctx))
}
pub type Fact = ast::Clause;
pub fn fact_c1(_ctx: &Ctx, head: Atom) -> Fact {
    ast::Clause::Fact(head)
}
pub type Rule = ast::Clause;
/// A rule whose body contains at least one constraint goal is a
/// `ConstraintRule` (the EBNF's `constraint_rule`); otherwise a plain `Rule`.
pub fn rule_c1(_ctx: &Ctx, head: Atom, body: Body) -> Rule {
    if ast::body_has_constraint(&body) {
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
/// CLM-0022: `Alt ';' Body` — an if-then-else when the alternative is
/// `C -> T`, a disjunction otherwise.
pub fn body_or(_ctx: &Ctx, alt: Alt, rest: Body) -> Body {
    match alt {
        Alt::IfThen(cond, then) => {
            vec![ast::Goal::IfThenElse { cond, then, els : Some(rest) }]
        }
        Alt::Plain(conj) => vec![ast::Goal::Or(conj, rest)],
    }
}
/// CLM-0022: a body of one alternative — a conjunction, or an if-then whose
/// else fails.
pub fn body_one(_ctx: &Ctx, alt: Alt) -> Body {
    match alt {
        Alt::IfThen(cond, then) => vec![ast::Goal::IfThenElse { cond, then, els : None }],
        Alt::Plain(conj) => conj,
    }
}
/// CLM-0022: an alternative of a body: a conjunction, or `C -> T`.
#[derive(Debug)]
pub enum Alt {
    Plain(Vec<ast::Goal>),
    IfThen(Vec<ast::Goal>, Vec<ast::Goal>),
}
pub fn alt_if_then(_ctx: &Ctx, cond: Conj, then: Conj) -> Alt {
    Alt::IfThen(cond, then)
}
pub fn alt_plain(_ctx: &Ctx, conj: Conj) -> Alt {
    Alt::Plain(conj)
}
pub type Conj = Vec<ast::Goal>;
pub fn conj_c1(_ctx: &Ctx, goals: Goal1) -> Conj {
    goals
}
/// The goals of a conjunction. A `Goal` production yields a list — one goal,
/// or the goals of a parenthesised conjunction — which is spliced here, so
/// parentheses leave no trace (CLM-0022).
pub type Goal1 = Vec<ast::Goal>;
pub fn goal1_c1(_ctx: &Ctx, mut goal1: Goal1, goal: Goal) -> Goal1 {
    goal1.extend(goal);
    goal1
}
pub fn goal1_goal(_ctx: &Ctx, goal: Goal) -> Goal1 {
    goal
}
pub type Goal = Vec<ast::Goal>;
pub fn goal_atom_goal(_ctx: &Ctx, atom: Atom) -> Goal {
    vec![ast::Goal::Atom(atom)]
}
pub fn goal_constraint_goal(_ctx: &Ctx, constraints: ConstraintExpr) -> Goal {
    vec![ast::Goal::Constraint(constraints)]
}
/// CLM-0022: `\+ G`; the goal may be a parenthesised body.
pub fn goal_not_goal(_ctx: &Ctx, _not_op: NotOp, goal: Goal) -> Goal {
    vec![ast::Goal::Not(goal)]
}
/// CLM-0022: `( Body )` — a disjunction or if-then-else stays one goal; a
/// grouped conjunction is its goals, spliced into the enclosing one.
pub fn goal_group_goal(_ctx: &Ctx, body: Body) -> Goal {
    body
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
fn binary(op: ast::ArithOp, left: Expr, right: Expr) -> Expr {
    ast::Expr::Binary {
        op,
        left: Box::new(left),
        right: Box::new(right),
    }
}
pub fn expr_add(_ctx: &Ctx, left: Expr, right: Expr) -> Expr {
    binary(ast::ArithOp::Add, left, right)
}
pub fn expr_sub(_ctx: &Ctx, left: Expr, right: Expr) -> Expr {
    binary(ast::ArithOp::Sub, left, right)
}
pub fn expr_mul(_ctx: &Ctx, left: Expr, right: Expr) -> Expr {
    binary(ast::ArithOp::Mul, left, right)
}
pub fn expr_div(_ctx: &Ctx, left: Expr, right: Expr) -> Expr {
    binary(ast::ArithOp::Div, left, right)
}
pub fn expr_neg(_ctx: &Ctx, operand: Expr) -> Expr {
    ast::Expr::Neg(Box::new(operand))
}
/// Parentheses only group; they leave no trace in the AST.
pub fn expr_paren(_ctx: &Ctx, inner: Expr) -> Expr {
    inner
}
/// CLM-0014: `<>` and `<a, b, c>` — a list with no tail (it ends in `<>`).
pub fn expr_list_expr(_ctx: &Ctx, tuple: Tuple) -> Expr {
    ast::Expr::List {
        items: tuple,
        tail: None,
    }
}
/// CLM-0016: `X . Y` — the concatenation of two lists. With a literal
/// tuple on the left it is the chain of CLM-0014: `<a, b> . T` is the items
/// followed by the tail `T`, the empty chain `<> . T` is `T` itself, and
/// `<a> . <b> . T` is the one chain `<a, b> . T` (the same term; the answer
/// printer would print it so). Any other left operand is a concatenation
/// constraint.
pub fn expr_concat_expr(_ctx: &Ctx, mut left: Expr, mut right: Expr) -> Expr {
    if !matches!(left, ast::Expr::List { tail : None, .. }) {
        return ast::Expr::Concat(Box::new(left), Box::new(right));
    }
    let mut tuple = match &mut left {
        ast::Expr::List { items, .. } => std::mem::take(items),
        _ => Vec::new(),
    };
    if tuple.is_empty() {
        return right;
    }
    if let ast::Expr::List { items, tail } = &mut right {
        tuple.extend(std::mem::take(items));
        return ast::Expr::List {
            items: tuple,
            tail: tail.take(),
        };
    }
    ast::Expr::List {
        items: tuple,
        tail: Some(Box::new(right)),
    }
}
/// CLM-0017: resolve backslash escapes between the delimiters of a quoted
/// atom or a string literal.
fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            }
            c => out.push(c),
        }
    }
    out
}
/// CLM-0017: a string literal is the list of its characters, each a
/// one-character atom: `"abc"` is `<a, b, c>`, `""` is `<>`.
pub type StringLit = Vec<ast::Expr>;
pub fn string_lit(_ctx: &Ctx, token: Token) -> StringLit {
    let inner = &token.value[1..token.value.len() - 1];
    unescape(inner).chars().map(|c| ast::Expr::Ident(c.to_string())).collect()
}
pub fn expr_string_expr(_ctx: &Ctx, string: StringLit) -> Expr {
    ast::Expr::List {
        items: string,
        tail: None,
    }
}
/// CLM-0015: `|L|` — the length of the list `L`, a numeric term.
pub fn expr_len_expr(_ctx: &Ctx, inner: Expr) -> Expr {
    ast::Expr::Len(Box::new(inner))
}
/// The items of a literal tuple `<a, b, c>` (`<>` has none).
pub type Tuple = Vec<ast::Expr>;
pub fn tuple_c1(_ctx: &Ctx, items: ArgsOpt) -> Tuple {
    items.unwrap_or_default()
}
pub type QuotedIdent = String;
/// CLM-0011: strip the outer quotes and resolve backslash escapes —
/// `\'` is a quote, `\\` a backslash, any other `\c` is `c`. The
/// AST carries CONTENT; identity is content, quoting is spelling
/// (docs/design/2026-09-10-quoted-atoms.md).
pub fn quoted_ident(_ctx: &Ctx, token: Token) -> QuotedIdent {
    unescape(&token.value[1..token.value.len() - 1])
}
/// A name is its content, however spelled — both alternatives are the
/// same `String`, so `Atom.name` and `Expr::Ident` are untouched.
pub type Name = String;
pub fn name_plain_name(_ctx: &Ctx, ident: Ident) -> Name {
    ident
}
pub fn name_quoted_name(_ctx: &Ctx, quoted: QuotedIdent) -> Name {
    quoted
}
/// CLM-0022: the `\+` token (a regex terminal, so it carries its text).
pub type NotOp = String;
pub fn not_op(_ctx: &Ctx, token: Token) -> NotOp {
    token.value.into()
}
