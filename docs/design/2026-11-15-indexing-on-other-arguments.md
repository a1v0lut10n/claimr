---
date: 2026-11-15
type: design
status: proposed
components: [evaluator]
aspects: [answer-soundness]
tags: [indexing, machine, performance, explanations, json]
---

# Indexing on arguments other than the first

## Context

CLM-0021 gave each predicate a first-argument index: a call whose first
argument is bound visits only the clauses that can match it, in program
order. It was an implementation choice with no semantic effect and took no
design record; `components/evaluator.md` has listed "indexing on other
arguments" under *Not yet* since.

A call bound anywhere else still visits every clause. So a join through a
second argument is quadratic in the number of facts:

```
same_n(A, B) :- n(A, N), n(B, N).
```

Measured on 0.6.0 (release build, all answers, one `n` fact per id with a
distinct value): 0.54 s, 2.18 s and 8.34 s at 1 820, 3 640 and 7 280 facts.

This is the shape of every rule that reads a relation against its natural
direction. Fact tables keyed by an id are queried by id; a graph stored as
`precedes(A, B)` or `part_of(Part, Whole)` is walked from either end, and
"two records about the same thing" joins on a value, not on the key. The
integrity rules that CLM-0013, CLM-0018 and CLM-0021 made linear all bind
the first argument; rules over typed edges do not.

The workaround today is to state each relation twice, once per direction
(`n_by(N, Id)` beside `n(Id, N)`), and write each rule against the right
copy. Measured at 500 000 facts: load time goes from 2.85 s to 5.62 s and
peak memory from 633 MB to 1 064 MB, the program text doubles, and the
choice of copy leaks into every rule.

Unlike CLM-0021, this change is visible in two places, which is why it
takes a record:

- a failed call's explanation counts the candidates the index admitted,
  and its wording names "the first argument"
  (`no clause (none of 2 clauses can match the first argument)`);
- the `failure` object under `--json` carries the same `candidates` count.

A spike on a scratch branch (`spike/other-argument-indexing`, not merged)
implemented the decision below in about ninety lines across `compile.rs`
and `machine.rs`. All 160 unit tests pass and every golden file is
byte-identical.

| | 0.6.0 | spike |
|---|---|---|
| The join above, 1 820 facts | 0.54 s | 0.01 s |
| 3 640 facts | 2.18 s | 0.03 s |
| 7 280 facts | 8.34 s | 0.05 s |
| 29 120 facts | (not run) | 0.21 s |
| Four integrity rules over 104 728 facts (first argument bound) | 0.62 s | 0.62 s |
| The same over 418 912 facts | 2.52 s | 2.50 s |
| `examples/queens.claimr` | 0.05 s | 0.05 s |

Answer files are identical to 0.6.0's wherever both ran. Memory, at
500 000 facts with a distinct value each (the worst case, one index entry
per clause): 608 MB with no further index built, 737 MB once the
second-argument index exists.

## Decision

**1. An index per argument position, built on first use.** A predicate
keeps, for each argument position after the first, a slot for an index of
the same kind as the first-argument one: the principal functor of the
clause's argument at that position (a constant, a number, or a functor
with its arity) mapped to the positions of the clauses that carry it,
merged with the clauses whose argument there is a variable or an
arithmetic term, **in program order**. A slot is empty until a call needs
it and is then built once, in one pass over the predicate's clauses. The
first-argument index is built at compile time, as today.

**2. The call takes the shortest list.** A call's candidates start as the
first-argument list (every clause, if that argument is unbound). While
more than one candidate remains, each further position whose argument is
bound is consulted, left to right, and the shortest list wins; a tie keeps
the earlier position. A position is never consulted, and so never built,
when an earlier one already leaves at most one candidate. The integrity
rules, which bind a unique id first, therefore build nothing and cost what
they cost today.

**3. Small predicates are left alone.** A predicate with fewer than eight
clauses is indexed on its first argument only: scanning a handful of
clauses is cheaper than hashing. The number is a tuning constant with no
effect on answers.

**4. What counts as bound is unchanged.** The key of a call's argument is
computed exactly as CLM-0021 computes the first one: an unbound variable,
numeric or not, has no key, since a determined numeric variable is still a
variable cell and may unify with a number.

**Soundness.** Each list is a superset of the clauses that can unify with
the call. A clause left out of the list for position *i* has, at *i*, a
constant, number or compound whose key differs from the call's; a
constant, a number and a compound never unify with one another, and two of
the same sort unify only if their keys agree. So the clause could not
have unified whatever the other arguments hold, and leaving it out removes
no answer. Every list is in program order, so the clauses that remain are
tried in the order the program wrote them, whichever list is chosen:
answers, their order and their proof trees are those of the unindexed
machine. The `answer-soundness` invariant is untouched; no store operation
changes.

**Resolution steps are unchanged.** A step is counted per goal reduced
(CLM-0029), not per clause tried, so `--steps` budgets and the step-limit
error behave as before.

**5. The explanation of a failed call names the argument.** `NoClause`
records which position's index gave the candidates.

- When no clause can match and the first argument decided it, the text is
  unchanged: `none of N clauses can match the first argument`, and
  `the clause cannot match the first argument`.
- When another position decided it, the text names it by number:
  `none of N clauses can match argument 2`.
- When candidates remain and none unifies, the text is unchanged
  (`none of N candidates unifies`); `N` may now be smaller than on 0.6.0
  for a call bound beyond its first argument.

**6. The JSON `failure` object gains `argument`.** Beside `clauses` and
`candidates`, a `no-clause` reason carries `argument`: the 1-based position
whose index admitted the candidates, present whenever an index narrowed
them and absent when every clause was a candidate. `candidates` is
redefined in `docs/reference/json-output.md` from "the number the
first-argument index admitted" to "the number the indexes admitted". The
field is additive; every existing field keeps its name and place.

**Lifetime.** An index lives in the compiled `Program` and is shared by
every store the program runs, tabled evaluations included. The REPL
recompiles the program whenever the session changes, which discards the
built indexes with it; they are rebuilt on demand. The program is
single-threaded (it shares its symbol table through `Rc<RefCell<…>>`), so
a `OnceCell` per slot is enough.

**Alternatives weighed.**

- *Index every argument at compile time.* The simplest form, and no
  first-call cost. But it pays memory and load time for every position of
  every large predicate, most of which no rule ever binds; on the worst
  case above that is about 130 MB per position per 500 000 facts.
- *A directive* (`:- index n/2, 2.`). Explicit and cheap, but it puts a
  performance concern into the language and into every program, and a
  missing directive fails silently as a slow query. Building on first use
  needs no surface.
- *Take the first bound position rather than the shortest list.* One
  lookup instead of several, but `p(common, rare)` then scans the long
  list. Consulting each bound position costs one hash lookup apiece and
  stops as soon as one candidate remains.
- *Intersect the lists of all bound positions.* Tighter still, but it
  allocates per call and breaks the choice point's borrowed slice of
  positions; the shortest list is a superset the head unification prunes
  at once.
- *Composite keys over several arguments.* Needs a key per bound
  combination; not earned by any measured query.
- *Keep stating relations twice.* The status quo measured in the context.

## Consequences

- A join through any argument of a large predicate is linear; rules read a
  relation in either direction without a second copy of it.
- Calls bound on their first argument, and all small predicates, run
  exactly as on 0.6.0: same candidates, same time, no index built.
- Memory grows only for positions a query actually binds: at worst about
  260 bytes per clause for a position with a distinct key in every clause,
  far less where keys repeat. A compact form for single-clause lists would
  cut the worst case and is left to a later task if a program shows the
  need.
- The first call that binds a new position pays one pass over the
  predicate's clauses.
- Programs print the same answers. A failed call's explanation can show a
  smaller candidate count, and a new sentence when a later argument rules
  every clause out; `--json` gains the `argument` field. Every existing
  golden file is byte-identical in the spike, which implements points 1
  to 4 and not yet the wording of points 5 and 6.
- Not addressed: lookups by range (`{ N > 400 }` before a call), and keys
  deeper than the principal functor (`f(a)` and `f(b)` share a key, as for
  the first argument).
- `components/evaluator.md` changes where it describes the index and the
  lookup, and drops "indexing on other arguments" from *Not yet*.
- The implementation task lands the spike with tests for clause order and
  answers across positions, for laziness, for the threshold, and for the
  explanation and JSON forms; and adds the join to `tests/benchmarks.rs`
  and `docs/reference/benchmarks.md`.
