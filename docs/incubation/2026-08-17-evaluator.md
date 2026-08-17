# Evaluator: resolution + constraint solving over the parsed AST

Captured 2026-08-17 — prompted by finishing the claimr rename/restructure
(CLM-0001): the parser is done and the obvious next piece is something
that *runs* a program.

## The idea

Give claimr an evaluator that takes the `Vec<Clause>` produced by
`parse_program` and answers queries. Two halves, in the spirit of
Prolog III:

- **Resolution** — SLD-style search over facts and rules (`Clause::Fact`,
  `Clause::Rule`, `Clause::ConstraintRule`, `Clause::Implication`), with
  unification over `Expr` terms (`Atom`, `Var`, `Number`, `Ident`),
  backtracking, and answer substitutions for `Clause::Query`.
- **Constraint solving** — a constraint store that accumulates the
  `Constraint { left, op: RelOp, right }` terms met during resolution
  (from `Goal::Constraint`, `Clause::ConstraintFact`, and implications)
  and checks them for consistency incrementally, so a branch is pruned as
  soon as its constraints become unsatisfiable, and any still-open
  constraints are reported alongside the answer rather than forced to a
  ground value.

The point is that constraints are first-class: a query can succeed with a
residual constraint set (`X >= 18`) instead of enumerating values.

## Open questions (deliberately unresolved)

- **Constraint domain.** `Expr::Number` is `f64` today; the relops are
  `= != < > <= >=`. Is the first solver over reals/rationals (linear
  arithmetic, à la Prolog III), over integers/finite domains, or just
  syntactic-equality-plus-comparison of ground numbers as a stepping
  stone? Which one shapes everything else.
- **What "Ident" vs "Atom" mean at runtime** — atoms with zero args are
  currently `Ident`; unification needs a settled term model. The AST may
  need a runtime `Term` type distinct from the parse-tree `Expr`.
- **Semantics of `Implication`** (`{c} => head.`) — is it sugar for a
  constraint rule (`head :- {c}.`), as the README suggests, or a forward
  rule that asserts `head` whenever the store entails `c`?
- **Where answers surface** — the CLI currently just prints parsed clauses;
  the evaluator would turn `?-` queries into printed solutions, and a REPL
  becomes attractive.
- **Search strategy / termination** — depth-first like Prolog, or something
  else; occurs check or not.

## Adjacent work to connect with

- `docs/reference/grammar.md` — the grammar has no comment syntax, no
  arithmetic expressions inside constraints (only `identifier | number |
  atom | variable`); an evaluator will probably push on the grammar
  (arithmetic, negative numbers, `\=`/disequality on terms) — grammar
  first, then parser, per the `grammar-authority` aspect.
- `docs/architecture/README.md` already reserves the names for future
  `evaluator` / `constraint solver` / `REPL` components.
- Prolog III / CLP(R) as the reference designs.
