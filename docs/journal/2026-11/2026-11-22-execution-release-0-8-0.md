---
date: 2026-11-22
type: journal
components: [evaluator, cli]
aspects: []
meta-impact:
  note: "claimr 0.8.0 carries everything since 0.7.0, for a program that embeds the crate: `Program::undefined()` / `undefined_from(&Query)` and a `{\"warning\": …}` stderr document under `--json` (exit code and stdout unchanged; a consumer reading stderr only on failure sees nothing new); `Term`, `Binding`, `Answer::bindings` and `Answer::binding(&str)` beside the unchanged text fields; and the default feature `cli`, so genite's `genite-mind-rules` should depend on `claimr = { version = \"0.8\", default-features = false }` and may replace its equation-string parsing with `binding(\"Rule\")`. The JSON per-query documents are unchanged; aicogito's adapter needs nothing. The meta-impact notes of CLM-0036, CLM-0037 and CLM-0038 are reachable from this version."
---

# Release 0.8.0: the release for a program that embeds claimr

- **When:** 2026-11-22 13:16:44

## Context

genite's GEN-141 linked the crate on 2026-11-20, the first consumer to do
so instead of running the binary, and left three incubation notes. All
three were taken today: the undefined-call report as a design record and
CLM-0036 (PRs #93–#95), answers as values as a record and CLM-0037 (PRs
#96–#98), and the `cli` feature as CLM-0038 (PR #99). None reaches
genite until a release.

## Details

Release branch `release/0.8.0`, one commit as before. The crate is bumped
to 0.8.0 and the changelog's `[Unreleased]` section becomes the entry,
with a lead paragraph and the CLM-0036 item added beside the other two
(it had landed before the section existed):

- *Added*: undefined predicates reported (CLM-0036) — the compile-time
  list, the batch warning line, the `--json` warning document, the REPL's
  two moments; answers as values (CLM-0037) — `Term`, `Binding`,
  `Answer::bindings`, `Answer::binding`.
- *Changed*: the command line is a default feature, `cli` (CLM-0038);
  `default-features = false` is the engine alone, 34 crates instead of 51.

The crate-level doc in `lib.rs`, which still called evaluation future
work, now describes the language the crate is, `Term` and the `cli`
feature; the README already carried the 0.8 dependency line and the
binding example from the two tickets. Programs written for 0.7.0 run
unchanged and print the same answers in the same order; the only new
output is a stderr warning for a call that could never succeed.

Verified: `cargo fmt`, `clippy -D warnings`, 232 tests (three benchmarks
ignored), `aivolution lint` clean; `cargo publish --dry-run` packaged 84
files and verified. A pre-existing rustdoc warning in the solver (a public
doc linking a private item) is left for a chore. Tagging `v0.8.0` on the
merge commit and `cargo publish` follow the merge, as before.

## Links

- The release commit on `release/0.8.0`; the PR; tag `v0.8.0` (after merge)
- `CHANGELOG.md`
- Previous release: `2026-11-15-execution-release-0-7-0.md`
- Design, acceptance and execution entries for CLM-0036 and CLM-0037, and
  the execution entry for CLM-0038, in `docs/journal/2026-11/`
- Consumer: genite GEN-141, `rust/crates/genite-mind-rules`
