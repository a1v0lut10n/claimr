---
date: 2026-11-09
type: design
status: accepted
components: [grammar, parser, evaluator, solver, highlight]
aspects: [grammar-authority, answer-soundness, exact-arithmetic]
tags: [aggregation, findall, comprehension, sum, count, lists, soundness, prolog-iii]
---

# Aggregation: list comprehensions and the sum relation

## Context

The incubation note of 2026-11-08 came from integrity rules that need
counts: a cohort's size against a floor, the number of distinct sources
behind an assertion, "at least k precedents". Comparing a stored number
with a threshold is a linear constraint and works; *deriving* the number
needs all the solutions of a goal, which claimr cannot collect. The note
asked for an all-solutions primitive and aggregates over it, and raised
two questions: what to do with a solution that carries residual
constraints, and what to do with infinitely many solutions.

**What exists.** Lists are a constraint domain (CLM-0014..16): `<a, b>`,
`<a> . T`, the length `|L|` as a numeric term in the store, concatenation
as a relation. The groundness rule of CLM-0022 decides when a condition
may be run over a constraint store: its external variables, those shared
with the clause outside the construct, must be ground, or the query stops.
The commit of CLM-0022 finalizes a condition's solution exactly before
acting on it. Explanations (CLM-0023) record what resolved each goal.

**What Prolog III does.** The manual (Version 1.3, December 1991) has
`findall(V, P, L)`: "unifies the list `L` with the collection of all
solutions `V` when `P` is solved; the goal `P` must be sufficiently known
to be executed" (p. 250), and `enum(R)`, which enumerates the integers `n`
such that `{ R = n }` is satisfiable, the manual's way of turning a
bounded numeric unknown into solutions (p. 244; the CACM paper's
*Enumeration of integers*, p. 21). No `bagof`, `setof`, `count` or `sum`
as primitives: counting is `findall` then the length, summing is a
recursion.

**Why the spelling is the hard part.** `findall(T, G, L)` in claimr would
make `G` an argument, and arguments are terms. A goal is not a term here:
a constraint block `{ X > 2 }` and a conjunction `(p(X), q(X))` have no
term spelling, and giving them one (a reified body) is a far larger
change than the feature. A keyword `findall` in goal position would work
in the grammar but would reserve the word in every position, atoms
included, which the reserved built-in names of CLM-0017 deliberately do
not. The list notation already has a bracket pair and a bar.

## Decision

1. **A list comprehension `<T | Body>`** is an expression denoting the
   list of the instances of the template `T` for each solution of `Body`,
   in solution order (program order), `<>` when there is none. `Body` is a
   body: a conjunction of atoms, constraint blocks and the constructs of
   CLM-0022. `T` is a term; a pair is `<<X, Y> | p(X, Y)>`. It is admitted
   wherever a term is, in a body: as a goal's argument and inside a
   constraint block. **Count** needs no primitive: `|<X | p(X)>|` is the
   length of the comprehension, a numeric term in the store, so
   `cohort_too_small(C) :- cohort(C), { |<M | member_of(M, C)>| < 3 }.`
   is one clause.

2. **The comprehension is evaluated when its goal is reached.** The
   compile step desugars a goal containing comprehensions into one
   collection step per comprehension, left to right, each binding a fresh
   clause variable, followed by the goal with the variables in place. A
   comprehension in a **head** is a compile error with its position: a
   head is a pattern that is matched, not a goal that runs, and running a
   search inside head unification would be a surprise. A later record may
   lift this by moving the collection into the body.

3. **The groundness rule applies.** The comprehension's external
   variables, those of `T` and `Body` that also occur in the clause
   outside the whole comprehension (in the head, in other goals, and in
   the rest of the goal it sits in), must be ground when it is reached;
   otherwise the query stops with the runtime error of CLM-0022, naming
   the comprehension and the variable. `{ N = |<X | p(X, N)>| }` is
   therefore an error, as it should be: `N` is both the result and an input
   of the search. Variables local to `T` and `Body` are the collected ones.

4. **Every solution is exact and every instance is ground.** Each
   solution of `Body` is finalized as an answer would be before its
   instance of `T` is taken (a pending disequation or a non-linear residue
   is decided or stops the query, as at a commit). The instance must be
   ground: no unbound variable, every numeric variable determined. An
   instance that is not ground stops the query with a runtime error naming
   the comprehension and the instance: a solution carrying residual
   constraints on the template has no single value to collect, and copying
   it with fresh variables, as Prolog does, would silently drop the
   constraints. The ground instance is copied out of the solution before
   the search moves on (the solution's cells are undone with it); the list
   is built from the copies once `Body` is exhausted, in a store restored
   to what it was when the comprehension was reached, and unified with the
   fresh variable.

5. **`sum(L, S)` is a built-in relation**, the fifth after CLM-0017's four:
   `S` is the sum of the elements of the list `L`. On a complete list whose
   elements are numbers or numeric unknowns it posts the linear equation
   `S = e1 + … + en` (exact rationals, so `sum(<1/3, 1/3, 1/3>, S)` gives
   `S = 1`, and an unknown element is solved for: `sum(<1, X, 3>, 10)`
   gives `X = 6`); `sum(<>, S)` gives `S = 0`; an element that is not a
   number fails; an open spine suspends on the tail, as length does, and a
   pending one prints in goal form, `sum(L, S)`. With a comprehension,
   `total(T) :- sum(<A | amount(_, A)>, T).` is a sum over all solutions,
   and `{ Avg = T / |L| }` an exact average.

6. **Not in this record.** `bagof`/`setof` with `^`-quantification and
   grouping by free variables: the comprehension's locals are its
   quantified variables, and grouping is a loop over a key, which the
   programmer writes. `min`, `max`, `average`, `distinct` as primitives: a
   fold with the if-then-else of CLM-0022, or a constraint, writes each in
   two clauses, and none is linear. A bound on infinitely many solutions:
   a comprehension over a goal with infinitely many solutions does not
   terminate, as `findall` does not; the REPL's Ctrl-C interrupts it, and a
   limit would be a language feature in its own right. Integer enumeration
   under constraints (Prolog III's `enum`): the labelling primitive the
   finite-domain future needs, an incubation note of its own.

## Consequences

- **Soundness.** With the external variables ground at entry, the
  solution set of `Body` and the instance of `T` at each solution are the
  same in every completion of the derivation (the argument of CLM-0022: a
  later goal can only constrain variables the comprehension does not
  reach), so the list is a determined value and the comprehension is a
  function of ground inputs, sound in any context, including under
  negation and in a condition. Exactness of each solution and groundness
  of each instance keep invariant 2 (no unsatisfiable answer) and
  invariant 4 (nothing printed that the store does not entail): nothing is
  collected that the store does not determine. Invariant 5 of
  `answer-soundness` is restated for the three constructs that run a
  sub-search: if-then-else, negation and the comprehension.

- **Grammar path** per `grammar-authority`: `Tuple` gains the alternative
  `'<' template=Expr '|' body=Body '>'`, making `Expr` and `Body` mutually
  recursive; no new terminal (`|` is the length bar). The automaton is
  checked for silenced conflicts; the bar after a complete template inside
  an unclosed `<` can only open the body, and `|<X | p(X)>|` nests as the
  length bars do. The EBNF view, the README, `examples/aggregation.claimr`
  with answers (the cohort floor, distinct sources, precedents, the exact
  average), and the highlight lexer (no new token kind).

- **Compile.** `Expr::Comprehension { template, body }` in the AST, printed
  back as written; the lowering walks a goal's terms, replaces each
  comprehension by a fresh clause variable and emits
  `TGoal::Collect { template, body, into, check }` before the goal, with
  the external-variable analysis of CLM-0022 extended to the comprehension
  (the rest of the enclosing goal counts as outside). A head containing one
  is `EvalError::ComprehensionInHead` with the clause's position.

- **Machine.** Opening a collection records the choice-point height, pushes
  a goal-list choice point that *finishes* the collection (builds the
  list from the accumulated copies, unifies it with `into`, continues
  with the rest) and runs `Body` followed by a `Collect` goal. Reaching
  `Collect` finalizes, checks the instance is ground, copies it to the
  accumulator held by the finishing choice point, and fails, so the
  search moves to the next solution; exhausting `Body` lands on the
  finishing choice point with the store restored. The copy is a closed
  term template (constants, numbers, compounds), rebuilt on the heap by the
  existing instantiation. Explanations (CLM-0023) record the comprehension
  as one leaf, `collected`, with the number of solutions; the solutions'
  own proofs are discarded with their derivations.

- **Store.** `post_sum(L, S)` walks the spine as `post_len` does: a
  complete list posts the linear equation through `unknown_of` (an element
  that is not a number, a constant or a compound in numeric position being
  an attribute term as everywhere else); an open spine registers a pending
  sum entry suspended on the tail, re-checked on binding; the answer
  printer prints a pending one as `sum(L, S)` after the other goal-form
  residuals; the JSON `constraints` vocabulary grows by that string (shape
  unchanged, a `meta-impact` note at acceptance). `sum/2` joins the
  reserved built-ins and `builtins.md`.

- **One task** implements the comprehension and `sum/2` together: the
  example needs both, and the two share nothing but the ticket. The
  explanations and groundness docs, `evaluator.md`, `solver.md`,
  `builtins.md`, `grammar.md` and the README follow.

## Sources

- PrologIA, *Prolog III, Version 1.3*, reference manual, December 1991:
  `findall(V, P, L)` p. 250, `enum(R)` p. 244.
- A. Colmerauer, *An Introduction to Prolog III*, CACM 33(7), 1990,
  *Enumeration of integers*, p. 21.
- Incubation note `2026-11-08-aggregation-and-all-solutions.md`
  (graduated by this record).
