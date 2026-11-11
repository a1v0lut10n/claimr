---
date: 2026-11-11
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-11-step-limit-for-queries.md
---

# Design record accepted: a step limit for queries

- **When:** 2026-11-11 00:13:18

## Context

Tabling made declared recursion terminate; everything undeclared still
runs under SLD, and an SLD search over cyclic data or over a definition
that generates without end does not stop. The tabling incubation note had
named "a documented depth or step limit that reports non-termination as an
error, never as a hang" as the smaller alternative, and the record kept it
available if hangs in untabled code proved a problem in practice. Two days
of practice settled it: the CLM-0027 test suite waited two minutes on a
left recursion when the REPL swallowed its directive, and a claimr run
inside another program hangs with the query. Asked which deferred item
mattered most, I put this one first, because the point of embedding
claimr is that every query comes back with answers or an error, and
because it is cheap: the machine already counts its steps to poll the
interrupt flag. The record landed as PR #72 and is accepted here.

## Details

**Decision.** A query may be given a budget of *resolution steps*, one
iteration of the machine loop, the count the interrupt poll keeps; steps
inside sub-searches and table evaluations are steps of the query, one
counter. The budget covers the whole query across all its answers, so it
bounds work where the answer limit bounds output, and the two compose.
Reaching it is an ordinary runtime error, `EvalError::StepLimit { steps,
goal }`, naming the count and the goal the machine was about to resolve,
with the remedy in the message: batch mode keeps the answers found before
it, `--json` emits the existing error document with no new field, the
REPL keeps the session, incomplete tables are abandoned as on any stop,
and `interrupted()` stays false. Unlimited by default: `--steps N` and
`:steps N` (0 = unlimited) set it, mirroring `--limit` and `:limit`, with
`Solutions::with_step_limit(n)` in the API. The check is exact and one
comparison on the hot loop.

**Why steps.** A time limit is not reproducible across machines and
loads; a depth limit misses a search that backtracks forever at bounded
depth, and the machine's depth is heap memory, not the resource that runs
out. Prolog III has only the stack overflow its manual's own transitivity
example ends in; SWI-Prolog's inference limit and ISO's `resource_error`
are the prior art. The alternatives weighed in the record: a default
bound (rejected: a legitimate long query, such as the right-recursive
closure of a 4 000-node chain, would fail on a number nobody chose); a
per-answer budget (rejected in favour of the whole query, which is what
bounds a subprocess); a per-goal budget inside the language and a
machine-readable `kind` on the JSON error object (deferred, the latter
until a consumer asks for it).

**Pre-write checks.** A design record lists no `affects` and closes no
task. `answer-soundness` is committed by the implementing task to one
added sentence under invariant 3: a stop under a caller's step budget is
a resource error, not a judgment about the query, and is never reported
as `false`; the invariant's meaning is unchanged. No cross-repo impact
from the decision alone: the JSON error document keeps its shape, and the
new message is noted in `json-output.md` when the task lands.

## Links

- Design record: `docs/design/2026-11-11-step-limit-for-queries.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/72
- Where it was deferred: `docs/design/2026-11-10-tabling-for-recursive-rules.md`, item 7
- The hang that prompted it:
  `docs/journal/2026-11/2026-11-10-execution-clm-0027-tabling.md`
- Implementing task: to follow as CLM-0029
