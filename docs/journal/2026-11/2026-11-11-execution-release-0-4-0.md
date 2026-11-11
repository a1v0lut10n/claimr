---
date: 2026-11-11
type: journal
components: [cli, evaluator, solver, parser, grammar]
aspects: []
meta-impact:
  note: "claimr 0.4.0 is the first release since 0.3.0 (2026-09-10) and carries everything CLM-0013 to CLM-0029 added: lists, strings, `_`, control in bodies, `--explain` / `:explain` (the `--json` answer object gains an `explanation` field), aggregation, `:- table`, and `--steps N` / `:steps N` whose over-budget error message opens with `step limit of N reached at`. The `--json` document shape is unchanged; constraint strings may now mention `|L|`, `X . Y`, `atom_chars` and `sum`. aicogito's reasoning adapter (`aicogito-reason`, `claimr --json file`) gets the step budget and tabling by installing 0.4.0 and passing `--steps`; the meta-impact notes of CLM-0015, CLM-0016, CLM-0017, CLM-0023, CLM-0025, CLM-0027 and CLM-0029 are reachable from this version. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Release 0.4.0: two months of language work reach the crate

- **When:** 2026-11-11 15:35:57

## Context

The published crate and the last tag were still 0.3.0, from 10
September, while main had gained 131 commits across sixteen tickets.
Every accepted design record had a done implementation plan and nothing
was left in incubation or catalyst, so a release was the most useful
next step: the one consumer, aicogito's reasoning adapter, runs the
installed `claimr` binary with `--json`, and the step limit (CLM-0029)
was written to end its hangs. None of that reaches it until a version
exists.

## Details

Release branch `release/0.4.0`, one commit as for 0.3.0. The crate is
bumped to 0.4.0. A `CHANGELOG.md` is introduced (Keep a Changelog
format) with a full 0.4.0 entry and short back-entries for 0.3.0, 0.2.0
and 0.1.0 at their tag dates:

- *Added*: lists as a constraint domain (CLM-0014/15/16), strings and
  type relations (CLM-0017), the anonymous variable (CLM-0020),
  negation, disjunction and if-then-else (CLM-0022), explanations
  (CLM-0023), aggregation (CLM-0025), tabling (CLM-0027), the step
  limit (CLM-0029).
- *Changed*: where `=` is numeric (CLM-0026); the performance work
  (fold ground arithmetic CLM-0013, tableau truncation CLM-0018,
  first-argument indexing CLM-0021, incremental determination
  CLM-0024); iterative term handling (CLM-0019); `--json` content, shape
  unchanged.
- *Fixed*: the shared symbol table (CLM-0028).

The README catches up so the crates.io page is accurate on publish:
features list, status paragraph, the `:explain` row the REPL table had
never gained, the layout tree. Programs written for 0.3.0 run unchanged.

Verified: `cargo fmt`, `clippy -D warnings`, 190 tests, `aivolution
lint` 89 files clean; `cargo package --list` ships LICENSE, NOTICE,
CHANGELOG and README. Tagging `v0.4.0` on the merge commit and
`cargo publish` follow the merge, as for 0.3.0.

## Links

- Commit d00f167 on `release/0.4.0`; the PR; tag `v0.4.0` (after merge)
- `CHANGELOG.md`
- Previous release: `2026-09-10-execution-json-output-quoted-atoms-and-0-3-0.md`
- Execution entries CLM-0013 … CLM-0029 in `docs/journal/2026-11/`
- Consumer: aicogito `crates/aicogito-reason` (`claimr --json`)
