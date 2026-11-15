---
date: 2026-11-15
type: task
status: done
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
ruled the clauses out. The record was accepted on 2026-11-15.

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
   **Done:** `examples/directions.claimr` (a backwards walk, a tabled
   closure over it, and a join on a shared value); `owner/2` and
   `?- owner(C, dee).` in the failure example.
5. Benchmark: the second-argument join in `tests/benchmarks.rs` (ignored
   by default, asserting only the answer count), and its row in
   `docs/reference/benchmarks.md` for 0.6.0 and for this change.
6. Docs: `evaluator.md` (the per-position indexes, the lookup, *Not yet*
   without "indexing on other arguments"); `json-output.md` (`candidates`
   redefined, `argument` added); the README if it describes indexing;
   `CHANGELOG.md` under the next version. **Done**, except the changelog:
   this repository writes its entry in the release commit, so the entry
   waits for the next release.

Items 1, 2, 3 and 5 are done as written.

## Findings

- **The spike landed almost unchanged.** The index and the lookup are the
  spike's; the task added the position returned beside the list, which is
  all the explanation needed.
- **`argument` means "narrowed", not "consulted".** It is reported only
  when the candidates are fewer than the clauses. A bound first argument
  that every clause could match (all variable-first clauses, say) reports
  none, which is what the record's "present whenever an index narrowed
  them" says.
- **One existing expectation moved, as decided.** The JSON failure
  document of `tests/json_output.rs` for `?- k(c99, K).` gained
  `"argument":1`. No human-mode line changed.
- **A golden file cites line numbers.** A comment added at the head of
  `examples/failure.claimr` renumbered every cited clause in its answers
  file; the note sits beside the new facts instead, after the existing
  clauses, so the file gains three lines and changes none.
- **Only the leaf call site reports.** The machine has two places that
  look up candidates; the failure detail is built at one of them, and the
  other discards the position.
- **The first run of a new binary is slow.** The benchmark's first
  process took 262 ms where the same program then took 30 ms, the
  operating system's first-start cost; the test takes one unmeasured run
  first.
- **Left for later, as the record says:** a compact form for
  single-clause lists (the worst case measured about 260 bytes per
  clause), lookups by range, and keys below the principal functor.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` (165 unit tests, five new; one new JSON test); every
      existing golden line byte-identical — `failure.answers` gains the
      three lines of its new query, `directions.answers` is new.
- [x] The join is linear: 0.01 s, 0.03 s and 0.05 s at 1 820, 3 640 and
      7 280 facts on the binary (0.55 s, 2.20 s, 8.41 s on 0.6.0), and
      0.22 s at 29 120.
- [x] No regression where the first argument is bound: four integrity
      rules over 104 728 and 418 912 facts take 0.63 s and 2.53 s against
      0.64 s and 2.56 s on 0.6.0; the queens benchmark is unchanged.
- [x] Answers identical to 0.6.0's on the join at every size run, on the
      integrity rules, and on every example that 0.6.0 also has except
      `failure.claimr`, which has a new query.
- [x] `aivolution lint .` clean; the three `affects` docs current.
