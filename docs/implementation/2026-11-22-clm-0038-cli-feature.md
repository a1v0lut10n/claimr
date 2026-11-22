---
date: 2026-11-22
type: task
status: in-progress
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

(filled in as the work proceeds)

## Verification

- [ ] Default features: `cargo fmt --all -- --check`,
      `cargo clippy --all-targets -- -D warnings`, `cargo test` as before.
- [ ] `--no-default-features`: `cargo build --lib`, `cargo clippy
      --all-targets --no-default-features -- -D warnings`, `cargo test
      --no-default-features` (library and doc tests; the binary's tests
      skipped), `cargo tree` without `rustyline`/`ctrlc`.
- [ ] `cargo build` still produces the binary; `cargo package --list`
      unchanged.
- [ ] `aivolution lint .` clean; the two `affects` docs current.
