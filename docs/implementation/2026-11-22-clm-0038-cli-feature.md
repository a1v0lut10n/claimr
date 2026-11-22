---
date: 2026-11-22
type: task
status: done
affects:
  - README.md
  - docs/architecture/README.md
components: [cli]
aspects: []
tags: [CLM-0038, features, packaging, library, rustyline, ctrlc, genite]
---

# CLM-0038: The library without the command line's dependencies

## Premise

Incubation note `2026-11-20-a-library-without-the-command-line.md`
(graduated by this task): one crate holds the library and the binary, and
`rustyline` (the REPL's line editor) and `ctrlc` (interrupting a query)
are plain dependencies, so a program that only parses, compiles and
solves — genite's rule runner, GEN-141 — builds them and their trees as
well. Only `src/repl.rs` uses them. A packaging change with no semantics
to decide, so a task rather than a record.

## Shape of the implementation

**A default feature `cli`** owns the two dependencies (`optional = true`,
`cli = ["dep:rustyline", "dep:ctrlc"]`) and is required by the binary
(`required-features = ["cli"]` on `[[bin]]`), so that

```toml
claimr = { version = "0.8", default-features = false }
```

is the engine alone — parser, evaluator, solver, `Term`, `highlight` —
and `cargo install claimr`, `cargo build`, `cargo run` keep working
unchanged, the feature being on by default. No `cfg` inside the sources:
the binary (`main.rs`, `repl.rs`, `json_out.rs`) is one target and goes
behind the feature whole.

**The integration tests that drive the binary** (`tests/run_examples.rs`,
`tests/json_output.rs`, `tests/repl.rs`, `tests/benchmarks.rs`, through
`CARGO_BIN_EXE_claimr`) are declared as `[[test]]` targets with the same
`required-features`, so `cargo test --no-default-features` runs the
library's tests and skips them rather than failing to compile; Cargo
keeps discovering the other tests automatically.

## Action items

1. `Cargo.toml`: `[features]`, the two optional dependencies, the
   `[[bin]]` and `[[test]]` `required-features`.
2. Verify both shapes: default (the full suite as before) and
   `--no-default-features` (`build --lib`, `clippy --all-targets`,
   `test`); measure the dependency count and a clean library build with
   and without the feature.
3. Docs: README (the library section names `default-features = false`),
   the architecture README's `cli` row, `CHANGELOG.md` `[Unreleased]`
   (Changed), this plan; remove the incubation note.

## Findings

- **Seventeen crates fewer, half a second faster.** `cargo tree -e normal`
  lists 51 crates with the feature and 34 without; the ones that leave are
  `rustyline`, `ctrlc` and their trees — on macOS the `objc2`,
  `dispatch2`, `block2` and `nix` stacks, `radix_trie`, `home`, `log`,
  `smallvec`, `unicode-segmentation`, `utf8parse`. A clean `cargo build
  --lib` on the M4 Pro takes 6.7–7.2 s with the feature and 6.3–6.5 s
  without (two runs each, fresh target directory): the removed crates
  compile in parallel beside the parser generation, which is the long
  pole, so the gain is dependency surface — what an embedding program
  audits, vendors and rebuilds on a one-core CI runner — more than
  wall-clock here.
- **The binary's tests needed declaring.** A test that reads
  `CARGO_BIN_EXE_claimr` through `env!` fails to compile when the binary
  is not built, so the four such tests are `[[test]]` targets with
  `required-features = ["cli"]`; Cargo keeps discovering the others
  (`parse_examples`, `quoted_atoms`, …) on its own, and `cargo test
  --no-default-features` runs 174 library tests, the two discovered
  integration suites and the doc test, skipping the binary's four and
  the `json_out` unit tests that live in the binary.
- **No `cfg` in the sources.** `main.rs`, `repl.rs` and `json_out.rs`
  form the one binary target and go behind the feature whole; `src/lib.rs`
  never named either dependency.
- **`cargo package --list` is unchanged** (84 files).

## Verification

- [x] Default features: `cargo fmt --all -- --check`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test` as before
      (232 pass, 3 benchmarks ignored).
- [x] `--no-default-features`: `cargo build --lib`, `cargo clippy
      --all-targets --no-default-features -- -D warnings`, `cargo test
      --no-default-features` (library and doc tests; the binary's tests
      skipped), `cargo tree` without `rustyline`/`ctrlc`.
- [x] `cargo build` still produces the binary; `cargo package --list`
      unchanged.
- [x] `aivolution lint .` clean; the two `affects` docs current.
