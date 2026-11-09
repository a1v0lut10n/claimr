---
date: 2026-11-09
type: design
status: proposed
components: [grammar, parser, evaluator, solver, cli, highlight]
aspects: [grammar-authority, exact-arithmetic, answer-soundness]
tags: [lists, tuples, concatenation, grammar, solver, prolog-iii]
---

# Lists as a constraint domain: Prolog III tuples, length and concatenation

## Context

Lists already work as terms (`examples/lists.claimr`: `nil`, `cons(H, T)`,
with `append`, `member`, `reverse`), but writing them is heavy, and they
are algebraic structures only: no length, no concatenation, no connection
to the numeric store. Data with multi-valued fields is the immediate need;
the longer-term one is that sequences are where a constraint language
earns its keep.

**Prolog III** (Colmerauer, *An Introduction to Prolog III*, CACM 33(7),
1990) treats lists as a constraint domain. "A tree a whose initial node is
labeled by `<>α` is called a list and is written `<a1,...,an>α`"; `<>` is
the empty list. Three things follow:

- **Length is a number in the store.** "The length `|a|` of the list a is
  thus the real n+α", so `{ |z| = 10 }` is an ordinary linear constraint
  and `|z| = |x| + |y|` relates lists through the simplex.
- **Concatenation has two faces that share one symbol.** `<a1,…,an> • l`
  with a literal tuple on the left is an *operation*, always defined: "The
  operation a1...an• applied to a list l1 consists in concatenating the
  true list <a1,...,an> to the left of l1." `Leaves(<a>•u, z)` in a rule
  head is this; over cons cells it is the cons pattern. `z =. x • y` with
  an unknown-length left operand is the *approximated concatenation*
  relation: it asserts "in all cases … `|an+1| = |a1| + … + |an|`" at once,
  and completes the concatenation when "the lists a1, …, ak–1 are all
  true lists", that is, of known length. That length may come from the
  store: if `|z|` and `|y|` are known, `|x|` is determined and the
  constraint wakes.
- **Only integer lengths are lists.** "True lists have as their length a
  non-negative integer." Improper lists `<a1,…,an>α` with irrational `α`
  are how the paper keeps the algebra total for unknown tails.

The paper's examples run on exactly this: `Value(y, 10m+n) → Value(x, m),
{ y =. x•<n> }` reads the *last* element; `Reverse` is two clauses through
a palindrome; the context-free recognizer is a transcription of the
grammar; the periodic-sequence query
`{ |z| = 10, <1,2,3>•z =. z•<2,3,1> }` is solved, not searched.

**What claimr has.** Rational-tree unification over compound terms; a
linear store over exact rationals (Dutertre–de Moura simplex, δ-rationals
for strict bounds, exact determined-variable detection); attribute terms
that attach a numeric unknown to a non-numeric term; delayed products that
wait in `settle()` until a factor is determined. Those are the three
mechanisms the list domain needs, each already built for something else.

**The spelling is not forced by the parser.** `<` and `>` are the
relational operators, but a relational operator occurs only in
`Constraint`, never inside `Expr`. Verified 2026-11-09 with the project's
generator (rustemo 0.9.1, `rcomp`, default LALR-Pager tables) on the
current grammar, all with zero conflicts: a `'<' Args? '>'` tuple
production; `'|' Expr '|'` on top of it; `Expr '.' Expr` on top of both
(58 productions, 91 states), where `'.'` resolves to the existing `Dot`
terminal, so the clause terminator and the concatenation operator are one
token in two grammatical roles that the LR state tells apart (a terminator
is expected only after a complete clause, never inside a term); `++` in
place of `.` as a control; and, for comparison, Edinburgh `[a, b]` /
`[H | T]` with the same length and concatenation forms. GLR remains
available (`2026-08-17-rustemo-parser-generator-over-nom.md`) and is not
needed. `.` is kept as the operator: it is Prolog III's own (the paper's
`•`), and the dot is the standard notation for concatenation in formal
language theory.

**Edinburgh brackets were weighed and not chosen.** The constraint domain
works identically under either spelling, since both sit on cons cells.
Angle brackets were chosen because they read as Prolog III, because `|L|`
stays legible (`< |L| >` against `[ |L| | T ]`), and because the paper's
string examples then hold verbatim.

## Decision

Lists become a constraint domain in Prolog III's sense, delivered in three
stages under this one decision. Each stage is useful alone and none is
undone by the next.

### Representation, common to all stages

- A list is a chain of binary cells ending in the empty list. The reserved
  label is `'<>'`: the empty list is the nullary `'<>'`, a cell is
  `'<>'(H, T)`. Prolog III's "trees whose nodes are labeled `<>`" is then
  literally true of the representation, user atoms named `cons` or `nil`
  keep their ordinary meaning, and `'<>'(a, '<>')` is the same term as
  `<a>`; the angle-bracket form is the canonical spelling.
- Lists are rational trees like every other term. A cyclic chain
  (`X = <1> . X`) is a legal term with no finite length; `|X|` on it
  fails (see stage 2). Prolog III lists always have finitely many
  daughters, and this is the corresponding rule.
- Improper lists are not modelled with an irrational `α`. An unknown tail
  is an unbound variable whose length is an undetermined numeric variable;
  the residual linear constraints on it are the answer. This carries the
  same information as the paper's `α` without inventing a number.

### Stage 1 — notation

- `<>`, `<a, b, c>` denote lists; `<a, b> . T` denotes the chain
  `a`, `b` then `T` (the operation face of Prolog III's `•`, admissible in
  heads as the cons pattern: `member(X, <X> . _).`). `.` is right
  associative and binds looser than arithmetic. In stage 1 the grammar
  admits `.` only with a literal tuple on its left (one production,
  `'<' Args '>' '.' Expr`); the general form arrives in stage 3.
- Printing: the answer printer, `--parse` and the REPL print a `'<>'`
  chain as `<1, 2, 3>`, a chain with a variable tail as `<1, 2> . T`,
  the empty list as `<>`, and cyclic chains as equations, the printer's
  existing finite form for rational trees (`src/eval/answer.rs`).
- Grammar path per `grammar-authority`: `src/parser/claimr.rustemo` gains
  the two `Expr` productions and no new terminal (`<`, `>` and `.` exist);
  `claimr_actions.rs` builds the chains; `examples/lists.claimr` is
  rewritten in the notation and keeps its golden answers;
  `docs/reference/grammar.md` follows; `src/highlight.rs` is unchanged,
  since its line-based lexer names tokens, not roles.
- No evaluator or solver change. `append`, `member`, `reverse` remain
  library predicates.

### Stage 2 — length

- `|L|` is a numeric term, admissible wherever a term is (as arithmetic is,
  per `2026-08-17-arithmetic-in-terms.md`), lowered at instantiation to a
  numeric variable attached to `L` as an attribute. Binding `L` to a cell
  posts `|L| = 1 + |T|` for its tail; binding to `'<>'` posts `|L| = 0`;
  the attachment follows unification through the existing wake lists.
- Lengths live in the simplex like any number, so `{ |Z| = |X| + |Y| }`,
  `{ |L| > 2 }` and `{ |L| = N / 2 }` are ordinary linear constraints, and
  residual constraints on lengths are printed as such.
- When a length becomes determined it must be a non-negative integer, or
  the branch fails. Undetermined lengths stay as linear residue. Both
  follow `exact-arithmetic` and keep `answer-soundness`: no length is ever
  approximated, and no answer claims a list of non-integer length.
- `|L|` on a chain that cycles fails. `|L|` on a non-list (a number, an
  atom, a compound with another label) fails, as Prolog III's operations
  are undefined there.
- Grammar: the `|` terminal and one `Expr` production, example, EBNF view,
  highlight.

### Stage 3 — concatenation

- `X . Y` with a non-literal left operand is lowered at instantiation to a
  fresh list variable `Z` and a pending constraint `Z =. X . Y`, which
  immediately posts `|Z| = |X| + |Y|` to the store and waits.
- The constraint completes, by elementwise unification of `Z`'s first
  `|X|` cells with `X` and the rest with `Y`, as soon as `|X|` is
  determined, whether by instantiation of `X`'s spine or by the simplex
  (Prolog III's "defined only if the lists a1, …, ak–1 are all true
  lists"). It wakes through `settle()`, the mechanism delayed products use.
  A pending concatenation at answer time is printed as a residual
  constraint, `Z = X . Y`.
- `Z =. X . Y` where `|X|` is unknown is **not** searched: Prolog III
  would use `enum` on a length, and so would a claimr program (`append/3`
  stays as the nondeterministic predicate for that). This is the paper's
  design and it keeps answers sound.
- The stage 1 literal-left production is subsumed by the general one; the
  action recognises a literal left operand and builds the chain directly.

## Consequences

- Stage 1 gives the sugar on its own: `aliases(p1, <'alpha', 'beta'>)`
  replaces one fact per value, `member(X, <X> . _)` is the cons pattern,
  and nothing below the parser changes.
- Stage 2 connects sequences to the numeric store: `{ |Keys| >= 2 }` as an
  integrity rule, lengths in arithmetic, the periodic-sequence query's
  `|z| = 10`. It adds one attribute kind to the store and one wake rule.
- Stage 3 gives the paper's idioms: last-element access, two-clause
  reverse, recognizers as grammar transcriptions, and prefix tests on
  character lists as constraints
  (`2026-11-09-strings-as-character-lists-and-type-relations.md`). It adds
  one pending-constraint kind beside products.
- Stages 2 and 3 create solver variables per list variable under search
  and are therefore subject to the same row-persistence cost that
  `2026-11-09-fold-ground-arithmetic-before-the-solver.md` addresses for
  arithmetic; that record's deferred tableau truncation may be forced
  sooner by this one.
- The JSON output (`docs/reference/json-output.md`) is unchanged in stage
  1 (a list is the compound it is). Stages 2 and 3 add residual-constraint
  kinds (`|L|` terms, pending concatenations) and need a contract version
  note.
- `'<>'` and `|…|` are reserved spellings; a program cannot define its
  own `'<>'/2`. The grammar's `<` `>` keep their relational meaning in
  `Constraint` and gain the tuple meaning only in `Expr`, and `.` keeps
  its terminator meaning after a clause and gains concatenation only
  inside a term; the generator check above shows neither pair ever meets.
  A number literal still needs digits on both sides of its point, so
  `<1> . 2` lexes as a concatenation and `1.5` as a number.
- Each stage is one task with its own ticket and implementation plan;
  stage 1 can start at once.
