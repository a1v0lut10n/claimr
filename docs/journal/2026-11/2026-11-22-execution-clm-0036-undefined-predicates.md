---
date: 2026-11-22
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-22-clm-0036-undefined-predicates-reported.md
design:
  - docs/design/2026-11-22-undefined-predicates-are-reported.md
meta-impact:
  note: "`claimr --json` now emits, before any query runs, one stderr document per call to a predicate no clause defines: `{\"warning\": {\"file\", \"line\"?, \"column\"?, \"kind\": \"undefined-predicate\", \"predicate\": \"nosuch/1\", \"clause\": \"…\", \"message\"}}` — the error document's shape with `warning` for `error` and no `class`; stdout and the exit code are unchanged, so a consumer that reads stderr only on a non-zero exit (aicogito's reasoning adapter) sees no change. The library gains `Program::undefined()`, `Program::undefined_from(&Query)`, `UndefinedCall` and `Query::span()`; genite's rule runner (GEN-141) can refuse a rule file with a non-empty list. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0036 executed: undefined predicates are reported

- **When:** 2026-11-22 10:26:48

## Context

The record accepted earlier today (PR #94): a call to a predicate no
clause defines still fails, but is reported — statically, at compile
time, once per load — so that genite's misspelt `dif(V, W)` and the
vacuously true `\+ nosuch(X)` are heard instead of reading as a clean
result. One task, with the compiler, the two CLI modes and the REPL to
touch.

## Details

**What was built.** After the predicates are indexed, a pass walks every
body goal of every clause and every query, constructs included, and
records each call whose key is in neither `preds` nor `builtins` as an
`UndefinedCall` — the name as source, the arity, the calling clause's
position and text, and crate-privately the caller — in program order
through a `Site` list appended as the compiler goes, since `preds` is a
hash map and could not give a deterministic order on its own. A
`:- table` declaration is not a definition. `Program::undefined()`
returns the list; `Program::undefined_from(&Query)` the entries a query
can reach, by a worklist over the call graph; `Query::span()` is public.
Nothing changes at run time. Batch mode prints one stderr line per call
site before any query — `file:line:col: warning: nosuch/1 is called in
`s(X) :- p(X), nosuch(X).` and no clause defines it` — with the exit
code and stdout unchanged; under `--json` one `warning` document each,
the error document's tail without `class` (`error_tail` takes the class
as an option). The REPL warns on `:load` for the entries whose clause
text is among the file's clauses, and before answering a query for
`undefined_from`; a clause typed at the prompt produces nothing.

**Verified.** 226 tests pass, six new: every call site reported with
its clause and position while the answers stay what they were; calls
inside a disjunction, an if-then-else and a comprehension body counted,
a tabled declaration without clauses counted, a quoted name spelled as
source, built-ins and defined predicates not reported, the clause cited
without a position; reachability through `u/1` to `s/1` and a query's
own call; batch stdout and exit code unchanged with three warning lines;
the JSON warning document; the REPL silent on a typed clause and warning
on the query and on `:load`. Goldens byte-identical; fmt, clippy and
lint clean. Invariant 3 of `answer-soundness` gains its sentence.

**Findings.** Program order needed its own record, the `Site` list,
because `preds` is a hash map. One wording everywhere: the record's
sample said "is called here"; every mode cites the clause, which is what
a reader fixes and all the REPL has. `:load` matches the file's clauses
by text, so an identical clause typed earlier is reported too — correct,
it is an undefined call site of the session. Nullary predicates are
written with their parentheses (`r() :- nosuch().`), which a test
learned the hard way.

**Pre-write checks.** The task's four `affects` docs were updated in the
PR: `evaluator.md`, `answer-soundness.md`, `json-output.md`, the
architecture README. Invariants untouched in meaning: the call still
fails; no decision the machine takes changes. Cross-repo impact: the
warning document and the library additions, noted above.

## Links

- Task: `docs/implementation/2026-11-22-clm-0036-undefined-predicates-reported.md`
- Design: `docs/design/2026-11-22-undefined-predicates-are-reported.md`
- PR #95: commits 068f4be (plan), 155e8bf (implementation)
- Consumer: genite GEN-141, `rust/crates/genite-mind-rules`
