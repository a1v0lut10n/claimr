---
date: 2026-11-09
type: journal
components: [evaluator, solver]
aspects: [exact-arithmetic, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0013-fold-ground-arithmetic.md
design:
  - docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md
---

# CLM-0013 executed: ground arithmetic folds before the solver; body goals are instantiated when reached

- **When:** 2026-11-09 07:01:08

## Context

The accepted design record `fold-ground-arithmetic-before-the-solver`
answers the quadratic cost found when claimr runs as an integrity checker
over a few thousand facts: `Store::post_arith` and `Store::post_neg`
allocated a solver variable and a simplex row for every arithmetic term,
even one whose operands were already numbers, and rows persist across
backtracking while every `Simplex::update` walks all of them. `{ 10 * K >
10 * N }` over 3 640 facts took 7.3 s where `{ K > N }` took 0.04 s with the
same answers. CLM-0013 is the first of the three accepted records to be
implemented; it was picked because it is small and purely internal.

## Details

**The fold, in the store.** `post_neg` and `post_arith` now check whether
the lowered operands are constants. When they are, the exact rational is
computed (`Add`, `Sub`, `Mul`, `Div`, negation) and a `Num` heap cell is
returned without touching the simplex. Division by a ground zero returns
`None`, so the step fails, as the `recip()` path already did for a
determined zero divisor. A product with exactly one constant operand is
unchanged: it is linear and is defined as a row as before. A store unit
test asserts that the ground cases leave `Simplex::num_vars` unchanged and
that the mixed case still allocates.

**Lazy body instantiation, which the record did not foresee.** The fold on
its own changed nothing for the motivating rule: the machine built every
body goal's terms at clause entry, so `10 * K` was lowered while `K` was
still an unbound head variable, before the fact lookup bound it. The
record's premise, that instantiation sees call-time bindings, holds only
when a goal's terms are built when the goal is reached. The machine
therefore now allocates a clause's variables once into a shared frame
(`Rc<[Addr]>`), builds the head from it for unification, and carries each
body goal as `Goal::Pending { frame, goal }` until it reaches the front of
the goal list, where `instantiate_goal` builds it against the current
store. Query goals remain eager because their variables are the answer's.
Declaratively nothing changes: goals are still solved left to right over
the same store, and the trail undoes everything on backtracking. Head
arithmetic (`sum(X, Y, X + Y)` called with numbers) is still lowered before
head unification and is not folded; this is recorded in the plan as a
possible follow-up, not a regression.

**Verification.** All 113 tests pass, including the golden runs of every
example and two new machine-level tests: ground expressions fold to the
expected rationals (`{ X = 2 * 3 + 4 / 2 }` gives `X = 8`, `{ X = 1 / 0 }`
has no answer), and a rule whose constraint follows a fact lookup
(`violation(C) :- kn(C, K, N), { 10 * K > 7 * N }.`) yields exactly the
violating candidates. The benchmark against the pre-change binary gives
byte-identical answer files at every size:

| Constraint | 1 820 facts | 3 640 facts | 7 280 facts |
|---|---|---|---|
| `{ K > N }` | 0.02 s | 0.03 s | 0.07 s |
| `{ 10 * K > 10 * N }` before | 1.87 s | 7.34 s | 55.4 s |
| `{ 10 * K > 10 * N }` after | 0.02 s | 0.04 s | 0.07 s |

Formatting, clippy with warnings denied and the docs linter are clean.

**Pre-write checks.** Both docs in the task's `affects` were updated and
re-verified: `solver.md` states that ground arithmetic never reaches the
solver and revises the backtracking note; `evaluator.md` describes
`build_with` folding in `compile.rs` and lazy body instantiation in
`machine.rs`. The task is `done`. The `exact-arithmetic` invariant holds
(the fold uses the same rational arithmetic the solver would have) and
`answer-soundness` holds (identical answer sets, verified by diff). No
cross-repo impact: the JSON output and the highlight token API are
untouched.

**Deviation from the record.** The record is append-mostly after
acceptance, so the lazy-instantiation change is recorded in the task's
"Deviation from the design record" section and here, not by editing the
record.

## Links

- Task: `docs/implementation/2026-11-09-clm-0013-fold-ground-arithmetic.md`
- Design record: `docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-fold-arithmetic-lists-strings.md`
- Commits `09fee0a` (plan, ticket claim), `3f71402` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/40
- Component docs updated: `docs/architecture/components/solver.md`,
  `docs/architecture/components/evaluator.md`
