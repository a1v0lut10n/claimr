---
date: 2026-11-09
type: design
status: proposed
components: [grammar, parser, evaluator, cli, highlight]
aspects: [grammar-authority]
tags: [lists, grammar, prolog-iii]
---

# List notation: Edinburgh brackets over cons and nil

## Context

Lists already work as terms (`examples/lists.claimr`: `nil`,
`cons(H, T)`, with `append`, `member`, `reverse`), but writing them is
heavy, `cons(a, cons(b, cons(c, nil)))` for three elements, in every
argument and every answer. Data with multi-valued fields (keys, codes,
aliases) is the immediate need.

There are two reference designs.

**Prolog III** (Colmerauer, *An Introduction to Prolog III*, CACM 33(7),
1990). A list is a tree with a special root label: "A tree a whose
initial node is labeled by `<>α` is called a list and is written
`<a1,...,an>α`"; `<>` is the empty list. Three things make it a constraint
domain rather than a notation:

- its length is a number in the store: "The length `|a|` of the list a is
  thus the real n+α", and `{ |z| = 10 }` is an ordinary linear constraint;
- concatenation `•` is a delayed ("approximated") constraint:
  `an+1 =. a1 • … • an` asserts `|an+1| = |a1| + … + |an|` in all cases,
  and completes once "the lists a1, …, ak–1 are all true lists", that is,
  of known length;
- head patterns use it: `Leaves(<a>•u, z)` is the cons pattern, and
  `Value(y, 10m+n) → Value(x, m), { y =. x•<n> }` reads the *last*
  element, something Edinburgh lists cannot do without walking.

Improper lists `<a1,…,an>α` (irrational `α`) stand for a known prefix
followed by something of unknown length.

**Edinburgh Prolog.** `[]`, `[a, b, c]`, `[H | T]` as spelling for a
binary cons functor and an empty-list constant. Length and concatenation
are predicates (`length/2`, `append/3`), not constraints.

What claimr has: rational-tree unification over compound terms, a linear
store over exact rationals, and no list term kind.

The spelling is not forced by the parser. `<` and `>` are claimr's
relational operators, but a relational operator occurs only in
`Constraint`, never inside `Expr`, so a `<` after a complete term can only
be an operator and a tuple can only start where a term starts. Verified
2026-11-09 with the project's generator (rustemo 0.9.1, `rcomp`, default
LALR-Pager tables) on the current grammar: adding `'<' items=Args? '>'`
to `Expr` compiles with no conflicts (56 productions, 86 states); adding
Prolog III's length bars `'|' Expr '|'` on top still has none (57
productions, 89 states); the Edinburgh variant with `[`, `]` and `|`
likewise (57 productions, 90 states). rustemo's GLR mode would be
available for a genuine ambiguity in any case
(`2026-08-17-rustemo-parser-generator-over-nom.md`), but none arises.

What does decide the question is the semantics. Prolog III's notation
without its length and concatenation constraints would be fidelity in
spelling only: the constraints are what the design is for, and they are a
component of the order of the linear store (lengths as solver variables
bound to list variables, a delayed concatenation with its own waking
rules, improper lists). Without them a cons pattern is still needed in
rule heads, and Prolog III's own cons pattern, `<a> • u`, *is* a
concatenation. Edinburgh's `[H | T]` is the standard cons pattern that
needs no constraint, and it is what the wider Prolog ecosystem, and the
editors around claimr, already read.

## Decision

1. **Edinburgh notation, as sugar over two reserved names.** `[]` denotes
   the empty list; `[a, b, c]` and `[H | T]` denote nested cons cells. The
   underlying functors are the reserved names `'[]'` (nullary) and
   `'[|]'`/2, following SWI-Prolog, so that user atoms and predicates
   named `cons` or `nil` keep their ordinary meaning and nothing a program
   writes by hand is silently reinterpreted as a list. `'[]'` and `'[|]'`
   are spellable as quoted atoms, and `'[|]'(a, '[]')` is the same term as
   `[a]`; the brackets are the canonical spelling. Brackets rather than
   Prolog III's angle brackets because `[H | T]` is a cons pattern that
   exists without a concatenation constraint, because every Prolog reader
   knows it, and because `<` and `>` then keep a single meaning in source.
   Angle brackets parse without conflict (Context) and stay available to
   the deferred tuple domain should it want a spelling of its own.
2. **`[a | b]` is a term** (`'[|]'(a, b)`), not an error. The grammar
   admits any term after `|`; the printer prints it back as `[a | b]`.
   This is the Edinburgh improper list; Prolog III's improper lists
   (unknown-length tails) are a different notion and belong to the
   deferred constraint domain.
3. **Printing.** The answer printer, `--parse`, and the REPL print a
   `'[|]'` chain in bracket form, unfolding the tail while it is a cons
   cell, and `'[]'` as `[]`. Cyclic terms keep the printer's existing
   representation as equations (`src/eval/answer.rs`: `X = f(X)` is the
   finite form of a rational tree), so a cyclic list prints as
   `X = [1 | X]`, never unfolded without bound.
4. **Grammar path**, per `grammar-authority`: `src/parser/claimr.rustemo`
   gains the terminals `[`, `]`, `|` and a list production under `Expr`;
   `claimr_actions.rs` builds the cons terms; `examples/lists.claimr` is
   rewritten in the new notation and keeps its golden answers; the EBNF
   view `docs/reference/grammar.md` follows. `src/highlight.rs` gains the
   three terminals (its grammar-coverage test enforces this).
5. **Length and concatenation constraints are deferred, not rejected.**
   They are desirable and compatible with this notation: a length can be a
   numeric variable attached to a list variable, and concatenation a
   delayed constraint over cons lists, exactly as Prolog III's approximated
   concatenation completes when prefixes have known length. Recorded in
   `docs/incubation/2026-11-09-list-length-and-concatenation-constraints.md`.

## Consequences

- No evaluator or solver change: lists are compound terms, unification
  and `dif` already handle them, and `append`, `member`, `reverse` are
  library predicates as before. `aliases(p1, ['alpha', 'beta'])` replaces
  one fact per value.
- The JSON output (`docs/reference/json-output.md`) is unchanged: a list
  is serialised as the compound it is, under the current contract. A
  dedicated array rendering would be a separate contract version, decided
  when a consumer needs it.
- Programs that used `cons`/`nil` by convention keep working but gain
  nothing until rewritten; `examples/lists.claimr` is the rewrite.
- The character-list reading of strings
  (`2026-11-09-strings-as-character-lists-and-type-relations.md`) depends
  on this record.
