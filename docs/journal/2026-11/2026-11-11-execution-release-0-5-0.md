---
date: 2026-11-11
type: journal
components: [cli, evaluator]
aspects: []
meta-impact:
  note: "claimr 0.5.0 carries everything since 0.4.0 (this morning): explanations of failure under `--explain` — the `--json` outcome-none document gains `failure` (`{\"branches\": [[node, …], …], \"omitted\": N}`, failed nodes with `\"by\": \"failed\"`, a `reason` and its detail), the load error for unsatisfiable constraint facts names a minimal conflicting set in its `message`, and a `constraint` node's `goal` string is the goal as written with the bindings. Document shapes unchanged; documents without the flag byte-identical. aicogito's reasoning adapter gets the conflict sets by installing 0.5.0 and passing `--explain`; the meta-impact notes of CLM-0030, CLM-0031 and CLM-0032 are reachable from this version. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Release 0.5.0: explanations of failure reach the crate

- **When:** 2026-11-11 21:00:26

## Context

0.4.0 shipped this morning with the explanations of answers; the
explanations-of-failure record was accepted at midday and implemented by
the evening in two tasks, with a rendering fix taken on before the
release so the explanations read as the program was written. The failure
tree is the feature the integrity checker was named for — "this claim
cannot be satisfied because of these three constraints" — and it reaches
aicogito's reasoning adapter only through a release; the load-error
message changed and `EvalError::InitialStoreUnsatisfiable` gained a
field, which fits a minor bump.

## Details

Release branch `release/0.5.0`, one commit as for 0.4.0. The crate is
bumped to 0.5.0 and the changelog gains its entry:

- *Added*: explanations of failure (CLM-0030, CLM-0031) — the failure
  tree under `--explain` and `:explain`, every derivation tried as a
  branch ending at the goal that failed and why, with a minimal conflict
  set verified by replay for an unsatisfiable step, at most 64 branches
  and the rest counted, the `failure` JSON field; the load error naming a
  minimal conflicting set of constraint facts.
- *Changed*: a constraint goal in an explanation renders as written
  (CLM-0032); the API additions; the clipboard-win lockfile fix.

The README's status paragraph no longer lists explanations of failure as
open and its layout tree names them. Programs written for 0.4.0 run
unchanged and print the same without `--explain`.

Verified: `cargo fmt`, `clippy -D warnings`, 212 tests, `aivolution
lint` 97 files clean; `cargo package --list` ships LICENSE, NOTICE,
CHANGELOG and README; `cargo publish --dry-run` packaged and verified
without a warning (the yanked clipboard-win is gone). Tagging `v0.5.0` on
the merge commit and `cargo publish` follow the merge, as for 0.4.0.

## Links

- Commit e9ea847 on `release/0.5.0`; the PR; tag `v0.5.0` (after merge)
- `CHANGELOG.md`
- Previous release: `2026-11-11-execution-release-0-4-0.md`
- Execution entries CLM-0030, CLM-0031, CLM-0032 in `docs/journal/2026-11/`
- Consumer: aicogito `crates/aicogito-reason` (`claimr --json`)
