---
date: 2026-11-11
type: design
status: accepted
components: [evaluator, solver, cli]
aspects: [answer-soundness]
tags: [explanations, failure, minimal-conflict, unsat-core, provenance, json-output, integrity-checker]
---

# Explanations of failure: a failure tree with minimal conflict sets

## Context

CLM-0023 (`2026-11-09-explanations-of-answers.md`) gave an answer its
proof tree: the clauses that resolved each goal and the constraint
instances posted, cited by position and text. It left failure out, in
one sentence that this record now takes up: "a failure tree is the whole
search space, and for a constraint failure the useful object is a
minimal infeasible subset of the posted constraints … the one the
integrity checker will want next ('this claim cannot be satisfied
because of these three constraints')."

**What a failure is today.** `false` for a query with no answer; under
`--json`, `"outcome": "none"` with an empty answer list; under
`--explain`, exactly the same. A program whose constraint facts are
unsatisfiable does not load: `EvalError::InitialStoreUnsatisfiable`,
"the program's constraint facts are unsatisfiable (no models)", with no
indication of which facts. Nothing in either case says where the search
died or which constraints collided.

**Where failure happens in the machine.** A derivation is a sequence of
*steps*, each of which changes the store and each of which may fail:
a clause head unified with the call (with the head's arithmetic
definitions posted), a constraint goal `{ … }` posted, a built-in run, a
tabled call unified with a table answer, a comprehension's list unified
with its target, and at the end the exact `finalize` that decides every
pending disequation and numeric variable before an answer is printed.
Every step is transactional (invariant 1 of `answer-soundness`): on
failure the store is restored and the machine backtracks. A call may
also fail before any step, when no clause can match it: the predicate is
undefined, or first-argument indexing (CLM-0021) leaves no candidate.
The solver detects numeric infeasibility either at a bound assertion
(`l > u` on one variable) or in `check()` when a violated row has no
pivot; Dutertre and de Moura read a conflicting bound set Γ off that
row. The store, however, folds determined values into constants before
they reach the simplex (CLM-0024), wakes `dif` and type relations from
bindings, and derives list lengths and sums through their own paths, so
the bounds Γ names are not the posted constraints a person needs cited,
and the chain from one to the other runs through every store path.

**What exists to build on.** The proof arena of CLM-0023: when
explaining, every successful step appends a node under its parent goal,
choice points remember the arena length, and backtracking truncates to
it. Each node knows the clause (position, text) and the goal instance.
A fresh store can be built over the program's symbol table
(`Store::with_symbols`, CLM-0028) and the clause templates can be
instantiated into any store with `build_with` and a variable frame.

**Prior art.** Prolog systems explain failure by the debugger's trace
or by algorithmic debugging (Shapiro 1983), an interactive dialogue with
an oracle — neither is a machine-readable object a caller can route.
The constraint community has two exact objects: an irreducible
infeasible subsystem (IIS; Chinneck and Dravnieks 1991, found by a
deletion filter) and a preferred minimal conflict (Junker's QuickXplain,
2004, which finds one in O(c · log(k/c)) consistency checks for a
conflict of size c among k constraints). Both need only a satisfiability
oracle, which claimr has exactly.

## Decision

1. **A conflict set is a minimal unsatisfiable subset of derivation
   steps, found by replay.** The unit of citation is the *step*, as the
   proof tree already cites it: a clause whose head unified with its
   call (cited as `fact 118:1` / `rule 42:1` with its text), a
   constraint goal as posted (`{ age(bob) >= 18 }  constraint 2:1`), a
   built-in, a table answer, a collected list — and a constraint fact of
   the program, cited by its position. A failed step `s` is explained by
   a set `C` of earlier steps such that `C ∪ {s}` has no solution and
   every proper subset of `C ∪ {s}` has one, decided by posting the
   steps' constraints into a fresh store over the program's symbol
   table and finalizing it — the same exact check as an answer. The
   failing step is in every such set, since the steps before it were
   satisfiable together; `C` is found by QuickXplain over the earlier
   steps in derivation order, constraint facts first, so the set
   reported is deterministic when several minimal sets exist. The
   solver's Γ is **not** the mechanism: tracing origins through
   determination folding, wake-ups, lengths and sums would make every
   store path a soundness risk, whereas a replayed set is unsatisfiable
   and minimal *by the check that reports it*. If the replays prove
   costly, Γ may later seed the candidate set; the output does not
   change.

2. **Under `--explain`, `false` is followed by the failure tree.** The
   query's failed derivations, in search order, each a *branch*: the
   steps taken, nested under their rules as in a proof tree, ending at
   the goal that failed with its reason:
   - **unsatisfiable**: a step whose store effect had no solution — the
     failed goal is listed with its conflict set from decision 1; the
     answer-time `finalize` is such a step and is listed as `answer`;
   - **no clause**: a call no candidate clause could resolve — the
     predicate is undefined, indexing excluded every clause, or every
     candidate failed at its head; the leaf gives the counts (`none of
     3 candidates unifies`). Head failures are *not* branches: a branch
     is a clause whose head unified and whose body then failed, so a
     call against a fact table is one leaf, not one branch per fact;
   - **has a solution**: a negation `\+ G` whose `G` succeeded — the
     leaf carries `G`'s proof tree, which the arena already holds;
   - **no table answer**: a tabled call whose complete table has no
     matching answer; the table's own computation is not explained
     (decision 6).
   An if-then-else whose condition failed continues with its else
   branch inside the same derivation, as today; a committed condition's
   steps are steps of the branch.

3. **A budget bounds the tree; truncation is stated.** The machine
   records at most **64 failed branches** per query, the first in
   search order; beyond that it only counts, and the explanation ends
   with `… and N more failed branches`. Within the budget the failure
   tree is a proof of finite failure: every branch the search tried is
   present with the step at which it died. The budget is a named
   constant documented in `evaluator.md`; it becomes a flag if practice
   asks for one. A conflict set is computed only for a branch that is
   kept, at the moment it fails and before the store is undone, so its
   instances render with the branch's bindings. Failed branches are
   discarded when the query yields an answer: this record explains
   `false`, not the answers that were not found (decision 6).

4. **Unsatisfiable constraint facts are explained always.** Loading a
   program whose `{ … }.` facts have no model fails as today, and the
   error names a minimal conflicting set of the facts, each by position
   and text, found as in decision 1 with the facts as the steps. No
   flag: it is an error message, and a minimal set is small by
   construction. The `--json` error document on stderr keeps its shape;
   the set is in the message.

5. **Output.** Human mode, pinned by a golden example:

   ```
   ?- eligible(bob), { age(bob) < 18 }.
   false
     eligible(bob)  rule 2:1  eligible(X) :- { age(X) >= 18 }.
       { age(bob) >= 18 }  constraint 2:1
     { age(bob) < 18 }  failed: unsatisfiable with
       eligible(bob)  rule 2:1
       { age(bob) >= 18 }  constraint 2:1
   ?- k(c99, K).
   false
     k(c99, K)  failed: no clause (none of 1820 candidates unifies)
   ```

   A second and later branch is introduced by a line `branch N` at
   the first indent; a single branch has none. Under `--json`, an
   `"outcome": "none"` document gains a `failure` field,
   `{"branches": [[node, …], …], "omitted": N}`, each branch a list of
   root nodes in the shape of `explanation`; the failed node has
   `"by": "failed"`, a `"reason"` of `unsatisfiable`, `no-clause`,
   `has-solution` or `no-table-answer`, and, by reason, `"conflict"`
   (cited nodes), `"candidates"` (a count) or `"subgoals"` (the
   solution's proof). Without `--explain` the document is byte-identical
   to today's. The REPL's `:explain` covers it as it covers answers.

6. **Not in this record.** Why an answer that was expected is missing
   among answers that were found: the failure tree of a query with
   answers is not kept; narrow the query until it fails. The failure
   tree of a condition that failed, of a comprehension body, or of a
   tabled predicate's own evaluation: each is a sub-search with a tree
   of its own, under the same budget, and is a refinement once this
   record's output has been used. A reified `why_not/1` in the
   language, as the answers record declined. A Farkas certificate (the
   rational multipliers that sum the conflict to `0 < 0`) as an output:
   the minimal set is what a reader and a caller act on; the certificate
   is available from the same simplex if a verifier ever wants it.

## Consequences

- **Soundness gains a checkable property.** `answer-soundness` gains an
  invariant: a failure explanation cites only derivations the search
  tried and steps it took; every conflict set, posted alone into a fresh
  store, has no solution, and removing any one of its members leaves a
  system that has one, both decided exactly; a truncated tree says so.
  Recording and explaining change no decision the machine takes. *Tests:*
  the conflict sets of a golden run re-posted with each member removed in
  turn (satisfiable) and whole (unsatisfiable); a fact-table failure
  counts one leaf with the right candidate count; a `\+` leaf carries
  the solution's proof; the budget truncates with the right remainder.

- **Evaluator.** The proof arena keeps, when explaining, what replay
  needs per node: the clause or goal template and its variable frame,
  resolved against the derivation's heap at failure time into a closed
  record of the step. On a step's failure with budget left, the machine
  builds the branch from the arena path, runs QuickXplain over the
  earlier steps with a replay store, and stores the branch before
  undoing; `backtrack` counts the rest. Failed branches live in the
  arena beside the live derivation until an answer or the end of the
  query. `finalize` failing at an answer is a step like the others.
  `Program::compile` runs the same minimisation over the constraint
  facts when the world store is unsatisfiable, and `TClause`-style
  position and text are kept for constraint facts too.

- **Cost.** Off, nothing. On, failure recording costs what answers cost
  plus the replays: for a branch of k steps and a conflict of size c,
  O(c · log(k/c)) replays of at most k posts each, for at most 64
  branches per query. A query over a large constraint-fact world pays k
  in the facts; that is the case where Γ-seeding would be reached for.

- **CLI and docs.** `--explain` and `:explain` need no new switch.
  `json-output.md` documents the `failure` field and the load error's
  message; `evaluator.md` the arena's failure records and the budget;
  `answer-soundness.md` the invariant; the README's explanations bullet
  gains the failure case; `examples/failure.claimr` with its `.args` and
  golden `.answers`. The explanations record is not amended
  (append-mostly); this record is the second it announced.

- **Contract impact.** The `failure` field is an addition under a flag,
  so no consumer sees a change it did not ask for; aicogito's reasoning
  adapter can read `failure.branches[*]` for the conflict sets. The
  acceptance entry carries a `meta-impact` note.

- **Two tasks.** The first lands the step records, the replay
  minimiser and the load-time explanation, with its exactness tests —
  no output change for queries yet. The second lands the failure tree:
  recording, the budget, both renderers, the golden example and the
  docs.

## Sources

- `docs/design/2026-11-09-explanations-of-answers.md`, item 7, the
  deferral this record takes up; its incubation note of 2026-11-08
  (the integrity-checker experiment).
- B. Dutertre, L. de Moura, *A Fast Linear-Arithmetic Solver for
  DPLL(T)*, CAV 2006: the conflicting bound set Γ, considered as the
  mechanism and kept as a possible seed.
- U. Junker, *QuickXplain: Preferred Explanations and Relaxations for
  Over-Constrained Problems*, AAAI 2004: the divide-and-conquer minimal
  conflict with a consistency oracle.
- J. W. Chinneck, E. W. Dravnieks, *Locating Minimal Infeasible
  Constraint Sets in Linear Programs*, ORSA Journal on Computing 3(2),
  1991: irreducible infeasible subsystems and the deletion filter.
- E. Y. Shapiro, *Algorithmic Program Debugging*, MIT Press 1983: the
  interactive alternative, not taken.
- `src/eval/machine.rs` (the proof arena, `try_clauses`, `backtrack`),
  `src/eval/store.rs` (`post_*`, `finalize`), `src/solver/simplex.rs`
  (`check`), `src/eval/compile.rs` (`post_initial`).
