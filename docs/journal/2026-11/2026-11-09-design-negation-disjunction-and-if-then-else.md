---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
design:
  - docs/design/2026-11-09-negation-disjunction-and-if-then-else.md
---

# Design record accepted: negation, disjunction and if-then-else in bodies

- **When:** 2026-11-09 18:37:38

## Context

The integrity-checker experiment of 2026-11-08 left two incubation notes:
absence rules ("a model file that has no residual-error claim") need a
goal that succeeds when another has no solution, and committed choice
("the curator's value if there is one, else the harvested one") needs
disjunction and if-then-else. After CLM-0021 they were ranked first among
the open ideas as the largest expressive gap, and taken as one record. The
record was drafted around the question the notes raised, soundness of a
negated or committed condition over a constraint store, checked against
the Prolog III reference manual (Version 1.3, December 1991) on request,
and landed as PR #55. It is accepted here. The two incubation notes are
removed in this change: a graduation moves the content, it does not copy
it.

## Details

**Decision.** ISO spellings and precedence: `A ; B`, `C -> T ; E`,
`C -> T`, `\+ G`. Disjunction opens a choice point and stays in the AST,
so clauses print back as written. If-then-else runs its condition to a
first solution, finalizes the store exactly as for an answer, commits, and
runs the then-branch; without a solution the store is restored and the
else-branch runs. Negation is `(G -> fail ; true)`. There is no cut.

**The groundness rule.** A condition's *external* variables are those
that also occur in the clause outside the whole construct: in the head,
in goals before or after it. They must be ground when the construct is
reached, numeric variables determined by the solver included. Variables
local to the construct, shared only with its branches, may be anything.
With that, later goals can only constrain variables the condition does
not reach, so its first solution or failure is the same in every
completion of the derivation; the construct is a function of ground
inputs and stays sound under nesting and negation. Committing drops the
condition's other solutions, which is what committing means, and that is
the only incompleteness introduced.

**The non-ground case is a runtime error**, `EvalError::NonGroundCondition`,
naming the clause, the construct and the variable by its source name.
Nothing is decided in either direction. The alternative, letting `\+ G`
succeed when `G` happens to have no solution and erroring only when it
has one, was rejected: a misordered program would then pass on one data
set and fail on another.

**Prolog III.** The manual has the cut `/` (`!`), defines `not(P) -> P !
fail;` through it, offers `X ; Y` as a choice point in Edinburgh syntax,
and warns that enumeration must precede negation or the negation is
"systematically verified", a program it then rejects as against its own
philosophy of constraints before enumeration. Its answers are the
meta-predicate `known` and, for Booleans, negation inside the Boolean
algebra; it also has `freeze`. claimr's constructs compute what the
manual's `not`, `;` and `si_alors_sinon` idiom compute; the departures
are no cut and an error where the manual documents a wrong answer. The
groundness check is `known` performed by the machine.

**Deferred, named in the record:** constructive negation, a delayed
negation in the manner of `freeze`, `true/0` and `fail/0` as built-ins.

**One idiom moves.** `value(K, X) :- (curated(K, X) -> true() ;
harvested(K, X)).` errors when `X` is unbound, correctly; the sound form
is `value(K, V) :- (curated(K, X) -> { V = X } ; harvested(K, V)).`. The
reference will document the pair.

**Pre-write checks.** The record lists no `affects` and closes no task,
so nothing to resolve. `grammar-authority` and `answer-soundness` are
unchanged by the decision alone; the record commits the implementing task
to a new invariant in the soundness aspect (a condition is decided only
on ground external variables, otherwise the query stops) and to the usual
grammar path. No cross-repo impact: the runtime error reaches the JSON
output as the existing error document.

## Links

- Design record: `docs/design/2026-11-09-negation-disjunction-and-if-then-else.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/55
- Removed incubation notes: `docs/incubation/2026-11-08-negation-as-failure.md`,
  `docs/incubation/2026-11-08-disjunction-and-if-then-else.md`
- Implementing task: to follow as CLM-0022
