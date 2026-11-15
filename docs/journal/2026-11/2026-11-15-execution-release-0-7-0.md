---
date: 2026-11-15
type: journal
components: [evaluator]
aspects: []
meta-impact:
  note: "claimr 0.7.0 carries everything since 0.6.0: a predicate with eight or more clauses is indexed on whichever argument a call binds, so a join through any argument is linear in the facts and a consumer that stated a relation twice to join through its second argument can drop the second copy; answers and their order are unchanged. Under `--json --explain` a `no-clause` reason carries `argument`, the 1-based position of the argument whose index narrowed the candidates, present whenever `candidates` is smaller than `clauses` (so existing first-argument failure documents gain `\"argument\":1`), and `candidates` can be smaller than on 0.6.0; every existing field keeps its name, value and place. `Failure` gains the public field `argument`. The meta-impact notes of CLM-0035 are reachable from this version."
---

# Release 0.7.0: a relation read in either direction

- **When:** 2026-11-15 02:43:16

## Context

0.6.0 made rules that bind an id first linear in the facts; a re-run of
the integrity-rule experiment confirmed it to 1.7 million facts and
measured what remained, a join through a second argument that was still
quadratic. CLM-0035 removed that today, from design record to merged
implementation (PRs #89 and #90). The change reaches a consumer only
through a release.

## Details

Release branch `release/0.7.0`, one commit as before. The crate is bumped
to 0.7.0 and the changelog gains its entry:

- *Added*: indexing on arguments other than the first (CLM-0035), with
  the join's before-and-after times; the `argument` field on a JSON
  `no-clause` failure; `examples/directions.claimr` and the join in
  `tests/benchmarks.rs`.
- *Changed*: a failed call names the argument that ruled the clauses out,
  and the candidate count can be smaller than on 0.6.0; existing
  `--json --explain` failure documents narrowed by the first argument gain
  `"argument":1`; the API addition `Failure::argument`.

The README gains a sentence on reading a relation in either direction,
with the new example. Programs written for 0.6.0 run unchanged and print
the same answers in the same order.

Verified: `cargo fmt`, `clippy -D warnings`, 220 tests (three benchmarks
ignored), `aivolution lint` clean; `cargo publish --dry-run` packaged 84
files and verified without a warning. Tagging `v0.7.0` on the merge commit
and `cargo publish` follow the merge, as before.

## Links

- The release commit on `release/0.7.0`; the PR; tag `v0.7.0` (after merge)
- `CHANGELOG.md`
- Previous release: `2026-11-11-execution-release-0-6-0.md`
- Design, acceptance and execution entries for CLM-0035 in
  `docs/journal/2026-11/`
