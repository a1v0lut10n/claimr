---
date: 2026-11-22
type: journal
components: [cli]
aspects: []
tasks:
  - docs/implementation/2026-11-22-clm-0038-cli-feature.md
meta-impact:
  note: "The `claimr` crate's command line is now a default feature, `cli`, owning `rustyline` and `ctrlc`; an embedding program depends on `claimr = { version = \"0.8\", default-features = false }` for the engine alone (34 crates instead of 51). genite's `genite-mind-rules` (GEN-141) should switch to that form at 0.8.0; `cargo install claimr` and every binary consumer (aicogito's adapter) are unchanged."
---

# CLM-0038 executed: the command line is a default feature

- **When:** 2026-11-22 13:10:58

## Context

The third note from genite's embedding of claimr (2026-11-20): one crate
holds the library and the binary, and the REPL's line editor and the
Ctrl-C handler were plain dependencies, so a program that only parses,
compiles and solves built them and their trees too. A packaging change
with nothing to decide, so it went to a task directly, after the
undefined-call report (CLM-0036) and the answers as values (CLM-0037);
the three together are what 0.8.0 brings genite.

## Details

**What was built.** `[features] default = ["cli"]`,
`cli = ["dep:rustyline", "dep:ctrlc"]`, the two dependencies optional;
`required-features = ["cli"]` on the `[[bin]]` and on the four
`[[test]]` targets that drive the binary through `CARGO_BIN_EXE_claimr`
(`run_examples`, `json_output`, `repl`, `benchmarks`), the other tests
staying auto-discovered. No `cfg` in the sources: `main.rs`, `repl.rs`
and `json_out.rs` are the one binary target and go behind the feature
whole; `lib.rs` never named either dependency. The README's library
section shows `default-features = false`; the architecture README's
`cli` row and the `CHANGELOG.md` `[Unreleased]` section record it.

**Verified.** With the feature: fmt, clippy and the full suite as before
(232 pass, 3 benchmarks ignored), `cargo build` producing the binary,
`cargo package --list` unchanged at 84 files. Without it: `cargo build
--lib`, `clippy --all-targets`, and `cargo test --no-default-features`
running 174 library tests, `parse_examples`, `quoted_atoms` and the doc
test while skipping the binary's suites; `cargo tree` free of `rustyline`
and `ctrlc`.

**Findings.** Seventeen crates leave — `rustyline`, `ctrlc`, on macOS
the `objc2`, `dispatch2`, `block2` and `nix` stacks, `radix_trie`,
`home`, `log`, `smallvec`, `unicode-segmentation`, `utf8parse` — but a
clean library build on the M4 Pro is 6.7–7.2 s with the feature and
6.3–6.5 s without: the removed crates compile in parallel beside the
parser generation, the long pole, so the gain is dependency surface
(what an embedding program audits, vendors and rebuilds on a one-core
runner) more than wall-clock here. The binary's tests had to be declared:
`env!("CARGO_BIN_EXE_claimr")` fails to compile when the binary is not
built. The `json_out` unit tests live in the binary and are skipped with
it.

**Pre-write checks.** The task's two `affects` docs were updated in the
PR: the README and the architecture README. No aspect constrains `cli`.
Cross-repo impact: genite's dependency line, noted above.

## Links

- Task: `docs/implementation/2026-11-22-clm-0038-cli-feature.md`
- Graduated note: `docs/incubation/2026-11-20-a-library-without-the-command-line.md` (removed by the PR)
- PR #99: commits f6dc8fe (plan), 93c062a (change)
- Consumer: genite GEN-141, `rust/crates/genite-mind-rules/Cargo.toml` (`claimr = "0.7"` today)
