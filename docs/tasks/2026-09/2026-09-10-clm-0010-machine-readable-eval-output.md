---
date: 2026-09-10
type: task
status: done
affects:
  - docs/reference/grammar.md
components: [cli]
aspects: []
design: []
tags: [CLM-0010, cli, json, aicogito]
---

# CLM-0010: Machine-readable evaluation output

## Objective

A structured output mode for batch evaluation — `claimr eval
<file.claimr>` (or `claimr --json <file>`) emitting one JSON document
per query — so programmatic consumers stop parsing the human batch
format. The human format stays exactly as it is.

## Context

A change proposal from aicogito (Stage D, design record
`2026-09-10-reason-is-a-subprocess-on-a-leash` in the aicogito repo):
its reasoning adapter drives claimr as a subprocess and reads the
batch output — the echoed `?- query.`, then `true` / `false` /
solved-form binding lines — pinned by contract tests on both sides
of the pipe. That works, but the contract is implicit in prose
formatting: `true` vs `false` vs bindings vs a diagnostic is a
line-shape dispatch, and any future change to answer printing is a
silent break for consumers. The adapter's five-way outcome taxonomy
(success / no solution / rejected / resource limit / unsupported)
wants a structural source.

## Deliverables

- A batch mode emitting, per query, one JSON document, e.g.:
  `{"query": "?- ok(s1).", "outcome": "solutions", "answers":
  [{"text": "R = expression, B = supported"}]}` — with `"outcome":
  "none"` for no solutions and answers optionally structured further
  (bindings as a map, constraints as a list) as the solved form
  allows.
- Diagnostics as structured objects (`file`, `line`, `column`,
  `message`) on stderr or in-stream, with a documented exit-code
  contract.
- The human batch format and the REPL untouched.
- The format documented in `docs/reference/` and exercised by an
  `examples/*.answers`-style golden test.

## Verification

- [x] A consumer can dispatch on `outcome` without parsing prose —
      and answers arrive MORE structured than proposed: one
      `X = t` string per variable in `equations`, with
      `disequations` and `constraints` as their own lists (the
      solved form's own split).
- [ ] aicogito-reason's `read_outcome` replaced over this format
      (tracked aicogito-side).
- [x] `cargo test` green; the human format byte-identical (pinned by
      `tests/json_output.rs`, which would break on any change).

Delivered as `--json` (a flag beside `--parse`/`--limit`, fitting
the CLI's grain) rather than an `eval` subcommand; diagnostics on
stderr; exit codes unchanged. Format: `docs/reference/json-output.md`.
