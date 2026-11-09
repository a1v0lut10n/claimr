---
date: 2026-11-09
type: task
status: in-progress
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

## Decision (no design record: an implementation choice with no semantic effect)

The store keeps the **candidates** of the determination pass as sets
maintained incrementally, in the same trail discipline as everything else:

- `undetermined: BTreeSet<Addr>` — numeric heap variables that are unbound
  and not yet fired. Inserted when a variable becomes numeric
  (`set_numvar`), removed when it fires or is bound (trailed, restored on
  undo), removed when its `numvar` entry is undone. The pass iterates this
  set (already sorted) instead of `numvar`.
- `pending_numdifs: BTreeSet<usize>` and `pending_products: BTreeSet<usize>`
  — the ids still pending. Inserted on creation, removed when settled
  (trailed with the existing `NumDifPending` / `ProductPending` records),
  truncated on undo with the entry vectors.

The pass visits exactly the entries it visited before, in the same
(sorted) order, so every decision it takes is unchanged; only the skipped
entries are no longer walked. `answer-soundness` and `exact-arithmetic`
are untouched.

## Action items

1. **Store** (`src/eval/store.rs`): the three sets, their maintenance at
   every creation, settle, bind and undo site, and the pass over them.
2. **Tests**: the existing suite (every golden file byte-identical), plus
   a store test that the sets agree with a recomputation from `numvar`,
   `numdifs` and `products` after a sequence of posts and undos; a machine
   test that a 2 000-step countdown with a disequation per step runs under
   a time budget in the test profile.
3. **Measurement** (release build): the countdown at 2 000 / 4 000 / 8 000 /
   16 000 steps, with and without a `{ M != -1 }` per step, before and
   after.
4. **Docs**: `solver.md` (the determination pass), the incubation note
   (the finding), this plan.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every golden file byte-identical.
- [ ] The countdown scales linearly.
- [ ] `aivolution lint .` clean; `solver.md` current.
