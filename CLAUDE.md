# claimr — project directives

Project-specific guidance for this repo. Universal conventions live in the
workspace-level `CLAUDE.md` (aivolution-meta `workspace/CLAUDE.md`, inherited
via the workspace-root symlink).

## What is Claimr

A constraint logic programming language (Prolog III inspired) implemented in
Rust. Single crate `claimr`:

- `src/lib.rs` public API, `src/ast.rs` AST, `src/number.rs` exact rational
  `Number` (no floats anywhere — `exact-arithmetic` aspect);
- `src/parser/` — an LR parser generated at build time by rustemo from
  `src/parser/claimr.rustemo`, with hand-maintained semantic actions in
  `claimr_actions.rs`;
- `src/eval/` — the evaluator: rational-tree unification, `dif`, compile step
  with arithmetic lowering, iterative SLD machine, answers in solved form
  (`components/evaluator.md`);
- `src/solver/` + the numeric glue in `src/eval/store.rs` — the linear
  constraint store over exact rationals (Dutertre–de Moura simplex,
  attribute terms, numeric disequations, delayed products;
  `components/solver.md`). Evaluator stages 2–3 are done; stage 4 (answer
  projection/simplification) and a REPL are open;
- `src/main.rs` + `src/repl.rs` — the CLI: `claimr file.claimr` runs the
  program's queries (batch); `claimr` / `claimr -i file` is the REPL
  (`docs/design/2026-08-18-repl-interaction-model.md`: file syntax at the
  prompt, `?- …` queries stepped with `;`, other clauses extend the session,
  `:load/:reload/:list/:clear/:limit/:all/:help/:quit`, Ctrl-C interrupts).

Source files use the `.claimr` extension. **`src/parser/claimr.rustemo` is the
authoritative grammar**; `docs/reference/grammar.md` is an EBNF view of it and
must be kept in step. Grammar changes go grammar → actions → example under
`examples/` → EBNF view (see the `grammar-authority` aspect in
`docs/architecture/README.md`). Semantics decisions live in `docs/design/`
(evaluator, exact rationals, arithmetic in terms) — read them before touching
`src/eval/`.

The project was renamed from *claim* to *claimr* (crates.io name clash).

## Build & development commands

```bash
cargo build
cargo test                                # unit + integration; parses every examples/*.claimr and
                                          # golden-runs each one that has an examples/*.answers file
cargo clippy --all-targets -- -D warnings
cargo run -- examples/family.claimr       # run a program: prints each query and its answers
cargo run -- --parse examples/socrates.claimr   # dump the parsed clauses instead
cargo run -- --limit 5 file.claimr        # cap answers per query (unlimited by default)
cargo run                                 # the REPL (claimr> prompt); `cargo run -- -i file` runs then continues
printf '?- p(X).\n;\n' | cargo run -q     # the REPL through a pipe (how tests/repl.rs drives it)
```

Adding a sample program under `examples/` automatically puts it under parse
test; adding a sibling `.answers` file (the expected stdout) puts it under
golden run test.
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

## Conventions from Aivolution SWE

Generated from Aivolution SWE's conventions (mastermind:
aivolution-mastermind).

- Journal significant events under `docs/journal/`.
- Record decisions as ADRs under `docs/decisions/`.
- Plan non-trivial work under `docs/implementation/`.
- Everything lands reviewably; nothing is written
silently.
