---
date: 2026-11-09
type: task
status: done
affects:
  - docs/architecture/components/solver.md
components: [solver]
aspects: [answer-soundness, exact-arithmetic]
design: []
tags: [CLM-0024, solver, store, performance, determination]
---

# CLM-0024: Determination candidates kept incrementally

## Premise

The incubation note `2026-11-09-bind-numeric-constants-directly.md` (from
the CLM-0023 deep test) reports that a numeric countdown,

```
down(0).
down(N) :- { N > 0, M = N - 1 }, down(M).
```

is quadratic: 13 s for 1 000 steps, 55 s for 2 000 (debug build). It
proposed binding `M` to the number directly so the solver never sees it. A
probe of the current semantics shows that proposal is not an
implementation choice: today `{ M = 2 }, { M = f(a) }` answers
`f(a) = 2` (the compound becomes an attribute term through the numeric
variable, D4) while `p(2). ?- p(M), { M = f(a) }.` fails (a number bound by
unification is a tree leaf), and `{ X = age(bob) }, { X = 31 }` and
`{ X = 31 }, { X = age(bob) }` both answer `age(bob) = 31`. Direct binding
would make the first pair agree (both fail) but make the second pair
disagree by goal order, which `answer-soundness` cannot allow. Whether
`{ M = 2 }` should make `M` a number leaf is therefore a semantic question
for a design record; the note stays in incubation with the finding added.

The quadratic cost itself has a cause with no semantics attached: after
every post, `Store::settle` runs the cheap determination pass
`numeric_consequences(false)`, which iterates **every** numeric heap
variable in `numvar`, filters out the ones already fired or bound, sorts
the rest, and tests each cheaply. Each countdown step adds one fixed
numeric variable that is fired at once and then filtered out again on every
later post: O(n) per post, O(n²) per run. The same holds for the loops over
numeric disequations and delayed products, which walk every registered
entry and skip the settled ones.

## Decision (no design record: implementation choices with no semantic effect)

Two changes, both keeping every decision the store takes:

1. **Determination candidates kept incrementally.** The store keeps the
   candidates of the pass as sets maintained in the trail discipline:
   `undetermined: BTreeSet<Addr>` (numeric heap variables unbound and not
   yet fired; inserted when a variable becomes numeric, removed when it
   fires or is bound — the latter trailed as `Undo::Undetermined` and
   restored on undo — and when its `numvar` entry is undone),
   `pending_numdifs` and `pending_products: BTreeSet<usize>` (inserted on
   creation, removed when settled with the existing trail records,
   truncated with the entry vectors on undo). The pass visits exactly the
   entries it visited before, in the same order. `class_value` looks at a
   class's root and its aliases instead of every solver variable, since
   `unknown_of` now asks on every post.
2. **Determined values substituted in `unknown_of`.** The solver's view of a
   term in numeric position returns a determined variable's (or attribute
   term's) value instead of the variable. In every solution of the store
   the variable has that value, so the constraint built from the value is
   the same constraint; the trail undoes it together with the
   determination it relied on. The effect: `N - 1` over a determined `N`
   folds (CLM-0013's rule now applies), `M = <value>` is a bound, and
   `M != -1` is decided at once — no row, no slack, no probe per step.

Measurement showed the first change alone did not move the countdown
(4.74 s at 2 000 steps): the dominant cost was the rows, two per step,
each scanned by the simplex's `check` and `update` on every assertion. The
second change removes the rows for this shape of program. Rows that are
genuinely open still cost per row per assertion inside the simplex; a
column index there is noted in `solver.md` as the next step if it shows up.

## Action items

1. **Store** (`src/eval/store.rs`): the three sets and their maintenance at
   every creation, settle, bind and undo site; the pass over them;
   `unknown_of` substituting determined values; `class_value` over the
   aliases. **Done.**
2. **Tests**: the existing suite (every golden file byte-identical, 160
   tests); `determination_candidates_agree_with_a_recomputation` (store
   unit test: the sets equal a recomputation from `numvar`, `numdifs` and
   `products` through posts, determinations, binds and undos);
   `a_long_numeric_derivation_is_linear` (2 000 steps with a disequation per
   step under a 20 s budget in the test profile; 73 s before). **Done.**
3. **Measurement** (release build; `down(N) :- { N > 0, M = N - 1 }, down(M).`
   and the same with `M != -1` per step; answers byte-identical to `main`,
   as are all examples):

   | Steps  | `main`  | `main`, with `!=` | new    | new, with `!=` |
   |--------|---------|-------------------|--------|----------------|
   | 2 000  | 4.60 s  | 6.05 s            | 0.01 s | 0.01 s         |
   | 4 000  | 19.47 s | 27.63 s           | 0.01 s | 0.01 s         |
   | 8 000  | —       | —                 | 0.02 s | 0.03 s         |
   | 16 000 | —       | —                 | 0.05 s | 0.06 s         |

   (`main` at 8 000 and 16 000 not run: the quadratic extrapolates to
   minutes.) **Done.**
4. **Docs**: `solver.md` (the glue's `unknown_of`, the determination pass's
   candidates, the remaining per-row cost), the incubation note (an
   addendum with the probe and the semantic question that remains), this
   plan. **Done.**

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every golden file byte-identical.
- [x] The countdown scales linearly.
- [x] `aivolution lint .` clean; `solver.md` current.
