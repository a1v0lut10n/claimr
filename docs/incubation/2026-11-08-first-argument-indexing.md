# First-argument clause indexing

Captured 2026-11-08. Prompted by the same integrity-rule experiment: about 10 500 facts,
claimr `abd14aa`.

Clauses are grouped by predicate (`Program::clauses(&PredKey)`), but within a predicate
the machine tries every clause in order (`try_clauses`). A call whose first argument is
bound, such as `n(C, N)` with `C` already known, still builds and unifies the head of
every `n` clause. A join like

```
violation(C, k_exceeds_n) :- k(C, K), n(C, N), { K > N }.
```

therefore costs O(F²) over F facts, even with no arithmetic. Fact tables keyed by an id
(records, claims, entities) are exactly this shape, and the join is how rules over them
are written.

The idea: index each predicate's clauses by the principal functor of their first argument
(atom, number, or functor and arity). A call with a bound first argument then visits only
the clauses that can match. The usual refinements can follow:

- an index on other arguments when the first is unbound;
- skipping the choice point when only one clause remains, which also reduces memory on
  deterministic lookups.

Clause order is the language's semantics, so the index must return candidates in program
order.

Relevant: `src/eval/compile.rs` (`Program`), `src/eval/machine.rs` (`try_clauses`),
`docs/architecture/components/evaluator.md`.
