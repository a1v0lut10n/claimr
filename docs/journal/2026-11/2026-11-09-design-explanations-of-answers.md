---
date: 2026-11-09
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-09-explanations-of-answers.md
meta-impact:
  note: "Under --explain, each answer object in the --json output gains an optional `explanation` field (a proof tree); documents produced without the flag are unchanged. aicogito's reasoning adapter reads the documents through generic field lookups and ignores unknown fields, so nothing breaks; it may later read the tree. The suggestion from CLM-0015 stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Design record accepted: explanations of answers

- **When:** 2026-11-09 19:52:02

## Context

Using claimr as an integrity checker produced the incubation note of
2026-11-08: a violation found by a rule over fact tables is reported as a
binding, `C = c17`, without the rule that fired or the facts that fired
it, and a finding is only actionable if it can be cited. After CLM-0022
the note was the first of the three remaining ideas to take, because an
explanation changes no semantics: it reports the derivation an answer
already has. The record was drafted against what exists — a source
position per clause that only the two positioned load errors use, a
machine that records nothing about which clause resolved which goal, and
a `--json` contract whose consumer reads documents through generic field
lookups — and landed as PR #58. It is accepted here. The incubation note
is removed in this change: a graduation moves the content, it does not
copy it.

## Details

**Decision.** An explanation is the proof tree of an answer, recorded by
the machine on request. Each node is a goal of the derivation as it
stands at the answer, rendered with the answer's bindings and variable
names, with what resolved it: a clause cited by source position and text
as written, with its body goals as children; a constraint goal as the
instance that was posted; a built-in; an if-then-else whose condition
succeeded, with the condition's proof and the then-branch's proofs
marked committed; a condition or negated goal that had no solution, as a
leaf stating so. Disjunction is transparent. The query's goals are the
roots. Failure has no proof tree in this record.

**Opt-in, with the reason stated.** `--explain` for a batch run, human
and `--json` output alike; `:explain` to toggle in the REPL;
`Solutions::with_explanations()` in the library. Off, nothing is
recorded. On, one node per successful resolution step stays alive until
backtracking discards it, since the machine reclaims nothing on
deterministic exit, so memory grows with the derivation's length.

**Cited by position and text.** Every compiled clause keeps its source
position and its text: position is what a caller joins on, text is what a
person reads. Both are bounded by the program's size, which claimr
already treats as small.

**Output.** Human: the tree indented two spaces per level under the
answer line, one node per line, pinned by a golden example. JSON: an
`explanation` field per answer object, present only under the flag, so a
document produced without it is byte-identical to today's. The field is
documented in `json-output.md`; the impact is the `meta-impact` note
above.

**Settled on principle.** Explanations cite the constraints the derivation
posted, never the projected ones: the printed constraints are derived
from the posted ones, not the reverse. A meta-interpreter
(`solve(Goal, Proof)` over clause reflection), the Prolog road to
explanations, is rejected: it needs clause reflection and call-by-term in
the language, which claimr does not have and should not grow for this.

**Deferred, named.** Why a query failed. A failure tree is the whole
search space; for a constraint failure the useful object is a minimal
infeasible subset of the posted constraints, which the simplex can
produce exactly once each asserted bound carries its origin (Dutertre and
de Moura's explanation of a conflicting bound set). That is the next
record the integrity checker will want. Also out: a reified proof term in
the language.

**Pre-write checks.** A design record lists no `affects` and closes no
task, so nothing to resolve. `answer-soundness` is unchanged by the
decision alone; the record commits the implementing task to a new
invariant, that an explanation cites only steps on the answer's own
derivation, with a test that re-running the cited facts and rules
reproduces the answer. Cross-repo impact is the `meta-impact` note: an
additive, flag-gated field in the JSON output, read by aicogito's adapter
as unknown and ignored.

## Links

- Design record: `docs/design/2026-11-09-explanations-of-answers.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/58
- Removed incubation note: `docs/incubation/2026-11-08-explanations-of-answers.md`
- Previous flag of the same contract gap:
  `docs/journal/2026-11/2026-11-09-execution-clm-0015-list-length.md`
- Implementing task: to follow as CLM-0023
