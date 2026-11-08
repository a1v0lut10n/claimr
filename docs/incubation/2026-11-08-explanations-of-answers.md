# Explanations of answers

Captured 2026-11-08. Prompted by using claimr as an integrity checker. A violation is
only useful if it says which facts and which rule produced it: "these three claims, under
rule R2, are inconsistent". That lets a caller cite them, route them to whoever can fix
them, or show them to a person. Today an answer is the projected constraint store: the
bindings and residual constraints, but not why.

The idea is an opt-in explanation of each answer:

- **The proof tree.** For each solved goal, the clause used (with its source span, which
  the parser already keeps for `compile_spanned`) and the subproofs of its body.
- **The constraints.** For constraint goals, which constraints were posted and where they
  came from. For a failed query that was expected to succeed, a minimal set of
  conflicting constraints would be the dual. The simplex can produce an infeasible subset
  (a Farkas certificate), which is exact over rationals.

Output: a `--explain` flag, and a field in the JSON output (`docs/reference/json-output.md`)
under a new contract version. Explanation records cost memory, so they should be off by
default.

For the integrity use, the source spans of the facts used are enough to start with. The
full tree and the infeasible subsets can come later.
