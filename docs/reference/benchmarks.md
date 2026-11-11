# Benchmarks

Measuring instruments, not gates (CLM-0034). `tests/benchmarks.rs` holds
tests that are ignored by default: each builds a program from one of the
examples, times the `claimr` binary on it and asserts only what the program
must answer (the solution counts), then prints the wall time. Run them
against a release build, **serially** — two timings running in parallel
skew each other:

```bash
cargo test --release --test benchmarks -- --ignored --nocapture --test-threads=1
```

Record a run here when a release is cut or when the evaluator's search or
store changes in a way that should show; keep the machine the same across
rows, or say which machine a row is.

## N queens — `examples/queens.claimr`

**What it measures.** Constrain-then-search over the rational store: the N
columns are unknowns, every pair of rows gets its three disequations before
any value is chosen, and the search picks values from `1..N` through
`member/2`. Each candidate value is a resolution step plus the store's
check of the disequations that mention it (decided the moment both sides
are determined, CLM-0024); nothing is pruned ahead of the search, since
claimr has no integer domains and no propagation. The numbers therefore
measure the machine's resolution loop, the store's incremental checks and
the cost of a choice point — and set the baseline that a finite-domain
layer would have to beat
(`docs/incubation/2026-11-11-integer-domains-and-propagation.md`).

| version | 8 queens, first | 10 queens, first | 12 queens, first | 8 queens, all 92 |
|---|---|---|---|---|
| 0.5.0 + CLM-0034 (2026-11-11) | 70 ms | 122 ms | 513 ms | 1.24 s |

Machine for every row unless noted: Apple M4 Pro, macOS 27.0.1, rustc
1.95.0, release profile; two serial runs agreed within 2 %. Each figure
includes the process start and the parse of the program, a few
milliseconds.

**Reading the row.** A first solution grows about sevenfold from 8 to 12
queens, as the backtracking search grows; all 92 solutions of 8 queens
visit the whole search space and cost about eighteen times the first. Both
are the shape of backtracking with constraint checking, and both are what
propagation over integer domains would change.
