# claimr — project directives

Project-specific guidance for this repo. Universal conventions live in the
workspace-level `CLAUDE.md` (aivolution-meta `workspace/CLAUDE.md`, inherited
via the workspace-root symlink).

## What is Claimr

A constraint logic programming language (Prolog III inspired) implemented in
Rust. Single crate `claimr`: a library (`src/lib.rs` — AST + nom parser) and a
thin CLI binary (`src/main.rs`). Only the parser exists so far; evaluation and
constraint solving are future work. Source files use the `.claimr` extension;
the formal grammar is `docs/reference/grammar.md` and is the authority when
parser and README disagree.

The project was renamed from *claim* to *claimr* (crates.io name clash).

## Build & development commands

```bash
cargo build
cargo test                                # unit tests + tests/ (parses every examples/*.claimr)
cargo clippy --all-targets -- -D warnings
cargo run -- examples/socrates.claimr     # parse a program, print its clauses
```

Adding a sample program under `examples/` automatically puts it under test.

## Ticket prefix

- This repo's ticket prefix is `CLM` (see "Branch naming & ticket numbers" in
  the universal directives; `docs/NEXT-TICKET` holds the next free number).

## Documentation

The cross-repo documentation workflow (taxonomy, schemas, mutation patterns)
is owned by aivolution-meta (`docs/README.md` there); this repo's
`docs/README.md` carries repo-specific additions only. Journal entries go in
`docs/journal/yyyy-mm/`, and are drafted and confirmed before writing.
