---
date: 2026-08-17
type: design
status: accepted
components: [parser, grammar]
aspects: [grammar-authority]
tags: [parser, rustemo, nom, grammar]
---

# Generate the parser with rustemo instead of hand-writing it with nom

## Context

The claimr parser (`src/lib.rs`) is hand-written with nom 7 combinators
against the EBNF in `docs/reference/grammar.md`. Before the evaluator work
(see `docs/incubation/2026-08-17-evaluator.md`) the grammar will grow:
arithmetic inside constraints (precedence, unary minus), comments,
disequality on terms, probably lists, strings, disjunction and negation.

Observed with the current parser:

- **Grammar drift.** `expr ::= … | atom` per the grammar, but atom arguments
  are parsed with a hand-made `parse_expr_noatom`, so `likes(mary,
  father(john)).` fails. Nothing checks the Rust control flow against the
  written grammar; the `grammar-authority` aspect is enforced by discipline
  only.
- **Unusable diagnostics.** A failure reports `Error { input: <rest of
  file>, code: Eof }` — no position, no expected-token set. Fixing that in
  nom means `nom_locate` + `VerboseError`/`context()` + careful `cut`
  placement, all by hand.
- Left recursion must be refactored away and operator precedence hand-rolled
  (precedence climbing); grammar interactions surface only through tests.
- nom 7 → 8 is an API migration pending anyway.

Two sibling projects (phenotyper, colap) already use the **rustemo** parser
generator (LR(1)/LALR with optional GLR): a declarative `.rustemo` grammar
with EBNF operators, priorities/associativity, a context-aware lexer, a
`Layout` rule for whitespace/comments, build-time conflict reporting,
position-carrying errors, and a regenerated parser plus a persistent,
customizable actions file. phenotyper's `build.rs` (`actions_in_source_tree()`,
parser in `OUT_DIR`) is the clean pattern to copy.

Performance is not a differentiator at claimr's scale: programs are small and
the evaluator will dominate. Prolog-style user-definable operators (`op/3`),
if ever wanted, need a runtime Pratt pass under either tool.

Alternatives considered:

- **Keep nom, rewrite carefully** (nom 8, `nom_locate`, precedence
  climbing). Viable while the language stays tiny; grammar drift remains a
  discipline problem.
- **chumsky** — combinators with built-in Pratt parsing, error recovery and
  spans. Better than nom for this, but a third parsing stack in the
  workspace for no gain over rustemo.

## Decision

Replace the nom parser with a **rustemo-generated LR parser**:

- `src/parser/claimr.rustemo` becomes the **authoritative grammar**; the
  `grammar-authority` aspect is thereby enforced by the compiler.
  `docs/reference/grammar.md` becomes a rendered/annotated view of that
  file (or is dropped), not a second source of truth.
- Plain LR(1)/LALR; enable GLR only if a genuine, wanted ambiguity appears.
- The existing AST (`Clause`, `Goal`, `Atom`, `Expr`, `Constraint`,
  `ConstraintExpr`, `RelOp`) stays the public output of parsing; the actions
  file maps productions onto it. `parse_program` / a per-clause entry point
  keep their shape; nom's `IResult` leaves the public API in favour of a
  position-carrying error type.
- Follow phenotyper's `build.rs` shape (actions in source tree, parser in
  `OUT_DIR`); pin the same rustemo version line as phenotyper (0.9.x).

## Consequences

- Easier: growing the grammar (left recursion, precedence, comments) is
  declarative; conflicts are build-time errors; diagnostics carry positions;
  keyword/identifier growth is handled by the context-aware lexer.
- Harder / new: a `build.rs` and generated code; longer compile; rustemo
  API churn on upgrades (colap 0.7 vs phenotyper 0.9 shows the surface
  moves); a single-maintainer dependency.
- The rewrite happens **before** the evaluator, so evaluator work targets a
  parser that matches its grammar. Public API break: `IResult` → a
  claimr error type — acceptable at 0.1.
- `docs/architecture/README.md` (`parser`, `grammar` rows), `README.md`,
  `CLAUDE.md`, and `docs/reference/grammar.md` must be updated to describe
  the new authority — tracked by the implementing task.
