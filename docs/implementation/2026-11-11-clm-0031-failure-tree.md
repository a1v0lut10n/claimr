---
date: 2026-11-11
type: task
status: done
affects:
  - docs/architecture/components/evaluator.md
  - docs/architecture/aspects/answer-soundness.md
  - docs/architecture/README.md
  - docs/reference/json-output.md
components: [evaluator, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-11-explanations-of-failure.md
tags: [CLM-0031, explanations, failure, failure-tree, json-output, repl]
---

# CLM-0031: The failure tree under `--explain`

## Premise

The accepted record `2026-11-11-explanations-of-failure.md`, decisions 2,
3 and 5, on the machinery of CLM-0030: under `--explain`, `false` is
followed by the query's failed derivations as *branches*, each the steps
taken, nested under their rules as in a proof tree, ending at the goal
that failed with its reason — unsatisfiable (with a minimal conflict set),
no clause (with counts; head failures aggregate into the call's leaf), has
a solution (a negation, carrying the solution's proof), no table answer.
At most 64 branches, the first in search order, the rest counted and
stated. Branches are discarded when an answer arrives. JSON: an
outcome-none document gains a `failure` field under the flag; byte-identical
without it.

## Shape of the implementation

**Where a derivation fails.** Every failure the machine can take outside a
sub-search or a table evaluation is a *leaf*: a call for which no candidate
unifies (`try_clauses` from position 0 returns false — a retry that runs
out is the end of alternatives, not a new derivation); a constraint goal or
a built-in the store rejects; the answer-time `finalize` rejecting the
store (the step is `answer`, with `Step::Answer` back in the enum); a
negation whose condition succeeded (`Then::Fail`), whose proof is the arena
slice since the construct opened; a tabled call whose complete table has no
unifying answer (`consume` from 0 returns false); and an if-then-else
without an else whose condition fails — observable only through a marker
choice point, `Retry::FailNote`, pushed under the condition when
explaining, dropped by the commit when the condition succeeds, recording
the leaf when backtracking pops it, and ignored by `may_continue`. The
reason `no-solution` is added to the record's four for that case. A
failure inside a condition, a comprehension body or a table evaluation is
not a branch (`enclosing_sub_search`, `eval_stack`), as the record defers.

**Recording a branch.** `Solutions::fail` gates on recording (explaining,
no answer yet, not in a sub-search), then on the budget (`FAILURE_BUDGET =
64`; beyond it `omitted` counts), computes the detail — for an
unsatisfiable step, `derivation_steps()` now returns the steps with their
*origins* (a fact index or an arena index) and `minimal_conflict` gives the
origins to cite — appends a `ProofKind::Failed` record under the failing
goal's parent, renders the whole arena with the branch's bindings
(`render_proof(0)`), stores the roots, and truncates the record away. The
failed node's citations are built in `render_proof` from the other
records' rendered nodes (shallow copies) and from the facts' text. An
oracle error (a non-linear residue a subset replay leaves pending) does
not stop the query: the leaf then cites every step as the conflict and says
`not minimised`.

**Public shape.** `ProofNode` gains `failure: Option<Box<Failure>>`
(`Failure { reason, conflict, clauses, candidates, answers, note }`),
`Resolution::Failed`, `Reason` (`unsatisfiable`, `no-clause`,
`has-solution`, `no-table-answer`, `no-solution`), and `FailureTree {
branches, omitted }` with `lines()` for the human format;
`Solutions::failure()` returns it after an exhausted search without an
answer, error or interrupt. Human: the record's format, with `branch N`
headers when there are several branches and `… and N more failed branches`
when truncated. JSON: `query_document` takes the optional tree and writes
`"failure":{"branches":[[node,…],…],"omitted":N}`; a failed node has
`"by":"failed"`, `"reason"`, and by reason `conflict`, `clauses` +
`candidates`, `answers`, or its solution under `subgoals`. The CLI prints
the lines after `false`, the REPL after `false.`, under `--explain` /
`:explain`.

## Action items

1. `answer.rs`: `Failure`, `Reason`, `Resolution::Failed`, `FailureTree`,
   the line renderer shared with answers; `json_out.rs`: the fields and
   the `failure` object.
2. Machine: `Failed` records, `fail`, the budget, origins from
   `derivation_steps`, the failure sites above, `FailNote`, `failure()`,
   discarding on an answer; the `expect(dead_code)` marker of CLM-0030
   goes.
3. CLI and REPL print the tree; `json_out::query_document` takes it.
4. Tests — unit (`failure_trees`): the record's example line for line; two
   branches with `branch N` headers; the three no-clause texts; a negation
   carrying its solution; a tabled call with no answer; an if-then-else
   without else; failures inside a negation are not branches; the budget
   truncates and counts; branches are discarded when an answer arrives;
   `may_continue` ignores the marker. Integration: `examples/failure.claimr`
   with `.args` and golden `.answers`; the JSON `failure` field and
   byte-identity without the flag; the REPL under `:explain`.
5. Docs: `evaluator.md`, `answer-soundness.md` (invariant 9 completed),
   `json-output.md`, README, architecture README (`cli` row); this plan.

## Findings

- **A failed goal renders as it stood.** A constraint goal's instance is
  rendered from its two built sides, as CLM-0023 renders a posted one, so an
  arithmetic term over variables still unbound when the goal failed shows
  as the lowered numeric variable: `{ X - Y = 0 }` failing before `X` and
  `Y` are known prints `{ _1 = 0 }`. The same holds for an answer's proof
  tree today; rendering the template instead is a refinement for both.
- **No program found that fails only at the answer-time check.** Every
  unsatisfiable store met in testing was rejected at a post, since the
  incremental determination pass (CLM-0024) decides a disequation as soon
  as its sides are determined. The `answer` leaf and `Step::Answer` are in
  place for a store the exact finalize alone rejects, but no test reaches
  them.
- **The condition's text is the goal of its leaves.** A negation's
  `has-solution` leaf and an if-then-else's `no-solution` leaf show the
  construct as written (`\+ claim(F, residual_error, _)`), as the
  `committed` and `no-solution` proof nodes of CLM-0023 do, with clause
  variables under their source names rather than the derivation's bindings.
- **The reason `no-solution` joins the record's four.** An if-then-else
  without an else whose condition fails is a failed branch the record did
  not list; the vocabulary gains the reason, and `json-output.md` pins it.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` (eleven new unit tests in `failure_trees`, the JSON
      integration test, the REPL test, the golden run of
      `examples/failure.claimr`); existing golden files byte-identical.
- [x] A program run without `--explain` behaves and prints exactly as before
      (the golden runs and the JSON byte-identity test).
- [x] `aivolution lint .` clean; the four `affects` docs current.
