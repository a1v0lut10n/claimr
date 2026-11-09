---
date: 2026-11-09
type: design
status: proposed
components: [evaluator, solver]
aspects: [answer-soundness, exact-arithmetic]
tags: [numbers, numeric-variables, attribute-terms, equality, unification, d4, d5, prolog-iii]
---

# Numbers, numeric variables and attribute terms: where `=` is numeric

## Context

The incubation note of 2026-11-09 proposed binding a variable equated to a
number directly, for speed; CLM-0024 removed the speed problem another
way and left the note as a semantic question. Probing the current
behaviour shows that the question is real and has two faces.

```
{ age(alice) = 30 }.   person(alice, 30).   q(f(a)).

?- person(alice, A), { A = age(alice) }.      false
?- { A = 30 }, { A = age(alice) }.            A = 30, age(alice) = 30
?- { f(a) = 2 }.                              f(a) = 2
?- q(2).                                      false
?- { M = 2 }, q(M).                           M = f(a), f(a) = 2
?- { M > 1 }, q(M).                           M = f(a), f(a) > 1
?- { X = <2> }, { X = <f(a)> }.               false
```

The first pair breaks referential transparency: a variable bound to `30` by
a fact and a numeric variable fixed at `30` by a constraint behave
differently in the same equation, and the former answer is wrong for the
integrity checker, whose stored numbers meet its attribute terms exactly
this way. The second pair breaks it the other way: `{ M = 2 }, q(M)`
succeeds where `q(2)` fails, so `M` is not the number 2 after `{ M = 2 }`.
Both come from one drift between the evaluator record and the store:

- **D5 as written**: a numeric variable "with a single possible value is
  bound to it, so tree unification, disequations and answers see it". The
  store keeps a determined numeric variable as an unbound `Var` cell with
  coinciding bounds, so unification does not see it as a number.
- **D4 as written**: a numeric variable "may only ever be bound to a
  number (binding it to any other tree makes the store unsatisfiable, as
  Prolog III's `+` constrains `x` and `y` to denote numbers)". The store's
  `bind` instead makes the compound an attribute term when a numeric
  variable meets it in unification.
- The test for a numeric `=` in a constraint block (`is_numericish`)
  counts a number cell and an unbound numeric variable, but not a variable
  bound to a number, so `{ A = age(alice) }` with `A` bound to `30` is tree
  unification and fails.

Prolog III's position is unambiguous: numbers are leaves of the tree
domain, `=` is tree equality, and a numeric relation on a non-number has
no solution (the CACM paper's definition of the operations; the evaluator
record's D4 quotes it). Attribute terms are claimr's own extension, "a
conservative extension of Prolog III (uninterpreted functions from trees
to Q)", motivated by `{ age(socrates) > 70 }`: a compound *in numeric
position* names an unknown rational. The question is where numeric
position begins and ends.

## Decision

1. **A determined numeric variable is bound to its number**, as D5 says.
   The moment the store knows a numeric variable's single value — a fixing
   equation, coinciding bounds, the exact probe before an answer — the
   variable is bound to that number's cell, and from then on it *is* the
   number: a tree leaf for unification, a number for `is_numericish`, the
   value for arithmetic and printing. `{ M = 2 }` therefore makes `M` the
   number 2, the incubation note's proposal, as a consequence rather than
   a special case; so does `{ M > 1, M < 3, M != 2 }`-style determination,
   whenever it happens.

2. **`=` and `!=` in a constraint block are numeric when either side
   denotes a number**: a number literal, an arithmetic result, a variable
   bound to a number, or a numeric variable. The other side, if it is a
   compound or a constant, is then in numeric position and names an
   attribute term, exactly as the literal case `{ f(a) = 2 }` does today.
   Two trees with no number on either side are unified, as D4 says. This
   makes `person(alice, A), { A = age(alice) }` true, with
   `age(alice) = 30`, in either goal order, and makes a variable bound to a
   number behave like the number it is bound to.

3. **Unification never creates an attribute term.** A number meeting a
   compound or constant fails, as it always has; a numeric variable that is
   not yet determined meeting a compound or constant fails too, as D4
   wrote. Attribute terms arise only from a compound or constant written in
   a numeric position of a constraint block: an operand of `< > <= >=`, of
   arithmetic, or of a numeric `=`/`!=` by rule 2. A program that wants "the unknown named `age(X)`" to be the
   value of `V` says so in a block, `{ V = age(X) }`, and gets the attribute
   term there.

4. **Inside structures, numbers are leaves.** `{ <2> = <f(a)> }` and
   `{ p(2) = p(f(a)) }` unify their arguments and fail; numeric position is
   a *direct* operand of a block relation, not any position reachable from
   one. This is unchanged and is stated so that rule 2 is not read as
   deeper than it is.

5. **Not in this record.** Attribute terms for lists (`<a> = 2` names an
   attribute term today and will go on doing so under rule 2, being a
   compound in numeric position); whether a bare constant should be
   admitted as an attribute term at all; `enum`-style enumeration of an
   undetermined numeric variable. The performance motive of the incubation
   note is already served by CLM-0024.

## Consequences

- **Soundness and goal-order independence.** Every answer under the new
  rules is a consequence of the program: a determined variable has its
  value in every solution, so binding it changes no solution set; rule 2
  decides numeric-ness by what a term *denotes*, which later goals cannot
  change for a number, so a block equation means the same thing wherever
  it stands in a conjunction; rule 3 removes the one path by which the
  history of a variable (numeric before or after being fixed) changed what
  a head unification did. `answer-soundness` gains a stated property:
  the meaning of a constraint-block relation depends on the terms its
  operands denote, not on how they came to denote them.

- **Behaviour that changes.** `{ M = 2 }, q(M)` with `q(f(a))` now fails,
  as `q(2)` does; `{ M > 1 }, q(M)` fails instead of answering
  `M = f(a), f(a) > 1`; `person(alice, A), { A = age(alice) }` now
  succeeds with `age(alice) = 30` instead of failing; `{ X = 31 },
  { X = age(bob) }` keeps its answer and `{ X = age(bob) }, { X = 31 }`
  too. Programs that relied on head unification to turn a numeric unknown
  into an attribute term must write the block equation; the golden
  examples will show whether any did. The answer printer is untouched: a
  bound numeric variable prints as its number, as the determined one did.

- **Store.** Determination (`numeric_consequences`, the cheap pass and the
  exact one) binds the heap variable to a fresh number cell after fixing
  the solver variable; `bind` of an undetermined numeric variable meeting a
  constant or compound returns false; `is_numericish` dereferences through
  bindings to a number. The determination candidates of CLM-0024 shrink by
  the same binding. `unknown_of` is unchanged (a bound variable
  dereferences to its number). The solver doc's *Semantics as implemented*
  loses the sentence about a numeric variable meeting a compound.

- **Evaluator record.** D4 and D5 are restated as written, not changed;
  this record is the one that notes the drift and closes it, and is added
  to the `decisions` of the solver and evaluator component docs.

- **One task** implements the three rules with tests for each line of the
  probe above in both goal orders, the integrity-checker shape, and the
  examples; `solver.md`, `evaluator.md` and `answer-soundness.md` follow.

## Sources

- `docs/design/2026-08-17-evaluator.md`, D4 and D5.
- A. Colmerauer, *An Introduction to Prolog III*, CACM 33(7), 1990: the
  domain and the operations on numbers.
- Incubation note `2026-11-09-bind-numeric-constants-directly.md` and its
  CLM-0024 addendum (graduated by this record).
