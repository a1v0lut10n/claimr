# Machine-readable evaluation output (`--json`)

CLM-0010. `claimr --json <file.claimr>` runs the program's `?-`
queries in order — exactly as the human batch mode does — and emits
**one JSON document per query** on stdout (NDJSON: one document per
line). Diagnostics go to stderr as one JSON document. The human batch
format and the REPL are untouched; `--limit N` caps the answers
collected per query as in human mode, `--explain` (CLM-0023) adds the
`explanation` field below. `--json` is batch-only: it refuses the REPL
forms and `--parse`.

The contract is pinned by `tests/json_output.rs`; consumers (the
aicogito reasoning adapter among them) dispatch on these shapes, so a
change here is a consumer break and must be deliberate.

## Per query

```json
{"query": "?- slot(s1, R, B).",
 "outcome": "solutions",
 "answers": [
   {"equations": ["R = expression", "B = supported"],
    "disequations": [],
    "constraints": []}
 ]}
```

- `query` — the query text as written, with `?-` and the dot.
- `outcome` — `"solutions"` when at least one answer exists,
  `"none"` otherwise (the human mode's `false`). Failure to find a
  solution is failure within this program, not real-world negation.
- `answers` — one object per answer, in solved form:
  - `equations` — one `X = t` string per bound query variable
    (cyclic terms print as their finite equations, e.g. `X = f(X)`);
  - `disequations` — pending tree disequations (`t1 != t2`);
  - `constraints` — numeric bounds, linear equations, and numeric
    disequations (`age(alice) = 30`). Since CLM-0015 a constraint may
    mention the length of a list variable, `|T| >= 1`, since CLM-0016 a
    pending list concatenation, `Z = X . Y`, and since CLM-0017 a pending
    built-in in goal form, `atom(X)`, `list(X)`, `atom_chars(A, L)`; the
    shape of the document is unchanged, the vocabulary of the strings grew.
  A proof that binds nothing (the human mode's `true`) is one answer
  whose three lists are all empty.

## Explanations (`--explain`, CLM-0023)

With `--explain` each answer object carries one more field, `explanation`,
the answer's proof tree
(`docs/design/2026-11-09-explanations-of-answers.md`): a list of root
nodes, one per query goal, each

```json
{"goal": "violation(c17)", "by": "rule", "line": 3, "column": 1,
 "clause": "violation(C) :- k(C, K), n(C, N), { K > N }.",
 "subgoals": [
   {"goal": "k(c17, 5)", "by": "fact", "line": 1, "column": 1, "clause": "k(c17, 5).", "subgoals": []},
   {"goal": "n(c17, 3)", "by": "fact", "line": 2, "column": 1, "clause": "n(c17, 3).", "subgoals": []},
   {"goal": "{ 5 > 3 }", "by": "constraint", "line": 3, "column": 1, "subgoals": []}
 ]}
```

- `goal` — the goal as it stands at the answer, rendered with the answer's
  bindings and variable names (a committed or failed condition: the
  construct as written);
- `by` — what resolved it: `fact`, `rule` (its body goals are the
  `subgoals`), `constraint` (a `{ … }` goal, as the instance posted),
  `builtin`, `committed` (an if-then-else whose condition succeeded; the
  condition's proofs are the `subgoals`, the then branch follows as
  siblings), `no-solution` (a condition, or a negated goal, that had none);
- `line`, `column`, `clause` — the cited clause's position (1-based) and
  text as written, present for facts and rules, position only for a
  constraint (the clause it is in); absent when nothing is cited.

Without `--explain` the field is absent and the document is byte-identical
to the shape above; a consumer that does not know the field can ignore it.

## A query that fails at runtime

```json
{"query": "?- q(X).", "outcome": "error", "error": {"message": "…"}}
```

Printed for the failing query (stdout), then the run stops — exit 1,
as in human mode.

## A file that never runs

Read, parse, and compile failures emit one document on **stderr**:

```json
{"error": {"file": "f.claimr", "line": 2, "column": 1, "message": "Expected one of …"}}
```

`line` and `column` (1-based) appear only when known.

## Exit codes

Identical to human mode: `0` all queries ran; `1` parse/compile
failure or a query's runtime error; `2` usage or unreadable file.
