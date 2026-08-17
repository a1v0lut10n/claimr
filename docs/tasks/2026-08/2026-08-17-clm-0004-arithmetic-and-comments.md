---
date: 2026-08-17
type: task
status: planned
affects:
  - docs/reference/grammar.md
  - README.md
components: [grammar, parser, cli]
aspects: [grammar-authority, exact-arithmetic]
design:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
tags: [CLM-0004, grammar, arithmetic, comments, evaluator-stage-1]
---

# CLM-0004: Arithmetic in constraint expressions, and comments

## Objective

Stage 1 of the evaluator plan
([evaluator record](../../design/2026-08-17-evaluator.md), D6, as refined by
[arithmetic-in-terms](../../design/2026-08-17-arithmetic-in-terms.md)):
extend the language — grammar first — so that terms admit arithmetic
anywhere (`{ X + Y = 10, 2*X - Y >= 1/3 }`, `p(X+1)`), and programs can
carry line comments (`% …`). Purely a front-end change: grammar, AST,
actions, tests, docs. No evaluation.

## Context

Terms are `identifier | number | atom | variable` today, which gives a
solver nothing to solve. Prolog III's operations are term constructors —
`x+y` denotes a number wherever it appears — and claimr follows it, with
`1/3` an exact constant and linearity checked by the solver, not the
grammar. Comment syntax has been missing since the start and costs one
`Layout` rule.

## Deliverables

1. **Grammar** (`src/parser/claimr.rustemo`, the authority):
   - `Expr` becomes the arithmetic term grammar:
     ```
     Expr: Expr '+' Expr   {left, 1}
         | Expr '-' Expr   {left, 1}
         | Expr '*' Expr   {left, 2}
         | Expr '/' Expr   {left, 2}
         | '-' Expr        {3}          // unary minus, binds tightest
         | '(' Expr ')'
         | Atom | Var | Number | Ident  // as today
         ;
     ```
     `Args: Expr+[Comma]` and `Constraint: Expr RelOp Expr` are unchanged
     in shape, so arithmetic is admitted **everywhere a term goes** — atom
     arguments (`p(X+1)`) and constraint operands alike. Standard
     precedence and left associativity via rustemo priorities; **zero LR
     conflicts** is a hard requirement (no GLR). Negative numbers are
     `'-' Number` (unary minus on a literal); the `Number` terminal stays
     non-negative.
   - Line comments: `%` to end of line, anywhere whitespace is allowed.
     Add a `Layout` rule (`Layout: LayoutItem*; LayoutItem: WS | Comment;`)
     with terminals `WS: /\s+/` and `Comment: /%[^\n]*/`; once a `Layout`
     rule exists rustemo no longer skips whitespace implicitly, so `WS`
     must be part of it. Update the header comment ("no comment syntax").
2. **AST** (`src/ast.rs`): `Expr` gains
   ```rust
   Neg(Box<Expr>),
   Binary { op: ArithOp, left: Box<Expr>, right: Box<Expr> },
   ```
   with `pub enum ArithOp { Add, Sub, Mul, Div }`; `Constraint` keeps
   `left: Expr, right: Expr`. Keep `Eq, Hash` derives.
3. **Actions** (`src/parser/claimr_actions.rs`): new productions build
   the `Expr` variants; no constant folding (`1/3` stays
   `Binary(Div, 1, 3)` — the evaluator's compile step folds constants
   exactly).
4. **Tests**: precedence and associativity (`1 - 2 - 3` → `(1-2)-3`,
   `1 + 2 * 3` → `1 + (2*3)`, `-X * Y` → `(-X) * Y`, parentheses),
   `1/3` as a division node, attribute terms in arithmetic
   (`age(X) + 1 >= 18`), arithmetic in atom arguments (`p(X+1)`,
   `f(-(2*Y))`), comments at line start / after a clause / inside a clause
   across lines / at EOF without newline; the `errors_are_positioned`
   cases still hold.
   New `examples/arithmetic.claimr` (with comments) — every example still
   parses; `cargo clippy --all-targets -- -D warnings` clean.
5. **CLI**: no change beyond the AST `Debug` output.
6. **Docs** — every path in `affects`:
   - `docs/reference/grammar.md`: the `expr` productions with precedence
     (arithmetic anywhere a term goes), `comment` syntax, and drop "there
     is currently no comment syntax"; keep the authority banner.
   - `README.md`: Grammar excerpt (expr with arithmetic, comments) and an
     example with arithmetic and a comment.
   - Also (not current-state docs, so not in `affects`): the `.rustemo`
     header comment; nothing else.

## Explicitly out of scope

- Any evaluation, constant folding, linearity checking (evaluator stages
  2–3), block comments, strings, lists, disjunction, negation.
- Changing what a `Number` literal denotes (exact rationals, unchanged).

## Completion

Before setting `status: done`: every path in `affects` describes the
post-task state, `cargo test`/clippy/`aivolution lint` are clean, and a
journal entry links back to this task via `tasks:`. Then freeze this file.
