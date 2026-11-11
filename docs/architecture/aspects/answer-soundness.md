---
type: architecture-aspect
last-verified: 2026-11-09
applies-to: [evaluator, solver]
decisions:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-exact-rational-arithmetic.md
  - docs/design/2026-11-09-negation-disjunction-and-if-then-else.md
  - docs/design/2026-11-09-explanations-of-answers.md
  - docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md
  - docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md
  - docs/design/2026-11-10-tabling-for-recursive-rules.md
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
   time. A stop under a caller's step budget (CLM-0029, `--steps N`) is a
   resource error that names its count and goal, not a judgment about the
   query: the answers found before it stand and nothing is reported
   `false`. *Test:* stage-boundary tests in `src/eval/tests.rs`; the
   step-budget tests there (`a_step_budget_stops_a_looping_query_and_names_the_goal`,
   `answers_before_the_step_budget_are_yielded_then_the_error`).
4. **Projection is sound.** What is printed is equivalent, on the public
   variables, to the store: internal numeric variables are eliminated by
   equivalence-preserving substitution and Fourier–Motzkin (or, over the
   budget, kept and named — never silently dropped); redundancy removal
   drops only constraints entailed by the rest, decided exactly by a
   simplex; a numeric disequation is dropped only when substitution and
   elimination leave it unlinked to any public variable; tree
   disequations mentioning no query-reachable variable, and world
   constraints on unknowns the query never touched, are omitted as
   independently satisfiable. *Test:* `internal_variables_are_eliminated`
   (incl. the disequation case `A != 3`), `redundant_constraints_are_dropped`,
   `projection_budget_fallback_stays_correct`, `answers_are_deterministic`,
   and the golden runs.
5. **A sub-search is decided only on ground external variables.** An
   if-then-else or negation (CLM-0022) is decided — its condition run to a
   first solution, which is finalized exactly as an answer would be, or to
   failure — and a list comprehension (CLM-0025) is collected — every
   solution of its body finalized exactly, every instance of its template
   ground, so that nothing is collected the store does not determine —
   only when every variable the construct shares with the clause outside
   it is ground at that moment, and — CLM-0027 — only on **complete
   tables**: a tabled call inside the construct that finds its table still
   being computed (the predicate's own recursive group) stops the query
   with `EvalError::IncompleteTable` rather than deciding on a partial
   answer set; the outcome is then the same in every completion of the
   derivation, so committing, negating or collecting never produces an
   answer the program does not entail, under any nesting. A construct
   reached with a non-ground external variable stops the query with
   `EvalError::NonGroundCondition`, a non-ground instance with
   `EvalError::NonGroundSolution`: nothing is decided in either direction.
   *Test:* `sub_searches_are_decided_on_complete_tables_only`,
   `comprehensions_apply_the_groundness_rules`,
   `a_non_ground_external_variable_is_an_error_not_a_decision` (the
   classic wrong positive `s :- \+ r(X)` is an error, not `true`),
   `a_condition_is_finalized_before_the_commit`,
   `the_groundness_rule_exempts_the_constructs_own_variables`, and the
   golden run `examples/control.claimr`.
6. **An explanation cites only the answer's own derivation.** The proof
   tree an answer carries under `--explain` (CLM-0023) names, for every
   goal, the clause that resolved it and the constraint instances posted,
   and nothing from a derivation that was backtracked over, a condition's
   discarded alternatives, or an else branch not taken; the cited
   constraints are the posted ones, which entail the printed, projected
   ones. Recording changes no decision the machine takes. *Test:*
   `cited_clauses_reproduce_the_answer` (the cited clauses alone, as a
   program, give the same answer), `explanations_discard_abandoned_derivations`,
   `explanation_names_agree_with_the_answer`, and the golden run
   `examples/explain.claimr` under `--explain`.
7. **A block relation means what its operands denote.** Whether `=` / `!=`
   in a `{ … }` goal is a numeric relation or tree unification / `dif` is
   decided by the terms its operands denote when the goal is reached — a
   number by any route (literal, arithmetic result, variable bound to a
   number, numeric variable) or a tree — never by how they came to denote
   them (CLM-0026,
   `2026-11-09-numbers-numeric-variables-and-attribute-terms.md`). A
   variable bound to a number by a fact and a numeric variable fixed by a
   constraint behave alike; a determined numeric variable is bound to its
   number, so it has that value in every solution and every later goal
   sees the number; unification never creates an attribute term, so a head
   unification's outcome does not depend on whether a variable was numeric
   before it. *Test:* `numeric_equality_follows_what_the_operands_denote`
   (the record's probe in both goal orders),
   `first_argument_indexing_preserves_clause_order_and_answers` (a
   determined variable indexes as its number),
   `attribute_terms_and_congruence`.
8. **A table is a function of the program.** The answers a tabled
   predicate (CLM-0027,
   `2026-11-10-tabling-for-recursive-rules.md`) is answered from depend on
   the program's clauses and constraint facts and on nothing else: each
   table is computed over the world store alone, never the calling query's,
   and holds only solutions that are finite ground terms and hold
   unconditionally — a solution carrying an unbound variable, a cycle, or a
   constraint on a world unknown beyond what the world entails stops the
   query (`EvalError::NonGroundTabledAnswer`,
   `EvalError::ConstrainedTabledAnswer`) instead of being recorded. A table
   is complete before any construct decides on it (invariant 5), and a
   consumption is unification of the call with a recorded answer in the
   caller's store, so invariants 1 and 2 hold for it as for a fact. A query
   that stops inside a table's computation forgets every incomplete table.
   The visible consequence, each answer once, is the set semantics the
   record chose. *Test:*
   `tabled_transitive_closure_terminates_on_a_cycle_in_either_direction`,
   `tables_are_sets_and_belong_to_the_program`,
   `a_tabled_call_is_filtered_by_the_callers_store`,
   `a_table_is_computed_over_the_world_not_the_callers_store`,
   `tabled_answers_must_be_finite_ground_terms`,
   `an_interrupt_inside_an_evaluation_cleans_the_tables`, and the golden
   run `examples/tabling.claimr`.
