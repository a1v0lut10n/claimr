---
date: 2026-11-09
type: journal
components: [evaluator, solver]
aspects: [answer-soundness, exact-arithmetic]
design:
  - docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md
---

# Design record accepted: numbers, numeric variables and attribute terms

- **When:** 2026-11-09 22:43:21

## Context

The incubation note of 2026-11-09 came out of the CLM-0023 deep test as a
speed idea: bind a variable equated to a number directly, so the solver
never hears of it. CLM-0024 removed the speed problem by other means and
left the note as a semantic question. A probe sharpened the question into
two asymmetries. A variable bound to `30` by a fact and a numeric variable
fixed at `30` by a constraint behave differently in `{ A = age(alice) }`,
the first failing — the integrity checker's exact shape, where stored
numbers meet attribute terms. And `{ M = 2 }, q(M)` with `q(f(a))`
succeeds where `q(2)` fails, so `M` is not the number 2 after `{ M = 2 }`.
Both trace to one drift between the evaluator record and the store: D5
says a determined numeric variable "is bound to its value" and the store
keeps it an unbound cell with coinciding bounds; D4 says a numeric variable
"may only ever be bound to a number" and the store's unification instead
turns the compound it meets into an attribute term; and the test for a
numeric block equation does not look through a variable bound to a number.
The record landed as PR #65 and is accepted here. The incubation note is
removed in this change: a graduation moves the content, it does not copy
it.

## Details

**Decision, three rules.** (1) A determined numeric variable is bound to
its number, as D5 says: from the moment the store knows its single value
it is a tree leaf for unification, a number for the numeric test, the
value for arithmetic and printing; `{ M = 2 }` makes `M` the number 2 as a
consequence. (2) `=` and `!=` in a constraint block are numeric when
either side *denotes* a number by any route — a literal, an arithmetic
result, a variable bound to a number, a numeric variable — and a compound
or constant on the other side is then an attribute term, exactly as the
literal case `{ f(a) = 2 }` already is; two trees with no number on either
side are unified. (3) Unification never creates an attribute term: a
number meeting a compound fails as always, and an undetermined numeric
variable meeting a compound or constant fails too, as D4 wrote. Inside
structures numbers are leaves, unchanged.

**What changes.** `person(alice, A), { A = age(alice) }` succeeds with
`age(alice) = 30` in either goal order. `{ M = 2 }, q(M)` fails like
`q(2)`. `{ M > 1 }, q(M)` with `q(f(a))` fails instead of answering
`M = f(a), f(a) > 1`; a program wanting that reading writes the block
equation `{ V = age(X) }`. The answer printer is untouched.

**Why these rules.** Prolog III's position is unambiguous — numbers are
leaves, `=` is tree equality, a numeric relation on a non-number has no
solution — and attribute terms are claimr's extension for compounds in
numeric position. The rules keep the extension where the record put it,
constraint blocks, and make numeric-ness a matter of what a term denotes
rather than of how it came to denote it, so a block relation means the
same thing wherever it stands in a conjunction. The alternatives weighed
in the record: the status quo (both asymmetries stay); looking through
bindings in the numeric test alone (fixes the first pair, leaves `{ M = 2 },
q(M)` apart from `q(2)`); keeping unification-created attribute terms
alongside rule 1 (makes a head unification depend on whether the variable
was determined before or after it met the compound).

**Deferred, named in the record.** Attribute terms for lists and for bare
constants; `enum`-style enumeration of an undetermined numeric variable.
The performance motive of the note is served by CLM-0024.

**Pre-write checks.** A design record lists no `affects` and closes no
task. Both aspects are unchanged by the decision alone; the record commits
the implementing task to a stated property in `answer-soundness.md` (the
meaning of a block relation depends on the terms its operands denote) and
to amending `solver.md`'s sentence about a numeric variable meeting a
compound, with D4 and D5 of the evaluator record restated as written. No
cross-repo impact: no output shape or vocabulary changes.

## Links

- Design record: `docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/65
- Removed incubation note, with its CLM-0024 addendum:
  `docs/incubation/2026-11-09-bind-numeric-constants-directly.md`
- The rules restated: `docs/design/2026-08-17-evaluator.md`, D4 and D5
- The probe that prompted it:
  `docs/journal/2026-11/2026-11-09-execution-clm-0024-determination-candidates.md`
- Implementing task: to follow as CLM-0026
