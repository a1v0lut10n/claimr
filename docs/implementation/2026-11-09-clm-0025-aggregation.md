---
date: 2026-11-09
type: task
status: in-progress
affects:
  - docs/architecture/components/evaluator.md
  - docs/architecture/components/solver.md
  - docs/architecture/aspects/answer-soundness.md
  - docs/reference/grammar.md
  - docs/reference/builtins.md
  - docs/reference/json-output.md
components: [grammar, parser, evaluator, solver, highlight]
aspects: [grammar-authority, answer-soundness, exact-arithmetic]
design:
  - docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md
tags: [CLM-0025, aggregation, comprehension, sum, grammar, machine, store]
---

# CLM-0025: Aggregation — list comprehensions and the sum relation

## Premise

The accepted record `2026-11-09-aggregation-list-comprehensions-and-sum.md`:
`<T | Body>` is an expression denoting the list of the instances of `T` for
each solution of `Body`, admitted wherever a term is in a body, evaluated
when its goal is reached, forbidden in heads; count is its length;
`sum(L, S)` is a built-in linear relation. The groundness rule of CLM-0022
applies to the comprehension's external variables, every solution is
finalized exactly, and every instance must be ground.

## Action items

1. **Grammar** (`claimr.rustemo`): `Expr` gains
   `'<' template=Expr '|' body=Body '>' {ComprehensionExpr}`; no new
   terminal. `rcomp --force --dot` checked for silenced conflicts.
2. **AST and printer** (`src/ast.rs`): `Expr::Comprehension { template,
   body }`, printed `<T | body>`; precedence atomic; drop iterative.
3. **Actions**: `expr_comprehension_expr`.
4. **Compile** (`compile.rs`): `TGoal::Collect { template, body, into,
   check }`; `Lowerer::term` replaces a comprehension by a fresh clause
   variable and queues the collect, `Lowerer::body` emits the queued
   collects before each goal (saving the queue around nested bodies); a
   comprehension in a head or a constraint fact is
   `EvalError::ComprehensionInHead` with the clause's position; the
   external-variable analysis covers `Collect` (template and body, with the
   rest of the enclosing goal as outside); `Builtin::Sum` (`sum/2`).
5. **Store** (`store.rs`): `snapshot(addr)` — a closed term template of a
   ground instance (determined numeric variables as their values), `None`
   on an unbound variable or a cycle; `post_sum(l, s)` with a pending `Sum`
   entry, `Waker::Sum`, `recheck_sum`, undo and mark fields,
   `pending_sums()` for the printer.
6. **Machine** (`machine.rs`): opening a `Collect` checks the groundness
   rule, pushes a `Retry::Finish` choice point holding the accumulator, and
   runs the body followed by `Goal::CollectOne`; reaching it finalizes,
   snapshots the instance (`EvalError::NonGroundSolution` otherwise), pushes
   it and fails; the finish builds the list, unifies it with `into` and
   continues, recording a `collected` proof leaf. `run_builtin` gains
   `Sum`.
7. **Answer and JSON**: `Resolution::Collected` (`"collected"`); the
   pending `sum(L, S)` goal-form residual.
8. **Error** (`error.rs`): `ComprehensionInHead { span }` (positioned),
   `NonGroundSolution { construct, instance }`.
9. **Example** `examples/aggregation.claimr` + `.answers`: the cohort
   floor, distinct sources, precedents, a sum and an exact average, `sum`
   solving for an element and pending on an open tail, a comprehension
   under negation, a nested one.
10. **Tests**: machine tests for the semantics (order, empty, nested,
    under negation and in a condition, the two errors, the external rule
    with the rest of the goal as outside, a cyclic instance, exactness of
    each solution), `sum` cases, parse round trip, head error, explanations
    leaf, JSON vocabulary; a deep collection (20 000 solutions).
11. **Docs**: EBNF view and README; `builtins.md` (sum); `evaluator.md`;
    `solver.md`; `answer-soundness.md` (invariant 5 restated for the three
    sub-search constructs); `json-output.md` (`collected`, `sum(L, S)`);
    this plan.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every existing golden file byte-identical.
- [ ] `rcomp --force --dot` automaton inspected for silenced conflicts.
- [ ] `aivolution lint .` clean; the six `affects` docs current.
