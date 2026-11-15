---
date: 2026-11-15
type: journal
components: [evaluator]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-15-clm-0035-indexing-on-other-arguments.md
design:
  - docs/design/2026-11-15-indexing-on-other-arguments.md
meta-impact:
  note: "A join through any argument of a predicate with eight or more clauses is now linear in the facts, so a consumer that stated a relation twice (`n_by(N, Id)` beside `n(Id, N)`) to join through its second argument can drop the second copy once this is released: answers and their order are unchanged. Under `--json --explain`, a `no-clause` reason gains `argument`, the 1-based position of the argument whose index narrowed the candidates, present whenever `candidates` is smaller than `clauses` (the first argument included) and absent otherwise; `candidates` can be smaller than on 0.6.0 for a call bound beyond its first argument. In human mode a failed call can read `none of N clauses can match argument 2`; the first-argument sentences are unchanged. `Failure` gains the public field `argument`. Not yet released: the changelog entry follows in the release commit."
---

# CLM-0035 executed: indexing on arguments other than the first

- **When:** 2026-11-15 02:35:39

## Context

The record accepted earlier today (PR #89) ends the *Not yet* that
CLM-0021 left: a call bound on any argument but the first visited every
clause, so a join through a second argument was quadratic. A re-run of
the integrity-rule experiment on 0.6.0 had shown rules that bind an id
first to be linear, and this join to be what remains. The record rested
on a spike; this task landed it.

## Details

**What was built.** `Pred` keeps `others: Vec<OnceCell<ArgIndex>>`, a slot
per argument position after the first, sized from the predicate's arity
when the program is compiled and filled by `ArgIndex::build` in one pass
the first time a call consults that position. `Pred::candidates_by` starts
from the first-argument list and, while more than one candidate remains,
consults each further bound position left to right, keeps the shortest
list, and returns it with the position that gave it; a tie keeps the
earlier position, and a predicate under eight clauses returns the
first-argument list at once. `arg_key_at` in the machine is CLM-0021's key
function at any position. `FailDetail::NoClause` and `Failure` carry
`argument`, reported only when the candidates are fewer than the clauses;
the two "cannot match" sentences name `argument N` for a later position
and keep "the first argument" otherwise; `json_out` writes the field.

**Measured** (release build, all answers; Apple M4 Pro, macOS 27.2.1,
rustc 1.100.0).

| Join through the second argument | 0.6.0 | now |
|---|---|---|
| 1 820 facts | 0.55 s | 0.01 s |
| 3 640 facts | 2.20 s | 0.03 s |
| 7 280 facts | 8.41 s | 0.05 s |
| 29 120 facts | (not run) | 0.22 s |

Answers are identical to 0.6.0's at every size both ran. Where the first
argument is bound nothing moves: four integrity rules over 104 728 and
418 912 facts take 0.63 s and 2.53 s against 0.64 s and 2.56 s, and the
queens benchmark reads 69, 122 and 517 ms for a first solution of 8, 10
and 12 queens.

**Verified.** 220 tests pass. Five new unit tests in
`other_argument_indexing`: answers and clause order for calls bound on
the second and third arguments over a predicate mixing constants,
numbers, compounds and variables there, every expectation read off 0.6.0;
an index built only when a call needs it, observed through a test-only
count of built slots; a seven-clause predicate building none; the failure
detail for the first argument, a later one, the shorter list and a tie;
and a tabled closure walked against a relation's direction over a cyclic
graph. One new JSON test for `argument` at the command line.
`examples/directions.claimr` puts a later-argument lookup under the golden
run. `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and
`aivolution lint .` are clean.

**What moved.** One existing expectation, as decided at acceptance: the
JSON failure document for `?- k(c99, K).` gained `"argument":1`. No
human-mode line changed; `examples/failure.answers` gains the three lines
of its new query.

**Findings.**

- A golden file cites line numbers. A comment added at the head of
  `examples/failure.claimr` renumbered every clause its answers file
  cites; the note went beside the new facts, after the existing clauses.
- The first run of a freshly built binary is slow: the benchmark's first
  process took 262 ms where the same program then took 30 ms. The join
  benchmark takes one unmeasured run first.
- `argument` means narrowed, not consulted: a bound first argument that
  every clause could match reports none.
- The changelog entry is not in this change. The plan listed it; this
  repository writes it in the release commit.

**Left for later**, as the record says: a compact form for single-clause
lists (the worst case measured about 260 bytes per clause), lookups by
range, and keys below an argument's principal functor.

**Pre-write checks.** The task's three `affects` docs were updated in the
same change: `components/evaluator.md` (the per-position indexes, the
lookup, *Not yet*; `last-verified` bumped), `reference/json-output.md`
(`candidates` redefined, `argument` added) and `reference/benchmarks.md`
(the join's rows). The `answer-soundness` invariant holds: each list is a
superset of the clauses that can unify and is in program order, and no
store operation changed; the tests compare answers with 0.6.0's.
Cross-repo impact: noted above; no listed contract covers claimr's JSON
output.

## Links

- Task: `docs/implementation/2026-11-15-clm-0035-indexing-on-other-arguments.md`
- Design record: `docs/design/2026-11-15-indexing-on-other-arguments.md`
- Acceptance: `docs/journal/2026-11/2026-11-15-design-indexing-on-other-arguments.md`
- Change proposal: https://github.com/a1v0lut10n/claimr/pull/90
- Predecessor: `docs/implementation/2026-11-09-clm-0021-first-argument-indexing.md`
