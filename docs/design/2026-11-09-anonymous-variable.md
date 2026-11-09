---
date: 2026-11-09
type: design
status: proposed
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
tags: [anonymous-variable, grammar, variables, prolog-iii]
---

# The anonymous variable `_`

## Context

The lists record writes `member(X, <X> . _)` and the strings record
`has_prefix(Id, Prefix) :- atom_chars(Id, Cs), { Cs = Prefix . _ }.`;
neither parsed when the records were implemented (CLM-0014, CLM-0017),
because the grammar has no anonymous variable: `Var` requires an
uppercase initial, so `_` is a lexical error. The shipped programs invent
names for don't-care positions (`member(X, <X> . T)`), and those names
then appear in `:list` and, in a query, among the answer's variables. The
incubation note `2026-11-09-anonymous-variable.md` captured the gap; this
record graduates it.

Prolog III has `_` with the Prolog-family meaning: a variable whose every
occurrence is a different variable, and which is never reported. Several
Prologs also treat `_`-prefixed names (`_Tail`) as named but hidden; that
is a separate question.

## Decision

1. **One terminal, `_`**, admitted wherever a variable is: `Expr` gains no
   production, the `Var` alternative's terminal becomes "a variable or the
   anonymous variable", and the AST carries it as `Expr::Var("_")`. No
   new AST node: the name `_` cannot be written as a variable otherwise,
   so it is unambiguous.
2. **A fresh variable per occurrence.** The compile step's variable map
   allocates a new clause-variable index for every `_` instead of looking
   the name up. Two `_` in one clause are two variables.
3. **Hidden in answers.** In a query, an `_` is not an answer variable: it
   is not among the query variables, prints no equation, and does not
   name anything. Anything reachable from it that is also reachable from
   a named variable prints as today (internal variables as `_1`, `_2`).
   A residual constraint that mentions only variables reachable from `_`
   is projected away like any internal one.
4. **Printed as `_`** by the clause printer: `:list`, query echoes and
   `--parse` show `_` where it was written, so a program round-trips.
5. **`_`-prefixed names stay what they are**, lexical errors, until a
   program wants the "named but hidden" reading; that is a later record,
   not a consequence of this one.

## Consequences

- Grammar path per `grammar-authority`: the terminal, the action (none:
  `Var` already yields its text), an example (the lists example's
  `member/2` and the strings example's `has_prefix/2` take the records'
  spelling), the EBNF view, the README, and `src/highlight.rs` (the `Var`
  rule recognises `_` as a variable; the coverage test's regex examples
  gain one). The two records' examples become valid as written.
- The evaluator's change is in the compile step's variable numbering and
  in the query's variable list; the machine and the store are untouched.
  `answer-soundness` is unaffected: `_` changes what is named, not what is
  derived or printed about named variables.
- The anonymous variable is the only name with this behaviour; the
  printer's internal names `_1`, `_2`, … remain unparseable, so no written
  program can collide with them.
