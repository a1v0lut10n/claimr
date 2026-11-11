---
date: 2026-11-11
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-11-clm-0030-conflict-sets.md
design:
  - docs/design/2026-11-11-explanations-of-failure.md
meta-impact:
  note: "The load error for unsatisfiable constraint facts now continues after `the program's constraint facts are unsatisfiable (no models)` with `; a minimal conflicting set:` and one line per fact (`  { age(bob) >= 18 }.  fact 3:1`); the `--json` stderr error document keeps its shape and carries the lines in `message`. A library consumer sees `EvalError::InitialStoreUnsatisfiable` gain a `conflict` field. No query output changes. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0030 executed: step records, the replay minimiser and the load-time conflict set

- **When:** 2026-11-11 18:32:48

## Context

The explanations-of-failure record (accepted this afternoon, PR #78)
splits its implementation in two: first the machinery that finds a
minimal conflict set and the one place it is visible today, then the
failure tree under `--explain`. This is the first half. The record's
central choice — replay as the oracle, rather than origins traced through
the simplex — had to be shown to work before the tree is built on it.

## Details

**What was built.** `src/eval/explain.rs`: a derivation's steps as
closed, replayable records (`Step`: a clause head unified with its call,
a constraint goal, a built-in, a table answer, a collected list, a
constraint fact), where *closed* means a template whose variables are the
heap addresses of the frame variables they stood for, so steps taken
under different clause frames agree on which variable is which. The
oracle, `satisfiable`, posts a step list into a fresh store over the
program's symbol table and finalizes it, the exact check an answer
passes; a store error or a non-linear residue is an `Err`, never a
verdict. The minimiser, `minimal_conflict`, is QuickXplain over the steps
before a failing one in their order, returning a minimal subset that is
unsatisfiable with the failing step and satisfiable with any member
removed; deterministic when several exist, the earlier steps preferred.
The proof arena keeps what closing needs — a clause's frame and the
call's source as a `CallRef`, a constraint goal's and a built-in's
template and frame — threaded through the clause, tabling and evaluation
paths, at the cost of one `Rc` clone per record; `derivation_steps`
closes the live arena. Load time: `Program.initial` keeps each
constraint fact's position and text, and the load error names a minimal
conflicting set of facts, in human and JSON mode alike.

**Verified.** 197 tests pass, six new in `conflict_sets`: an answer's
steps across a fact, heads, a tabled call, a comprehension, a built-in
and a constraint replay satisfiable; a conflict set is unsatisfiable
whole and satisfiable without any member, and is head plus constraint
for `eligible(bob)` against `{ age(bob) < 18 }`; a world fact is cited
when it is the cause; a head clash is explained by the failing step
alone; the set among several minimal ones is deterministic; the load
error cites exactly the conflicting facts with positions. Golden files
byte-identical; fmt, clippy and lint clean. `answer-soundness` gains
invariant 9, the conflict-set part; CLM-0031 adds the failure tree's.

**Findings.** Outside tests nothing asks for a derivation's steps until
CLM-0031, so the new machinery is dead code to rustc; one
`#[cfg_attr(not(test), expect(dead_code, reason = …))]` on
`derivation_steps` covers it, because an item with an expected dead-code
lint counts as a live root and everything it reaches is live — markers on
the reached items were unfulfilled and had to go. `expect` rather than
`allow`, so CLM-0031 is forced to remove it. `abstract` is a reserved
word. A subset replay re-reads `=` and `!=` by what the operands denote
when posted (CLM-0026), so the subset's system is claimr's operational
reading of those steps in that order; the property tested is exactly the
record's. The head step is part of a conflict: without it the body's
constraint is about another unknown, so the record's "three constraints"
will usually read as a head and two constraints.

**Pre-write checks.** The task's three `affects` docs were updated in the
PR: `evaluator.md` (`explain.rs`, the arena's records, the load error),
`answer-soundness.md` (invariant 9), `json-output.md` (the load-error
message). Invariants 1 to 8 are untouched: recording and replaying change
no decision the machine takes. Cross-repo impact: the message change
above, noted.

## Links

- Task: `docs/implementation/2026-11-11-clm-0030-conflict-sets.md`
- Design: `docs/design/2026-11-11-explanations-of-failure.md`
- PR #79 (merge d191d21): commits dc3cc26 (plan), 9263c9f (implementation)
- Next: CLM-0031, the failure tree under `--explain`
