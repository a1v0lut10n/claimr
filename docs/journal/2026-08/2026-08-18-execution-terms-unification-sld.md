---
date: 2026-08-18
type: journal
components: [evaluator, cli, parser]
aspects: [answer-soundness, grammar-authority]
tasks: [docs/tasks/2026-08/2026-08-17-clm-0005-terms-unification-sld.md]
design:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
---

# Terms, unification and the SLD machine (CLM-0005)

## Context
Stage 2 of the evaluator design: make claimr run pure programs, building
the store/trail/suspension skeleton stage 3 plugs the numeric solver into.

## Details
- `src/eval/`: interned symbols; a cell heap with a trail (address–value
  undo) and per-variable suspension; unification over rational trees with
  bind-before-descend (no occurs check); `dif` by trial unification —
  satisfied/violated/suspended on the would-be-bound variables; compile
  step (`=>` sugar, constraint facts as the validated initial store,
  structure copying); iterative SLD machine with choice points; answers in
  solved form (aliases in variable order, cyclic terms as equations,
  internal-only disequations projected away). Numeric relations,
  arithmetic and attribute terms rejected at load with line:column.
- Parser: clause spans (`parse_program_spanned`), AST `Display`.
- CLI runs programs (`--parse`, `--limit`, exit codes, GCC-style
  positions for parse and load errors).
- 62 tests incl. golden runs of `family`, `lists`, `dif`, `cyclic`; deep
  derivations; clippy clean.
- Docs: `components/evaluator.md`, `aspects/answer-soundness.md`,
  vocabulary, README, CLAUDE.md.

## Links
- Task: docs/tasks/2026-08/2026-08-17-clm-0005-terms-unification-sld.md
