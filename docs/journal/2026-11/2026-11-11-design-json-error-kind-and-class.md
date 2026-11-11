---
date: 2026-11-11
type: journal
components: [cli, evaluator]
aspects: []
design:
  - docs/design/2026-11-11-json-error-kind-and-class.md
meta-impact:
  note: "Accepted: every `--json` error object — the `error` of an `\"outcome\": \"error\"` query document and of the stderr file document — will gain `kind` (a stable kebab-case name per error, open to additions: `step-limit`, `non-linear`, `unsatisfiable-facts`, `non-ground-condition`, `syntax`, `read`, …) and `class` (a closed set of six: `syntax`, `program`, `unsupported`, `resource`, `io`, `internal`), plus the error's data as fields named as in the variant (`steps` and `goal`; `conflict` as `[{line?, column?, clause}]`; `constraint`; `construct`, `variable`, `clause`; …). `message`, `line`, `column`, the documents' places and exit codes are unchanged. aicogito's reasoning adapter can route `class` to its five outcomes — `resource` → `ResourceLimit`, `unsupported` → `Unsupported`, the rest `Rejected` — instead of mapping every error to `Rejected`, and read the conflicting facts as records. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Design record accepted: a `kind` and a `class` on the JSON error object

- **When:** 2026-11-11 21:19:19

## Context

The step-limit record (CLM-0029) kept a machine-readable `kind` out of
the JSON error object "until a consumer asks for it rather than guessed
at", and offered the message's fixed opening phrase meanwhile. Reading
the one consumer settled whether anyone had asked: aicogito's reasoning
adapter ends every evaluation in one of five outcomes it refuses to
collapse, yet maps every claimr error, in both documents, to `Rejected`
with the message as the diagnostic, because the object carries nothing
else; the one distinction it draws, an old binary without `--json`, it
draws by searching the text. With 0.5.0 published this evening, three
different stops — a budget running out, a residue claimr declines to
approximate, a program with conflicting facts — share one `message`
field, and the conflicting facts ride inside it as lines, after `--json`
was introduced to end prose parsing. The record landed as PR #84 and is
accepted here.

## Details

**Decision.** Every JSON error object gains `kind`, a stable kebab-case
name per error — one per `EvalError` variant that reaches a document,
plus `syntax` and `read` for the file-level failures — open to
additions; and `class`, a closed set of six that says what the caller
should do: `syntax` (the text is not a program), `program` (the program
or query breaks a rule of the language, at load or at run),
`unsupported` (claimr declines to decide this; it never approximates),
`resource` (a budget the caller set ran out), `io` (the file could not be
read), `internal` (an invariant failed; a bug to report). The error's
data become fields named as in the variant, so a consumer reads values
instead of prose: `steps` and `goal` for the step limit, `conflict` as a
list of cited clauses for unsatisfiable facts, `constraint` for a
non-linear residue, `construct`, `variable` and `clause` for a
groundness error, and so on; the rule for a future variant is the same.
`message` is unchanged, as are the documents' places, exit codes and
human mode: the addition is unconditional, since an error object without
`kind` would defeat the purpose, but purely additive.

**Why two fields.** `kind` alone would leave a consumer to keep a
kind-to-outcome table that goes stale whenever a kind is added; `class`
is closed and changes only by a record, so a consumer routes on it and is
never stranded. The precedent is ISO Prolog's error classification
(ISO/IEC 13211-1 §7.12.2): a closed set of classes, a formal term
carrying the culprit, the message left to the implementation. Rejected:
the message prefix as the mechanism (fragile, and what the adapter
already has to do once); `kind` only; numeric codes; ISO's own class
names, which describe Prolog's rules rather than claimr's; rewriting the
messages now that the data has fields. Deferred: a kind on usage errors,
error documents in human mode, localised messages.

**Pre-write checks.** A design record lists no `affects` and closes no
task. No aspect invariant is touched: no decision changes and no message
changes; the object says more about the same event. Cross-repo impact:
the two fields and the data fields, noted above.

## Links

- Design record: `docs/design/2026-11-11-json-error-kind-and-class.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/84
- Where it was deferred: `docs/design/2026-11-11-step-limit-for-queries.md`, item 5
- The consumer: aicogito `crates/aicogito-reason/src/lib.rs` (`Outcome`, `read_outcome`)
- Implementing task: to follow as CLM-0033
