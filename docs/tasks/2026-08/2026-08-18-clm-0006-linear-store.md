---
date: 2026-08-18
type: task
status: planned
affects:
  - docs/architecture/README.md
  - docs/architecture/components/evaluator.md
  - docs/architecture/aspects/answer-soundness.md
  - README.md
  - CLAUDE.md
components: [solver, evaluator, cli]
aspects: [answer-soundness, exact-arithmetic]
design:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
  - docs/design/2026-08-17-exact-rational-arithmetic.md
tags: [CLM-0006, solver, simplex, constraints, evaluator-stage-3]
---

# CLM-0006: The linear constraint store over exact rationals

## Objective

Stage 3 of the evaluator plan
([evaluator record](../../design/2026-08-17-evaluator.md) D4/D5,
[arithmetic-in-terms](../../design/2026-08-17-arithmetic-in-terms.md)):
turn claimr from a Prolog into a constraint language. Numeric relations
(`< > <= >= = !=` on numbers), arithmetic terms and attribute terms are
accepted and solved exactly over Q by an incremental linear solver plugged
into the stage-2 store, trail and suspension; determined variables are bound;
answers show the numeric store. After this task the README's examples run.

## Context

Stage 2 left the `solver` component as the one missing piece: the store has
a heap, trail, per-variable suspension, rational-tree unification and `dif`;
the compile step rejects anything numeric. Prolog III's numerical module was
an incremental simplex over unbounded-integer fractions, used "both to verify
if the numerical constraints have solutions and to detect those variables
having only one possible value". Claimr's is the same in spirit, built on a
modern, well-documented general simplex.

## Design details fixed by this task

- **Solver algorithm.** The general simplex of Dutertre & de Moura (*A Fast
  Linear-Arithmetic Solver for DPLL(T)*, CAV 2006): a tableau of linear rows
  over solver variables, per-variable lower/upper bounds, Bland's rule for
  termination, incremental `assert bound` with backtracking by restoring
  bounds (rows are never removed within a query — a row whose bounds are
  relaxed to (−∞, +∞) constrains nothing). Strict inequalities via
  δ-rationals (values `c + kδ`, both exact `Number`s). All arithmetic on
  `Number` (`exact-arithmetic`); no floats anywhere.
- **What is a numeric position.** Operands of `< > <= >=`; operands of
  `+ - * /` and unary `-`; and both sides of `=` / `!=` when either side is
  a `Number`, a numeric variable, an arithmetic term, or an attribute term
  already registered. Everything else is tree unification / `dif` (stage 2).
- **Unknown of a term** (`unknown_of(addr) → SolverVar`): a `Number` is a
  constant; a heap `Var` gets (or has) a solver variable — the variable is
  then *numeric*; a `Const`/`Struct` in numeric position is an **attribute
  term**: its unknown is looked up in a registry keyed by the term's
  structure modulo current bindings (functor, arity, dereferenced arguments
  recursively), created if absent, and the attribute term is **suspended on
  every unbound variable inside it** — when one is bound the key is
  recomputed and, if another entry now has the same key, the two unknowns
  are equated (congruence, D4/D5). Cyclic attribute terms are rejected
  (`EvalError`-level: unsupported).
- **Numeric variables and the heap.** When the heap binds a numeric
  variable `v` (in `bind`): to a `Number` → assert `sv(v) = c`; to a
  `Var` → equate solver vars (or transfer if the other has none); to a
  `Const`/`Struct` → that term becomes an attribute term and its unknown is
  equated with `sv(v)`. Because attribute terms make every non-arithmetic
  tree denote *some* number, "numeric typing" never fails on its own — this
  is a stated consequence of D4, recorded in `components/solver.md`.
  Conversely, when the solver **determines** a variable (its value is
  forced), the heap variable is bound to a fresh `Number` cell (D5), which
  fires suspensions (`dif`, attribute-term congruence) through the existing
  mechanism.
- **Lowering arithmetic terms** (arithmetic-in-terms record): at clause
  instantiation, every `Neg`/`Binary` in head or body is replaced by a fresh
  numeric variable `N` and the linear constraint `N = t1 op t2` is posted
  immediately (part of the clause's constraint system `R`, as in Prolog III's
  abstract machine). Constant sub-expressions fold exactly. `t1 * t2` and
  `t1 / t2` are linear only if one factor is a constant *at posting time*:
  otherwise the product is **delayed** (suspended until one factor becomes
  determined; Prolog III's "approximated multiplication"). Division by a
  determined zero fails the step. If a delayed non-linear constraint is
  still pending when an answer would be produced, the query does **not**
  print that answer as if solved: it stops with `EvalError::NonLinear`
  naming the constraint — never an approximate answer (`answer-soundness`).
- **Numeric disequations.** `t1 != t2` in numeric position posts `D = t1 −
  t2` and the disequation `D != 0`, decided exactly: violated iff `D` is
  determined and equal to 0 (probe: is `D < 0` feasible? else is `D > 0`
  feasible? neither ⇒ violated); satisfied and dropped iff `D` is determined
  and non-zero; else pending — re-checked whenever the solver reports new
  determined variables and, exhaustively, before every answer.
- **Determined variables.** After every assertion the solver reports newly
  fixed variables cheaply (bounds coincide; a basic row whose non-basic
  variables are all fixed) and binds them; **before an answer is produced**,
  every numeric variable reachable from the query and every disequation
  difference is checked exactly by the two-probe method, so answers are in
  Prolog III's solved form ("the solution … is explicitly given, whenever
  this solution is unique").
- **Answer rendering (stage-3 form; stage 4 refines projection).** After the
  tree equations: `X = 3/2` for determined variables (rendered as terms
  since they are bound `Number` cells); then, for each solver variable
  reachable from the query, its bounds (`X > 3`, `Y <= 5`) and, for basic
  variables, its row as an equation (`Y = X + 1`); slack rows print as the
  original linear form with its bounds (`X + Y <= 10`); attribute terms
  print as terms (`age(bob) >= 18`); numeric disequations as `t1 != t2`;
  internal solver variables as `_1`, `_2`. Reachability for the numeric
  part: solver variables of query-reachable heap variables and attribute
  terms, plus their rows' variables (transitively). Constraints mentioning
  only unreachable variables are omitted (sound: they were checked).

## Deliverables

1. **`src/solver/`** — the `solver` component: `number` helpers (δ-rationals),
   `linexpr.rs` (sparse linear expressions over `SolverVar` with `Number`
   coefficients, exact), `simplex.rs` (tableau, bounds, Bland pivoting,
   `assert_lower/upper`, `check` returning feasibility, `mark`/`undo`
   restoring bounds, `is_determined(var)` via probes, `value_hint`),
   `store.rs` glue: `NumericStore { unknowns, attribute registry, pending
   disequations, delayed products }` with an API the evaluator's `Store`
   calls: `unknown_of`, `post_relation(op, a, b)`, `post_arith(N, op, a,
   b)`, `on_bind(v, target)`, `settle() → Result<Vec<Determined>, Fail>`,
   `render(...)`. Every mutation trailed via the evaluator's trail (extend
   `Undo` with solver entries) so `undo_to` restores it exactly.
2. **Evaluator integration** (`src/eval/`): `store.rs` calls into the
   numeric store from `bind`; `settle` also processes solver-woken items and
   binds newly determined variables; `compile.rs` lowers arithmetic (drops
   the stage-2 `Unsupported` rejections; keeps rejection of cyclic attribute
   terms), posts clause constraints at instantiation, and posts numeric
   relations as solver constraints; `answer.rs` renders the numeric part;
   `machine.rs` performs the exhaustive determined/disequation check before
   yielding and surfaces `EvalError::NonLinear` (Solutions yields
   `Result<Answer, EvalError>`? — no: keep `Answer` items and let
   `Solutions::error()` expose a terminal error; the CLI prints it and exits
   1. Decide in implementation, document the choice).
3. **CLI**: unchanged flags; prints numeric answers; the stage-3 error path.
4. **Tests** (`src/solver/` unit, `src/eval/tests.rs`, golden runs):
   - simplex: feasibility/infeasibility, strict vs non-strict, equalities
     as bounds, Bland's rule on a cycling-prone instance, undo restores
     bounds, `is_determined` probes, exactness (`0.1 + 0.2 = 0.3`, `1/3`).
   - equations: `{ X + Y = 10, X - Y = 2 }` → `X = 6, Y = 4`; chains;
     `average(3, 4, A)` → `A = 7/2`; `temperature(fahrenheit(212), C)` →
     `C = celsius(100)`; `?- sum(1, 2, S)` → `S = 3`.
   - inequalities: `{ X >= 3, X <= 3 }` → `X = 3`; `{ X > 3, X < 3 }` fails;
     residuals print (`X > 3`); implied equality `{ X - Y = 0 }, { X != Y }`
     fails; `{ X != 3 }` pending then `X = 3` fails, `X = 4` drops it.
   - attribute terms: the README examples — `{ age(socrates) > 70 }.` as
     initial store, `?- eligible(socrates)` → `true`, `?- eligible(alice)`
     → `age(alice) >= 18`, congruence via `eligible(X), X = socrates`;
     constant attribute (`{ foo > 3 }`); `X > 3, X = foo` succeeds with
     `foo > 3` (stated consequence).
   - lowering: arithmetic in atom arguments and heads; unification
     `f(X + 1) = f(2)` → `X = 1`; unary minus; nested; delayed product
     `{ Y = X * Z }, X = 2` → `Y = 2 * Z` … `Z = 3` → `Y = 6`;
     `{ Y = X * Z }` alone at answer time → `NonLinear` error; division by
     zero fails.
   - determined detection binds heap vars so `dif`/congruence fire.
   - golden `.answers` for `socrates`, `numbers`, `arithmetic`,
     `nested_terms` (review by hand); all earlier golden runs unchanged.
   - `cargo clippy --all-targets -- -D warnings` clean.
5. **Docs** — every path in `affects`, plus new docs:
   - `docs/architecture/components/solver.md` (schema
     `architecture-component`; decisions: the three records; the stated
     consequence about numeric typing; the delay/`NonLinear` rule; the
     rows-never-removed memory note).
   - `docs/architecture/aspects/exact-arithmetic.md` (schema
     `architecture-aspect`, `applies-to: [parser, evaluator, solver]`):
     invariants as testable rules (no `f64` in the crate — a test greps
     `src/` for `f64`/`f32` outside comments; literals exact; solver
     arithmetic exact; printing exact).
   - `components/evaluator.md` (store/compile/answer changes; stage-2
     boundary removed), `aspects/answer-soundness.md` (numeric checks,
     `last-verified`), `docs/architecture/README.md` rows (`solver` drops
     *(planned)*), `README.md` (status: constraint solving works; the
     diagnostic example replaced by a numeric answer example), `CLAUDE.md`
     (stage 3 done; the "rejected until stage 3" sentence goes).

## Explicitly out of scope

- Non-linear constraints beyond delaying products (a separate record if
  ever wanted); integer/finite-domain constraints; interval reals.
- Full answer projection / variable elimination (Fourier–Motzkin) and
  disequation simplification to reduced form — stage 4.
- REPL; grammar changes; performance work (small-integer fast path, tableau
  garbage collection, incremental implied-equality detection beyond the
  cheap cases + exhaustive answer-time check).

## Completion

Before setting `status: done`: every path in `affects` describes the
post-task state, the new architecture docs exist and lint clean, all
example `.answers` are reviewed, `cargo test`/clippy/`aivolution lint` are
clean, and a journal entry links back to this task via `tasks:`. Then
freeze this file.
