# Disjunction and if-then-else in bodies

Captured 2026-11-08. A rule body is a conjunction only (`Goal+` separated by commas in
`src/parser/claimr.rustemo`). Alternatives are written as separate clauses. That works,
but it duplicates shared goals, and some idioms cannot be expressed at all:

- "use the curator's value if there is one, else the harvested one";
- "the first matching rule decides".

Both need a committed choice.

The idea:

- **Disjunction `(A ; B)`** in bodies, which is sugar for an auxiliary predicate with two
  clauses.
- **If-then-else `(C -> T ; E)`,** which commits to the first solution of `C`. It is the
  sound alternative to cut, and with negation as failure (`\+ C` ≡ `(C -> fail ; true)`)
  it shares the ground-condition question. If `C` leaves constraints in the store,
  committing to it may drop answers. A ground-only condition, or an error, keeps the
  `answer-soundness` aspect.

Cut (`!`) is deliberately left out. If-then-else covers its legitimate uses without its
non-local effects.

The grammar change follows `grammar-authority`: grammar → actions → example → EBNF view.
