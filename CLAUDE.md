# claimr — project directives

Project-specific guidance for this repo. Universal conventions live in the
workspace-level `CLAUDE.md` (aivolution-meta `workspace/CLAUDE.md`, inherited
via the workspace-root symlink).

## What is Claimr

A constraint logic programming language (Prolog III inspired) implemented in
Rust. Single crate `claimr`: a library (`src/lib.rs` public API, `src/ast.rs`
AST, `src/parser/` — an LR parser generated at build time by rustemo from
`src/parser/claimr.rustemo`, with hand-maintained semantic actions in
`claimr_actions.rs`) and a thin CLI binary (`src/main.rs`). Only the parser
exists so far; evaluation and constraint solving are future work. Source files
use the `.claimr` extension. **`src/parser/claimr.rustemo` is the authoritative
grammar**; `docs/reference/grammar.md` is an EBNF view of it and must be kept
in step. Grammar changes go grammar → actions → example under `examples/` →
EBNF view (see the `grammar-authority` aspect in `docs/architecture/README.md`).

The project was renamed from *claim* to *claimr* (crates.io name clash).

## Build & development commands

```bash
cargo build
cargo test                                # unit tests + tests/ (parses every examples/*.claimr)
cargo clippy --all-targets -- -D warnings
cargo run -- examples/socrates.claimr     # parse a program, print its clauses
```

Adding a sample program under `examples/` automatically puts it under test.
The generated parser lives in `OUT_DIR` (never committed); rustemo regenerates
it when the grammar changes and appends action stubs for new productions to
`claimr_actions.rs` without touching existing ones.

## Ticket prefix

- This repo's ticket prefix is `CLM` (see "Branch naming & ticket numbers" in
  the universal directives; `docs/NEXT-TICKET` holds the next free number).

## Documentation

The cross-repo documentation workflow (taxonomy, schemas, mutation patterns)
is owned by aivolution-meta (`docs/README.md` there); this repo's
`docs/README.md` carries repo-specific additions only. Journal entries go in
`docs/journal/yyyy-mm/`, and are drafted and confirmed before writing.
