---
date: 2026-11-11
type: journal
components: [evaluator, solver, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-11-explanations-of-failure.md
meta-impact:
  note: "Accepted: under `--explain`, a `--json` document with `\"outcome\": \"none\"` will gain a `failure` field (`{\"branches\": [[node, …], …], \"omitted\": N}`, nodes in the shape of `explanation`, the failed node with `\"by\": \"failed\"`, a `reason` and, by reason, `conflict`, `candidates` or `subgoals`); documents without the flag stay byte-identical, and the load error for unsatisfiable constraint facts carries the minimal conflicting set in its message, shape unchanged. aicogito's reasoning adapter can read `failure.branches[*]` for the conflict sets once the implementing tasks land. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Design record accepted: explanations of failure

- **When:** 2026-11-11 17:38:27

## Context

The explanations record (CLM-0023) gave an answer its proof tree and set
failure aside in one sentence: a failure tree is the whole search space,
and for a constraint failure the useful object is a minimal infeasible
subset of the posted constraints, "the one the integrity checker will
want next". With 0.4.0 published and every accepted record implemented,
it was the strongest remaining item, and the one with a named consumer:
a finding of the form "this claim cannot be satisfied because of these
three constraints" is what an integrity checker routes. The record
landed as PR #77 and is accepted here.

## Details

**Decision.** A failed step is explained by a minimal unsatisfiable
subset of the earlier steps of its derivation — clause heads, posted
constraints, built-ins, table answers, collected lists and the program's
constraint facts, cited as the proof tree cites them — found by
QuickXplain with the store as the exact oracle: the steps are posted
into a fresh store over the program's symbol table and finalized, and the
verdict is the check. The failing step is in every such set, because the
steps before it were satisfiable together. Under `--explain`, `false` is
followed by the failure tree: the failed derivations as branches, each
ending at the goal that failed with its reason (unsatisfiable, with the
conflict set; no clause, with candidate counts, head failures aggregated
into the call's leaf; has a solution, for a negation, carrying the
proof; no table answer). At most 64 failed branches are kept, the first
in search order, and the remainder is counted and stated. Unsatisfiable
constraint facts are explained always, in the load error. The JSON
outcome-none document gains a `failure` field under the flag.

**Why replay and not the solver's conflict row.** Dutertre and de
Moura's Γ was the expected mechanism, and it was weighed: the bounds it
names are not the posted constraints a reader needs cited, and the
store folds determined values into constants (CLM-0024), wakes `dif` and
type relations from bindings, and derives lengths and sums through paths
of their own, so an origin would have to be threaded through every one
of them, and a missed origin would print a set that is not infeasible.
A replayed set is unsatisfiable and minimal by the very check that
reports it, which the aspect can state as an invariant and a test can
verify exactly. Γ stays available as a seed for the candidate set if
the replays prove costly over a large constraint-fact world. Also
rejected: the "deepest failure" heuristic of Prolog debuggers (not a
proof), algorithmic debugging (interactive), one branch per failing fact
(a fact table is one leaf). Deferred: missing answers among found ones,
the failure trees of sub-searches and table evaluations, `why_not/1`,
Farkas certificates as output.

**Pre-write checks.** A design record lists no `affects` and closes no
task. `answer-soundness` gains an invariant when the tasks land: a
failure explanation cites only derivations the search tried; every
conflict set is unsatisfiable alone and satisfiable with any member
removed, decided exactly; truncation is stated. Cross-repo impact: the
`failure` field under the flag, noted above.

## Links

- Design record: `docs/design/2026-11-11-explanations-of-failure.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/77
- Where it was deferred: `docs/design/2026-11-09-explanations-of-answers.md`, item 7
- Implementing tasks: two, to follow as CLM-0030 and CLM-0031
