# Machine-readable evaluation output (`--json`)

CLM-0010. `claimr --json <file.claimr>` runs the program's `?-`
queries in order — exactly as the human batch mode does — and emits
**one JSON document per query** on stdout (NDJSON: one document per
line). Diagnostics go to stderr as one JSON document. The human batch
format and the REPL are untouched; `--limit N` caps the answers
collected per query as in human mode. `--json` is batch-only: it
refuses the REPL forms and `--parse`.

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
    disequations (`age(alice) = 30`).
  A proof that binds nothing (the human mode's `true`) is one answer
  whose three lists are all empty.

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
