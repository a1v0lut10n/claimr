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
   unification plus any `{ … }` goals reached in the body) is taken only if
   the store — tree equations, disequations, and (from stage 3) numeric
   constraints — remains satisfiable; otherwise the step fails and the
   machine backtracks. *Test:* every `post_eq`/`post_dif`/solver posting is
   transactional (returns `false` and restores the store on failure), and the
   machine never proceeds past a `false`. Realised in
   [`evaluator`](../components/evaluator.md) (`store.rs`, `machine.rs`).
2. **No unsatisfiable answer.** An answer is printed only from a store in
   which every constraint has been checked exactly — trial unification for
   disequations, exact rational arithmetic (see
   [`exact-arithmetic`](../README.md#aspects)) for numeric constraints. There
   is no approximate satisfiability check anywhere. *Test:* golden runs
   (`tests/run_examples.rs`) and machine tests assert that queries whose
   only derivations violate a disequation yield `false`, and that reported
   disequations are re-checkable.
3. **Nothing satisfiable is dropped as an error.** Undefined predicates
   fail (no clauses ⇒ no solutions); unsupported constructs are rejected at
   load time with a positioned diagnostic, never silently ignored at run
   time. *Test:* stage-boundary tests in `src/eval/tests.rs`.
4. **Projection is sound.** Constraints omitted from a printed answer are
   only ones satisfiable independently of the printed variables (currently:
   disequations mentioning no variable reachable from the query). *Test:*
   `constraint_facts_form_the_initial_store` and the dif golden run.
