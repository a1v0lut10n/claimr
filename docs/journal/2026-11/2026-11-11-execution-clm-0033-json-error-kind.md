---
date: 2026-11-11
type: journal
components: [cli, evaluator]
aspects: []
tasks:
  - docs/implementation/2026-11-11-clm-0033-json-error-kind.md
design:
  - docs/design/2026-11-11-json-error-kind-and-class.md
meta-impact:
  note: "Every `--json` error object now carries `kind` (a stable kebab-case name per error: `step-limit`, `non-linear`, `unsatisfiable-facts`, `non-ground-condition`, `syntax`, `read`, …), `class` (closed: `syntax`, `program`, `unsupported`, `resource`, `io`, `internal`) and the error's data as fields (`steps` and `goal`; `conflict` as `[{line?, column?, clause}]`; `constraint`; `construct`, `variable`, `clause`; …), ahead of the unchanged `message`. `line`, `column`, `query`, `outcome`, the documents' places and exit codes are unchanged. aicogito's reasoning adapter can route `class` to its five outcomes — `resource` → `ResourceLimit`, `unsupported` → `Unsupported`, the rest `Rejected` — and read the conflicting facts as records; `docs/reference/json-output.md` carries the table. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0033 executed: `kind`, `class` and the error's data on the JSON error object

- **When:** 2026-11-11 21:58:14

## Context

The record accepted this evening (PR #85) gives every JSON error object a
`kind` and a `class` and lifts the error's data out of the message into
fields, because the one consumer — aicogito's reasoning adapter, with its
five outcomes — had nothing but the message to classify on and mapped
every error to `Rejected`. One task, bounded and mechanical.

## Details

**What was built.** `EvalError::kind()`, `class()` and `fields()`, each
an exhaustive `match` with no wildcard: a stable name per variant, one
of the closed `ErrorClass` set, and the variant's data as named
`ErrorValue`s (`Text`, `Count`, `Clauses` — the conflicting facts of
`unsatisfiable-facts`, written as records with their positions).
`json_out::error_tail` writes the shared tail `kind`, `class`, the data,
`message` into both documents; `query_error_document` takes the error,
`error_document` the kind, class and fields beside the file and
position; the CLI classifies a parse error as `syntax`/`syntax` and an
unreadable file as `read`/`io`. `message` is `EvalError`'s `Display`, as
before; positions, the documents' places, exit codes, human mode and the
REPL are untouched. `ErrorClass` and `ErrorValue` are exported.

**Verified.** 214 tests pass: a unit test constructs every variant and
asserts kind, class and field names; the writer's tests take the new
shapes (a step limit with its count and goal, a syntax error, a read
failure, unsatisfiable facts with cited clauses, one without a
position); a new integration test checks the facts' `conflict` records
and that a non-linear residue's `message` is byte for byte the
human-mode diagnostic. Four prefix assertions that pinned
`"error":{"message":…` were updated for the field order. Human-mode
goldens byte-identical; fmt, clippy and lint clean.

**Findings.** Key order is not part of a JSON consumer's contract, but
our own tests had pinned `message` first; the record's order — kind,
class, data, message — is what they pin now. `ReservedBuiltin::arity`
rides as a count like `steps`, so a consumer sees a JSON number for
both. No aspect invariant is touched: nothing decided or printed for a
person changes.

**Pre-write checks.** The task's two `affects` docs were updated in the
PR: `json-output.md` (the two error sections rewritten around the
kind/class table, one example each, `kind` open and `class` closed) and
`evaluator.md` (the classification on `EvalError`). Cross-repo impact:
the fields, noted above.

## Links

- Task: `docs/implementation/2026-11-11-clm-0033-json-error-kind.md`
- Design: `docs/design/2026-11-11-json-error-kind-and-class.md`
- PR #86: commits 6eb3a09 (plan), 5b37791 (implementation)
- The consumer: aicogito `crates/aicogito-reason/src/lib.rs` (`read_outcome`)
