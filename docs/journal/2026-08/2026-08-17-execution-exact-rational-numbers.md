---
date: 2026-08-17
type: journal
components: [parser, cli]
aspects: [exact-arithmetic]
tasks: [docs/tasks/2026-08/2026-08-17-clm-0003-exact-rational-numbers.md]
design: [docs/design/2026-08-17-exact-rational-arithmetic.md]
---

# Numbers are exact rationals (CLM-0003)

## Context
The numeric domain was pinned on Prolog III's model (exact rationals, no
floats, no separate real type) before any evaluator code exists, so no
float ever has to be migrated out.

## Details
- `src/number.rs`: `Number` newtype over `num_rational::BigRational`
  (`num-bigint` backed); exact `from_literal` (`18.5` → 37/2), `from_ratio`,
  Prolog III printing (`7`, `33/32`), `Eq/Hash/Ord`; no `f64` API.
- `Expr::Number(Number)`; AST derives `Eq, Hash`. Actions convert literal
  text exactly; grammar unchanged apart from a semantics comment.
- 29 tests incl. an example literal beyond `f64` precision; clippy clean.
- Docs: architecture `parser` row, `grammar.md` semantics note, README.

## Links
- Design: docs/design/2026-08-17-exact-rational-arithmetic.md
- Task: docs/tasks/2026-08/2026-08-17-clm-0003-exact-rational-numbers.md
