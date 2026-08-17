---
type: architecture-component
last-verified: 2026-08-18
decisions:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
  - docs/design/2026-08-17-exact-rational-arithmetic.md
aspects: [exact-arithmetic, answer-soundness]
---

# solver

The linear constraint store over exact rationals — the numeric half of
Claimr's constraint store. Two layers:

- **`src/solver/`** — the pure solver: `Delta` (δ-rationals `c + kδ` for
  strict bounds), `LinExpr` (sparse linear expressions with `Number`
  coefficients), and `Simplex` — the general simplex of Dutertre & de Moura
  (CAV 2006): a tableau of rows over non-basic variables, per-variable
  lower/upper bounds, Bland's rule for termination, `assert_lower/upper`,
  `assert_constraint(expr op 0)`, `define(fresh_var, expr)`, `check`,
  `mark`/`undo_to` (bounds only), and `is_determined(x)` — exact, by probing
  `x < value` and `x > value` for feasibility.
- **the numeric glue in `src/eval/store.rs`** — how the heap and the solver
  meet: `numvar` (unbound heap variable → solver variable), the attribute
  term registry, numeric disequations, delayed products, alias classes, and
  the "determined" event that fires suspended wakers.

## Semantics as implemented (stage 3 — CLM-0006)

- **Numeric position.** Operands of `< > <= >=`; operands of `+ - * /` and
  unary `-`; and both sides of `=` / `!=` inside `{ … }` when either side is
  a number (a literal, or an arithmetic result) or an unbound *numeric
  variable*. Any other `=`/`!=` is tree unification / `dif`; head
  unification is always tree unification.
- **Unknown of a term** (`unknown_of`): a number is a constant; a heap
  variable gets (or has) a solver variable and is thereafter *numeric*; a
  compound or constant is an **attribute term** — its unknown is looked up in
  a registry keyed by the term's structure modulo bindings (a determined
  numeric variable keys by its value), created if absent, and the entry is
  suspended on every unbound variable inside it: when one is bound (or
  determined) the key is recomputed and, if another entry now has that key,
  the two unknowns are equated (**congruence**). Cyclic attribute terms
  raise `EvalError::CyclicAttributeTerm`.
- **Numeric variables stay variables of the solver.** Unifying one with a
  number is an equation (no heap binding); with another variable a merge
  (heap binding plus `equate`, or a transfer of the solver variable); with a
  compound or constant, that term becomes an attribute term equated with it.
  A determined numeric variable meeting a number or another determined
  variable compares values without touching the store, so trial
  unification (`dif`) sees "already equal". **Stated consequence of D4**:
  because any non-arithmetic tree can be an attribute term, "numeric
  typing" never fails on its own — `{ X > 3 }, X = foo` succeeds with
  residual `foo > 3` (Prolog III proper would fail); `p(3). ?- p(foo).`
  still fails, because a number literal in a term is a tree.
- **Arithmetic terms** are lowered at clause instantiation: each `Neg` /
  `Binary` becomes a fresh numeric heap variable `N` defined in the solver
  (`define`) as the linear expression; constants fold exactly. `t1 * t2` and
  `t1 / t2` are linear only if a factor (the divisor, for `/`) is a
  constant at posting time; otherwise the product is **delayed** and
  linearised as soon as a factor becomes determined (division by a
  determined zero fails). A product still pending when an answer would be
  produced stops the query with `EvalError::NonLinear` — never an
  approximate answer.
- **Numeric disequations** post `D = t1 − t2` (a defined slack) and are
  decided exactly: violated iff `D` is determined and zero, dropped iff
  determined and non-zero, otherwise pending — re-checked cheaply on
  determinations and exactly before every answer.
- **Determined variables.** After every store change, variables whose bounds
  coincide (or whose row is over such variables) count as determined and
  fire their heap variable's wakers (`dif`, attribute congruence); before an
  answer, `finalize` runs the exact probe on every numeric variable and
  disequation, tightens bounds to fixed values, re-checks every pending
  tree disequation, and rejects a non-linear residue.
- **Alias classes.** `equate(a, b)` asserts `a − b = 0` through a defined
  slack and records `b → a`; printing treats a class as one variable.
- **Backtracking.** Bounds are trailed in the simplex; the registry, numeric
  variable map, disequations, products, aliases and "fired" set are trailed
  in the store; rows and solver variables created since a mark **persist**
  (relaxed bounds make them inert) — memory grows with a query's search,
  which is acceptable at Claimr's scale and noted for later work.

## Answer rendering

Answers are produced by the evaluator's projection (`src/eval/project.rs`,
see [`evaluator.md`](evaluator.md)): the visible part of this store is
collected as a residual linear system, internal variables are eliminated
(Gaussian, then Fourier–Motzkin within a budget — a survivor is named
`_N`, never mis-printed), redundant constraints are removed exactly, and
the result prints in solved form: fixed values inline, `Y = X + 1`, `X > Y`,
`X + Y <= 10`, `age(alice) >= 18`, `X != 3`. World constraints appear only
on unknowns the query touched. Rows and definitions never print directly.
