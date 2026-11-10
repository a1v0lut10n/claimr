---
date: 2026-11-11
type: design
status: proposed
components: [evaluator, cli]
aspects: [answer-soundness]
tags: [step-limit, termination, resource-bound, non-termination, inference-limit, cli, repl, prolog-iii]
---

# A step limit for queries: non-termination is reported, never a hang

## Context

Tabling (CLM-0027) makes a *declared* recursive predicate terminate on
finite data, cycles included. Everything undeclared runs under SLD as
before, and an SLD search over cyclic data, or over a definition that
generates without end, does not terminate: the natural left-recursive
`ancestor` without its directive, a right-recursive closure over a graph
with a cycle, `nat(s(X)) :- nat(X)` queried open, a tabled predicate with
infinitely many ground answers. The tabling incubation note named "a
documented depth or step limit that reports non-termination as an error,
never as a hang" as the smaller alternative, and the record kept it
available "if hangs in untabled code prove to be a problem in practice".
They have: the CLM-0027 test suite hung for two minutes when one
directive was swallowed by the REPL's `:` prefix, and a claimr run inside
another program (aicogito's reasoning adapter reads `--json` documents from
the subprocess) hangs with the query.

**What exists.** The machine (`machine.rs`) is an iterative loop: each
iteration takes one goal from the goal list — a call resolved against a
clause, a constraint posted, a built-in run — or retries a choice point.
`Solutions` already counts these iterations (`steps: u64`) to poll the
REPL's interrupt flag every 256 of them; Ctrl-C stops the search with no
further answers and no error, and the REPL prints `interrupted.`. A
runtime error (`EvalError`) stops the query: in batch mode the answers
already printed stand, the error goes to stderr and the run stops with
exit 1; in `--json` mode the query's document is the error document
`{"query", "outcome": "error", "error": {"message"}}` and the run stops. A
query that stops inside a table evaluation forgets every non-complete
table. `--limit N` caps the *answers* printed per query and seeds the
REPL's `:limit`; it bounds a query's output, not its work: a query with
one answer that is never found runs forever under any `--limit`.

**What Prolog III does.** Nothing: the manual's transitivity example
(pp. 162–163) "se termine par un débordement de pile" — a stack overflow
is the only stop, and a search that loops in bounded space has none.
Tabled and untabled Prologs since have added resource bounds outside the
language's semantics: SWI-Prolog's `call_with_inference_limit/3` (a count
of inferences, deterministic) and `call_with_depth_limit/3`; ISO's
`resource_error` exception. An *inference* count is the usual choice over
time, because it is reproducible across machines and loads, and over
depth, because a search that backtracks forever at bounded depth is as
infinite as one that recurses, and because the machine's depth is heap
memory, not the resource that runs out.

## Decision

1. **A query may be given a budget of resolution steps.** A *step* is one
   iteration of the machine's loop: a goal resolved, a constraint posted,
   a built-in run, or a choice point retried — the count `Solutions`
   already keeps. Steps inside a sub-search (a condition, a comprehension
   body) and inside a table evaluation are steps of the query; there is
   one counter. The budget is for the **whole query**, across all its
   answers: in batch mode across every answer printed, in the REPL across
   the answers stepped with `;`. A budget bounds the query's work, which
   is what an embedding caller needs; the answer limit bounds its output,
   and the two compose.

2. **Reaching the budget is a query error.** When the count reaches the
   budget the search stops with
   `EvalError::StepLimit { steps, goal }`, where `goal` is the goal the
   machine was about to resolve, rendered with the query's variable
   names. The message reads
   `step limit of N reached at `ancestor(a, _1)`; the query may not
   terminate (an untabled recursion over cyclic data?) — raise the limit,
   or table the predicate`. It is an ordinary runtime error: batch mode
   prints the answers found before it, then the error, exit 1; `--json`
   mode emits the error document in the existing shape, no new field, the
   message distinguishable by its fixed opening; the REPL prints the error
   and keeps the session, as for every runtime error. A stop inside a
   table evaluation discards the non-complete tables, as any stop does.
   A search stopped by the budget is not an interrupt: `interrupted()`
   stays false, `error()` carries the limit.

3. **Unlimited by default; set by the caller.** `--steps N` on the command
   line (batch and interactive) and `:steps N` at the prompt (`0` =
   unlimited, like `:limit`), `--steps` seeding `:steps` as `--limit` seeds
   `:limit`; `Solutions::with_step_limit(n)` in the API, a builder like
   `with_interrupt` and `with_explanations`. The default stays what it is
   today, no bound: a legitimate long query (the right-recursive closure of
   a 4 000-node chain consumes eight million table answers) must not start
   failing because of a number chosen for it, and the caller who needs the
   guarantee is the one who knows the budget. The REPL's Ctrl-C remains
   the interactive stop.

4. **The check is exact and cheap.** The budget is compared on every
   step, where the interrupt flag is polled every 256: one integer
   comparison on the hot loop, no allocation; the interrupt poll stays as
   it is. A budget of `N` stops the query at exactly its `N`-th step, so a
   program's behaviour under a budget is reproducible on any machine.

5. **Not in this record.** A time limit (not reproducible; the step count
   is its deterministic stand-in); a memory limit; a default bound; a
   per-goal budget inside the language (a `call_with_steps(G, N)`
   predicate) — the budget is a property of the run, set outside the
   program; a machine-readable `kind` on the JSON error object — the shape
   stays, and the field is added when a consumer asks for it rather than
   guessed at; a depth limit, rejected above.

## Consequences

- **Every query terminates under a budget**, with answers, `false`, or an
  error that says the budget ran out and where. An embedding caller sets
  `--steps` and `--limit` and gets a bounded subprocess; a test suite sets
  `--steps` and gets a failure instead of a hang. Untabled recursion is
  not made to terminate: the limit reports it, and the message points at
  the remedy, which is the directive.

- **Soundness.** Invariant 3 of `answer-soundness`, "nothing satisfiable
  is dropped as an error", is about the evaluator's own judgment: it does
  not fail a goal it cannot decide, nor reject at load time what it could
  run. A budget is a resource bound the caller declared, reached with a
  stated count and goal; the answers found before it are sound and are
  kept; nothing is reported *false*. The invariant gains a sentence saying
  so: a stop under a caller's step budget is a resource error, not a
  judgment about the query, and is never reported as `false`. Invariants
  1, 2 and 4 are untouched: no answer is produced by the stop.

- **Evaluator.** One field (`step_limit: Option<u64>`), one builder, one
  comparison in `check_interrupt` (renamed for what it now does), one
  error variant with the current goal rendered by the existing
  `render_one`. `evaluator.md` documents the budget next to the interrupt.

- **CLI.** `--steps N` parsed like `--limit`, passed to `Solutions` in both
  batch paths and to the REPL; `:steps N` and its line in `:help`; the
  usage string and README; `json-output.md` adds the step-limit message to
  the runtime-error section, shape unchanged; the architecture README's
  `cli` row lists the flag and the command. The REPL design record is not
  amended (append-mostly), as for `:explain` and `:- table`.

- **Harder.** Nothing: a program without `--steps` behaves exactly as
  today. A budget set too low for a legitimate query fails it with a count
  the user can raise.

- **One task** implements the flag, the command, the builder, the error,
  the message and tests: the limit reached on an untabled left recursion
  (the error names the goal; the answers before it are printed in batch
  mode, the error document replaces them in JSON mode as for any runtime
  error); an exact count (a budget of `N` on a query of `N` steps
  succeeds, `N - 1` fails); a tabled predicate with infinite answers
  stopped with its tables abandoned; the limit across `;` in the REPL;
  `--steps` with `--limit`; `:steps 0`; `interrupted()` false after a
  limit stop. `evaluator.md`, `answer-soundness.md`, `json-output.md`, the
  README and the architecture README follow.

## Sources

- `docs/design/2026-11-10-tabling-for-recursive-rules.md`, item 7 ("a
  step or depth limit for untabled recursion … stays available as its own
  record") and the incubation note it graduated,
  `2026-11-08-tabling-for-recursive-rules.md` ("a documented depth or step
  limit that reports non-termination as an error, never as a hang").
- `docs/journal/2026-11/2026-11-10-execution-clm-0027-tabling.md`: the
  REPL `:` prefix finding, a hang in the test suite.
- Prolog III manual, Version 1.3 (PrologIA, 1991), pp. 162–163: the
  transitivity example ends in a stack overflow.
- SWI-Prolog reference manual, `call_with_inference_limit/3` and
  `call_with_depth_limit/3`; ISO/IEC 13211-1, `resource_error`.
- `docs/design/2026-08-18-repl-interaction-model.md`: `:limit`, Ctrl-C,
  and the rule that a runtime error keeps the session.
- `docs/reference/json-output.md`: the runtime-error document.
