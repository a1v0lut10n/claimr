---
date: 2026-11-09
type: journal
components: [evaluator]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0021-first-argument-indexing.md
---

# CLM-0021 executed: first-argument clause indexing

- **When:** 2026-11-09 16:19:25

## Context

The machine tried every clause of a called predicate in turn, so a join
over two fact tables keyed by an id cost a full scan of the second table
per row of the first, and the choice point left behind for every call
kept the solver's trail and the dif waiters alive for the whole scan.
The incubation note of 2026-11-08 proposed the standard remedy, an index
on the principal functor of the first argument, and ranked it first among
the open ideas because it changes nothing a program can observe. The
note graduated straight into the plan: an implementation choice with no
semantic content needs no design record.

## Details

**The index.** The compile step builds, per predicate, three lists over
clause positions: all clauses, the clauses whose first argument has a
constant, number or `functor/arity` key, grouped by that key, and the
clauses whose first argument is a variable, an arithmetic term, a length
or a pending concatenation. The variable-first list is merged into every
keyed list in program order once all clauses are read, so a keyed lookup
yields exactly the clauses a plain scan would have tried, in the order it
would have tried them, minus the ones whose head could not unify. A
predicate with no first argument indexes nothing.

**The lookup.** When a goal is called, the machine dereferences its first
argument through the store. A constant, number or compound yields the key
and the merged list for it (empty when no clause carries the key and no
clause is variable-first, so the call fails without a choice point). A
variable, including a numeric variable whose value the simplex has
determined, yields the full list. A choice point now holds the candidate
list and the position within it rather than the next clause index, and
none is pushed when the candidate just tried was the last.

**Soundness.** A skipped clause is one whose first head argument is a
constant, number or compound with a different principal functor than the
dereferenced call argument, which unification would have rejected before
any store work. Answers are therefore the same set in the same order, and
the solver's state at each answer is the same, since a failed head
unification undoes everything it did. The golden answer files are
byte-identical, as are the answers of the measurement programs on the two
builds.

**One observation, unchanged by this task.** A determined numeric
variable in the first argument of a call matches constant and compound
heads as well as numbers: the variable unifies with the head term as an
attribute term under the solver's D4 rule, and the machine does not
reject it. The index treats such a variable as unbound exactly so this
behaviour stays as it is; the test records it.

**Measurement** of `violation(C) :- k(C, K), n(C, N), { K > N }.` over two
fact tables sharing the id column, on this machine, release build, one
query listing all violations:

| Facts per table | main    | indexed |
|-----------------|---------|---------|
| 1 820           | 0.71 s  | 0.03 s  |
| 3 640           | 1.83 s  | 0.05 s  |
| 7 280           | 7.13 s  | 0.09 s  |
| 14 560          | 28.24 s | 0.18 s  |

Quadratic becomes linear, and the variation across runs is within the
timer's resolution.

**Verification.** 140 tests pass, among them
`first_argument_indexing_preserves_clause_order_and_answers` (a seven
clause predicate mixing constant, variable, compound and rational first
arguments, queried ten ways, including a determined numeric variable)
and `deterministic_lookups_leave_no_choice_point`. Formatting, clippy with
warnings denied and the docs linter are clean.

**Pre-write checks.** `evaluator.md`, the one doc in the task's `affects`,
is current: the compile step paragraph names the index, the machine
paragraph names the candidate lists and the absent choice point, and the
"not yet" list now says indexing on other arguments. `answer-soundness`
held: no answer changed and no clause that could have contributed is
skipped. No cross-repo impact: the JSON output and the token API are
untouched.

## Links

- Task: `docs/implementation/2026-11-09-clm-0021-first-argument-indexing.md`
  (the incubation note of 2026-11-08, moved)
- Ranking of the incubation items: the CLM-0020 closing conversation,
  not journaled
- Commits `77ed20c` (plan, ticket claim), `d782d1a` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/54
- View updated: `docs/architecture/components/evaluator.md`
