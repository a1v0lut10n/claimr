---
date: 2026-11-09
type: design
status: proposed
components: [evaluator, solver]
aspects: [exact-arithmetic, answer-soundness]
tags: [arithmetic, solver, performance, integrity-checker]
---

# Fold ground arithmetic before it reaches the solver

## Context

Using claimr as an integrity checker over a knowledge base (about 2 500
claims, about 10 500 facts) showed solving time growing quadratically with
the number of facts whenever a rule's constraint contained arithmetic.
Measured on one fact per claim, `kn(C, K, N, T)`, with the rule
`violation(C) :- kn(C, K, N, T), { … }.` and all answers requested
(release build, `main` at `ea38acc`):

| Constraint              | 1 820 facts | 3 640 facts |
|-------------------------|-------------|-------------|
| `{ K > N }`             | 0.02 s      | 0.04 s      |
| `{ 10 * K > 10 * N }`   | 1.88 s      | 7.41 s      |

Both forms print identical answers, so this is cost, not soundness. The
catalyst entry this record graduates from (`git log --follow` on this
file) holds the full evidence; the mechanism is:

- The arithmetic-in-terms record (`2026-08-17-arithmetic-in-terms.md`)
  lowers every arithmetic term to a fresh numeric variable `N` and a
  linear equation defining it. `Store::post_arith` (`src/eval/store.rs`)
  implements this with `define_fresh`: a heap variable, a solver variable
  and a defined simplex row for every `Add` and `Sub`, and for a `Mul` or
  `Div` with a constant operand. It does so **even when both operands are
  already numbers**: `unknown_of` turns a bound number into
  `LinExpr::constant(c)`, but only the `Mul`/`Div` arms consult
  `as_constant()`, and only to scale the other side.
- `Simplex::undo_to` (`src/solver/simplex.rs`) restores bounds only. Rows
  and solver variables created since a mark persist, a deferral recorded
  in `docs/architecture/components/solver.md` as "acceptable at Claimr's
  scale". Inert rows are not free: `Simplex::update` iterates every row on
  each value change, so each assertion costs time proportional to every
  row ever made, and a query that tries F candidates costs O(F²).
- The same arithmetic record already says "folding is exact and happens
  when the evaluator compiles a clause". No folding exists today, in
  `src/eval/compile.rs` or in `post_arith`.

So `{ 10 * K > 10 * N }` with `K` and `N` bound creates two solver
variables and two rows per trial that `{ K > N }` does not, and every
later trial pays for them.

## Decision

1. **Fold at instantiation, in the store.** When `post_arith` (and
   `post_neg`) finds that every operand is a constant (`as_constant()` on
   each `LinExpr`), it computes the exact rational result and returns a
   heap cell bound to that number. No solver variable, no row. This covers
   all four operators and unary minus, including `Mul` and `Div` of two
   constants, which today fall into the scale-the-other-side arm.
2. **Division by zero keeps failing**, as it does now (`recip()` returning
   `None`): `{ X = 1 / 0 }` has no solution. Folding changes where the
   check happens, not the answer.
3. **One-constant products are unchanged.** `10 * K` with `K` unbound
   still defines a row; that is the linear case the solver exists for.
4. **The arithmetic record's "when the evaluator compiles a clause" is
   read as "at instantiation".** Instantiation is the earliest point at
   which operand values are known for arguments bound at call time, and
   it subsumes compile-time folding of literal-only subterms. The earlier
   record is refined, not superseded.
5. **Tableau truncation on undo is deferred**, not rejected. Pivots
   rewrite pre-mark rows in terms of post-mark variables, so truncating
   `vars` and `rows` needs either pivot trailing or a pivot restriction,
   and a soundness argument for the restored basis. That is a separate
   record if the non-ground case ever shows the same growth.

## Consequences

- Rule-heavy queries over fact tables whose arithmetic is ground at call
  time cost the same as their arithmetic-free form. The expected result is
  the second row of the table above collapsing onto the first.
- Answers are unchanged in content and form: a folded term is a number
  cell, exactly what the solver's determined-variable detection would have
  printed. `--json` output is unchanged.
- `docs/architecture/components/solver.md`'s *Backtracking* note is
  updated to say that ground arithmetic never reaches the solver, and that
  the persistence of rows is now a concern only for non-ground arithmetic
  under search.
- A test pins the behaviour: after `{ 10 * 3 > 10 * 2 }` the simplex has
  no more variables than before; after `{ 10 * K > 10 * 2 }` with `K`
  unbound it has exactly the expected ones. Existing golden runs under
  `examples/` must pass unchanged.
- The join's own quadratic (clause scanning across two fact tables) is
  untouched; that is the first-argument-indexing idea in
  `docs/incubation/`.
