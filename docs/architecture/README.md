# Architecture — claimr

Index and **controlled vocabulary** for claimr's architecture. The names listed
here are canonical: any `components:` / `aspects:` field in a claimr document
must use them verbatim, and skills validate against this file.

Individual component and aspect docs live under [`components/`](components/)
and [`aspects/`](aspects/) and are written on demand — a name can exist in the
vocabulary before its doc does. Schemas live in aivolution-meta
(`docs/schemas/architecture-component.md`, `docs/schemas/architecture-aspect.md`).

## Components

Nameable building blocks — a crate, module, or binary you can point at.

| Name      | What it is |
|-----------|------------|
| `parser`  | `src/lib.rs` — the AST types and the nom parser for `.claimr` programs (`parse_program`, `all_consuming_parse_clause`). |
| `cli`     | `src/main.rs` — the `claimr` binary: parses a file and prints its clauses. |
| `grammar` | `docs/reference/grammar.md` — the formal grammar of the language, which the parser implements. |

Future components (evaluator, constraint solver, REPL) are added here when
they exist.

## Aspects

Cross-component invariants — rules that survive replacing any single component.

| Name                | Invariant (summary) |
|---------------------|---------------------|
| `grammar-authority` | The grammar doc is the single source of truth for the language's syntax; parser and README follow it, never the other way round. Every construct in the grammar has at least one parsed example under `examples/`. |

## Litmus test

If you can name the crate/module/binary that implements it, it's a
**component**; if it's a rule that survives replacing any single component,
it's an **aspect**.
