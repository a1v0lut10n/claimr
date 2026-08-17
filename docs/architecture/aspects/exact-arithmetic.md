---
type: architecture-aspect
last-verified: 2026-08-18
applies-to: [parser, evaluator, solver, cli]
decisions:
  - docs/design/2026-08-17-exact-rational-arithmetic.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
---

# exact-arithmetic

Every number in Claimr is an exact arbitrary-precision rational; there is
no floating point anywhere in the language core, so satisfiability,
entailment and answers are exact (Prolog III: "infinite precision, that is
to say, on fractions whose numerators and denominators are unbounded
integers").

## Invariants (testable rules)

1. **No floating point in the crate.** `src/` contains no `f32`/`f64` (in
   code; comments excepted). *Test:* `no_floating_point_in_the_crate` in
   `src/eval/tests.rs` scans the tree.
2. **Literals are exact.** A decimal literal denotes the exact rational it
   writes (`18.5` = 37/2, `0.10` = 1/10); integers are the denominator-1
   case. *Test:* `src/number.rs` tests; parser tests asserting
   `Number::from_ratio`. Realised in [`parser`](../README.md#components)
   (`claimr_actions.rs`) via `Number::from_literal`.
3. **Solver arithmetic is exact.** Coefficients, bounds, values and
   δ-rationals are `Number`s; `0.1 + 0.2 = 0.3` holds; `1/3` is exact.
   *Test:* `src/solver/simplex.rs` `exact_rationals_no_drift`;
   `linear_equations_determine_variables`. Realised in
   [`solver`](../components/solver.md).
4. **Printing is exact.** Numbers print as integers or fractions in lowest
   terms (`33/32`), never rounded. *Test:* `Number` display tests; golden
   runs of `examples/numbers.claimr` (a literal beyond `f64` precision).
5. **No numeric escape hatch.** `Number` exposes no conversion to or from
   `f64`; any future I/O convenience needs its own decision.
