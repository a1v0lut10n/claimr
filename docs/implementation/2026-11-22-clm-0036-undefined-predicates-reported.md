---
date: 2026-11-22
type: task
status: done
affects:
  - docs/architecture/components/evaluator.md
  - docs/architecture/aspects/answer-soundness.md
  - docs/architecture/README.md
  - docs/reference/json-output.md
components: [evaluator, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-22-undefined-predicates-are-reported.md
tags: [CLM-0036, undefined-predicate, warnings, compile, cli, repl, json-output]
---

# CLM-0036: Undefined predicates are reported

## Premise

The accepted record `2026-11-22-undefined-predicates-are-reported.md`: a
call to a predicate no clause defines still fails, but the compile step
reports every such call site — `Program::undefined()` for the library,
one warning per call site per load in batch mode (stderr, exit code
unchanged; a `{"warning": …}` stderr document under `--json`), and in
the REPL a warning on `:load` and before a query for the undefined
predicates it can reach, never on a clause typed at the prompt.

## Shape of the implementation

**Compiler.** After the predicates are indexed, a pass walks every body
goal of every clause and every query — calls inside disjunctions,
conditions, negations and comprehension bodies included, mirroring
`goals_vars` — and records each `TGoal::Call` whose key is in neither
`preds` nor `builtins` as an `UndefinedCall { name, arity, span, clause }`
(the name as source, quoted when needed; the calling clause or query as
written), plus a crate-private `caller` — the calling predicate's key, or
the query's index — for reachability. A `:- table` declaration is not a
definition. `Query` gains its `span`. `Program::undefined()` returns the
list in program order; `Program::undefined_from(&Query)` returns the
entries a query can reach: its own, and those of every predicate reached
from its goals through the call graph (a worklist over `preds`).

**CLI.** After a successful compile and before any query, batch mode
prints one stderr line per entry, `file:line:col: warning: nosuch/1 is
called in `s(X) :- p(X), nosuch(X).` and no clause defines it` (without
the position when the entry has none), and under `--json` one stderr
document per entry, `{"warning": {"file", "line"?, "column"?, "kind":
"undefined-predicate", "predicate", "clause", "message"}}` — the error
document's tail without `class`, so `error_tail` takes the class as an
option. Stdout and the exit code are untouched.

**REPL.** `load` warns, after a successful recompile, for the entries
whose calling clause came from the loaded file (matched by the clause's
text among the file's clauses); `answer` warns, before solving, for
`undefined_from` of the query it just compiled. A clause typed at the
prompt produces no warning. Both print `warning: …` on stderr in the
batch form without a position (the session compiles without spans).

**Message.** `UndefinedCall::describe()`: `nosuch/1 is called in
`<clause>` and no clause defines it` — the record's REPL wording, used
everywhere so the clause is always cited.

## Action items

1. `compile.rs`: `UndefinedCall`, the pass, `Query.span`, `undefined()`,
   `undefined_from()`; exports.
2. `json_out.rs`: `warning_document`; `error_tail` with an optional class.
3. `main.rs`: the warnings after compile; `repl.rs`: `load` and `answer`.
4. Tests — unit: the record's three programs (each call site with its
   clause and position; the negated call; a query's own goal; a tabled
   declaration without clauses; calls inside a disjunction, an
   if-then-else and a comprehension body; built-ins and defined predicates
   not reported; `undefined_from` reaching `nosuch/1` through `s/1` and
   nothing through `p/1`). Integration: batch stderr lines with stdout and
   exit code unchanged; the JSON warning documents with stdout unchanged;
   the REPL silent on a typed clause, warning on `:load` and on the query.
5. Docs: `evaluator.md` (the pass and accessors), `answer-soundness.md`
   (invariant 3's sentence), `json-output.md` (a *Warnings* section),
   README (the diagnostics paragraph), architecture README (`cli` row);
   this plan.

## Findings

- **Program order needs its own record.** `preds` is a `HashMap`, so the
  pass cannot iterate it for a deterministic report; a `Site` list
  (clause by predicate and position, or query by index) is appended as
  clauses and queries are compiled, and the pass walks it. Without
  positions (the REPL) the order would otherwise differ from run to run.
- **One wording everywhere.** The record's batch sample said "is called
  here"; the implementation cites the clause in every mode —
  `nosuch/1 is called in `s(X) :- p(X), nosuch(X).` and no clause defines
  it` — since the clause is what a reader fixes, and the REPL has no
  position to point at.
- **`:load` matches the file's clauses by text.** The session keeps each
  clause with its origin; an undefined call's clause text is matched
  against the loaded file's clauses to decide whether `:load` reports it.
  A clause typed earlier with the same text is reported too, which is
  correct: it is an undefined call site of the session either way.
- **The `--json` shape needed one refactor.** `error_tail` now takes the
  class as an option, so the warning document shares the writer with the
  two error documents and differs only in the outer key and the absent
  `class`.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` (three unit tests in `undefined_calls`, one integration
      test each for batch, JSON and the REPL); existing golden files
      byte-identical (no example calls an undefined predicate).
- [x] A program without undefined calls prints exactly as before, in every
      mode (the goldens and the existing JSON and REPL suites).
- [x] `aivolution lint .` clean; the four `affects` docs current.
