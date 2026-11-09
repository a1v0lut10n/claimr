---
date: 2026-11-09
type: task
status: in-progress
affects:
  - docs/architecture/components/solver.md
components: [evaluator, solver]
aspects: [exact-arithmetic, answer-soundness]
design: [docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md]
tags: [CLM-0013, arithmetic, solver, performance]
---

# CLM-0013: Fold ground arithmetic before it reaches the solver

## Premise

`Store::post_arith` and `Store::post_neg` (`src/eval/store.rs`) define a
fresh solver variable and a simplex row for every arithmetic term, even
when every operand is already a number. Rows persist across backtracking
(`Simplex::undo_to` restores bounds only), and `Simplex::update` walks
every row, so a query that tries F candidates with ground arithmetic in
its constraint costs O(F²). Measured: `{ 10 * K > 10 * N }` over 1 820 and
3 640 facts takes 1.88 s and 7.41 s where `{ K > N }` takes 0.02 s and
0.04 s, with identical answers. The accepted design record decides to fold
ground arithmetic at instantiation, in the store.

## Action items

1. `post_arith`: after `unknown_of` on both operands, if both `LinExpr`s
   are constants, compute the exact rational for `Add`, `Sub`, `Mul`, `Div`
   and return a `Cell::Num` heap cell. `Div` by zero returns `None`
   (failure), as the `recip()` path does today.
2. `post_neg`: same for a constant operand.
3. Store unit tests: a ground `Mul`, `Div` and negation leave
   `simplex.num_vars()` unchanged and bind the expected rationals; ground
   division by zero fails and allocates nothing; a one-constant `Mul` still
   allocates.
4. Machine-level tests: `{ X = 2 * 3 + 4 / 2 }` answers `X = 8`;
   `{ 10 * 3 > 10 * 2 }` answers `true`; `{ X = 1 / 0 }` has no answer;
   the existing arithmetic tests and golden examples are unchanged.
5. `docs/architecture/components/solver.md`: the *Backtracking* note says
   ground arithmetic never reaches the solver and that row persistence now
   concerns only non-ground arithmetic under search; bump `last-verified`.
6. Re-run the measurement on the release build and record it here.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean.
- [ ] Unit and machine tests from items 3 and 4 pass.
- [ ] `{ 10 * K > 10 * N }` over 3 640 facts is within a small factor of
      `{ K > N }`, answers identical.
- [ ] `solver.md` updated; this file updated to what was built.
