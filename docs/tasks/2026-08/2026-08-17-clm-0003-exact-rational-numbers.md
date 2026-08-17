---
date: 2026-08-17
type: task
status: done
affects:
  - docs/architecture/README.md
  - docs/reference/grammar.md
  - README.md
components: [parser, cli]
aspects: [exact-arithmetic]
design: [docs/design/2026-08-17-exact-rational-arithmetic.md]
tags: [CLM-0003, numbers, rationals]
---

# CLM-0003: Represent numbers as exact arbitrary-precision rationals

## Objective

Replace `Expr::Number(f64)` with an exact `Number` type (arbitrary-precision
rational) so that the AST — and everything the evaluator will build on it —
carries no floating point, per
[the design record](../../design/2026-08-17-exact-rational-arithmetic.md).
Grammar-neutral: literal syntax is unchanged; only what a literal denotes
changes.

## Context

The evaluator note (`docs/catalyst/2026-08-17-evaluator.md`) and the design
record settle the numeric domain on Prolog III's model: exact rationals,
integers as the denominator-1 case, no floats, no separate real type. Doing
this before any evaluator code exists means no float ever has to be
migrated out later.

## Deliverables

1. **`Number` newtype** in `src/number.rs`, wrapping
   `num_rational::BigRational` (dependencies `num-rational`, `num-bigint`,
   `num-traits` as needed). API: construct from integer, from
   numerator/denominator, and **exactly from decimal literal text**
   (`Number::from_literal("18.5")` → 37/2, `"007"` → 7, `"0.10"` → 1/10);
   `is_integer()`; `Display` printing integers plainly and other values as
   `numerator/denominator` in lowest terms (`33/32`); `Debug` readable;
   `Clone, PartialEq, Eq, Hash, PartialOrd, Ord`. No `From<f64>` and no
   `to_f64()` in the public API (an explicit escape hatch may be added later
   for I/O only, by a further decision).
2. **AST** — `Expr::Number(Number)`; derive `Eq` (and `Hash`) on the AST
   types now that no `f64` remains.
3. **Actions** — `src/parser/claimr_actions.rs`: the `Number` terminal
   action converts the token text via `Number::from_literal`; the `Number`
   terminal regex in `claimr.rustemo` is unchanged, with a comment stating
   that decimal literals denote exact rationals.
4. **CLI** — unchanged behaviour; `Debug` output of numbers must be readable
   (e.g. `Number(37/2)`), not the raw bignum internals.
5. **Tests** — unit tests for `Number` (literal conversion incl. leading
   zeros, trailing zeros, integers, `Display`/`Debug`, ordering, equality of
   `0.5` and `1/2`); update parser tests asserting `Expr::Number(...)`; add
   `examples/numbers.claimr` with integer and decimal literals; every test
   green, `cargo clippy --all-targets -- -D warnings` clean.
6. **Docs** — every path in `affects`:
   - `docs/architecture/README.md`: `parser` row mentions the `Number` type;
     the `exact-arithmetic` aspect row is already registered by the design
     record — verify it reflects the post-task state.
   - `docs/reference/grammar.md`: add a semantics note under *Atoms and
     Terms* — decimal literals denote exact rationals; no floats.
   - `README.md`: Features bullet for exact rational arithmetic; the
     Grammar section's note if any; Development notes unaffected.

## Explicitly out of scope

- Arithmetic expressions in the grammar (`+ - * /`, unary minus, negative
  literals) — a later grammar-first task.
- The evaluator, constraint solver, and any number *operations* beyond what
  `Number` needs for construction, comparison, and printing.
- Interval reals, integer/finite-domain constraints.
- Performance work (small-integer fast path, alternative bignum backends).

## Completion

Before setting `status: done`: every path in `affects` describes the
post-task state, `cargo test`/clippy/`aivolution lint` are clean, and a
journal entry links back to this task via `tasks:`. Then freeze this file.
