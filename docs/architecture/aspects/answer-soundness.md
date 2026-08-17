---
type: architecture-aspect
last-verified: 2026-08-18
applies-to: [evaluator, solver]
decisions:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-exact-rational-arithmetic.md
---

# answer-soundness

Every answer Claimr reports describes a satisfiable system, and every
resolution step is justified by a satisfiable store. Prolog III's abstract
machine takes a transition only "if the system of constraints … possesses at
least one solution"; Claimr holds the same rule across whatever components
realise the store.

## Invariants (testable rules)

1. **No step on an unsatisfiable store.** A resolution step (head
   unification, the clause's arithmetic definitions, and any `{ … }` goals
   reached in the body) is taken only if the store — tree equations,
   disequations, linear numeric constraints — remains satisfiable; otherwise
   the step fails and the machine backtracks. *Test:* every
   `post_eq`/`post_dif`/`post_rel`/`post_arith` is transactional (returns
   `false`/`None` and restores the store, simplex bounds included, on
   failure), and the machine never proceeds past a failure. Realised in
   [`evaluator`](../components/evaluator.md) (`store.rs`, `machine.rs`) and
   [`solver`](../components/solver.md).
2. **No unsatisfiable answer.** An answer is printed only after
   `Store::finalize`: every numeric variable and numeric disequation is
   decided exactly (two-sided feasibility probes), every pending tree
   disequation is re-checked by trial unification, and any still-pending
   non-linear product stops the query with `EvalError::NonLinear` instead of
   printing. There is no approximate satisfiability check anywhere (see
   [`exact-arithmetic`](exact-arithmetic.md)). *Test:* golden runs
   (`tests/run_examples.rs`) and machine tests (`numeric_disequations_are_exact`,
   `determined_variables_wake_difs_and_congruence`, `delayed_products`)
   assert that queries whose only derivations violate a disequation — even
   one implied only through the store — yield `false`.
3. **Nothing satisfiable is dropped as an error.** Undefined predicates
   fail (no clauses ⇒ no solutions); unsupported constructs are rejected at
   load time with a positioned diagnostic, never silently ignored at run
   time. *Test:* stage-boundary tests in `src/eval/tests.rs`.
4. **Projection is sound.** Constraints omitted from a printed answer are
   only ones satisfiable independently of what is printed: tree
   disequations mentioning no query-reachable variable, fully determined
   numeric lines (their values are shown), and world constraints on unknowns
   the query does not touch — all checked satisfiable before printing.
   *Test:* `constraint_facts_form_the_initial_store`,
   `attribute_terms_and_congruence`, and the golden runs.
