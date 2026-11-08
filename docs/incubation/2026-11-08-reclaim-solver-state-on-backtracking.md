# Reclaim solver state on backtracking

Captured 2026-11-08. Prompted by an experiment that checked integrity rules over a
knowledge base of about 2 500 claims (about 10 500 facts), on claimr `abd14aa`, where
solving time grew quadratically with the number of facts.

The cause is in the simplex:

- Arithmetic in a constraint (`{ 10 * K < 7 * N }`) is lowered into solver variables and
  defined rows.
- `Simplex::undo_to` (`src/solver/simplex.rs`) restores bounds only. Its own comment says
  "Values stay". The `vars` and `rows` vectors are never truncated.
- So every trial that fails leaves its variables and rows behind, and `update` iterates
  `for row in &self.rows`. Each new assertion costs time proportional to every row ever
  made, and a query that tries F candidates costs O(F²).

Measured on one fact per claim, `kn(C, K, N, T)`, with the rule
`violation(C) :- kn(C, K, N, T), { … }`:

| Constraint | 1 820 facts | 7 280 facts | 29 120 facts |
|---|---|---|---|
| `{ K > N }` | 0.02 s | 0.09 s | 0.47 s |
| `{ 10 * K > 10 * N }` | 2.63 s | 55.4 s | not run |

The answers are correct in both cases. Only the cost differs.

Two ways out, not exclusive:

1. **Truncate on undo.** A simplex mark records `vars.len()` and `rows.len()`, and undo
   truncates back to them. Pivoting rewrites existing rows in terms of new variables, so
   this needs either to undo pivots (trailing row changes), or to keep new variables out
   of older rows' bases. The `answer-soundness` aspect would need to hold either way.
2. **Fold ground arithmetic first.** When every variable in an arithmetic expression is
   already bound to a number, evaluate it during instantiation, before it reaches the
   solver. `10 * 181 < 7 * 199` is then a comparison of two rationals, and no solver
   variable is made. This removes the common case at once, whatever happens to option 1.

Either would make rule-heavy queries over many facts close to linear, which is what
using claimr as an integrity checker over a database needs.

Relevant: `docs/architecture/components/solver.md`, `docs/architecture/components/evaluator.md`,
`docs/design/2026-08-17-arithmetic-in-terms.md`.
