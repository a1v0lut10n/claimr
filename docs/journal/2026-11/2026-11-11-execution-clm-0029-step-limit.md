---
date: 2026-11-11
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-11-clm-0029-step-limit.md
design:
  - docs/design/2026-11-11-step-limit-for-queries.md
meta-impact:
  note: "claimr gains `--steps N`, a budget of resolution steps per query; a query over budget is the existing `--json` query error document (shape unchanged) whose message opens with the fixed phrase `step limit of N reached at`. aicogito's reasoning adapter can set the flag to bound the subprocess and recognise the phrase to tell a budget stop from a program error; a machine-readable `kind` on the error object is deferred until a consumer asks. The suggestion from CLM-0015 stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0029 executed: a step limit for queries

- **When:** 2026-11-11 11:14:39

## Context

The record accepted this morning (PR #73) gives a query a budget of
resolution steps, so that an untabled recursion over cyclic data, or a
definition that generates without end, is reported rather than hung. The
machine already counted its steps to poll the REPL's interrupt flag, so
the task was to make that count a bound, give the stop an error that
points at the remedy, and expose the budget on the command line and at
the prompt. It landed as PR #74 on `feature/CLM-0029-step-limit`.

## Details

**What was built.** `Solutions` keeps `step_limit: Option<u64>`, set by
the builder `with_step_limit(n)` and read by nobody else; `steps()`
reports the count so far. The former interrupt check is now
`account_step`: it increments the count, stops the search with
`EvalError::StepLimit { steps, goal }` once the count exceeds the budget,
and polls the interrupt flag every 256 steps as before. The budget is one
counter for the whole query across all its answers, sub-searches and
table evaluations included, checked exactly on every step with one
comparison. The error's `goal` is the head of the goal list at that
moment: a pending call or a tabled abstract call is built in the store
and rendered with the query's variable names (the query stops, so the
allocation is harmless); a constraint goal, a commit, a collected
solution or a table answer is named by kind; an empty list is "an
answer". The message reads `step limit of N reached at `anc(a, _1)`; the
query may not terminate (an untabled recursion over cyclic data?) — raise
the limit, or table the predicate`. The stop is not an interrupt, so
`interrupted()` stays false and `error()` carries the limit; tables under
evaluation are abandoned as on any stop. `--steps N` is parsed like
`--limit`, applied in both batch paths and passed to the REPL to seed
`:steps N` (0 = unlimited), which sits beside `:limit` in the help. Batch
keeps the answers printed before the error, writes the error with the
query to stderr and exits 1; `--json` emits the query's error document in
the existing shape; the REPL prints the error and keeps the session. The
default is unlimited, so a program run without the flag behaves exactly
as before, which the unchanged golden files and REPL suite show.

**What was verified.** Four unit tests: the limit on an untabled left
recursion over a two-node cycle names `anc(a, _1)` and is not an
interrupt; the budget is exact, a query that takes `steps()` steps
succeeding under that budget and failing under one less, and spans the
answers; a generator yields `X = a` and then the error at `loop(X)`; a
tabled `nat/1` with infinitely many ground answers stops and its table is
abandoned, so the next query starts clean. One integration test in each
of the batch, JSON and REPL suites: `--steps` with and without `--limit`,
a bad value as a usage error; the error document; `--steps` seeding
`:steps`, the budget across `;`, `:steps 0`, the session kept after the
error. 140 unit tests, 191 in all, every golden file byte-identical, fmt,
clippy and the docs linter clean.

**Findings.** The pipe-driven REPL echoes the query line before its
answers, which the new REPL test first forgot; nothing to change. The
goal named is the next one, not the last: the budget is checked before a
step is taken, so the message points at what the machine was about to do.
A constraint or control goal is named by kind rather than rendered, since
the message is a pointer, not an explanation.

**Pre-write checks.** The four docs in the task's `affects` are current:
`evaluator.md` (the budget beside the interrupt), `answer-soundness.md`
(invariant 3 gains the sentence the record asked for: a stop under a
caller's step budget is a resource error that names its count and goal,
not a judgment about the query; the answers found before it stand and
nothing is reported `false`), `json-output.md` (the message in the
runtime-error section, shape unchanged), the architecture README (the
`cli` row). `answer-soundness` is strengthened, not changed: no invariant
is restated, and the step-budget tests are listed under invariant 3. The
REPL design record is not amended, as for `:explain` and `:- table`.
Cross-repo impact is the `meta-impact` note.

## Links

- Task: `docs/implementation/2026-11-11-clm-0029-step-limit.md`
- Design record: `docs/design/2026-11-11-step-limit-for-queries.md`
- Acceptance entry:
  `docs/journal/2026-11/2026-11-11-design-step-limit-for-queries.md`
- PR: https://github.com/a1v0lut10n/claimr/pull/74
- Commits: `e2c80f0` (plan), `3c33a46` (implementation)
- The hang that prompted the record:
  `docs/journal/2026-11/2026-11-10-execution-clm-0027-tabling.md`
