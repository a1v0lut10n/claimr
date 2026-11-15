---
date: 2026-11-15
type: journal
components: [evaluator]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-15-indexing-on-other-arguments.md
meta-impact:
  note: "Accepted: under `--json`, the `failure` object's `no-clause` reason will gain `argument`, the 1-based position of the argument whose index admitted the candidates, present whenever an index narrowed them (the first argument included) and absent when every clause was a candidate. `candidates` keeps its name and place and now counts what the indexes admitted, so it can be smaller than on 0.6.0 for a call bound beyond its first argument. Answers, proof trees, step counts and every other field are unchanged. A consumer that states a relation twice to join through a second argument can drop the second copy once CLM-0035 is released."
---

# Design record accepted: indexing on arguments other than the first

- **When:** 2026-11-15 02:10:01

## Context

CLM-0021 indexed each predicate on its first argument and left "indexing
on other arguments" under *Not yet*. A re-run of the integrity-rule
experiment on 0.6.0 confirmed that rules binding an id first are linear
(1.7 million facts in ten seconds), and measured what remains: a join
through a second argument is still quadratic, 0.54 s, 2.18 s and 8.34 s at
1 820, 3 640 and 7 280 facts. That is the shape of every rule that reads
a relation against its direction, which is what rules over typed edges do.
Stating each relation twice removes the cost in time and moves it to
storage: at 500 000 facts, double the load time and 431 MB more peak
memory. The record landed as PR #89 together with the plan, and is
accepted here.

## Details

**Decision.** A predicate keeps an index per argument position after the
first, of the same kind as the first-argument index and in program order,
built the first time a call needs it. A call starts from the
first-argument list and, while more than one candidate remains, consults
each further bound position left to right and keeps the shortest list. A
predicate with fewer than eight clauses is indexed on its first argument
only. What counts as a bound argument is what CLM-0021 already decided.
A failed call's explanation names the argument that ruled the clauses
out: the two existing sentences keep their text for the first argument
and read `argument N` otherwise.

**Why it is sound.** Each list is a superset of the clauses that can
unify with the call, and every list is in program order, so whichever
list is chosen the remaining clauses are tried in the order the program
wrote them. Answers, their order and their proof trees are those of the
unindexed machine. A resolution step is counted per goal reduced, not per
clause tried, so step budgets are unaffected.

**The spike.** A scratch branch, not merged, implements the index and the
lookup in 91 added lines: the join runs in 0.01 s, 0.03 s and 0.05 s;
calls bound on the first argument are unchanged (2.52 s against 2.50 s
for four integrity rules over 418 912 facts); all 160 unit tests pass and
every golden file is byte-identical. The worst case for memory, a
distinct key in every clause, measured about 260 bytes per clause: 129 MB
at 500 000 facts, against the 431 MB of the second copy.

**The JSON field, decided at acceptance.** The record proposed that
`argument` appear whenever an index narrowed the candidates. The
alternative weighed at acceptance was to emit it only for a later
argument, which would leave every existing `--json` failure document
byte-identical. The owner chose the record's form: it is more
informative, and a consumer never has to infer which index applied. The
field is additive, and existing first-argument failure documents gain it.

**Rejected.** Indexing every argument at compile time (pays for positions
no rule binds); an `:- index` directive (a performance concern in the
language, failing silently as a slow query when absent); taking the first
bound position rather than the shortest list; intersecting the lists
(allocates per call); composite keys; keeping the second copy.
**Not addressed.** Lookups by range, and keys deeper than the principal
functor.

**Pre-write checks.** A design record lists no `affects` and closes no
task. The `answer-soundness` invariant holds by the superset argument
above; no store operation changes. Cross-repo impact: the `argument`
field and the redefined `candidates`, noted above; no listed contract
covers claimr's JSON output, so the note carries it.

## Links

- Design record: `docs/design/2026-11-15-indexing-on-other-arguments.md`
- Record and plan PR: https://github.com/a1v0lut10n/claimr/pull/89
- Where it was deferred: `docs/implementation/2026-11-09-clm-0021-first-argument-indexing.md`;
  `docs/architecture/components/evaluator.md`, *Not yet*
- Implementing task: `docs/implementation/2026-11-15-clm-0035-indexing-on-other-arguments.md` (CLM-0035)
