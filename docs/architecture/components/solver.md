---
type: architecture-component
last-verified: 2026-11-09
decisions:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md
  - docs/design/2026-11-09-lists-as-a-constraint-domain.md
  - docs/design/2026-11-09-truncate-the-tableau-on-undo.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
  - docs/design/2026-08-17-exact-rational-arithmetic.md
  - docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md
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
  `mark`/`undo_to` (bounds and the variable and row set, CLM-0018), and
  `is_determined(x)` — exact, by probing `x < value` and `x > value` for
  feasibility.
- **the numeric glue in `src/eval/store.rs`** — how the heap and the solver
  meet: `numvar` (unbound heap variable → solver variable), the attribute
  term registry, numeric disequations, delayed products, list lengths and
  concatenations, alias classes, and the "determined" event that fires
  suspended wakers. `unknown_of`, the solver's view of a term in numeric
  position, returns a determined variable's **value** rather than the
  variable (CLM-0024): a constraint over values the store already knows
  folds as if the numbers had been written, defines no row and asserts no
  slack, so a value flowing through arithmetic step after step costs the
  solver nothing; in every solution the variable has that value, and the
  trail undoes the posted constraint together with the determination it
  relied on.

## Semantics as implemented (stage 3 — CLM-0006)

- **Numeric position.** Operands of `< > <= >=`; operands of `+ - * /` and
  unary `-`; and both sides of `=` / `!=` inside `{ … }` when either side
  *denotes a number* — dereferenced, a number cell (a literal, an
  arithmetic result, or a variable bound to a number by whatever route) or
  an unbound *numeric variable* (rule 2 of
  `2026-11-09-numbers-numeric-variables-and-attribute-terms.md`, CLM-0026).
  Any other `=`/`!=` is tree unification / `dif`; head unification is
  always tree unification.
- **Unknown of a term** (`unknown_of`): a number is a constant; a heap
  variable gets (or has) a solver variable and is thereafter *numeric*; a
  compound or constant is an **attribute term** — its unknown is looked up in
  a registry keyed by the term's structure modulo bindings (a determined
  numeric variable keys by its value), created if absent, and the entry is
  suspended on every unbound variable inside it: when one is bound (or
  determined) the key is recomputed and, if another entry now has that key,
  the two unknowns are equated (**congruence**). Cyclic attribute terms
  raise `EvalError::CyclicAttributeTerm`.
- **Numeric variables and numbers** (CLM-0026,
  `2026-11-09-numbers-numeric-variables-and-attribute-terms.md`). A numeric
  variable is a variable of the solver until its value is known; the moment
  the store determines it — a fixing equation, coinciding bounds, the exact
  probe before an answer — it is **bound to its number** (a fresh number
  cell, trailed like any binding), and from then on it *is* the number: a
  tree leaf for unification and for first-argument indexing, a number for
  the `=`/`!=` rule above, its value for arithmetic and printing (D5 as
  written). Unifying an undetermined numeric variable with a number fixes
  it and binds it the same way; with another variable it merges (heap
  binding plus `equate`, or a transfer of the solver variable); with a
  compound or constant it **fails** — unification never creates an
  attribute term (D4 as written). Attribute terms arise only from a
  compound or constant that is a direct operand of a block relation:
  `{ X > 3 }` followed by a head binding `X` to `foo` fails, while `X`
  bound to `foo` first and then `{ X > 3 }` answers `foo > 3`; inside
  structures numbers are leaves (`{ <2> = <f(a)> }` fails); `p(3).
  ?- p(foo).` fails, as ever.
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
  coincide (or whose row is over such variables) count as determined, are
  bound to their number and fire their heap variable's wakers (`dif`,
  attribute congruence); before an
  answer, `finalize` runs the exact probe on every numeric variable and
  disequation, tightens bounds to fixed values, re-checks every pending
  tree disequation, and rejects a non-linear residue. The pass walks only
  its open **candidates** (CLM-0024): the numeric variables still unbound
  (a determined one has left by its binding, CLM-0026), the pending
  disequations and the pending products, kept as
  sets maintained at every creation, settle, bind and undo (a variable
  leaving the set by being bound is trailed), so a long derivation does not
  rescan what it has already settled; `class_value` likewise looks at a
  class's root and its aliases, not at every solver variable. Before this a
  numeric countdown was quadratic (4.6 s for 2 000 steps, 19.5 s for 4 000,
  release build); it now runs 16 000 steps in 0.05 s. Within the solver
  itself, `check`, `update` and `pivot` still scan every row, so a store
  with many **open** rows pays per row on each assertion — a column index
  is the natural next step if that shows up.
- **Alias classes.** `equate(a, b)` asserts `a − b = 0` through a defined
  slack and records `b → a`; printing treats a class as one variable.
- **Ground arithmetic never reaches the solver** (CLM-0013,
  `2026-11-09-fold-ground-arithmetic-before-the-solver.md`): when every
  operand of `+ - * /` or unary `-` is a number at instantiation,
  `post_arith`/`post_neg` compute the exact rational and bind a `Num` cell —
  no solver variable, no row. Ground division by zero fails there. Only
  terms with an unbound or attribute operand are lowered.
- **Sums** (CLM-0025, `sum(L, S)`): `post_sum` walks the spine as
  `post_len` does; a complete list posts the linear equation
  `S − e1 − … − en = 0` through `unknown_of` on each element (a number, a
  numeric variable, or an attribute term; a variable held by a type
  relation fails), so the equation is exact and solvable in either
  direction; an open spine registers a pending `Sum` entry suspended on the
  tail (`Waker::Sum`, `recheck_sum`), undone with the trail; a pending one
  prints in goal form. The design record is
  `2026-11-09-aggregation-list-comprehensions-and-sum.md`.
- **List lengths** (CLM-0015,
  `2026-11-09-lists-as-a-constraint-domain.md`, stage 2). `|L|` is posted
  by `post_len`, which follows the `'<>'` spine of `L`: a complete list
  folds to the number of its cells (no solver variable, as for ground
  arithmetic); a spine ending in an unbound non-numeric variable `T` after
  `k` cells is `k + |T|`, where `|T|` is a **length entry** registered once
  per list variable (`len_index`, so every `|T|` is one unknown): a numeric
  heap variable with `>= 0` asserted, suspended on `T` and on itself. A
  cycle or a non-list (a number, another constant or compound, a numeric
  variable) fails. Binding `T` re-walks the spine and asserts `|T| = k` or
  `|T| = k + |U|` through `U`'s entry; the entry is then closed. The
  determination of `|T|` while `T` is unbound requires a non-negative
  integer `n` (else failure) and binds `T` to `<_1, …, _n>`, Prolog III's
  solved form for a list of known length. A list variable bound to
  another variable asserts `|T| = |U|` through `U`'s entry. Entries and the
  index are trailed like products and the attribute index.
- **List concatenation** (CLM-0016, stage 3 of the same record). `X . Y`
  is posted by `post_concat`. A complete `X` gives the chain of its heads
  ending in `Y` at once, no constraint (the operation face). An open `X`
  gives a fresh list variable `Z`, posts `|Z| = |X| + |Y|` through the
  length entries (so a non-list `Y` fails here) and registers a pending
  **concatenation entry** suspended on `X`'s open tail. When that tail is
  bound the spine is re-walked: complete → `Z` is unified with the chain
  of `X`'s heads ending in `Y` and the entry closes; still open → the entry
  waits on the new tail; cyclic or non-list → failure. A length determined
  by the simplex reaches here through the stage 2 expansion, which binds
  the tail. Concatenations are never searched (Prolog III's `enum` is a
  program's business, `append/3` here); one still pending at answer time
  is a sound residual, printed `Z = X . Y`, unlike a pending product.
- **Backtracking.** Bounds are trailed in the simplex; the registry, numeric
  variable map, disequations, products, lengths, concatenations, type
  relations, `atom_chars`, aliases and the candidate sets are trailed in
  the store. Since CLM-0018 (`2026-11-09-truncate-the-tableau-on-undo.md`)
  `Simplex::undo_to` also **truncates the tableau**: a mark records the
  variable count, and on undo every post-mark variable that pivots wrote
  into a pre-mark row is pivoted back into basic position there, the rows
  whose basic is post-mark are dropped (they define fresh variables and
  nothing else), the tables are truncated, and surviving non-basics are
  moved back inside their restored bounds before `check`. The restored
  tableau is the pre-mark system in a possibly different basis, which
  nothing observes. Solver variable indices are therefore **reused**: no
  code may hold an `SVar` across a mark except through the trailed store,
  and the printing definitions of dropped variables are removed on undo.
  Rows no longer accumulate across a search, so a query over F candidates
  costs O(F) in the solver; the measurements are in the CLM-0018 plan.

## Answer rendering

Answers are produced by the evaluator's projection (`src/eval/project.rs`,
see [`evaluator.md`](evaluator.md)): the visible part of this store is
collected as a residual linear system, internal variables are eliminated
(Gaussian, then Fourier–Motzkin within a budget — a survivor is named
`_N`, never mis-printed), redundant constraints are removed exactly, and
the result prints in solved form: fixed values inline, `Y = X + 1`, `X > Y`,
`X + Y <= 10`, `age(alice) >= 18`, `X != 3`. World constraints appear only
on unknowns the query touched. Rows and definitions never print directly.
