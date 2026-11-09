# Aggregation and all-solutions

Captured 2026-11-08. Prompted by integrity rules over a knowledge base that needed counts:

- a cohort's size computed from its members, and checked against a floor;
- the number of distinct sources behind an assertion;
- "at least k precedents".

Comparing a stored count with a threshold is a linear constraint and already works.
Deriving the count needs all the solutions of a goal.

The idea, in two layers:

1. **All-solutions:** `findall(Template, Goal, List)`, and possibly `bagof`/`setof`, which
   need the list syntax to be pleasant (see
   `docs/design/2026-11-09-list-notation.md`).
2. **Aggregates over them:** `count(Goal, N)` and `sum(Template, Goal, S)`. `min`/`max`
   can be written as constraints, but are worth having. Exact rationals make `sum` and
   `average` exact, which is a selling point.

Design questions:

- **Constraints in the solutions.** A solution carrying residual constraints (not ground)
  has no single value to count or sum. As with negation, the simplest sound rule is to
  require ground solutions, and raise an error otherwise.
- **Termination.** `findall` over a goal with infinitely many solutions does not end. Is a
  limit a language feature, or left to the caller?

Relevant: `docs/architecture/aspects/exact-arithmetic.md`, `docs/architecture/aspects/answer-soundness.md`.
