# Changelog

All notable changes to claimr are recorded here, newest first. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [Semantic Versioning](https://semver.org/) with the 0.x caveat that
minor versions may change the language. Each entry names the ticket
(`CLM-NNNN`) whose implementation plan under `docs/implementation/` and
journal entry under `docs/journal/` carry the detail, and the design record
under `docs/design/` that decided it.

## [0.4.0] — 2026-11-11

The language grows from facts, rules and linear constraints over numbers
into a Prolog III–shaped constraint language: lists and strings as
constraint domains, control in bodies, aggregation, explanations, tabling
and a step budget. Programs written for 0.3.0 run unchanged; the `--json`
contract keeps its shape and gains documented content.

### Added

- **Lists as a constraint domain** (CLM-0014, CLM-0015, CLM-0016;
  `docs/design/2026-11-09-lists-as-a-constraint-domain.md`). Prolog III
  tuples: `<>` is the empty list, `<a, b, c>` a list, `<H> . T` the cons
  pattern in a head. `|L|` is the length of `L`, a number in the
  constraint store, so `{ |L| >= 2 }` is a linear constraint and a
  determined length makes its list a tuple of fresh variables. `X . Y` is
  list concatenation: a chain when the left operand is known, otherwise a
  constraint that completes as soon as `|X|` is, so `last/2`, `has_prefix/2`
  and a two-clause `reverse/2` run in both directions.
- **Strings and type relations** (CLM-0017;
  `docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md`).
  A string literal `"abc"` is the list of its characters `<a, b, c>`.
  `atom_chars(A, L)` bridges an atom and its characters as a delayed
  constraint; `number/1`, `atom/1` and `list/1` are type relations with
  Prolog III's semantics. These are the only built-in predicates
  (`docs/reference/builtins.md`).
- **The anonymous variable `_`** (CLM-0020;
  `docs/design/2026-11-09-anonymous-variable.md`): a fresh variable per
  occurrence, never printed in an answer.
- **Negation, disjunction and if-then-else in bodies** (CLM-0022;
  `docs/design/2026-11-09-negation-disjunction-and-if-then-else.md`).
  `A ; B` tries both alternatives, `C -> T ; E` commits to the first
  solution of `C`, `\+ G` succeeds when `G` has none. A construct's
  variables shared with the clause outside it must be bound when it is
  reached; otherwise the query stops with an error rather than deciding on
  an open store (`examples/control.claimr`).
- **Explanations of answers** (CLM-0023;
  `docs/design/2026-11-09-explanations-of-answers.md`). `claimr --explain`
  and `:explain` in the REPL print each answer with its proof tree: the
  clauses that resolved every goal, by position and text, and the
  constraint instances posted. Under `--json` the answer object gains an
  `explanation` field (`docs/reference/json-output.md`).
- **Aggregation** (CLM-0025;
  `docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md`).
  The list comprehension `<T | Body>` is the list of the instances of `T`
  for each solution of `Body`; `|<T | Body>|` counts them; `sum(L, S)` is
  the built-in sum relation, exact over rationals and linear.
- **Tabling for recursive rules** (CLM-0027;
  `docs/design/2026-11-10-tabling-for-recursive-rules.md`). `:- table
  ancestor/2.` declares a predicate whose calls are answered from a table
  of its ground answers computed once over the program, so transitive
  closures terminate on cyclic data in either direction of recursion and
  give each answer once. The directive works at the REPL prompt too.
- **A step limit for queries** (CLM-0029;
  `docs/design/2026-11-11-step-limit-for-queries.md`). `--steps N` on the
  command line, `:steps N` at the prompt (`0` = unlimited) and
  `Solutions::with_step_limit(n)` in the API give a query a budget of
  resolution steps across all its answers. Reaching it is an ordinary
  runtime error whose message opens with the fixed phrase
  `step limit of N reached at` and names the goal; the answers found
  before it stand. Unlimited by default.

### Changed

- **Where `=` is numeric** (CLM-0026;
  `docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md`).
  Whether `=` / `!=` inside `{ … }` is a numeric relation or tree
  unification / `dif` is decided by what its operands denote when the goal
  is reached, never by how they came to denote it. A variable bound to a
  number by a fact and a numeric variable fixed by a constraint behave
  alike; a determined numeric variable is bound to its number.
- **Performance of rule-heavy queries over fact tables.** Ground
  arithmetic folds before it reaches the solver, and body goals are
  instantiated when reached rather than at clause entry (CLM-0013;
  `docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md`);
  the simplex tableau is truncated on backtracking, so solver rows no
  longer accumulate across a search (CLM-0018;
  `docs/design/2026-11-09-truncate-the-tableau-on-undo.md`); calls with a
  bound first argument try only the clauses that can match (CLM-0021);
  determination candidates are kept incrementally and determined values
  fold (CLM-0024). Queries that were quadratic in the number of facts are
  linear; answers are unchanged.
- **Deep terms no longer overflow the stack** (CLM-0019). Lowering,
  instantiation, printing and dropping of terms are iterative, so a
  50 000-element list literal or a 10 000-deep term runs and echoes on
  the default stack.
- **`--json` content** (`docs/reference/json-output.md`). A constraint
  string may now mention a list length (`|T| >= 1`), a pending
  concatenation (`Z = X . Y`), a pending `atom_chars` or `sum`; `--explain`
  adds the `explanation` field; the step-limit error is a runtime-error
  document with a recognisable message. The document shape is unchanged.
- The README, the EBNF grammar view, the architecture docs and the
  `answer-soundness` aspect follow every item above.

### Fixed

- **One symbol table for the program and every store it runs**
  (CLM-0028). A tabled predicate's evaluation ran over a store with its
  own copy of the symbol table, so a symbol interned inside it (by
  `atom_chars`, for instance) was unknown or mismatched in the caller's
  store: a panic, or a silently wrong answer. The program and all its
  stores now share one table.

## [0.3.0] — 2026-09-10

### Added

- `claimr --json`: one JSON document per query, structured diagnostics, a
  pinned contract for programmatic consumers (CLM-0010;
  `docs/reference/json-output.md`).
- Quoted atoms `'situation:frame-in-spec'` with backslash escapes;
  identity is content, print-back quotes exactly when needed (CLM-0011;
  `docs/design/2026-09-10-quoted-atoms.md`).

## [0.2.0] — 2026-08-25

### Added

- `highlight::line_tokens`: an error-tolerant, line-based token API over
  the grammar's terminals, for editors and UIs (CLM-0009).
- CI: formatting, clippy and tests on every change proposal.

## [0.1.0] — 2026-08-18

First release: the rustemo-generated parser, the evaluator (SLD
resolution over rational trees, `=` / `!=` on terms, answers in solved
form projected onto the query), the exact linear constraint solver
(Dutertre–de Moura simplex over arbitrary-precision rationals, attribute
terms, numeric disequations, delayed products) and the REPL.

[0.4.0]: https://github.com/a1v0lut10n/claimr/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/a1v0lut10n/claimr/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/a1v0lut10n/claimr/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/a1v0lut10n/claimr/releases/tag/v0.1.0
