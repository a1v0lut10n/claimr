---
date: 2026-11-09
type: journal
components: [solver]
aspects: [exact-arithmetic, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0018-truncate-tableau.md
design:
  - docs/design/2026-11-09-truncate-the-tableau-on-undo.md
---

# CLM-0018 executed: the simplex tableau is truncated on undo

- **When:** 2026-11-09 14:03:44

## Context

The record accepted earlier today ends the deferral of the folding record:
rows and solver variables created since a mark persisted across
backtracking and were walked by every `update`, so a search over F
candidates each posting a row cost O(F²) — measured on open list fields
(CLM-0015) and on a rule with an open numeric unknown, which reached 161 s
at 14 560 facts. The task lands the spike that produced the record's
evidence, with the test the record asked for.

## Details

**What landed.** `Simplex::Mark` records the variable count beside the
bound-trail length. `undo_to` restores the bounds of pre-mark variables,
discards trail entries of post-mark ones, and when variables were created
since the mark calls `truncate`: while a pre-mark row contains a post-mark
variable, pivot that variable into basic position in that row (the
pre-mark basic becomes non-basic; each pivot retires one, so the loop
ends); drop the rows whose basic is post-mark; truncate the variable and
row-index tables and rebuild the index; move surviving non-basics back
inside their restored bounds with `update`; run `check`, with a debug
assertion that the pre-mark system is feasible. `changed_since` counts a
created variable as a change. The store drops the printing definitions of
removed variables on undo, since their indices are reused; everything
else holding an `SVar` was already trailed or truncated at the mark. The
module doc and `solver.md`'s *Backtracking* note say so.

**Tests.** A direct test defines a system, forces pivots across a mark,
undoes, and checks the counts, the tableau's consistency (values satisfy
rows, basics own their rows, no row names a dropped or basic variable),
the pre-mark system and index reuse. The decisive one is
`undo_is_equivalent_to_replaying_the_surviving_operations`: 160 seeds of
a small xorshift generator drive one solver through 40 random steps of
`new_var`, `define`, `slack`, strict and non-strict bounds, marks, and
undos to a random earlier mark, undoing to the innermost mark on a failed
assertion as the store does; after every undo a fresh solver replays only
the surviving operations and must agree on the variable count, every
bound, every `is_determined`, and the verdict on four further random
assertions. A mutation check removed the pivot step: the direct test
still passed, the replay test failed within the first seeds. The replay
test is what carries the record's soundness argument, and it is the test
shape to keep for the solver.

**Found on the way.** The generator first drew a bound the store never
produces, an upper bound `c + δ`, and tripped `is_determined`'s invariant
that a fixed value is a plain rational. The invariant is right for the
store's systems (strict lowers add δ, strict uppers subtract it), so the
generator took the store's convention; it is worth knowing if the simplex
is ever driven from elsewhere.

**Measurement**, release build, all answers, answer files byte-identical
to `main`'s at every size:

| Facts  | open lists `{ |Keys| < 2 }` | non-ground `{ K + Y > N }` |
|--------|-----------------------------|----------------------------|
| 1 820  | 0.26 s → 0.03 s             | 2.38 s → 0.03 s            |
| 3 640  | 0.10 s → 0.05 s             | 9.71 s → 0.06 s            |
| 7 280  | 0.32 s → 0.09 s             | 39.79 s → 0.12 s           |
| 14 560 | 1.15 s → 0.18 s             | 161.19 s → 0.23 s          |

Both shapes scale linearly now. 134 tests pass; formatting, clippy with
warnings denied and the docs linter are clean.

**Pre-write checks.** `solver.md`, the one doc in the task's `affects`, is
current (backtracking note, the record in its decisions).
`exact-arithmetic` is untouched: truncation is rational pivoting on the
existing tableau. `answer-soundness`: the record's argument now has the
replay test behind it, and every golden answer is unchanged. No
cross-repo impact.

## Links

- Task: `docs/implementation/2026-11-09-clm-0018-truncate-tableau.md`
- Design record: `docs/design/2026-11-09-truncate-the-tableau-on-undo.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-truncate-the-tableau-on-undo.md`
- Refined record: `docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md`
- Commits `874db3f` (plan, ticket claim), `8bc3ff8` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/49
- View updated: `docs/architecture/components/solver.md`
