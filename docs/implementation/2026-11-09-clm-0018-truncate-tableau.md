---
date: 2026-11-09
type: task
status: done
affects:
  - docs/architecture/components/solver.md
components: [solver]
aspects: [exact-arithmetic, answer-soundness]
design: [docs/design/2026-11-09-truncate-the-tableau-on-undo.md]
tags: [CLM-0018, solver, simplex, backtracking, performance]
---

# CLM-0018: Truncate the simplex tableau on undo

## Premise

`Simplex::undo_to` restores bounds only; rows and solver variables created
since a mark persist and are walked by every `update`, so a search over F
candidates each posting a row costs O(F²). The accepted record
`2026-11-09-truncate-the-tableau-on-undo.md` decides to restore the
variable and row set on undo as well, with the soundness argument that the
rows are an equivalent form of the posted equations and every post-mark
equation defined a then-fresh variable. A spike (local branch
`spike/tableau-truncation`) implemented it in about forty lines, passed
all tests with byte-identical golden files, and measured the growth gone.

## Scope

In: the spike, landed; a replay-equivalence test for the solver; the two
benchmarks of the record as a growth regression check in the plan (not in
`cargo test`, they are timing); the `solver.md` backtracking note.

Out: any change to what the solver decides; the folding record's
behaviour (ground arithmetic still folds, which is cheaper than posting
and truncating).

## Action items

1. **Simplex** (`src/solver/simplex.rs`): `Mark { trail, vars }`;
   `undo_to` restores pre-mark bounds, discards trail entries of post-mark
   variables, and calls `truncate(n)` when variables were created;
   `truncate` pivots each post-mark variable found in a pre-mark row into
   basic position there, drops rows with post-mark basics, truncates the
   tables, rebuilds the basic-to-row index, moves surviving non-basics
   back inside their bounds with `update`, and runs `check` (debug
   assertion: feasible). `changed_since` counts created variables. Module
   doc updated. **Done** (the spike, unchanged).
2. **Store** (`src/eval/store.rs`): after the simplex undo, printing
   definitions of removed variables are dropped (`defs.split_off`).
   **Done.**
3. **Tests.** `undo_drops_the_variables_and_rows_made_since_the_mark`:
   define, force pivots across the mark, undo, check counts, tableau
   consistency (values satisfy rows, basics own their rows, no row names a
   dropped or basic variable), the pre-mark system intact, and index
   reuse. `undo_is_equivalent_to_replaying_the_surviving_operations`: 160
   seeds of an xorshift generator, 40 steps each, over `new_var`,
   `define`, `slack`, strict and non-strict bounds in the store's
   convention (strict lower `c + δ`, strict upper `c − δ`), marks and undos
   to a random earlier mark; a failed assertion undoes to the innermost
   mark as the store does; after every undo a fresh solver replays the
   surviving operations and must agree on variable count, every bound,
   every `is_determined`, and the verdict on four random further
   assertions. A mutation check — the pivot step removed — passed the
   direct test and failed the replay test within the first seeds, so the
   replay test is the one carrying the soundness argument. **Done.**
4. **Benchmarks** (release build, all answers, answer files byte-identical
   to `main`'s at every size):

   | Facts  | open lists `{ |Keys| < 2 }` | non-ground `{ K + Y > N }` |
   |--------|-----------------------------|----------------------------|
   | 1 820  | 0.26 s → 0.03 s             | 2.38 s → 0.03 s            |
   | 3 640  | 0.10 s → 0.05 s             | 9.71 s → 0.06 s            |
   | 7 280  | 0.32 s → 0.09 s             | 39.79 s → 0.12 s           |
   | 14 560 | 1.15 s → 0.18 s             | 161.19 s → 0.23 s          |

   Both now scale linearly. **Done.**
5. **Docs**: `solver.md` backtracking note rewritten, the record added to
   its decisions; this plan. **Done.**

## Found on the way

- The replay test's generator first drew bounds the store never produces
  (an upper bound `c + δ`), which hit `is_determined`'s invariant that a
  fixed value is a plain rational. The invariant is right for the store's
  systems, so the generator took the store's convention; worth knowing if
  the simplex is ever driven from elsewhere.
- Hand-written structural tests are weak evidence for this kind of change:
  the direct test could not tell a truncation that skips the pivot step
  from the right one. Replay equivalence against a fresh solver is the
  test shape to keep for the solver.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (134 tests); every golden file byte-identical.
- [x] `aivolution lint .` clean; `solver.md` current.
