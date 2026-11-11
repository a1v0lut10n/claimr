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
    built-in in goal form, `atom(X)`, `list(X)`, `atom_chars(A, L)`, and
    since CLM-0025 a pending sum, `sum(L, S)`; the shape of the document is
    unchanged, the vocabulary of the strings grew.
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
  `subgoals`), `constraint` (a `{ … }` goal as written, with the derivation's bindings
  for its variables and its arithmetic, lengths and lists in source form —
  `{ 5 > 3 }`, `{ 3 = |<a, b, c>| }`, `{ Z = X + 1 }`; CLM-0032),
  `builtin`, `committed` (an if-then-else whose condition succeeded; the
  condition's proofs are the `subgoals`, the then branch follows as
  siblings), `no-solution` (a condition, or a negated goal, that had none),
  `collected` (a list comprehension, CLM-0025, as written with the list it
  produced: `<X | p(X)> = <1, 2>`; the solutions' own proofs are not kept),
  `tabled` (a call to a tabled predicate, CLM-0027, answered from its
  table; the `subgoals` are the derivation that first produced the answer);
- `line`, `column`, `clause` — the cited clause's position (1-based) and
  text as written, present for facts and rules, position only for a
  constraint (the clause it is in); absent when nothing is cited.

Without `--explain` the field is absent and the document is byte-identical
to the shape above; a consumer that does not know the field can ignore it.

## Failure (`--explain`, CLM-0031)

With `--explain` a query with `"outcome": "none"` carries one more field,
`failure`, its failure tree
(`docs/design/2026-11-11-explanations-of-failure.md`):

```json
{"query": "?- eligible(bob), { age(bob) < 18 }.", "outcome": "none", "answers": [],
 "failure": {"branches": [[
   {"goal": "eligible(bob)", "by": "rule", "line": 1, "column": 1,
    "clause": "eligible(X) :- { age(X) >= 18 }.",
    "subgoals": [{"goal": "{ age(bob) >= 18 }", "by": "constraint", "line": 1, "column": 1, "subgoals": []}]},
   {"goal": "{ age(bob) < 18 }", "by": "failed", "reason": "unsatisfiable",
    "conflict": [
      {"goal": "eligible(bob)", "by": "rule", "line": 1, "column": 1, "clause": "eligible(X) :- { age(X) >= 18 }.", "subgoals": []},
      {"goal": "{ age(bob) >= 18 }", "by": "constraint", "line": 1, "column": 1, "subgoals": []}],
    "subgoals": []}
 ]], "omitted": 0}}
```

- `branches` — the derivations the search tried, in search order, at most
  64; each a list of root nodes in the shape of `explanation`, ending at
  the node that failed;
- `omitted` — how many further failed branches the budget left out;
- a failed node has `"by": "failed"` and a `reason`: `unsatisfiable` (the
  step's store effect had no solution; `conflict` lists the earlier steps
  it conflicts with as shallow nodes, a minimal set, or every step with a
  `note` saying why it could not be minimised), `no-clause` (no candidate
  clause resolved the call; `clauses` is the predicate's clause count,
  `candidates` the number the first-argument index admitted), `has-solution`
  (a negation whose goal succeeded; the solution's proof is the node's
  `subgoals`), `no-table-answer` (a tabled call; `answers` is the table's
  size), `no-solution` (an if-then-else without an else whose condition
  failed). The goal is the failed goal as it stood, or `answer` for the
  answer-time check.

Without `--explain`, or when the query has answers, the field is absent and
the document is byte-identical to the shapes above.

## A query that fails at runtime

```json
{"query": "?- anc(a, X).", "outcome": "error",
 "error": {"kind": "step-limit", "class": "resource", "steps": 1000, "goal": "`anc(a, _1)`",
           "message": "step limit of 1000 reached at `anc(a, _1)`; the query may not terminate …"}}
```

Printed for the failing query (stdout), then the run stops — exit 1,
as in human mode. Since CLM-0033
(`docs/design/2026-11-11-json-error-kind-and-class.md`) the error object
says what kind of error it is, in both this document and the file-level
one below:

- `kind` — a stable kebab-case name for the precise error, one per row of
  the table; new kinds may be added over time;
- `class` — what the caller should do about it, from a **closed** set that
  changes only by a design record: `syntax` (the text is not a program),
  `program` (the program or query breaks a rule of the language, at load
  or at run), `unsupported` (claimr declines to decide this; it never
  approximates), `resource` (a budget the caller set ran out), `io` (the
  file could not be read), `internal` (an invariant failed; a bug to
  report). A consumer routes on `class` and is never stranded by a kind it
  has not seen;
- the error's **data as fields**, named as in the table, so a consumer
  reads values instead of parsing the message;
- `message` — the human diagnostic, unchanged: the same text human mode
  prints.

| `kind` | `class` | fields | where |
|---|---|---|---|
| `syntax` | `syntax` | — | file |
| `read` | `io` | — | file |
| `reserved-name` | `program` | `name` | file |
| `reserved-builtin` | `program` | `name`, `arity` | file |
| `bad-directive` | `program` | — | file |
| `comprehension-in-head` | `program` | — | file |
| `unsatisfiable-facts` | `program` | `conflict`: `[{"line"?, "column"?, "clause"}]`, a minimal conflicting set of constraint facts (CLM-0030) | file |
| `non-ground-condition` | `program` | `construct`, `variable`, `clause` | query |
| `non-ground-solution` | `program` | `construct`, `instance` | query |
| `non-ground-tabled-answer` | `program` | `predicate`, `instance` | query |
| `constrained-tabled-answer` | `program` | `predicate`, `instance`, `residual` | query |
| `incomplete-table` | `program` | `predicate`, `construct` | query |
| `cyclic-tabled-call` | `program` | `call` | query |
| `non-linear` | `unsupported` | `constraint` | query |
| `cyclic-attribute-term` | `unsupported` | `term` | query |
| `step-limit` | `resource` | `steps` (a number), `goal` | query |
| `internal` | `internal` | — | query |

Field order after the positional fields is `kind`, `class`, the data,
`message`. A consumer written against the earlier contract, which read
only `message`, sees no change.

## A file that never runs

Read, parse, and compile failures emit one document on **stderr**:

```json
{"error": {"file": "f.claimr", "line": 2, "column": 1, "kind": "syntax", "class": "syntax",
           "message": "Expected one of …"}}
```

`line` and `column` (1-based) appear only when known. A program whose
constraint facts are unsatisfiable is such a failure:

```json
{"error": {"file": "f.claimr", "kind": "unsatisfiable-facts", "class": "program",
           "conflict": [{"line": 1, "column": 1, "clause": "{ age(bob) >= 18 }."},
                        {"line": 3, "column": 1, "clause": "{ age(bob) < 10 }."}],
           "message": "the program's constraint facts are unsatisfiable (no models); a minimal conflicting set:\n  { age(bob) >= 18 }.  fact 1:1\n  { age(bob) < 10 }.  fact 3:1"}}
```

The `message` still carries the cited facts as lines, as human mode prints
them; `conflict` carries them as records.

## Exit codes

Identical to human mode: `0` all queries ran; `1` parse/compile
failure or a query's runtime error; `2` usage or unreadable file.
