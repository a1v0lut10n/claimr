# Reclaim solver state on backtracking

Captured 2026-11-08 (incubation), prompted by an experiment that checked
integrity rules over a knowledge base of about 2 500 claims (about 10 500
facts) on claimr `abd14aa`, where solving time grew quadratically with the
number of facts. Graduated to catalyst 2026-11-08 after the mechanism was
confirmed in the code and the measurements were reproduced on `main`
(`ea38acc`).

## The idea

A query that backtracks over many candidates should cost about the same per
candidate whether or not its constraint contains arithmetic. Today it does
not: every trial that fails leaves its solver variables and rows behind, and
later work pays for all of them. Two ways out, not exclusive:

1. **Truncate on undo.** A simplex mark records `vars.len()` and
   `rows.len()`, and undo truncates back to them.
2. **Fold ground arithmetic first.** When every operand of an arithmetic
   node is already a number, evaluate it exactly during instantiation
   instead of lowering it into the solver.

Either would make rule-heavy queries over many facts close to linear, which
is what using claimr as an integrity checker over a database needs.

## Evidence

### The mechanism, in the code

- `Simplex::undo_to` (`src/solver/simplex.rs`) restores bounds only; its
  own comment says "Values stay". `solver::Mark` is a single `usize`, the
  trail length. The `vars`, `rows` and `row_of` vectors are never truncated.
- This is a **documented deferral**, not an oversight:
  `docs/architecture/components/solver.md`, under *Backtracking*: "rows and
  solver variables created since a mark **persist** (relaxed bounds make
  them inert) — memory grows with a query's search, which is acceptable at
  Claimr's scale and noted for later work." The claim being revisited here is
  "acceptable at Claimr's scale": inert rows are not free, because
  `Simplex::update` iterates `for row in &self.rows` on every value change,
  so each assertion costs time proportional to every row ever made.
- `Store::post_arith` (`src/eval/store.rs`) allocates a fresh heap variable,
  a fresh solver variable and a defined row through `define_fresh` for every
  `Add` and `Sub`, and for a `Mul`/`Div` with one constant operand. It does
  so **even when both operands are constants**: `unknown_of` turns a bound
  number into `LinExpr::constant(c)`, and `LinExpr::as_constant()` exists,
  but only the `Mul`/`Div` arms consult it, and only to scale the other
  side. So `{ 10 * K > 10 * N }` with `K` and `N` bound creates two solver
  variables and two rows per trial, and `{ K > N }` creates none.
- The design record `docs/design/2026-08-17-arithmetic-in-terms.md` says
  "folding is exact and happens when the evaluator compiles a clause". No
  folding exists in `src/eval/compile.rs` or in `post_arith` today, so option
  2 implements a position already taken, at the instantiation step where the
  operands' values are actually known.

### Measurements

One fact per claim, `kn(C, K, N, T)`, rule
`violation(C) :- kn(C, K, N, T), { … }.`, query `?- violation(C).`, all
answers. Release build.

Original experiment (`abd14aa`):

| Constraint | 1 820 facts | 7 280 facts | 29 120 facts |
|---|---|---|---|
| `{ K > N }` | 0.02 s | 0.09 s | 0.47 s |
| `{ 10 * K > 10 * N }` | 2.63 s | 55.4 s | not run |

Reproduction on `main` (`ea38acc`), synthetic facts with `K`, `N` uniform in
100…300:

| Constraint | 1 820 facts | 3 640 facts |
|---|---|---|
| `{ K > N }` | 0.02 s | 0.04 s |
| `{ 10 * K > 10 * N }` | 1.88 s | 7.41 s |

Doubling the facts quadruples the arithmetic form and doubles the plain one.
The two forms print identical answers at both sizes (896 and 1 766
violations), so this is cost only; `answer-soundness` is not at stake.

## Shape

**Option 2 first.** It is small, local to `post_arith`, and removes the
common case regardless of what happens to option 1: when `la` and `lb` are
both constants, compute the rational result and return a heap cell bound to
it, with the existing division-by-zero failure kept. `Mul` of two constants
currently falls into the "scale the other side" arm and still defines a row,
so all four operators need the check. A test that counts solver variables
after a ground constraint would pin the behaviour.

**Option 1 is the structural fix** and harder. Rows are rewritten during
pivoting in terms of whichever variables are currently non-basic, so a row
created before the mark may, after a pivot, mention a variable created
after it. Truncating `vars` would leave dangling references. Two routes:

- trail pivots as well as bounds, so undo restores the tableau to its
  pre-mark basis before truncating; or
- never pivot a post-mark variable into a pre-mark row, which constrains
  the pivot rule (Bland's rule, as used for termination, may not be
  compatible).

Both have to keep the `answer-soundness` aspect: after undo the tableau
must still be a valid basis for the pre-mark system. The Dutertre–de Moura
paper's own backtracking scheme (bounds only, as implemented) is what led
here; what it assumes is that the set of rows is fixed per problem, which
does not hold once arithmetic terms create rows during search.

Not in scope: the join itself. `violation(C) :- k(C, K), n(C, N), …` over
two fact tables is quadratic for a different reason, clause scanning, and is
`2026-11-08-first-argument-indexing.md`'s problem. The measurements here
use a single fact table precisely to separate the two.

## Open questions

- Does `define_fresh` for a one-constant `Mul`/`Div` (`10 * K` with `K`
  unbound) also need a row, or could the scaled expression be carried as a
  `LinExpr` until a relation is asserted? That would reduce rows for the
  non-ground case too, at the cost of a representation change for numeric
  heap cells.
- For option 1, what does `row_of` look like after truncation, and does
  `is_determined` (probing) still see a consistent tableau?
- Is there a cheap guard worth adding now, such as a count of live solver
  rows reported by the CLI (and in the JSON output, under a new contract
  version), so the growth is visible before a design lands?

## Adjacent work to connect with

- `docs/architecture/components/solver.md` (the deferral quoted above) and
  `docs/architecture/components/evaluator.md`.
- `docs/design/2026-08-17-arithmetic-in-terms.md` (the folding promise) and
  `docs/design/2026-08-17-evaluator.md` D5/D6 (linearity as a solver
  check).
- `docs/architecture/aspects/answer-soundness.md`.
- `2026-11-08-first-argument-indexing.md` in incubation: the other half of
  making fact-table rules linear.
- CLM-0006 (`feature/CLM-0006-linear-store`) introduced the mark/undo
  scheme being revisited.
