---
date: 2026-08-17
type: journal
components: [parser, grammar, cli]
aspects: [grammar-authority]
tasks: [docs/tasks/2026-08/2026-08-17-clm-0002-rustemo-parser.md]
design: [docs/design/2026-08-17-rustemo-parser-generator-over-nom.md]
---

# Replaced the nom parser with a rustemo-generated LR parser (CLM-0002)

## Context
Before evaluator work the grammar will grow; the hand-written nom parser
already drifted from the EBNF (nested atoms as arguments failed) and
reported errors without positions. The design record chose rustemo (LR),
already used in phenotyper and colap.

## Details
- `src/parser/claimr.rustemo` is now the authoritative grammar (plain
  LR(1), no conflicts, no GLR); `build.rs` follows phenotyper's pattern.
  Actions build `crate::ast` directly; rustemo preserves the edits.
- Public API: `parse_program`, `parse_clause`, `ParseError` with
  line/column/offset; nom and `IResult` removed. CLI prints positioned
  errors.
- Tests: 15 unit + 4 integration + 1 doctest; nested-atom example added;
  malformed inputs assert positions. Clippy `-D warnings` clean.
- Docs: architecture vocabulary, `grammar.md` (now an EBNF view), README,
  CLAUDE.md updated; `aivolution lint` clean.

## Links
- Design: docs/design/2026-08-17-rustemo-parser-generator-over-nom.md
- Task: docs/tasks/2026-08/2026-08-17-clm-0002-rustemo-parser.md
