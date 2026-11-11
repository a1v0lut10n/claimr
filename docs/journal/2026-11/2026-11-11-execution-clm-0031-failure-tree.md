---
date: 2026-11-11
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-11-clm-0031-failure-tree.md
design:
  - docs/design/2026-11-11-explanations-of-failure.md
meta-impact:
  note: "Under `--explain`, a `--json` document with `\"outcome\": \"none\"` now carries `failure`: `{\"branches\": [[node, …], …], \"omitted\": N}`, nodes in the shape of `explanation`, the failed node with `\"by\": \"failed\"`, a `reason` (`unsatisfiable`, `no-clause`, `has-solution`, `no-table-answer`, `no-solution`) and, by reason, `conflict` (shallow cited nodes, plus `note` when not minimised), `clauses` and `candidates`, `answers`, or the solution's proof under `subgoals`. Documents without the flag, and documents with answers, are byte-identical to before. aicogito's reasoning adapter can read `failure.branches[*]` for the conflict sets. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0031 executed: the failure tree under `--explain`

- **When:** 2026-11-11 19:39:31

## Context

The second half of the explanations-of-failure record, on the step
records and replay minimiser CLM-0030 landed this afternoon: the failure
tree itself, the part a user and the integrity checker see. The record
fixed the shape — branches in search order, a reason per leaf, a minimal
conflict set for an unsatisfiable step, a budget of 64, the `failure`
field under the flag — and the machine had every site a derivation can
fail at; the work was to turn each site into a leaf without changing a
decision the search takes.

## Details

**What was built.** `Solutions::fail` is called at every failure site
outside a sub-search or a table evaluation: a call no candidate resolved
(from position 0 — a retry running out is the end of alternatives, not a
new derivation, so head failures aggregate into one `no-clause` leaf with
the clause and candidate counts), a constraint goal or built-in the store
rejected (`unsatisfiable`, with the conflict set `minimal_conflict` finds
over `derivation_steps`, which now returns each step's origin — a fact or
an arena record — for citation), the answer-time `finalize` (the step
`answer`), a negation whose condition succeeded (`has-solution`, carrying
the arena slice since the construct opened as the solution's proof), a
tabled call with no unifying answer in its complete table
(`no-table-answer`), and an if-then-else without an else whose condition
failed (`no-solution`), observable only through `Retry::FailNote`, a
marker choice point pushed under the condition when explaining, dropped by
the commit, recorded when backtracking pops it, and ignored by
`may_continue`. `fail` gates on recording and on the budget, appends a
`Failed` record under the failing goal's parent, renders the whole arena
with the branch's bindings and truncates the record away; citations are
shallow copies of the cited records' nodes or a constraint fact's text. An
answer discards the branches so far. `ProofNode` gains `failure`,
`Resolution::Failed`, `Reason` and `FailureTree` with `lines()`;
`Solutions::failure()` returns the tree after a search that ended without
an answer, error or interrupt; the CLI, the REPL and the JSON writer print
it. When a subset replay cannot be decided (a non-linear residue), every
step is cited and the leaf says `not minimised` rather than claim a set
it has not verified.

**Verified.** 210 tests pass: eleven new in `failure_trees` — the record's
example line for line, headed branches each citing their own conflict,
the no-clause counts, a negation carrying its proof, failures inside a
condition not branches, a tabled call with no answer, an if-then-else
without else with `may_continue` unchanged by the marker, a world fact
cited, the budget truncating and counting, branches discarded on an
answer, no tree after a stop — the JSON `failure` field and byte-identity
without the flag, the REPL under `:explain`, and the new golden run
`examples/failure.claimr`. Existing golden files byte-identical; fmt,
clippy and lint clean. Invariant 9 of `answer-soundness` is completed
with the failure tree's part.

**Findings.** The record listed four reasons; an if-then-else without an
else whose condition fails is a fifth case, so `no-solution` joins the
vocabulary, pinned in `json-output.md`. A constraint goal whose
arithmetic was over variables still unbound when it failed renders as the
lowered numeric variable (`{ _1 = 0 }`), as an answer's proof tree does
today; rendering the template instead is a refinement for both. No program
was found that fails only at the answer-time check: the incremental
determination pass of CLM-0024 rejects every unsatisfiable store met at a
post, so the `answer` leaf is in place but untested. A negation's and a
condition's leaves show the construct as written, with clause variables
under their source names, as the `committed` and `no-solution` nodes of
CLM-0023 do.

**Pre-write checks.** The task's four `affects` docs were updated in the
PR: `evaluator.md` (the failure sites, the budget, the marker),
`answer-soundness.md` (invariant 9 completed), `json-output.md` (the
`failure` section), the architecture README (`cli` row). Invariants 1 to
8 untouched: recording changes no decision; the marker choice point
resumes nothing and `may_continue` ignores it, which a test checks.
Cross-repo impact: the `failure` field, noted above.

## Links

- Task: `docs/implementation/2026-11-11-clm-0031-failure-tree.md`
- Design: `docs/design/2026-11-11-explanations-of-failure.md`
- PR #81: commits bcfbf25 (plan), 798ffee (implementation)
- First half: `2026-11-11-execution-clm-0030-conflict-sets.md`
- Golden example: `examples/failure.claimr`
