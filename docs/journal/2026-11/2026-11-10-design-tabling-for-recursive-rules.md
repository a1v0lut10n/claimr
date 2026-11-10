---
date: 2026-11-10
type: journal
components: [grammar, parser, evaluator, cli]
aspects: [grammar-authority, answer-soundness]
design:
  - docs/design/2026-11-10-tabling-for-recursive-rules.md
---

# Design record accepted: tabling for recursive rules

- **When:** 2026-11-10 08:25:55

## Context

The incubation note of 2026-11-08 came from rules over a knowledge graph,
where transitive predicates — part-of chains, lineage, broader-than — are
the norm and the data holds cycles. Under SLD the left-recursive
`ancestor` never terminates and the right-recursive one terminates on
acyclic data only, repeating its answers forever on a cycle; a
comprehension over either never finishes collecting. The note proposed
tabling, warned that constraints in answers make it research-grade, and
named a step limit as the smaller alternative. The record landed as
PR #68 and is accepted here. The incubation note is removed in this
change: a graduation moves the content, it does not copy it. Incubation
is empty for the first time since the slot was opened.

## Details

**Decision, seven points.** (1) A directive, `:- table ancestor/2.`, the
language's first, declares a tabled predicate; SLD stays the default.
(2) A table belongs to the program: keyed by the call's variant, computed
by a sub-machine over the world store alone, valid across backtracking,
queries and REPL steps, discarded when the clauses change; it holds
ground answers taken by `Store::snapshot` after `finalize`, as a
comprehension takes an instance, a non-ground answer being an error; it
is a set in insertion order. (3) A call consumes its table by unification
in the caller's store, so the caller's constraints filter the answers
exactly and no call is refused. (4) Completion by linear tabling: a
recursive variant call consumes the answers so far and marks a
dependency; the leading table re-runs until an iteration adds nothing.
(5) A sub-search — condition, negation, comprehension — over a table
under evaluation is an error, extending invariant 5. (6) Each answer
records its first derivation, shown under a `tabled` node, keeping
invariant 6. (7) Deferred: answer subsumption and modes, constrained
answers, negation through recursion, incremental tabling, a step limit
for untabled code.

**Why these choices.** The world-store sub-machine is forced by
soundness: a tabled `old(bob) :- { age(bob) > 70 }` must not be memoised
as empty because one caller had posted `{ age(bob) < 60 }`. Ground
answers make consumption plain SLD over facts, so invariants 1 and 2
carry over unchanged. Linear tabling was chosen over SLG because the
iterative machine with a trail has no suspension machinery and the
answers are the same for definite programs; recomputation is the price.
The alternatives weighed: a step or depth limit (reports a hang, computes
nothing over a cycle); automatic tabling of every predicate (changes
answer multiplicity everywhere and costs memory, where every tabled
Prolog declares); computing tables in the caller's store (unsound across
backtracking, the example above).

**Prolog III.** The manual's first example of control (pp. 162–163) is the
transitive `plus_grand` program, which loops to a stack overflow; the
stated remedy is to remove the left recursion. No memoisation exists
there, so tabling is a claimr extension in the sense attribute terms are.

**Pre-write checks.** A design record lists no `affects` and closes no
task. `grammar-authority` is bound to its order for the directive:
grammar, actions, example, EBNF view, README. `answer-soundness` is
committed to two stated properties by the implementing task — a table's
content is a function of the program, and a sub-search is decided on
complete tables only — with invariants 5 and 6 restated. Cross-repo
impact is confined to the implementing task: `by: "tabled"` will be a
new value in `--json` explanations with the document shape unchanged,
noted there when it lands.

## Links

- Design record: `docs/design/2026-11-10-tabling-for-recursive-rules.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/68
- Removed incubation note: `docs/incubation/2026-11-08-tabling-for-recursive-rules.md`
- Related records: `2026-11-09-negation-disjunction-and-if-then-else.md`,
  `2026-11-09-aggregation-list-comprehensions-and-sum.md`,
  `2026-11-09-explanations-of-answers.md`
- Implementing task: to follow as CLM-0027
