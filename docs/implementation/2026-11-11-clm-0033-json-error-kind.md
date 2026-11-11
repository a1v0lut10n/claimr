---
date: 2026-11-11
type: task
status: done
affects:
  - docs/reference/json-output.md
  - docs/architecture/components/evaluator.md
components: [cli, evaluator]
aspects: []
design:
  - docs/design/2026-11-11-json-error-kind-and-class.md
tags: [CLM-0033, json-output, errors, kind, class, contract]
---

# CLM-0033: `kind`, `class` and the error's data on the JSON error object

## Premise

The accepted record `2026-11-11-json-error-kind-and-class.md`: every JSON
error object — the `error` of an `"outcome": "error"` query document and
of the stderr file document — gains `kind` (a stable kebab-case name per
error, open) and `class` (a closed set of six: `syntax`, `program`,
`unsupported`, `resource`, `io`, `internal`), and the error's data appear
as fields named as in the variant. `message`, `line`, `column`, the
documents' places, exit codes and human mode are unchanged.

## Shape of the implementation

**Classification on the error.** `EvalError` gains `kind() -> &'static
str`, `class() -> ErrorClass` and `fields() -> Vec<(&'static str,
ErrorValue)>`, each an exhaustive `match` with no wildcard, so a new
variant cannot compile without its kind, class and fields. `ErrorClass`
is a public enum with `as_str`; `ErrorValue` is `Text(String)`,
`Count(u64)` or `Clauses(Vec<(Option<Span>, String)>)` — the conflicting
facts of `unsatisfiable-facts`, written as `[{"line"?, "column"?,
"clause"}]`. The two file-level failures the CLI classifies itself: a
parse error is `syntax` / `syntax`, an unreadable file `read` / `io`,
both without fields.

**The writer.** `json_out::error_object(kind, class, fields, message)`
renders the shared tail of both documents — `"kind"`, `"class"`, the
fields in order, `"message"` — and `query_error_document` takes the
`EvalError`; `error_document` takes the kind, class and fields beside
the file and position. Field order after the positional fields follows
the record: kind, class, data, message.

**Tests.** A unit test constructs every `EvalError` variant and asserts
its kind, class and field names (coverage of the match by construction);
the JSON writer's unit tests take the new shapes; integration: the step
limit's document carries `kind`, `class`, `steps`, `goal`; the
unsatisfiable facts' stderr document carries `conflict` with cited
clauses; a non-linear residue is `unsupported`; a groundness error
carries its `variable`; a syntax error and an unreadable file carry
their kinds and classes; and the byte-level checks — `message` equals
the human-mode text, `line`/`column`/`query`/`outcome` and exit codes
unchanged, stdout silent on a file-level failure.

## Action items

1. `error.rs`: `ErrorClass`, `ErrorValue`, `kind`, `class`, `fields`;
   exports.
2. `json_out.rs`: `error_object`, the two documents; `main.rs`: the four
   sites.
3. Tests as above.
4. Docs: `json-output.md` (the two error sections rewritten around the
   table and one example each; `kind` open, `class` closed),
   `evaluator.md` (the classification on `EvalError`); this plan.

## Findings

- **Four tests pinned the message's position.** The JSON suite asserted
  `"error":{"message":…` as a prefix for a groundness error, a step
  limit, a tabling error and the two file documents; with `kind`, `class`
  and the data now ahead of `message`, those prefixes moved. Key order is
  not part of the contract a JSON consumer relies on, and `message` itself
  is byte-identical, which the new test checks against the human-mode
  text.
- **`arity` is a count.** `ReservedBuiltin::arity` is a `usize`; it rides
  as `ErrorValue::Count(u64)` like `steps`, so a consumer sees a JSON
  number for both.
- **The REPL is untouched** — it has no JSON mode — and so is every human
  diagnostic; the `message` string is `EvalError`'s `Display`, as before.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` (one unit test over every variant, the writer's tests, one
      new integration test; four prefix assertions updated); human-mode
      goldens byte-identical.
- [x] Every existing JSON field keeps its name, value and place (`message`
      compared byte for byte with the human diagnostic); exit codes
      unchanged (the diagnostics test).
- [x] `aivolution lint .` clean; the two `affects` docs current.
