---
date: 2026-11-09
type: task
status: in-progress
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
   doc updated.
2. **Store** (`src/eval/store.rs`): after the simplex undo, drop printing
   definitions of removed variables (`defs.split_off`), since indices are
   reused.
3. **Replay-equivalence test** in the simplex's tests: a small
   deterministic generator (xorshift, many seeds) drives one solver
   through random `new_var`/`define`/`slack`/`assert_*`/`mark`/`undo_to`
   operations while recording them; after every undo, a fresh solver
   replays only the operations that survive (those before the mark, with
   the same variable numbering), and the two must agree on feasibility of
   the same subsequent assertions, on `is_determined` of every surviving
   variable, and on bounds. Also a direct test: define, pivot by
   assertions, undo, and check `num_vars`, the row set, and that values
   satisfy the rows.
4. **Benchmarks** (plan only): the record's two tables, re-measured with
   the landed code.
5. **Docs**: `solver.md` backtracking note and `last-verified`; this plan.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every golden file byte-identical.
- [ ] `aivolution lint .` clean; `solver.md` current.
