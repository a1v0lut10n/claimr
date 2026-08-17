# Evaluator: resolution + constraint solving over the parsed AST

Captured 2026-08-17 (incubation) — prompted by finishing the claimr
rename/restructure (CLM-0001): the parser is done and the obvious next piece
is something that *runs* a program. Graduated to catalyst 2026-08-17 after
the rustemo parser port (CLM-0002) landed and the numeric-domain question
was researched against Prolog III's primary source (below).

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

## Evidence: how Prolog III actually treats numbers

Source: A. Colmerauer, *An Introduction to Prolog III*, CACM 33(7), 1990
(<http://alain.colmerauer.free.fr/alcol/ArchivesPublications/Prolog3/acmprolog3e.pdf>).

- The **domain** is the perfect reals — "by real numbers we mean perfect
  real numbers and not floating point numbers" — partitioned into rationals
  ("of which the integers are a special case") and irrationals.
- But "**the machine will compute with rational numbers only**", justified by
  an essential property of the constraint language: "if a variable is
  sufficiently constrained to represent a unique real number then this
  number is necessarily a rational number." Linear constraints with rational
  coefficients never determine an irrational.
- Constants: "to each label corresponds a constant, with the exception of
  irrational numbers" — irrationals are unrepresentable as constants; they
  exist only semantically as values an under-constrained variable may take.
- Constraints handled: linear equations/inequations (`=`, `<`, `≤`, `≠`),
  addition, subtraction, multiplication and division *by a constant*.
  Multiplication of two variables is only "approximated" (delayed).
- Implementation: incremental Dantzig simplex with Balinski–Gomory pivoting
  (cycle-free), used both for satisfiability and to detect variables with a
  single possible value; arithmetic "in infinite precision, that is to say,
  on fractions whose numerators and denominators are unbounded integers".
- Integers: no integrality solver ("the algorithms ... are complex"); only
  enumeration (`enum`) over the rational solutions.

## Shape (position taken so far, to be pinned in a design record)

- **One exact numeric representation: arbitrary-precision rationals**, with
  integers as the denominator-1 case (printed as integers, otherwise as
  fractions in lowest terms, as Prolog III printed `33/32`). No floating
  point anywhere in the core. Decimal literals (`18.5`) denote exact
  rationals (37/2). Once arithmetic arrives, `1/3` is exact division — no
  dedicated rational-literal syntax needed.
- **No separate "real" representation** — following the paper: with linear
  rational constraints it would never be populated. Reals remain the
  semantic domain of variables. If non-linear constraints are ever wanted,
  the sound extension is Prolog IV-style interval reals, as a separate
  decision — never bare floats.
- **Integer constraints deferred**, as in Prolog III (enumeration only);
  a finite-domain solver would be its own component later.
- Rust backend: `num-rational` (`BigRational`) + `num-bigint` behind a
  `Number` newtype so `rug`/`malachite` can be swapped in if performance
  ever demands; a small-integer fast path can be added inside the newtype.
- Concrete first step (grammar-neutral): `Expr::Number(f64)` becomes the
  exact `Number`; literals are parsed exactly from their text; the AST then
  gains `Eq`/`Hash` (useful for the evaluator's term handling).

## Open questions (still unresolved)

- **Runtime term model.** Atoms with zero args are currently `Ident`;
  unification needs a settled term model. The AST may need a runtime
  `Term` type distinct from the parse-tree `Expr`.
- **Semantics of `Implication`** (`{c} => head.`) — sugar for a constraint
  rule (`head :- {c}.`), as the README suggests, or a forward rule that
  asserts `head` whenever the store entails `c`?
- **Where answers surface** — the CLI currently just prints parsed clauses;
  the evaluator would turn `?-` queries into printed solutions, and a REPL
  becomes attractive.
- **Search strategy / termination** — depth-first like Prolog, or something
  else; occurs check or not.
- **Solver algorithm** — incremental simplex as in Prolog III, or
  Fourier–Motzkin / bounds propagation for a first cut.

## Adjacent work to connect with

- `src/parser/claimr.rustemo` (authoritative grammar) — currently no
  arithmetic inside constraints (only `identifier | number | atom |
  variable`), no negative literals, no comment syntax; the evaluator will
  push on it (arithmetic with precedence, `1/3`, `!=` on terms) — grammar
  first, then actions, then examples, per the `grammar-authority` aspect.
  Design record for the parser choice:
  `docs/design/2026-08-17-rustemo-parser-generator-over-nom.md`.
- `docs/architecture/README.md` already reserves the names for future
  `evaluator` / `constraint solver` / `REPL` components.
- Prolog III (above) and CLP(Q) (e.g. SWI-Prolog `clpq`, exact) vs. CLP(R)
  (floats, documented as approximate) as reference designs for the
  exact-vs-float trade-off.
