---
date: 2026-09-10
type: journal
components: [cli, parser, grammar, eval, highlight]
aspects: [grammar-authority]
design: [docs/design/2026-09-10-quoted-atoms.md]
---

# JSON output, quoted atoms, and the 0.3.0 release

## Context

Two change proposals arrived from aicogito (its Stage D reasoning
adapter drives claimr as a subprocess): CLM-0010, a machine-readable
output mode to replace prose-parsing of the batch format, and
CLM-0011, quoted atoms so external identifiers (record ids like
`situation:frame-in-design-specification`) can be atoms without
consumer-side mangling. Both were filed as proposed tasks through
this repo's own front door and accepted.

## Details

CLM-0010 (#26) delivered `--json`: one JSON document per query
(NDJSON) with the solved form's own split (equations, disequations,
constraints — one `X = t` string per variable), structured
diagnostics, exit codes unchanged, the human format byte-identical
by test; the contract is documented in
`docs/reference/json-output.md` and pinned end to end by
`tests/json_output.rs`. CLM-0011 (#27) added quoted atoms in the
grammar-authority order, with a design record settling the
semantics: backslash escapes (not ISO's doubled quote), identity is
content (the AST, unification, store, and solver untouched — a
`Name` is a content `String`), and print-back quotes exactly when
the spelling isn't a plain identifier, so answers round-trip as
valid source; the empty atom `''` is admitted, not special-cased.
Release 0.3.0 (#28) bumped, caught the README up two releases (the
status paragraph still listed the REPL as open), tagged `v0.3.0`,
and published to crates.io. aicogito consumed both within the day —
its adapter now reads the JSON contract and its id↔atom manifest
dissolved to a spelling function.

## Links

- Tasks: `docs/tasks/2026-09/2026-09-10-clm-0010-…`, `…-clm-0011-…`
- Design: `docs/design/2026-09-10-quoted-atoms.md`
- PRs #25–#28; tag `v0.3.0`; crates.io claimr 0.3.0
- Consumer: aicogito SUM-0128/SUM-0129
