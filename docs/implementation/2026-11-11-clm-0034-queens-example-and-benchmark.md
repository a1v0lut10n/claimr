---
date: 2026-11-11
type: task
status: in-progress
affects:
  - docs/reference/README.md
components: [cli, evaluator]
aspects: []
tags: [CLM-0034, examples, n-queens, benchmark, readme, incubation, finite-domains]
---

# CLM-0034: N queens as an example, a benchmark and a compass

## Premise

The N-queens problem, the standard constraint-programming showcase (the
OR-Tools CP-SAT guide opens with it), fits claimr in about twenty lines:
the N columns are unknowns, every pair of rows gets its three
disequations before any value is chosen, and the search only picks values
from `1..N`. Measured on 2026-11-11 (release build): all 92 solutions of
8 queens in 0.93 s, a first solution for 12 queens in 0.42 s, 3 queens
`false` with a failure tree whose leaves name the attacking queen. What
claimr does *not* have shows just as clearly: no integer domains and no
propagation, so the store rejects a value the moment it contradicts a
constraint but never prunes a domain ahead of the search — the search is
backtracking with constraint checking, and it scales to a dozen queens,
not to the hundreds CP-SAT handles. The example is therefore three
things: a program for the README, a benchmark whose numbers are recorded
and reproducible, and the evidence an incubation note on integer domains
and propagation starts from.

## Shape of the implementation

**The example.** `examples/queens.claimr` with a golden `.answers`: the
program (`queens/2`, `safe/1`, `no_attack/3`, `place/2`, `member/2`,
`range/3`) and the queries `queens(4, Qs)` (2 answers), `queens(5, Qs)`
(10), `queens(6, Qs)` (4) and `queens(3, Qs)` (`false`) — sizes a debug
build runs in well under a second. A value is assigned through a
constraint goal, `member(V, Values), { Q = V }`, rather than by head
unification, so that under `--explain` a rejected value shows as the
constraint it breaks (the record's aggregation rule folds head failures
into one `no clause` leaf); a comment in the file says why.

**The benchmark.** `tests/benchmarks.rs`, ignored by default and run with
`cargo test --release --test benchmarks -- --ignored --nocapture`: it
takes the example's program text, appends one query per size, times the
binary and prints the timings — a first solution for 8, 10 and 12
queens, and all 92 solutions of 8 — asserting only the answer counts, so
the file is a measuring instrument, not a flaky gate. No dependency: the
standard library's `Instant` and the `CARGO_BIN_EXE_claimr` binary.
`docs/reference/benchmarks.md` records the numbers per version with the
command, the machine class and what the numbers say about the evaluator,
and `docs/reference/README.md` lists it.

**The compass.** `docs/incubation/2026-11-11-integer-domains-and-propagation.md`:
the idea — integer-valued variables with finite domains, `all_different`,
propagation that prunes before the search, labeling as a built-in — with
the queens numbers as its evidence, what exists today (exact rationals,
disequations decided on determination, Prolog III's own position:
enumeration only), the deferrals in the evaluator and exact-arithmetic
records, and the open questions (a separate solver component beside the
simplex, how integrality meets rational bounds, the surface syntax).
Prose, no ticket, no status, as the pipeline wants.

**The README.** A second block under *Examples*: the program, the
answers for 4 queens, one branch of the 3-queens failure tree under
`--explain`, and the honest framing in two sentences.

## Action items

1. `examples/queens.claimr` and `examples/queens.answers`.
2. `tests/benchmarks.rs`; `docs/reference/benchmarks.md`;
   `docs/reference/README.md`.
3. `docs/incubation/2026-11-11-integer-domains-and-propagation.md`.
4. README section; this plan.

## Findings

(filled in as the work proceeds)

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` (the new golden run; the benchmark tests ignored and
      compiled); `cargo test --release --test benchmarks -- --ignored --nocapture`
      runs and its numbers are the ones recorded.
- [ ] `aivolution lint .` clean.
