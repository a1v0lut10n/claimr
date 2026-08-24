---
date: 2026-08-24
type: task
status: done
affects:
  - src/lib.rs
components: [parser]
aspects: []
tags: [CLM-0009, highlighting, editor]
---

# CLM-0009: A token API for editors

## Objective

claimr owns its token vocabulary; editors should not re-guess it. A
`highlight` module exposes an **error-tolerant, line-based** lexer over
the grammar's terminals (`src/parser/claimr.rustemo`): comments, vars,
predicates (an identifier applied to arguments), identifiers, numbers,
operators and punctuation, as byte ranges within a line. Line-based and
stateless because an editor's buffer is mid-edit most of the time — a
full LR parse fails there; lexing never does. First consumer: the
aicogito editor (its tree-sitter pipeline gains a lexical engine); the
same API is the substrate for LSP semantic tokens later.

## Notes

- The lexer is hand-kept beside the grammar, with a test asserting it
  recognises every terminal the grammar declares — drift between the
  two fails the build, which is the alarm we want.
- No claimr behaviour changes; the module is additive.
