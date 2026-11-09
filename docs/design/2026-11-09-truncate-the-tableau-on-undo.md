---
date: 2026-11-09
type: design
status: proposed
components: [solver]
aspects: [exact-arithmetic, answer-soundness]
tags: [solver, simplex, backtracking, performance]
---

# Truncate the simplex tableau on undo

## Context

`Simplex::undo_to` restores bounds only. Rows and solver variables created
since a mark persist, unbounded and therefore inert, and `Simplex::update`
walks every row on every value change, so each assertion costs time
proportional to every row ever made. Under search that is quadratic: a
query that tries F candidates, each posting one row, costs O(F²).
`2026-11-09-fold-ground-arithmetic-before-the-solver.md` removed the
commonest source of such rows (ground arithmetic now folds) and **deferred
this record** with the words "a separate record if the non-ground case
ever shows the same growth", naming the obstacle: pivots rewrite pre-mark
rows in terms of post-mark variables, so truncating `vars` and `rows`
needs a soundness argument for the restored basis.

The non-ground case has shown the growth twice since:

- CLM-0015 measured `{ |Keys| < 2 }` over facts with an open list field
  (`<k1, k2> . T`): each candidate posts a length entry and a defined
  variable; 0.04 s, 0.11 s, 0.33 s, 1.14 s at 1 820, 3 640, 7 280 and
  14 560 facts, against a flat 0.02 s to 0.12 s for complete lists.
- A rule with an open numeric unknown, `violation(C, Y) :- kn(C, K, N),
  { K + Y > N }.` queried with `{ Y > 0, Y < 1 }, violation(C, Y)`: 2.38 s,
  9.71 s, 39.79 s, 161.19 s at the same sizes (release build, all
  answers). This is the integrity-checker shape with a parameter left to
  the solver, and it is unusable past a few thousand facts.

A spike on a scratch branch (not merged) implemented the truncation below
in about forty lines, passed all 132 tests with every golden file
byte-identical, and measured 0.03 s, 0.05 s, 0.09 s, 0.18 s for the open
lists and 0.03 s, 0.06 s, 0.12 s, 0.23 s for the arithmetic, with answer
files identical to the persisting solver's at every size.

## Decision

`Simplex::undo_to(mark)` restores the solver to the mark in full: the
bounds of the variables that existed then, **and the set of variables and
rows**. A `Mark` records the variable count beside the bound-trail length.

**Truncation.** Let `n` be the variable count at the mark; variables `>= n`
are post-mark.

1. Pop the bound trail to the mark as today, restoring bounds only for
   variables `< n` (entries for post-mark variables are discarded with
   them).
2. While some row whose basic variable is pre-mark contains a post-mark
   variable `x`, pivot `x` into basic position in that row. The row's
   pre-mark basic becomes non-basic; `x` disappears from every other row
   by the pivot's substitution. Each pivot reduces the number of pre-mark
   basics, so the loop terminates.
3. Drop every row whose basic variable is post-mark; truncate the
   variable and row-index tables to `n`; rebuild the basic-to-row index.
4. Restore the invariant that non-basic variables sit within their bounds:
   a pre-mark variable turned non-basic in step 2 may now be outside its
   restored bounds; move it to the bound with `update` (as an assertion
   does), then run `check`. The pre-mark system was feasible, so `check`
   succeeds.

**Soundness.** The rows are at all times an equivalent form of the
equations posted so far (pivoting rewrites, it never adds or loses an
equation). Every equation posted after the mark was a definition of a
then-fresh variable (`define` and `slack` both create the variable they
define), so eliminating the post-mark variables from the system leaves
exactly the pre-mark equations over the pre-mark variables. After step 2
every row with a pre-mark basic is over pre-mark variables only, and every
row with a post-mark basic is a definition of that variable in terms of
the rest; dropping those rows is that elimination. Pivoting does not touch
values, so the current assignment remains a solution of the rows
throughout; step 4 repairs bounds exactly as `assert_lower`/`assert_upper`
do after any assertion. The restored state is therefore a valid tableau of
the pre-mark system with a feasible assignment: the same constraint store
the persisting solver had, in a possibly different basis, which `check`,
`is_determined` and the printer are indifferent to.

**Consequences for the store.** Solver variable indices are **reused**
after an undo. Everything in the store that holds an `SVar` is already
trailed or truncated at the mark (the numeric variable map, the attribute
registry and index, numeric disequations, products, lengths,
concatenations, aliases) except the definitions map kept for printing,
which now drops its entries for removed variables on undo. No other code
may hold a solver variable across a mark. `changed_since(mark)` counts a
created variable as a numeric change, which is what its one caller (the
disequation trial) means by it.

**Alternatives weighed.**

- *Copy the tableau at each mark.* Simple and exact, but a copy is linear
  in the tableau on every clause trial, so the quadratic cost returns in a
  different place.
- *Trail the pivots* and replay them backwards. Restores the exact
  pre-mark basis, but the trail grows with the work done in the branch and
  the replay costs as much as the pivots did; the basis itself is not part
  of the store's meaning.
- *Forbid pivots that mix pre- and post-mark variables.* Would make
  truncation trivial, but `check` must pivot where Bland's rule says or it
  loses termination and completeness.
- *Keep persisting rows and rely on folding.* The deferral that this
  record ends; the two measurements above are its limit.

## Consequences

- Rule-heavy queries with an open numeric unknown or open list fields
  cost the same as their constraint-free form; the two tables above
  collapse onto their first columns.
- Answers are unchanged in content and form; the spike's answer files are
  byte-identical to the persisting solver's. `--json` is unchanged.
- `docs/architecture/components/solver.md`'s *Backtracking* note changes:
  rows and variables created since a mark no longer persist, and the
  tableau truncation deferred there is done.
- `2026-11-09-fold-ground-arithmetic-before-the-solver.md` is refined, not
  superseded: folding still keeps ground arithmetic out of the solver,
  which is cheaper than posting and truncating.
- The implementation task lands the spike with a **replay-equivalence
  test**: random sequences of definitions, assertions, marks and undos on
  one solver, compared at every step against a fresh solver fed only the
  operations that survive the undos (feasibility, determined values, and
  the pre-mark bounds), under a small deterministic generator and many
  seeds; plus the two benchmarks as a regression check on growth. A debug
  assertion guards step 4's feasibility.
