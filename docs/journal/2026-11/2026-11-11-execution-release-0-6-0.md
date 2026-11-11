---
date: 2026-11-11
type: journal
components: [cli, evaluator]
aspects: []
meta-impact:
  note: "claimr 0.6.0 carries everything since 0.5.0: every `--json` error object has `kind` (open: `step-limit`, `non-linear`, `unsatisfiable-facts`, `non-ground-condition`, `syntax`, `read`, …), `class` (closed: `syntax`, `program`, `unsupported`, `resource`, `io`, `internal`) and the error's data as fields (`steps`, `goal`, `conflict`, `constraint`, …) ahead of the unchanged `message`; `docs/reference/json-output.md` carries the table. aicogito's reasoning adapter can install 0.6.0 and route `class` to its five outcomes — `resource` → `ResourceLimit`, `unsupported` → `Unsupported`, the rest `Rejected` — in place of mapping every error to `Rejected`; the meta-impact note of CLM-0033 is reachable from this version. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Release 0.6.0: errors a program can route on, and N queens

- **When:** 2026-11-11 22:34:48

## Context

The third release of the day. 0.5.0 shipped the explanations of failure
this evening; since then the JSON error object learned to say what kind
of error it carries, which aicogito's reasoning adapter — mapping every
claimr error to `Rejected` for want of anything but the message — is the
named consumer of, and the N queens example landed with a benchmark and
the first incubation note toward a finite-domain layer. The error fields
reach the adapter only through a release.

## Details

Release branch `release/0.6.0`, one commit as before. The crate is
bumped to 0.6.0 and the changelog gains its entry:

- *Added*: `kind` and `class` on every JSON error object with the
  error's data as fields (CLM-0033); the N queens example with its README
  section, golden run, benchmark tests and recorded numbers (CLM-0034).
- *Changed*: the API additions — `EvalError::kind()`, `class()`,
  `fields()`, `ErrorClass`, `ErrorValue`, the error writers taking the
  error.

The README's JSON bullet names the kind and class. Programs written for
0.5.0 run unchanged and print the same; every existing JSON field keeps
its name, value and place.

Verified: `cargo fmt`, `clippy -D warnings`, 214 tests (two benchmarks
ignored), `aivolution lint` 106 files clean; `cargo publish --dry-run`
packaged 82 files and verified without a warning. Tagging `v0.6.0` on
the merge commit and `cargo publish` follow the merge, as before.

## Links

- Commit f6f5c1e on `release/0.6.0`; the PR; tag `v0.6.0` (after merge)
- `CHANGELOG.md`
- Previous release: `2026-11-11-execution-release-0-5-0.md`
- Execution entries CLM-0033 and CLM-0034 in `docs/journal/2026-11/`
- Consumer: aicogito `crates/aicogito-reason` (`claimr --json`)
