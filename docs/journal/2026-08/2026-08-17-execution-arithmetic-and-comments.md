---
date: 2026-08-17
type: journal
components: [grammar, parser, cli]
aspects: [grammar-authority, exact-arithmetic]
tasks: [docs/tasks/2026-08/2026-08-17-clm-0004-arithmetic-and-comments.md]
design:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
---

# Arithmetic in terms and line comments (CLM-0004)

## Context
Stage 1 of the evaluator plan: the constraint language needed arithmetic
before a solver has anything to solve, and comment syntax was missing.
Following Prolog III, arithmetic operators are term constructors admitted
anywhere a term goes (companion design record accepted this session).

## Details
- `claimr.rustemo`: `Expr` gains `+ - * /` (left-assoc, `* /` tighter),
  unary minus (tightest), parentheses — via rustemo priorities, zero LR
  conflicts; `Layout` rule with `%` line comments.
- AST: `Expr::Neg`, `Expr::Binary { ArithOp }`; no constant folding.
- 39 tests (precedence, associativity, unary minus, arithmetic in atom
  arguments, comments in all positions); `examples/arithmetic.claimr`;
  clippy clean.
- Docs: `grammar.md` expr productions + comment syntax, README.

## Links
- Design: docs/design/2026-08-17-arithmetic-in-terms.md
- Task: docs/tasks/2026-08/2026-08-17-clm-0004-arithmetic-and-comments.md
