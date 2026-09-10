---
date: 2026-09-10
type: task
status: proposed
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

- [ ] A consumer can dispatch on `outcome` without parsing prose.
- [ ] aicogito-reason's `read_outcome` can be replaced by serde over
      this format (tracked aicogito-side).
- [ ] `cargo test` green; human output byte-identical to today's.
