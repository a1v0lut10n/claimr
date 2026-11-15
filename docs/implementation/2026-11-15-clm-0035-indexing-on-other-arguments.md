---
date: 2026-11-15
type: task
status: planned
affects:
  - docs/architecture/components/evaluator.md
  - docs/reference/json-output.md
  - docs/reference/benchmarks.md
components: [evaluator]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-15-indexing-on-other-arguments.md
tags: [CLM-0035, indexing, machine, performance, explanations, json]
---

# CLM-0035: Indexing on arguments other than the first

## Premise

A call bound on any argument but the first visits every clause of its
predicate, so a join through a second argument is quadratic in the number
of facts (0.54 s, 2.18 s, 8.34 s at 1 820, 3 640 and 7 280 facts on
0.6.0). The design record decides the remedy: an index per further
argument position, built the first time a call needs it; the call takes
the shortest list its bound arguments give; predicates under eight clauses
are left alone; and a failed call's explanation names the argument that
ruled the clauses out. **This task starts once the record is accepted.**

The spike (`spike/other-argument-indexing`, a local scratch branch, not
merged) implements the index and the lookup and is the starting point: 91
lines added across `compile.rs` and `machine.rs`, 160 unit tests passing,
every golden file byte-identical. It does not yet carry the explanation
wording, the JSON field, the tests or the docs.

## Shape of the implementation

**Compile** (`src/eval/compile.rs`). `ArgIndex { by_key, var_only }` for
one position, built by `ArgIndex::build(clauses, pos)` in one pass, each
key list merged with the variable-at-that-position clauses in program
order. `Pred` gains `others: Vec<OnceCell<ArgIndex>>`, one slot per
position after the first, sized in `Pred::finish` from the predicate's
arity. `Pred::candidates_by(key_of)` starts from the first-argument list
and, while more than one candidate remains, consults each further bound
position left to right, building its slot on first use and keeping the
shortest list; it also reports which position gave the list.
`Program::candidates` takes the key function and returns the list with
that position. The threshold is a named constant.

**Machine** (`src/eval/machine.rs`). `arg_key_at(store, call, pos)`
replaces `first_arg_key` and is the same function at any position. The two
call sites pass it as the key function. `FailDetail::NoClause` carries the
position beside `clauses` and `candidates`.

**Explanation** (`src/eval/answer.rs`). `Failure` gains the position. The
two sentences for "no clause can match" keep their text when the position
is the first and read `argument N` otherwise. `json_out` writes `argument`
on a `no-clause` reason whenever an index narrowed the candidates.

## Action items

1. Land the spike's index and lookup, with the position returned beside
   the list.
2. The explanation: the position through `FailDetail`, `Failure`, the text
   and the JSON field.
3. Tests:
   - answers and clause order are those of the unindexed machine for calls
     bound on the second and third arguments, over a predicate that mixes
     constants, numbers, compounds and variables at those positions
     (including `0.5` matching `1/2`, and a determined numeric variable
     treated as unbound);
   - laziness: no further index is built by a call whose first argument
     leaves one candidate, and one is built by a call that binds only a
     later argument;
   - the threshold: a seven-clause predicate builds none;
   - a tie keeps the earlier position, and the shorter list wins;
   - a tabled predicate over a relation walked against its direction
     gives the same table as on `main`;
   - the explanation text for a first-argument and a later-argument
     exclusion, and the JSON `argument` field in both cases.
4. An example: a relation read in both directions in `examples/`, with
   its `.answers` file, so the golden run covers a later-argument lookup;
   a failing query in `examples/failure.claimr` for the new sentence.
5. Benchmark: the second-argument join in `tests/benchmarks.rs` (ignored
   by default, asserting only the answer count), and its row in
   `docs/reference/benchmarks.md` for 0.6.0 and for this change.
6. Docs: `evaluator.md` (the per-position indexes, the lookup, *Not yet*
   without "indexing on other arguments"); `json-output.md` (`candidates`
   redefined, `argument` added); the README if it describes indexing;
   `CHANGELOG.md` under the next version.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test`; every existing golden file byte-identical.
- [ ] The join is linear: the benchmark's times at 1 820, 3 640 and 7 280
      facts within a few hundredths of a second of the spike's (0.01 s,
      0.03 s, 0.05 s).
- [ ] No regression where the first argument is bound: the four-rule
      integrity check and `examples/queens.claimr` within noise of 0.6.0.
- [ ] Answer files identical to 0.6.0's on the join at every size run.
- [ ] `aivolution lint .` clean; the three `affects` docs current.
