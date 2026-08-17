---
date: 2026-08-17
type: task
status: done
affects:
  - docs/architecture/README.md
  - docs/reference/grammar.md
  - README.md
  - CLAUDE.md
components: [parser, grammar, cli]
aspects: [grammar-authority]
design: [docs/design/2026-08-17-rustemo-parser-generator-over-nom.md]
tags: [CLM-0002, parser, rustemo]
---

# CLM-0002: Replace the nom parser with a rustemo-generated parser

## Objective

Port claimr's parser from hand-written nom 7 combinators to a rustemo
(LR) generated parser whose grammar file is the single authoritative
definition of the language, with position-carrying errors — as decided in
[the design record](../../design/2026-08-17-rustemo-parser-generator-over-nom.md).
Same language, same AST; a parser the evaluator can build on.

## Context

The current parser drifts from `docs/reference/grammar.md` (nested atoms as
arguments do not parse), reports errors without positions, and would need a
rewrite (nom 8, `nom_locate`, precedence climbing) before the grammar grows
for the evaluator. Two sibling repos already run rustemo; phenotyper's
`build.rs` is the pattern to copy. See the design record for the full
comparison.

## Deliverables

1. **Grammar** — `src/parser/claimr.rustemo` transcribing
   `docs/reference/grammar.md` faithfully (no new constructs in this task),
   with a `Layout` rule for whitespace. Every construct has an example under
   `examples/`. Add a nested-atom example (`likes(mary, father(john)).`,
   `{ age(father(john)) > 60 }.`) — currently failing.
2. **Build integration** — `build.rs` + `rustemo-compiler` (0.9.x, matching
   phenotyper): actions in source tree, generated parser in `OUT_DIR`,
   `rustemo_mod!` in `src/parser/mod.rs`. Plain LR; no GLR unless a wanted
   ambiguity is found (document it if so).
3. **Actions → existing AST** — `src/parser/claimr_actions.rs` builds the
   current `Clause` / `Goal` / `Atom` / `Expr` / `Constraint` /
   `ConstraintExpr` / `RelOp` types (moved to `src/ast.rs`). Keep the
   `Rule` vs `ConstraintRule` distinction as today (body contains a
   constraint goal).
4. **Public API** — `claimr::parse_program(&str) -> Result<Vec<Clause>,
   ParseError>` and `claimr::parse_clause`; `ParseError` carries
   line/column and the expected-token set (adapt from rustemo's error, as
   phenotyper's `rustemo_error_to_diags` does). nom, `IResult`, and the
   nom-specific tests are removed; `thiserror` may back `ParseError`.
5. **CLI** — `src/main.rs` prints positioned errors; success path unchanged.
6. **Tests** — port the seven unit tests to the new API; keep
   `tests/parse_examples.rs` (parses every `examples/*.claimr`); add
   negative tests asserting error positions for a few malformed inputs.
7. **Docs** — update every path in `affects`:
   - `docs/architecture/README.md`: `parser` row (rustemo-generated,
     `src/parser/`), `grammar` row (authority is `claimr.rustemo`),
     `grammar-authority` aspect wording; add a `decisions:`-style link to
     the design record where the docs allow it.
   - `docs/reference/grammar.md`: either regenerated from / annotated as a
     view of `claimr.rustemo`, with a banner naming the authoritative file,
     or removed with the reference README pointing at the grammar file.
   - `README.md` (Features, Grammar, Project layout, Development sections)
     and `CLAUDE.md` (project description, "authority" sentence).
8. `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings`
   clean; `aivolution lint` clean.

## Explicitly out of scope

- Any grammar extension (arithmetic in constraints, comments, lists,
  strings, disjunction, negation) — those belong to evaluator-driven tasks
  and go grammar-first through the new file.
- Evaluator, constraint solver, REPL.
- Error *recovery* (continue after the first error).
- Publishing to crates.io.

## Completion

Before setting `status: done`: every path in `affects` describes the
post-task state (rustemo grammar as authority, nom gone), the design record
is `status: accepted`, and a journal entry links back to this task via
`tasks:`. Then freeze this file.
