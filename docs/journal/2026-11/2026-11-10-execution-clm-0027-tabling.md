---
date: 2026-11-10
type: journal
components: [grammar, parser, evaluator, cli]
aspects: [grammar-authority, answer-soundness]
tasks:
  - docs/implementation/2026-11-10-clm-0027-tabling.md
design:
  - docs/design/2026-11-10-tabling-for-recursive-rules.md
meta-impact:
  note: "Under --explain, the `by` field of an explanation node gains the value `tabled` (its subgoals are the recorded derivation; document shape unchanged), and a program may contain the directive `:- table name/arity.`, which consumers that generate or parse claimr source should admit. aicogito's reasoning adapter reads unknown `by` values as data. The suggestion from CLM-0015 stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0027 executed: tabling for recursive rules

- **When:** 2026-11-10 21:40:33

## Context

The record accepted earlier today (PR #69) gives claimr its first
directive. A transitive predicate over cyclic data — `part_of` on a graph
with a cycle, `ancestor` written left-recursively — does not terminate
under SLD, and Prolog III's only remedy is to rewrite the program. The
record chose memoised answers for declared predicates: `:- table
name/arity.`, tables that belong to the program and are computed over the
world store alone, consumption by unification in the caller's store,
completion by linear tabling in the machine's own stack. This task
implemented it as PR #70 on `feature/CLM-0027-tabling`.

## Details

**What was built.** The grammar gains `Directive` and `Indicator`
productions (72 productions, 119 states, no conflict); the AST a
`Clause::Directive`, printed back as written. The compile step validates
directives with positioned errors (`BadDirective`: an unknown name, an
arity that is not a non-negative integer, a table on a built-in or the
list constructor), collects the tabled predicates, builds the world store
once and keeps the table registry behind a `RefCell`. The machine keeps a
single store and *swaps* it for an evaluation: the caller's store is
parked in an evaluation frame, a clone of the world takes its place, and
the evaluation's own choice point swaps it back, so no address crosses
stores. An evaluation is the goal list `[Resolve(abstract call),
TableOne]` with an empty tail; each solution is snapshotted and recorded
if new, then failed, so the search moves on. Completion is linear
tabling: a table is `Complete`, `Evaluating(depth)` or `Incomplete`; a
call into a table under evaluation marks the frame as looping and
consumes what is there; the leader re-runs while a pass added an answer,
and its dependents complete with it. Consumption reads the answer count
at retry time, so a consumer inside the recursion sees answers added
after it started. A call that would consume an incomplete table from
inside a condition or a comprehension body stops the query
(`IncompleteTable`), as the record requires. Explanations carry a
`tabled` node whose subgoals are the answer's first derivation, rendered
from the arena slice the evaluation produced. A query that stops inside an
evaluation discards every non-complete table and restores the store.

**What was verified.** 134 unit tests (eleven new: left and right
recursion on a cycle giving the same answers once each, variant keys with
bound and constrained arguments, the caller-store independence case
`old(bob)` against `{ age(bob) < 60 }`, mutual recursion, the non-ground
and constrained answer errors, negation and comprehension over a complete
table and the error inside the recursive group, directive errors, the
explanation node, a 400-node chain on a small stack, interruption inside
an evaluation); one new integration test each for parsing, the REPL and
the JSON vocabulary; the example `examples/tabling.claimr` with ten golden
queries; every existing golden file byte-identical; fmt, clippy and the
docs linter clean.

**Finding: ground is not enough.** The record's error for a non-ground
answer covers an unbound argument, but a ground call can hold only under a
constraint on a world unknown: `old(bob) :- { age(bob) > 70 }` with
nothing fixing the age has the ground solution `old(bob)` under
`age(bob) > 70`, and recording it as a fact would answer a later caller
holding `{ age(bob) < 60 }` wrongly — the very case the world-store
sub-machine exists to prevent. The implementation adds
`ConstrainedTabledAnswer`: a cheap store test (any pending disequation or
delayed relation, any attribute term touched, any undetermined numeric
variable) sends the solution to an exact one, the answer printer's
projection onto the call compared with the world's projection on the same
unknowns, so a world fact the body looked up is not a residual. The
record's "ground answers" is read as "unconditional ground answers",
which is what its argument needs; invariant 8 of `answer-soundness`
states it.

**Finding: quadratic chains.** A right-recursive closure over a chain of
*n* nodes builds *n* tables holding *n²/2* answers; a left-recursive one
runs *n* passes and re-consumes every answer on every pass. Both are the
strategy's nature: 2 000 nodes take 14 s in the debug build, so the stack
test uses 400. Semi-naive consumption, where the recursive call sees only
the answers added since the previous pass, is the known improvement for
the left-recursive case and is noted as future work in `evaluator.md`.

**Finding: the REPL's `:` prefix.** A line starting with `:` was a
command, so `:- table p/2.` was "unknown command" and the test's left
recursion looped under plain SLD until the harness was killed. `:-` lines
now fall through to the clause path. Tables in the REPL live for one
query, since the session is recompiled per query; in batch mode they live
for the file.

**Pre-write checks.** The five docs in the task's `affects` are current:
`grammar.md` (the Directives section and the tabling paragraph),
`json-output.md` (the `tabled` value), `evaluator.md` (the sub-machine,
the completion rules, the semi-naive note), `answer-soundness.md`
(invariant 5 restated so a sub-search over an incomplete table is
excluded; invariant 8: a table is a function of the program),
`architecture/README.md` (the `cli` and `evaluator` rows). The record is
in the `decisions` of `evaluator.md`. `answer-soundness` is strengthened,
not changed: an answer consumed from a table is one the program entails
without any caller's constraints, and the caller's own filter it by
unification. `grammar-authority` held: grammar, actions, example, EBNF view
in that order. Cross-repo impact is the `meta-impact` note.

## Links

- Task: `docs/implementation/2026-11-10-clm-0027-tabling.md`
- Design record: `docs/design/2026-11-10-tabling-for-recursive-rules.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-10-design-tabling-for-recursive-rules.md`
- PR: https://github.com/a1v0lut10n/claimr/pull/70
- Commits: `0f5065e` (plan), `50227f7` (implementation)
- Example: `examples/tabling.claimr`, `examples/tabling.answers`
- Record for the deferred items (semi-naive consumption, answer
  subsumption, constrained answers, incremental tabling): the design
  record's *Not in this record*
