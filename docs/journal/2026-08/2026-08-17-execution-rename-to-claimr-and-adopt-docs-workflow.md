---
date: 2026-08-17
type: journal
components: [parser, cli, grammar]
aspects: [grammar-authority]
---

# Rename to claimr, flatten the crate layout, adopt the docs workflow

## Context
The repo held a placeholder root package (`rust`, hello-world `src/main.rs`)
with the real crate nested in `claim/`. The name `claim` is taken on
crates.io, and the project had no `docs/` tree following the aivolution
documentation workflow.

## Details
- Renamed the project, crate, binary, and language to **claimr**; source
  files use `.claimr`. Cargo metadata (description, repository, MIT license,
  `rust-version = "1.85"`) filled in.
- Flattened to a single root crate: `src/lib.rs` (parser), `src/main.rs`
  (thin CLI that parses a file and prints its clauses), `examples/`,
  `tests/`. Removed the nested `claim/` crate and the placeholder package.
- Added `parse_program` (whole-file entry point) next to the existing
  `all_consuming_parse_clause`; the CLI and integration tests use it.
  Integration test parses every `examples/*.claimr`. Fixed a stale unused
  import and two clippy nits; `cargo clippy --all-targets -- -D warnings`
  is clean, 9 tests pass.
- Docs: `docs/README.md` (defers to aivolution-meta), `NEXT-TICKET`
  (`CLM-0001`; prefix `CLM`), `journal/`, `tasks/`, `design/`,
  `architecture/` (vocabulary: components `parser`, `cli`, `grammar`;
  aspect `grammar-authority`), `reference/grammar.md` (moved from
  `src/claim_grammar.txt`, now markdown), `inbox/`, `incubation/`,
  `catalyst/`. Project `CLAUDE.md` and `LICENSE` (MIT) added.
  `aivolution lint`: 0 errors, 0 warnings.

## Links
- Follow-ups outside this repo: rename the GitHub repo `a1v0lut10n/claim` →
  `claimr`, rename the local checkout directory, register `claimr` in
  aivolution-meta `repos.yaml`.
