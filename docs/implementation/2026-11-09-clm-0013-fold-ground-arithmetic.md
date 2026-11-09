---
date: 2026-11-09
type: task
status: done
affects:
  - docs/architecture/components/solver.md
  - docs/architecture/components/evaluator.md
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
   (failure), as the `recip()` path does today. **Done.**
2. `post_neg`: same for a constant operand. **Done.**
3. Store unit tests: a ground `Mul`, `Div` and negation leave
   `simplex.num_vars()` unchanged and bind the expected rationals; ground
   division by zero fails and allocates nothing; a one-constant `Mul` still
   allocates. **Done** (`ground_arithmetic_folds_without_solver_variables`).
4. Machine-level tests: `{ X = 2 * 3 + 4 / 2 }` answers `X = 8`;
   `{ 10 * 3 > 10 * 2 }` answers `true`; `{ X = 1 / 0 }` has no answer;
   the existing arithmetic tests and golden examples are unchanged.
   **Done** (`ground_arithmetic_folds_to_numbers`).
5. **Added during implementation — lazy body instantiation.** Items 1–4
   alone did not move the measurement: `try_clauses` instantiated every
   body goal at clause entry, so in
   `violation(C) :- kn(C, K, N, T), { 10 * K > 10 * N }.` the arithmetic
   was lowered while `K` and `N` were still unbound, before the `kn` lookup
   bound them. The design record's premise ("instantiation is the earliest
   point at which operand values are known") holds only if a goal's terms
   are built when the goal is reached. So `machine.rs` now allocates a
   clause's variables once into a shared frame (`Rc<[Addr]>`) at clause
   entry, builds the head from it for unification, and queues each body
   goal as `Goal::Pending { frame, goal }`; `run` builds the goal's terms
   when it reaches the front. `compile.rs` gains `build_with` (variable
   resolution as a closure) with `build`/`lazy_var` as the eager-frame
   case, still used for query goals and the initial store. Declaratively
   nothing changes (answer sets are identical, all 85 unit tests and every
   golden example pass unchanged); operationally, arithmetic definitions
   are posted later and failures of a body goal surface when it runs rather
   than at clause entry. Regression test
   `body_goals_are_instantiated_when_reached` covers a body-only variable
   shared across goals. **Done.**
6. `docs/architecture/components/solver.md` (*Backtracking* note) and
   `docs/architecture/components/evaluator.md` (`compile.rs` and
   `machine.rs` paragraphs) updated; `last-verified` bumped; the design
   record added to both docs' `decisions`. **Done.**
7. Measurement re-run, release build, `main`-equivalent synthetic facts
   (`K`, `N` uniform in 100…300), all answers, identical output to the
   pre-change binary at every size:

   | Constraint            | 1 820  | 3 640  | 7 280  |
   |-----------------------|--------|--------|--------|
   | `{ K > N }`           | 0.02 s | 0.03 s | 0.07 s |
   | `{ 10 * K > 10 * N }` before | 1.87 s | 7.34 s | (55.4 s in the original experiment) |
   | `{ 10 * K > 10 * N }` after  | 0.02 s | 0.04 s | 0.07 s |

   **Done.**

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean.
- [x] Unit and machine tests from items 3, 4 and 5 pass.
- [x] `{ 10 * K > 10 * N }` over 3 640 facts is within a small factor of
      `{ K > N }` (0.04 s against 0.03 s), answers identical.
- [x] `solver.md` and `evaluator.md` updated; this file updated to what was
      built.

## Deviation from the design record

The record says "fold at instantiation, in the store" and treats
instantiation as the point where call-time bindings are visible. That
required the machine change in item 5, which the record did not foresee.
The record is append-mostly after acceptance, so the deviation is recorded
here and in the journal entry that closes this task, not by editing the
record. Head arithmetic (`sum(X, Y, X + Y)` called with numbers) is still
lowered before head unification and is not folded; that is a possible
follow-up, not a regression.
