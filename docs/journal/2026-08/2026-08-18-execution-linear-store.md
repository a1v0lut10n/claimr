---
date: 2026-08-18
type: journal
components: [solver, evaluator, cli]
aspects: [answer-soundness, exact-arithmetic]
tasks: [docs/tasks/2026-08/2026-08-18-clm-0006-linear-store.md]
design:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
  - docs/design/2026-08-17-exact-rational-arithmetic.md
---

# The linear constraint store over exact rationals (CLM-0006)

## Context
Stage 3 of the evaluator design: the store built in stage 2 gains its
numeric half, turning claimr from a Prolog into a constraint language.

## Details
- `src/solver/`: δ-rationals, sparse linear expressions, a Dutertre–de Moura
  general simplex over exact `Number`s (bounds, Bland's rule, backtracking
  by restoring bounds, exact `is_determined` by two-sided probes).
- Numeric glue in `src/eval/store.rs`: numeric variables (stay solver
  variables; unification with a number is an equation), attribute terms
  with a structural registry and congruence via suspension, numeric
  disequations decided exactly, delayed non-linear products, alias
  classes, "determined" events feeding `dif`/congruence, `finalize`
  before answers (exact determination, dif re-check, `NonLinear` error
  instead of an approximate answer).
- Compile lowers arithmetic at instantiation; `=`/`!=` route numeric or
  tree by operand; `< > <= >=` always numeric. `Number` gains exact
  arithmetic ops.
- Answers show the numeric store the query can see, in solved form
  (`Y = X + 1`, `X > Y`, `age(alice) >= 18`), fixed values inline;
  world constraints only when touched; determined lines omitted.
- Bugs found by tests: two determined-equal numeric variables must not
  "bind" (dif soundness); rows referencing defined variables were dropped
  by the printer; probing made printing order-dependent (fixed).
- 79 tests incl. golden runs of all examples; clippy clean.
- Docs: `components/solver.md`, `aspects/exact-arithmetic.md`, evaluator
  and answer-soundness docs, vocabulary, README, CLAUDE.md.

## Links
- Task: docs/tasks/2026-08/2026-08-18-clm-0006-linear-store.md
