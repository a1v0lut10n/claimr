---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, solver, highlight]
aspects: [grammar-authority, answer-soundness, exact-arithmetic]
design:
  - docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md
meta-impact:
  note: "The --json `constraints` strings will gain the goal-form residual `sum(L, S)` for a pending sum once the record is implemented (document shape unchanged, vocabulary grows, as with CLM-0015..17); aicogito's reasoning adapter treats the strings as opaque. The suggestion from CLM-0015 stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Design record accepted: aggregation — list comprehensions and the sum relation

- **When:** 2026-11-09 21:50:22

## Context

Integrity rules that need counts — a cohort's size against a floor, the
number of distinct sources behind an assertion, "at least k precedents" —
produced the incubation note of 2026-11-08: comparing a stored number is a
linear constraint and works, deriving it needs all the solutions of a
goal. After CLM-0024 it was the next item the integrity-checker use would
feel. The record was drafted against Prolog III's `findall(V, P, L)` and
`enum(R)` (manual pp. 250, 244) and against a fact about claimr that rules
the Prolog spelling out: a goal is not a term here, so a constraint block
or a conjunction has no spelling as an argument. It landed as PR #62 and is
accepted here. The incubation note is removed in this change: a graduation
moves the content, it does not copy it.

## Details

**Decision.** All solutions are collected by a **list comprehension**
`<T | Body>`: an expression denoting the list of the instances of `T` for
each solution of `Body`, in solution order, `<>` when there is none,
admitted wherever a term is in a body and evaluated when its goal is
reached, which the compile step arranges by desugaring each comprehension
into a collection step binding a fresh variable. A comprehension in a head
is a compile error with its position: a head is a pattern, not a goal that
runs. **Count** is the comprehension's length, `|<X | p(X)>|`, already a
numeric term in the store, so no counting primitive exists. **`sum(L, S)`**
is a built-in relation, the fifth: on a complete list it posts the linear
equation over the elements, exact and solvable in either direction
(`sum(<1, X, 3>, 10)` gives `X = 6`); a non-number element fails; an open
spine suspends on the tail as length does and prints in goal form.

**Two soundness rules.** The groundness rule of CLM-0022 applies to the
comprehension's external variables — those of `T` and `Body` also
occurring in the clause outside the comprehension, the rest of its own
goal included — so `{ N = |<X | p(X, N)>| }` is a runtime error, `N` being
both result and input. And every solution is finalized exactly before its
instance is taken, and the instance must be ground; otherwise the query
stops. A solution carrying residual constraints on the template has no
single value to collect, and Prolog's copy with fresh variables would drop
the constraints silently. With both rules the comprehension is a function
of ground inputs, sound under negation and in conditions, and nothing is
collected that the store does not determine.

**Why not `findall/3` or a keyword.** The goal argument needs goal syntax;
a reified body is a far larger change than the feature; a `findall`
keyword would reserve the word in every position, atoms included, which
the reserved built-in names of CLM-0017 deliberately avoid. The list
notation already had the bracket pair and the bar.

**Deferred, named in the record.** `bagof`/`setof` with grouping; `min`,
`max`, `average`, `distinct` as primitives (two clauses each with the
if-then-else of CLM-0022, none linear); a bound on infinitely many
solutions (the REPL's Ctrl-C interrupts; a limit is a feature of its own);
integer enumeration under constraints, Prolog III's `enum`, as an
incubation note for the finite-domain future.

**Pre-write checks.** A design record lists no `affects` and closes no
task. The three aspects are unchanged by the decision alone;
`grammar-authority` will be followed in order by the task, and the record
commits it to restating `answer-soundness` invariant 5 for the three
constructs that run a sub-search (if-then-else, negation, comprehension)
and to keeping `exact-arithmetic` through `sum`'s exact rationals.
Cross-repo impact is the `meta-impact` note: a new goal-form string in the
JSON `constraints` vocabulary once implemented.

## Links

- Design record: `docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/62
- Removed incubation note: `docs/incubation/2026-11-08-aggregation-and-all-solutions.md`
- Builds on: `docs/design/2026-11-09-lists-as-a-constraint-domain.md`,
  `docs/design/2026-11-09-negation-disjunction-and-if-then-else.md`
- Implementing task: to follow as CLM-0025
