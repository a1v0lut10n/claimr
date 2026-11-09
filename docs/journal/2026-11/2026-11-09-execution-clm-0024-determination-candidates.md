---
date: 2026-11-09
type: journal
components: [solver]
aspects: [answer-soundness, exact-arithmetic]
tasks:
  - docs/implementation/2026-11-09-clm-0024-determination-candidates.md
---

# CLM-0024 executed: determination candidates kept incrementally; determined values fold

- **When:** 2026-11-09 21:00:51

## Context

The deep test of CLM-0023 found a numeric countdown,
`down(N) :- { N > 0, M = N - 1 }, down(M).`, quadratic: 13 s for 1 000
steps and 55 s for 2 000 in the test profile, with or without
explanations. The incubation note written then proposed binding `M` to the
number directly, so that the solver never hears of it, and the task was
taken under that name. A probe of the current semantics before changing
anything showed the proposal is not an implementation choice, and the task
became the semantics-free removal of the cost instead.

## Details

**The probe.** `{ M = 2 }, { M = f(a) }` answers `f(a) = 2`: the compound
becomes an attribute term through the numeric variable (D4). `p(2).
?- p(M), { M = f(a) }.` fails: a number acquired by unification is a tree
leaf. `{ X = age(bob) }, { X = 31 }` and `{ X = 31 }, { X = age(bob) }`
both answer `age(bob) = 31`. Binding directly would make the first pair
agree (both fail) and the second pair disagree by goal order, unless the
numeric test looked through variables bound to numbers, which changes the
`p(M)` case the other way. Either way it is a decision on what `=` means
between a number and a tree in numeric position, with goal-order
independence as the constraint, so it belongs to a design record. The note
stays in incubation with an addendum recording the probe.

**Two causes, found in sequence.** The cheap determination pass that
`settle` runs after every post walked every numeric heap variable in
`numvar`, filtered out the fired and bound ones, sorted the rest, and then
walked every numeric disequation and every product, skipping the settled
ones. The plan named that as the cause; fixing it alone did not move the
countdown (4.74 s at 2 000 steps, release), which exposed the dominant
one: a fixed numeric variable stayed a variable in the solver's view, so
the next step's `N - 1` was not ground for CLM-0013's folding rule and
defined a row, and `M = N - 1` asserted a slack row — two rows per step,
each scanned by the simplex's `check` and `update` on every assertion.

**Candidate sets.** The store keeps `undetermined` (numeric variables
unbound and not yet fired), `pending_numdifs` and `pending_products` as
`BTreeSet`s in the trail discipline: inserted on creation, removed when a
variable fires or a disequation or product settles (restored by the
existing trail records), removed when a numeric variable is bound (a new
`Undo::Undetermined` record restores it), truncated with the entry vectors
on undo. The pass iterates them, already sorted, so it visits exactly the
entries it visited before, in the same order. `class_value`, which
`unknown_of` now asks on every post, looks at a class's root and its
aliases instead of every solver variable.

**Determined values fold.** `unknown_of`, the solver's view of a term in
numeric position, returns a determined variable's or attribute term's
value instead of the variable. In every solution of the store the variable
has that value, so the constraint built from the value is the same
constraint, and the trail undoes it together with the determination it
relied on. `N - 1` over a determined `N` folds to a number, `M = <value>`
is a bound, `M != -1` is decided at once: no row, no slack, no probe per
step. Programs whose values flow through arithmetic step after step, the
integrity checker's shape, no longer grow the tableau at all.

**Measurement** (release build; answers byte-identical to `main` on the
countdown and on every example):

| Steps  | `main`  | `main`, with `!=` | new    | new, with `!=` |
|--------|---------|-------------------|--------|----------------|
| 2 000  | 4.60 s  | 6.05 s            | 0.01 s | 0.01 s         |
| 4 000  | 19.47 s | 27.63 s           | 0.01 s | 0.01 s         |
| 16 000 | —       | —                 | 0.05 s | 0.06 s         |

Inside the simplex, `check`, `update` and `pivot` still scan every row, so
a store with many genuinely open rows pays per row on each assertion; a
column index is the next step if that shows up, and `solver.md` says so.

**Verification.** 160 tests: the store unit test
`determination_candidates_agree_with_a_recomputation` (the sets equal a
recomputation from `numvar`, `numdifs` and `products` through posts,
determinations, binds and undos), `a_long_numeric_derivation_is_linear`
(2 000 steps with a disequation per step under a 20 s budget in the test
profile, against 73 s before), and every golden file byte-identical.
Formatting, clippy with warnings denied and the docs linter are clean.

**Pre-write checks.** `solver.md`, the one doc in the task's `affects`, is
current: the glue bullet describes `unknown_of`'s substitution and the
determination bullet the candidate sets, the measurement and the remaining
per-row cost. `answer-soundness` held: the pass takes the same decisions on
the same entries, and a determined value stands for its variable in every
solution. `exact-arithmetic` held: values are exact rationals from
coinciding exact bounds. No cross-repo impact.

## Links

- Task: `docs/implementation/2026-11-09-clm-0024-determination-candidates.md`
- Incubation note, with its addendum:
  `docs/incubation/2026-11-09-bind-numeric-constants-directly.md`
- The deep test that found the cost:
  `docs/journal/2026-11/2026-11-09-execution-clm-0023-explanations-of-answers.md`
- Commits `6ef46be` (plan, ticket claim), `530af64` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/61
- View updated: `docs/architecture/components/solver.md`
