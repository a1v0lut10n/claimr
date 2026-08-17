---
date: 2026-08-18
type: task
status: done
affects:
  - docs/architecture/components/evaluator.md
  - docs/architecture/components/solver.md
  - docs/architecture/aspects/answer-soundness.md
  - README.md
components: [evaluator, solver, cli]
aspects: [answer-soundness, exact-arithmetic]
design:
  - docs/design/2026-08-17-evaluator.md
tags: [CLM-0007, answers, projection, simplification, evaluator-stage-4]
---

# CLM-0007: Answer projection and simplification

## Objective

Stage 4 of the evaluator plan
([design record](../../design/2026-08-17-evaluator.md) D7): answers are the
store **projected onto what the query can see and simplified** — internal
numeric variables eliminated, redundant constraints dropped, tree
disequations in reduced form — so that what is printed is Prolog III's
solved form: every uniquely determined variable explicit, the rest a small
equivalent system over the query's own variables and attribute terms.

## Context

Stage 3 prints the numeric store restricted to reachable variables plus
whatever their rows touch, naming internal variables `_N`. Concretely, on
`p(X) :- { X = Y + Z, Y > 0, Z > 0 }` the query `?- p(A)` prints
`_1 > 0, _2 > 0, A = _1 + _2` where the answer is `A > 0`; `r(A, B)` with
`{ X > Y, Y > Z, X > Z }` prints `A > B, B > _1, A > _1` for `A > B`;
`u(A)` with `{ X = 2*Y, Y >= 1, Y <= 3 }` prints `_1 >= 1, _1 <= 3,
A = 2*_1` for `A >= 2, A <= 6`; duplicated constraints print twice; and
tree disequations print as full terms (`f(Z) != f(W)`) instead of their
reduced form (`Z != W`). One case is a **soundness bug** of stage-3
projection: `q(X) :- { X = Y + 1, Y != 2 }` prints `A = _1 + 1` and drops
`Y != 2` because `Y` is internal — but `Y` is linked to `A` by an equation,
so the omitted constraint is not independent of the printed variables; the
answer must be `A != 3`. Prolog III itself lacked "systematic elimination of
useless numerical variables" and named it as a shortcoming (CACM 1990).

## Design details fixed by this task

- **Residual system.** At answer time, after `finalize`, the numeric store
  visible to the query is collected as a system over alias-class roots:
  equalities (rows of named or reached classes, definitions fixed to a
  value, alias equations), inequalities (bounds, bounded definitions),
  numeric disequations (`expr != 0`), all normalised (definitions expanded,
  fixed classes substituted). Roots are **public** if they belong to a
  query-reachable heap variable or a query-visible attribute term;
  everything else is **internal**.
- **Elimination of internal variables**, equivalence-preserving:
  1. *Equalities first (Gaussian):* while some equality contains an
     internal variable, solve for it and substitute everywhere (equalities,
     inequalities, disequations); the equality disappears. Prefer the
     internal variable with a unit coefficient; exact rationals otherwise.
  2. *Inequalities (Fourier–Motzkin):* eliminate each remaining internal
     variable by combining every lower bound with every upper bound (strict
     if either is strict — δ-rationals carry that); an internal variable
     with bounds on one side only vanishes. **Budget**: if the running size
     of the system would exceed a limit (say 256 constraints), stop
     eliminating further variables and name the survivors `_N` as today —
     never an incorrect answer, at worst a verbose one; note the fallback
     in the component doc.
  3. *Disequations:* after substitution, a numeric disequation over public
     variables only is kept (`A != 3`); one still mentioning an internal
     variable that survives FM is kept with `_N`; one over internal
     variables not linked to anything public is dropped (satisfiable
     independently). *Never* dropped merely because its variables are
     internal if an equality links them — that is the stage-3 bug.
- **Redundancy removal**, exact: normalise each constraint (divide by the
  gcd/leading coefficient so `2X <= 6` and `X <= 3` coincide; orient
  equalities to their solved-form subject), drop syntactic duplicates, then
  drop every inequality entailed by the others — decided with a fresh
  `Simplex` over the residual system: assert the negation of the candidate
  together with the rest; infeasible ⇒ redundant. Equalities are already in
  solved form (each subject once) after step 1. Bounds implied by others go
  the same way (`X > 3` makes `X > 2` redundant).
- **Solved-form printing** (as stage 3, now over the simplified system):
  determined values first (query variables in the equations pass,
  attribute terms as `age(bob) = 3`), then equalities `Y = X + 1` (subject
  = most recently introduced unit-coefficient public variable), then
  inequalities (`X > Y`, `X + Y <= 10`, `X >= 2`), then numeric
  disequations, in a **deterministic order** (by subject/first variable's
  introduction order, then text). Fixed values inline in terms as today.
- **Tree disequations in reduced form.** `check_dif` records the
  would-bind pairs `(variable, term)` of its trial unification on the
  disequation entry; when exactly one pair remains the answer prints
  `Z != W` (or `Y != b`), otherwise the full terms as today (a genuine
  disjunction has no shorter faithful form). Pairs are recomputed at every
  check, so after `finalize` they are current.
- **API.** `Answer` keeps `equations` / `disequations` / `constraints`
  strings; add `Answer::parts()` returning all lines in print order (for
  tests and future REPL) — no structural term API yet.

## Deliverables

1. `src/eval/project.rs` — residual-system construction over roots,
   Gaussian elimination of internal variables, Fourier–Motzkin with budget,
   normalisation, duplicate and entailment-based redundancy removal (using
   `crate::solver::Simplex`), returning the simplified system with public
   variables only (plus named survivors on budget fallback).
2. `src/eval/answer.rs` — render from the simplified system (replacing the
   stage-3 closure-based renderer); reduced-form tree disequations from the
   `Dif` entries' recorded pairs (`store.rs`: keep `would_bind` on `Dif`).
3. **Tests** (`src/eval/tests.rs`, plus solver-level tests for FM and
   redundancy in `src/eval/project.rs`):
   - the cases above: `p(A)` → `A > 0`; `q(A)` → `A != 3` (the bug);
     `r(A, B)` → `A > B`; `s(A)` → `A > 3`; `t(A)` → `true`; `u(A)` →
     `A >= 2, A <= 6`;
   - strictness through FM (`Y > 0, Z >= 0, X = Y + Z` → `X > 0`);
     elimination with non-unit coefficients (`X = 3*Y + 1, Y >= 1` →
     `X >= 4`); a two-sided internal variable producing several
     constraints (`X = Y + Z, Y >= 0, Y <= 1, Z >= 0, Z <= 1` → `X >= 0,
     X <= 2`); budget fallback still correct (a program that would blow up
     FM prints `_N` variables but stays right on a spot-check);
   - redundancy: `{ X > 3, X > 2 }` → `X > 3`; `{ X > Y, Y > Z }` keeps
     both; `{ X + Y <= 10, X + Y <= 20 }` → one line; `2*X <= 6` prints as
     `X <= 3`;
   - reduced tree disequations: `f(Z) != f(W)` → `Z != W`; `f(a, b) !=
     f(a, Y)` → `Y != b`; two-pair case keeps full terms;
   - determinism: the same answer text on repeated runs and independent of
     `HashMap` order (run each case a few times / with shuffled clause
     variable numbering);
   - all existing golden runs unchanged or updated deliberately (review
     every changed `.answers`).
4. **Docs** — every path in `affects`: `components/evaluator.md`
   (`answer.rs`/`project.rs`, reduced-form difs), `components/solver.md`
   (rendering section now points at projection; the budget fallback),
   `aspects/answer-soundness.md` (invariant 4: elimination is
   equivalence-preserving, redundancy removal is exact, disequations linked
   to public variables are never dropped), `README.md` (answers example
   updated if wording changes; the `p(A)`-style example is a nice showcase).

## Explicitly out of scope

- A structural (non-string) answer API; the REPL; non-linear residues;
  simplification of *world* constraints beyond what the query touches;
  performance of FM beyond the budget guard.

## Completion

Before setting `status: done`: every path in `affects` describes the
post-task state, changed `.answers` files are reviewed, `cargo test` /
clippy / `aivolution lint` are clean, and a journal entry links back to
this task via `tasks:`. Then freeze this file.
