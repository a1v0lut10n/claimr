---
date: 2026-11-11
type: journal
components: [cli, evaluator]
aspects: []
tasks:
  - docs/implementation/2026-11-11-clm-0034-queens-example-and-benchmark.md
---

# CLM-0034 executed: N queens as an example, a benchmark and a compass

- **When:** 2026-11-11 22:25:36

## Context

Asked how feasible N queens would be as a README example in the manner
of the OR-Tools CP-SAT guide, the honest answer was: twenty lines, fast
enough for a dozen queens, and a clear picture of what claimr does not
have — integer domains and propagation. The ticket took all three uses of
that picture: a program for the README and the golden tests, a benchmark
whose numbers are recorded and reproducible, and the first entry of the
idea pipeline toward a finite-domain layer, with the benchmark as the
measuring stick it would have to beat.

## Details

**What was built.** `examples/queens.claimr`: the N columns are unknowns,
`safe/1` posts the three disequations of every pair of rows before any
value is chosen, and `place/2` assigns values from `range(1, N)` through
`member(V, Values), { Q = V }` — a constraint goal rather than head
unification, so that under `--explain` a rejected value is an
`unsatisfiable` leaf naming the constraint it breaks instead of being
folded into a `no clause` leaf by CLM-0031's aggregation rule. The golden
file pins 4 queens (2 solutions), 5 (10), 6 (4) and 3 (`false`), sizes a
debug build runs in well under a second. `tests/benchmarks.rs` holds two
ignored tests that time the release binary on the same program for 8, 10
and 12 queens (first solution) and all 92 solutions of 8, asserting only
the counts; `docs/reference/benchmarks.md` records the run and says what
it measures; `docs/reference/README.md` lists it.
`docs/incubation/2026-11-11-integer-domains-and-propagation.md` captures
the idea — integer domains, `all_different`, propagation, labeling — with
the queens numbers as evidence, what exists today (exact rationals,
disequations decided on determination, Prolog III's enumeration-only
position), the records that deferred integer constraints, and the open
questions. The README gains an *N queens* block under Examples with the
program, the answers for 4, one branch of the 3-queens failure tree and
the framing: backtracking with constraint checking, not a finite-domain
solver.

**Measured.** Release build, Apple M4 Pro, two serial runs within 2 %: a
first solution for 8 queens in 70 ms, 10 in 122 ms, 12 in 513 ms; all 92
solutions of 8 in 1.24 s. A first solution grows about sevenfold from 8
to 12, the whole 8-queens space costs about eighteen first solutions —
the shape of backtracking search, which is what propagation would change.

**Findings.** Two timings in parallel skew each other: cargo ran the two
benchmark tests on two threads and the 8-queens first solution read
358 ms beside the all-solutions test, 70 ms alone; the documented command
passes `--test-threads=1`. The test harness adds a few milliseconds of
process start, temp file and parse over a shell run (1.24 s against
0.93–1.11 s); the recorded numbers are the reproducible ones. Assignment
by head unification hides the conflicts: with `member(Q, Values)` the
3-queens failure tree has 7 branches, 6 of them `member(_1, <>)` with no
clause; with the constraint goal it has 20 branches naming the attacking
queen, 989 lines, which is why the README quotes one branch.

**Pre-write checks.** The task's one `affects` doc, `docs/reference/README.md`,
lists the new page. No aspect invariant is touched: nothing in the
evaluator changed. No cross-repo impact: an example, a test and docs.

## Links

- Task: `docs/implementation/2026-11-11-clm-0034-queens-example-and-benchmark.md`
- Incubation: `docs/incubation/2026-11-11-integer-domains-and-propagation.md`
- Benchmarks: `docs/reference/benchmarks.md`, `tests/benchmarks.rs`
- PR #87: commits 7148d57 (plan), 59cf272 (work)
- The model it answers to: Google OR-Tools, *N-queens* (CP-SAT guide)
