---
date: 2026-08-18
type: journal
components: [cli, evaluator]
aspects: [answer-soundness]
tasks: [docs/tasks/2026-08/2026-08-18-clm-0008-repl.md]
design: [docs/design/2026-08-18-repl-interaction-model.md]
---

# The REPL (CLM-0008)

## Context
All four evaluator stages done; the interactive loop the evaluator record
left as a follow-up. The interaction-model record was accepted and amended
during implementation (prompt `claimr> `, exact file syntax — a bare
`p(a).` is both a fact and a query, so `?-` cannot be optional).

## Details
- `src/repl.rs`: rustyline editor on a TTY, plain lines on a pipe; parse-
  driven input completeness (syntax error at end of input = incomplete);
  `?- …` answered Prolog-style (`;` next, Enter/`.`/`q` stop, `false.`),
  other clauses extend the session (recompiled; unsatisfiable world
  rejected); `:load/:reload/:list/:clear/:limit/:all/:help/:quit`; Ctrl-C
  via `ctrlc` and a flag the machine polls; piped stdin drives the same
  loop (query echoed, prompts suppressed).
- Evaluator: `Solutions::with_interrupt`, `interrupted`, `may_continue`.
- CLI: `claimr` → REPL, `-i file` → run then continue; batch unchanged.
- 97 tests incl. 9 pipe-driven REPL tests and the interrupt hook.
- Docs: `cli` row, evaluator doc, README (transcript, commands), CLAUDE.md.

## Links
- Task: docs/tasks/2026-08/2026-08-18-clm-0008-repl.md
