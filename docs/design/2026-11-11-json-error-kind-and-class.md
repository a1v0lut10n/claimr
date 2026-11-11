---
date: 2026-11-11
type: design
status: proposed
components: [cli, evaluator]
aspects: []
tags: [json-output, errors, contract, kind, class, machine-readable, aicogito]
---

# A `kind` and a `class` on the JSON error object, with the error's data as fields

## Context

The `--json` contract (CLM-0010, `docs/reference/json-output.md`) reports
an error in two places and one shape: a query that stops at run time is
`{"query", "outcome": "error", "error": {"message"}}` on stdout, and a
file that never runs is `{"error": {"file", "line"?, "column"?,
"message"}}` on stderr. The `message` is the human diagnostic; nothing
else on the object says *what kind* of error it is. Two records set that
aside on purpose: the step-limit record (CLM-0029) kept "a
machine-readable `kind` on the JSON error object" out — "the shape stays,
and the field is added when a consumer asks for it rather than guessed
at" — and offered the message's fixed opening phrase as the way to tell a
budget stop apart; the explanations-of-answers record likewise added its
field only under a flag.

**What the one consumer does with the message.** aicogito's reasoning
adapter (`crates/aicogito-reason`, design record *reason is a subprocess
on a leash*, §12.2) ends every evaluation in one of five outcomes it
refuses to collapse: `Success`, `NoSolution`, `Rejected { diagnostic }`
(the context could not be evaluated), `ResourceLimit { budget_ms }` (its
own wall-clock kill) and `Unsupported { reason }` (the binding cannot
serve this at all). Its `read_outcome` maps `"outcome": "none"` to
`NoSolution`, `"solutions"` to `Success`, and **every** `"error"` — and
every stderr document — to `Rejected` with the message as the diagnostic.
The one distinction it does draw, an old binary without `--json`, it
draws by searching the text for `unknown option` and `usage:`. So the
taxonomy the consumer already has cannot be served by what claimr sends:
a step-limit stop (CLM-0029), the resource outcome by nature, would arrive
as `Rejected`; a non-linear residue, which is claimr declining to
approximate — `Unsupported` in the consumer's words — arrives as
`Rejected`; and the minimal conflicting set a load error now carries
(CLM-0030) arrives as lines inside a string, after `--json` was introduced
precisely to end prose parsing. The consumer has, in effect, asked.

**What the errors carry.** Every `EvalError` variant is a small record —
`StepLimit { steps, goal }`, `NonGroundCondition { construct, variable,
clause }`, `InitialStoreUnsatisfiable { conflict: [(position, text)] }`,
`NonLinear { constraint }`, … — and its message is those fields in prose.
A parse error carries `message`, `line`, `column`. The data exists; only
the writer flattens it.

**Prior art.** ISO Prolog (ISO/IEC 13211-1, §7.12.2) classifies errors by
a closed set of classes — `instantiation_error`, `type_error`,
`resource_error`, `syntax_error`, `system_error`, … — each with a
formal term carrying the culprit, and leaves the message implementation-
defined. The classes are what a caller routes on; the term is what a tool
inspects. That split is the one taken here, with claimr's own classes.

## Decision

1. **Every JSON error object gains `kind` and `class`.** `kind` is a
   stable kebab-case name for the precise error, one per `EvalError`
   variant that can reach a document plus the two file-level failures;
   `class` is one of a **closed** set of five that says what the caller
   should do about it:

   | `class`       | meaning                                                                 | `kind`s |
   |---------------|-------------------------------------------------------------------------|---------|
   | `syntax`      | the text is not a program                                               | `syntax` |
   | `program`     | the program or query breaks a rule of the language, at load or at run  | `reserved-name`, `reserved-builtin`, `bad-directive`, `comprehension-in-head`, `unsatisfiable-facts`, `non-ground-condition`, `non-ground-solution`, `non-ground-tabled-answer`, `constrained-tabled-answer`, `incomplete-table`, `cyclic-tabled-call` |
   | `unsupported` | claimr declines to decide this; it never approximates                   | `non-linear`, `cyclic-attribute-term` |
   | `resource`    | a budget the caller set ran out                                         | `step-limit` |
   | `io`          | the file could not be read                                              | `read` |
   | `internal`    | an invariant of the evaluator failed; a bug to report                  | `internal` |

   New kinds may be added to a class; the set of classes changes only by
   a record. A consumer routes on `class` and is never stranded by a kind
   it has not seen.

2. **The error's data become fields.** Alongside `kind`, `class` and
   `message`, every datum the error carries appears as a field named as
   in its variant, so a consumer reads values instead of parsing prose:
   `step-limit` carries `steps` (a number) and `goal`;
   `unsatisfiable-facts` carries `conflict`, a list of `{"line"?,
   "column"?, "clause"}`; `non-linear` carries `constraint`;
   `non-ground-condition` carries `construct`, `variable`, `clause`;
   `non-ground-solution` carries `construct`, `instance`;
   `non-ground-tabled-answer` and `constrained-tabled-answer` carry
   `predicate`, `instance` (and `residual`); `incomplete-table` carries
   `predicate`, `construct`; `cyclic-tabled-call` carries `call`;
   `cyclic-attribute-term` carries `term`; `reserved-name` and
   `reserved-builtin` carry `name` (and `arity`); `bad-directive`,
   `comprehension-in-head`, `syntax`, `read` and `internal` carry nothing
   beyond the message (and, where known, the existing `line` and
   `column`). The rule for a future variant is the same: its fields are
   the object's fields.

3. **`message` is unchanged, and so is everything else.** The message
   text stays what it is, for people and for a consumer written against
   today's contract: both existing fields keep their names, positions and
   meanings, the two documents keep their places (stdout for a query,
   stderr for a file), exit codes do not move, and human mode prints as
   before. The addition is unconditional — an error object without `kind`
   would defeat the purpose — but purely additive: a consumer that reads
   only `message` sees no change.

4. **Not in this record.** A `kind` on a usage error (exit 2): it is the
   CLI's complaint about its own arguments, printed as prose before any
   file is touched, and nothing routes on it. Warnings: claimr has none.
   Error documents in human mode. Localised messages. Rewriting messages
   to be shorter now that the data has fields: the texts are pinned by
   tests and read by people; they stay.

## Consequences

- **The consumer can serve its taxonomy from the contract.** `class`
  `resource` → `ResourceLimit` (with `steps` beside its own `budget_ms`),
  `unsupported` → `Unsupported` (with the `constraint` as the reason),
  `syntax` / `program` → `Rejected` with the message, `internal` →
  `Unsupported` or `Rejected` as it chooses; the load error's conflicting
  facts become citable records. The text matching for an old binary
  stays, since an old binary sends no `kind`.

- **Evaluator.** `EvalError` gains `kind() -> &'static str`, `class() ->
  ErrorClass` (a small public enum) and `fields()`, the variant's data as
  named values for the writer; `ParseError` and the read failure are
  classified by the CLI. The classification is exhaustive by construction
  (a `match` with no wildcard), so a new variant cannot be added without
  a kind and a class.

- **CLI.** `json_out::query_error_document` and `error_document` take
  the error rather than its message and write `kind`, `class`, the
  fields and `message`, in that order after the positional fields. The
  REPL is untouched (it has no JSON mode).

- **Contract.** `json-output.md` gains the table above and one example
  per document, and says that `kind` is open and `class` closed. The
  acceptance entry carries a `meta-impact` note for aicogito; the
  contract names `claimr-json-output` and `claimr-token-api`, suggested
  since CLM-0015, remain to be added to aivolution-meta's vocabulary.

- **Soundness.** Untouched: no decision changes, no message changes; the
  object says more about the same event.

- **One task** implements the classification, the writer, the
  documentation and the tests: one unit test per kind asserting `kind`,
  `class` and the fields (the step limit with its count and goal, the
  unsatisfiable facts with their cited clauses, a non-linear residue, a
  groundness error with its variable), the file-level document for a
  syntax error and an unreadable file, and byte-level checks that
  `message`, `line`, `column`, `query`, `outcome` and the exit codes are
  exactly what they were.

## Sources

- `docs/design/2026-11-11-step-limit-for-queries.md`, item 5: the
  deferral of `kind` "until a consumer asks for it".
- `docs/design/2026-11-09-explanations-of-answers.md`: the `--json`
  contract's rule that a consumer sees no change it did not ask for.
- aicogito, `crates/aicogito-reason/src/lib.rs` (`Outcome`,
  `read_outcome`) and `docs/design/2026-09-10-reason-is-a-subprocess-on-a-leash.md`
  §12.2: the five outcomes and how claimr's errors are classified today.
- `docs/reference/json-output.md`: the two error documents.
- ISO/IEC 13211-1:1995, §7.12.2 *Error classification*: a closed set of
  classes with a formal term carrying the culprit, the message left to
  the implementation.
- `src/eval/error.rs`, `src/json_out.rs`, `src/main.rs`: the variants,
  the writer, the two emission sites.
