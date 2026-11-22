---
date: 2026-11-22
type: design
status: proposed
components: [evaluator, cli]
aspects: [answer-soundness]
tags: [undefined-predicate, warnings, compile, cli, repl, json-output, integrity-checker, library]
---

# A call to a predicate no clause defines is reported, not silently failed past

## Context

A call to a predicate with no clause fails. That is what the evaluator
record decided and what invariant 3 of `answer-soundness` states —
"undefined predicates fail (no clauses ⇒ no solutions)" — and it is the
semantics Prolog gives under `unknown = fail`: a program is a set of
clauses, and a goal nothing defines has no derivation. Nothing in this
record changes that.

**What is wrong is the silence.** genite's first rule written against the
claimr library (GEN-141, incubation note
`2026-11-20-a-call-to-an-undefined-predicate.md`) used `dif(V, W)` where
claimr writes `{ V != W }`; the rule found nothing, and nothing said why.
Three shapes of the same accident, checked on 0.7.0:

```claimr
p(a).
s(X) :- p(X).
s(X) :- p(X), nosuch(X).
ok(X) :- p(X), \+ nosuch(X).
?- nosuch(X).
?- s(X).
?- ok(X).
```

The first query prints `false`, and `--explain` does name the cause
(`nosuch(X)  failed: no clause (undefined predicate)`). The second prints
`X = a` and never mentions `nosuch/1`: the clause that calls it fails
quietly beside one that succeeds, and no failure tree is recorded for a
query that has an answer. The third is the worst: `\+ nosuch(X)` succeeds
vacuously, so a check written as "no claim has a counter-example" passes
because the predicate that would find one is misspelt. For a rule file
whose job is to find violations, a clean result that is really a typo is
the most expensive failure there is, and through the library nothing short
of asking `Solutions::failure()` on every query would show it.

**What exists to say it.** The compile step (`compile.rs`) knows every
predicate that has a clause (`preds`), every built-in (`builtins`) and
every `:- table` declaration; it already walks every body goal, constructs
included, to compute the variables a goal mentions (`goals_vars`), and it
keeps each clause's position and text. Every call in a body, a condition,
a negation, a comprehension body or a query is therefore visible to it
with its clause and position. The CLI prints load errors as
`file:line:col: message` and, under `--json`, as a stderr document; it has
no warnings today. The REPL recompiles the whole session after every
clause typed or file loaded.

**Prior art.** ISO Prolog's `unknown` flag defaults to `error`: a call to
an unknown procedure raises `existence_error(procedure, Name/Arity)`, and
`fail` is the opt-in. SWI-Prolog adds `:- dynamic Name/Arity` to declare
a predicate that may legitimately have no clauses, and `list_undefined/0`
in `library(check)` as the static report. Mercury and Erlang make an
undefined call a compile error outright. Claimr keeps the failure
semantics — a clause set is a logic program and the answer to a goal with
no clauses is `false` — and takes the static report, which costs nothing
at run time and says what the author needs to hear once.

## Decision

1. **The compiled program carries its undefined calls.** `Program::compile`
   and `compile_spanned` keep returning `Ok`, and the program answers
   `undefined() -> &[UndefinedCall]`: one entry per call site whose
   predicate has no clause, is not a built-in, and — a declaration is not
   a definition — regardless of a `:- table` declaration;
   `UndefinedCall { name, arity, span: Option<Span>, clause: String }`,
   the clause being the calling clause or query as written, in program
   order. A call inside a condition, a negation, a comprehension body or a
   disjunction counts like any other; a query's own goals count too. The
   embedding program decides what the list means: genite treats a
   non-empty list as an error; a program loaded in parts may ignore it
   until the last part.

2. **The command line warns, once per load.** Batch mode prints, before
   running any query, one line per undefined call site on stderr in the
   diagnostic format: `file:line:col: warning: nosuch/1 is called here and
   no clause defines it`, with the calling clause's position; the run then
   proceeds as today and the exit code is unchanged — a warning is not a
   failure. Under `--json` each warning is one stderr document,
   `{"warning": {"file", "line"?, "column"?, "kind": "undefined-predicate",
   "predicate": "nosuch/1", "clause": "…", "message"}}`, in the shape of
   the error document with `warning` for `error` and no `class` (a
   warning asks nothing of the caller); stdout is untouched, so a
   consumer that reads stderr only on a non-zero exit sees no change.

3. **The REPL warns when it matters.** A session grows clause by clause,
   and a rule may call a predicate the next line will define, so the REPL
   does not warn on a clause typed at the prompt. It warns on `:load` for
   the file's own undefined calls, in the batch format, and before
   answering a query it warns for the undefined predicates the query can
   reach — the call sites in the query's goals and in the clauses of the
   predicates reachable from them, computed over the call graph the
   compiler has — so that `?- s(X).` says `warning: nosuch/1 is called in
   `s(X) :- p(X), nosuch(X).` and no clause defines it` and then answers
   `X = a`. The REPL design record is append-mostly and is not amended;
   the task documents the behaviour in `evaluator.md` and the README.

4. **Semantics unchanged.** No call is evaluated differently, no answer
   changes, no error is raised: invariant 3 holds word for word, and
   `--explain` keeps naming the undefined predicate at its leaf. The
   report is a property of the program text, computed at compile time,
   and costs nothing during a query.

5. **Not in this record.** A declaration for a predicate allowed to have
   no clauses (`:- dynamic p/1.` in Prolog's words), which this record
   leaves out until a program needs one — every call in the programs seen
   so far is meant to be defined. A `--strict` flag turning the warning
   into a load error in batch mode: the library gives the embedding
   program the choice, and the flag is a small follow-up if the binary is
   ever used as a checker on its own. Calls built at run time: claimr has
   none. Other static checks a compiler could make — singleton variables,
   unreachable clauses — each its own record if wanted.

## Consequences

- **The accident becomes loud at the right moment.** A misspelt predicate
  is reported at load, before any query runs, with the clause and position
  to fix; genite's rule file fails its check instead of reporting no
  violations; the REPL user sees it when the query that depends on it
  runs. The failure tree of CLM-0031 remains the place that says why a
  particular query failed; this record covers the queries that did not
  fail.

- **Soundness.** Untouched: `answer-soundness` invariant 3 stands, and the
  task adds a sentence that the undefined calls are reported statically
  while the call still fails. No decision the machine takes changes.

- **Compiler.** A pass after lowering collects `TGoal::Call` heads across
  bodies, constructs and queries against `preds`, `builtins`; the
  reachability for the REPL's per-query warning is a closure over the same
  call graph, small for the programs claimr runs. `UndefinedCall` is a
  public type; `Program::undefined()` borrows the list.

- **CLI and REPL.** The batch warning line and the stderr JSON document;
  `:load` and the pre-query warning in the REPL; `json-output.md` gains a
  *Warnings* section with the document (`kind` open, as for errors; no
  `class`); the README's diagnostics paragraph names the warning; the
  architecture README's `cli` row lists it.

- **Contract impact.** A new stderr document type, additive and emitted
  only with exit 0 alongside normal stdout, flagged to aivolution-meta at
  acceptance; aicogito's adapter reads stderr only on failure and is
  unaffected until it chooses to read warnings.

- **One task** implements the pass, the type and accessor, the batch and
  JSON warnings, the REPL's two moments, and tests: the three programs
  above (each call site reported with its clause and position; the
  negated call reported like any other; a query's own undefined goal
  reported; a tabled declaration without clauses reported; built-ins and
  defined predicates not reported); the REPL's silence on a typed clause
  and warning on the query; the JSON document; byte-identity of stdout
  and exit codes; the library's `undefined()` on a program with none.

## Sources

- Incubation note `2026-11-20-a-call-to-an-undefined-predicate.md`
  (graduated by this record); genite GEN-141,
  `rust/crates/genite-mind-rules` (the first library consumer).
- `docs/design/2026-08-17-evaluator.md` and
  `docs/architecture/aspects/answer-soundness.md`, invariant 3: undefined
  predicates fail.
- `docs/design/2026-11-11-explanations-of-failure.md`: the `no-clause`
  leaf that names an undefined predicate when the query fails.
- ISO/IEC 13211-1:1995, §7.12.2 (`existence_error`) and the `unknown`
  flag; SWI-Prolog, `dynamic/1` and `library(check)`'s `list_undefined/0`.
- `src/eval/compile.rs` (`preds`, `builtins`, `goals_vars`), `src/main.rs`,
  `src/repl.rs`, `docs/reference/json-output.md`.
