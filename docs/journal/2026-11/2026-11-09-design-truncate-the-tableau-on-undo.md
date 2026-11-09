---
date: 2026-11-09
type: journal
components: [solver]
aspects: [exact-arithmetic, answer-soundness]
design:
  - docs/design/2026-11-09-truncate-the-tableau-on-undo.md
---

# Design record accepted: truncate the simplex tableau on undo

- **When:** 2026-11-09 13:34:11

## Context

`Simplex::undo_to` has restored bounds only since the solver was built
(CLM-0006): rows and solver variables created since a mark persisted,
inert but walked by every `update`, so a search over F candidates each
posting a row cost O(F²). The folding record of this morning
(`2026-11-09-fold-ground-arithmetic-before-the-solver.md`) removed the
commonest source of such rows and deferred the structural fix "if the
non-ground case ever shows the same growth". It did, twice, the same day:
CLM-0015 measured open list fields under `{ |Keys| < 2 }` at 0.04 s to
1.14 s from 1 820 to 14 560 facts, and a rule with an open numeric
unknown, `violation(C, Y) :- kn(C, K, N), { K + Y > N }.`, took 2.38 s,
9.71 s, 39.79 s and 161.19 s at the same sizes — the integrity-checker
shape with one parameter left to the solver, unusable past a few thousand
facts. The record was written with a spike's evidence (PR #47) and is
accepted here.

## Details

**Decision.** A mark records the variable count beside the bound trail.
On undo, bounds of pre-mark variables are restored; every post-mark
variable that pivots wrote into a pre-mark row is pivoted back into basic
position in such a row (each pivot retires one pre-mark basic, so the
loop ends); the rows whose basic is post-mark are dropped; the tables are
truncated to the mark's count; a pre-mark variable turned non-basic and
now outside its restored bounds is moved to the bound and `check` runs,
as after any assertion. `changed_since` counts a created variable as a
numeric change. The store drops the printing definitions of removed
variables, since solver indices are reused after an undo, and nothing
outside the trailed store may hold one across a mark.

**Why it is sound.** Three facts carry the argument. Rows are at all times
an equivalent form of the equations posted so far: pivoting rewrites and
never adds or loses an equation. Every equation posted after the mark
defined a then-fresh variable (`define` and `slack` create the variable
they define), so eliminating the post-mark variables leaves exactly the
pre-mark equations over the pre-mark variables — and after the pivots of
step two, dropping the post-mark rows is that elimination. Pivoting never
touches values, so the assignment stays a solution of the rows
throughout; the bound repair is the one every assertion performs. The
restored state is a valid tableau of the pre-mark system with a feasible
assignment: the same store in a possibly different basis, to which
`check`, `is_determined` and the printer are indifferent.

**Alternatives rejected.** Copying the tableau at each mark is linear per
clause trial and brings the quadratic cost back elsewhere. Trailing the
pivots restores the exact basis but costs the branch's own pivot work
again on undo and keeps a trail proportional to it, for a basis that is
not part of the store's meaning. Forbidding pivots that mix pre- and
post-mark variables would cripple `check`, which must pivot where Bland's
rule says. Keeping the deferral has reached its measured limit.

**Evidence.** The spike (local branch `spike/tableau-truncation`, about
forty lines in the simplex and two in the store) passes all 132 tests with
every golden file byte-identical, and measures 0.03 s, 0.05 s, 0.09 s,
0.18 s for the open lists and 0.03 s, 0.06 s, 0.12 s, 0.23 s for the
arithmetic, with answer files identical to the persisting solver's at
every size.

**Pre-write checks.** No task closes with this entry. `exact-arithmetic`
is untouched: truncation is rational pivoting on the existing tableau.
`answer-soundness` rests on the argument above; the implementation task
adds a replay-equivalence test (random definitions, assertions, marks and
undos on one solver against a fresh solver fed only the surviving
operations, many seeds) and a debug assertion that the restored system is
feasible. No cross-repo impact: nothing on the wire or in the token API
changes.

**Next.** The implementation task claims the next ticket and lands the
spike with the replay test and the two benchmarks as growth regression
checks; `solver.md`'s *Backtracking* note changes with it.

## Links

- Design record: `docs/design/2026-11-09-truncate-the-tableau-on-undo.md`
  (status `proposed` → `accepted` in the same change)
- Refined record: `docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md`
- The open-list measurement: `docs/journal/2026-11/2026-11-09-execution-clm-0015-list-length.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/47
