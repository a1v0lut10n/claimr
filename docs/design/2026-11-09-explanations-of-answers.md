---
date: 2026-11-09
type: design
status: accepted
components: [evaluator, cli]
aspects: [answer-soundness]
tags: [explanations, proof-tree, provenance, json-output, integrity-checker]
---

# Explanations of answers

## Context

An answer is the projected store: the bindings and residual constraints
under which the query holds. It says *what* holds and never *why*. The
incubation note of 2026-11-08, written after using claimr as an integrity
checker over a knowledge base, names the cost: a violation found by

```
violation(C) :- k(C, K), n(C, N), { K > N }.
```

is reported as `C = c17`, without the rule that fired or the two facts
that combined to fire it. A finding is only actionable if it can be cited
— which rule, which records — so that a caller can route it to whoever
owns those records or show a person the evidence. The same holds for any
program whose answers are decisions about data rather than computed
values.

What exists today: the parser keeps a source position per clause
(`parse_program_spanned`), which the compile step uses only for its two
positioned load errors; the machine does structure copying and undoes
everything on backtracking, so nothing records which clause resolved which
goal; the `--json` output (`docs/reference/json-output.md`) is consumed by
aicogito's reasoning adapter through generic field lookups on each
document, picking `outcome`, `answers` and the three string lists of an
answer, so an added field it does not know is ignored.

Prolog III's answers are stores too, and the manual offers no provenance
beyond the debugger's trace. Prolog systems obtain explanations through a
meta-interpreter (`solve(Goal, Proof)`) over `clause/2`; that road needs
clause reflection and call-by-term in the language, which claimr does not
have and this record does not want to add for this purpose.

## Decision

1. **An explanation is the proof tree of an answer**, recorded by the
   machine, opt-in. Each node is one goal of the derivation as it stands
   at the answer, rendered with the answer's bindings and variable names,
   together with what resolved it:
   - a **clause**: fact or rule, cited by its source position
     (`line:column`) and its text as written; its body goals are the
     node's children;
   - a **constraint goal** `{ … }`, a leaf, rendered as the instance that
     was posted (`{ 5 > 3 }`), cited by the clause it came from;
   - a **built-in** (`atom_chars/2`, `number/1`, `atom/1`, `list/1`), a
     leaf;
   - an **if-then-else** whose condition succeeded: the condition's proof
     and the then-branch's proofs as children, marked *committed*; one
     whose condition failed: a leaf stating that the condition had no
     solution, followed by the else-branch's proofs;
   - a **negation** `\+ G`: a leaf stating that `G` had no solution;
   - a **disjunction**: transparent; the alternative taken appears as its
     own goals.
   The query's goals are the roots. Failure has no proof tree in this
   record: the leaves for a failed condition say *that* it failed, not
   why (see Consequences for the later stage).

2. **Recording is off unless asked for.** `claimr --explain file` turns
   it on for a batch run (human and `--json` output alike), the REPL gets
   `:explain` to toggle it, and the library exposes it as
   `Solutions::with_explanations()`. Off, the machine records nothing and
   pays nothing. On, one node per successful resolution step is kept
   until backtracking discards it: a derivation's whole proof stays alive
   to the answer, as there is no reclamation on deterministic exit, so
   memory grows with the derivation's length. That is the reason for the
   switch.

3. **Cited by position and text.** Every compiled clause keeps its source
   position and its text as written. Position is what a caller joins on
   (an editor, the integrity checker's record ids in comments); text is
   what a person reads. Both are bounded by the size of the program, which
   claimr already treats as small (the REPL recompiles on every change).

4. **Human output**: after the answer line, the tree indented two spaces
   per level, one node per line, in derivation order:

   ```
   ?- violation(C).
   C = c17
     violation(c17)  rule 42:1  violation(C) :- k(C, K), n(C, N), { K > N }.
       k(c17, 5)  fact 118:1
       n(c17, 3)  fact 907:1
       { 5 > 3 }  constraint 42:1
   ```

   A fact's text is its goal and is not repeated. The format is pinned by
   a golden example.

5. **JSON output**: under `--explain`, each answer object gains an
   `explanation` field, a list of root nodes
   `{"goal": "k(c17, 5)", "by": "fact", "line": 118, "column": 1,
   "clause": "k(c17, 5).", "subgoals": []}`, with `by` one of `fact`,
   `rule`, `constraint`, `builtin`, `committed`, `no-solution`; `line`,
   `column` and `clause` appear when a clause is cited. Without the flag
   the document is byte-identical to today's, so no consumer sees a change
   it did not ask for; the addition is documented in `json-output.md` and
   flagged to aivolution-meta as a contract impact at acceptance.

6. **Explanations cite posted constraints, not projected ones.** The
   printed constraints are a projection of the store; the explanation
   lists the constraint instances the derivation posted, which entail
   them. Nothing in the explanation is derived from the projection.

7. **Not in this record.** Why a query *failed*: a failure tree is the
   whole search space, and for a constraint failure the useful object is
   a minimal infeasible subset of the posted constraints, which the
   simplex can produce exactly (a Farkas certificate over rationals,
   Dutertre and de Moura's explanation of a conflicting bound set) once
   each asserted bound carries its origin. That is a second record, and
   the one the integrity checker will want next ("this claim cannot be
   satisfied because of these three constraints"). Also out: a reified
   proof term in the language, and any explanation of the REPL's stepping
   beyond the per-answer tree.

## Consequences

- **Soundness is untouched**, and gains a checkable property: an
  explanation cites only steps on the answer's own derivation — every
  clause cited was the one that resolved the goal, every constraint cited
  was posted and still holds in the answer's store, and nothing from a
  discarded alternative or a committed-away branch appears. Recording
  changes no decision the machine takes. The aspect doc gains that
  property as an invariant with its test (an answer's cited facts, run as
  a program on their own with the cited rules, reproduce the answer).

- **Machine.** `Solutions` holds an optional arena of proof nodes
  (`parent`, kind, the goal's address, the clause's position in its
  predicate); each pending goal carries the id of the node it belongs
  under; `try_clauses` and the constraint, built-in and construct paths
  append a node when they succeed; a choice point records the arena
  length and backtracking truncates to it, as the store's trail does for
  bindings. At an answer the tree is rendered from the arena with the
  answer printer's term renderer, so `_1`-style names agree between the
  answer and its explanation. The commit of CLM-0022 truncates choice
  points but keeps the arena: the condition's nodes are the committed
  proof.

- **Compile.** `TClause` gains `span: Option<Span>` and `text`; the
  construct diagnostics of CLM-0022 can then cite the clause's position
  too (a small follow-up, not part of this record). `Program::compile`
  without positions still works; cited positions are then absent and the
  text alone is given.

- **CLI and REPL.** `--explain` (batch, with or without `--json`),
  `:explain` toggling in the session; `--help` and the REPL's `:help`
  gain a line. The record `2026-08-18-repl-interaction-model.md` lists
  the commands and is amended in place by the task.

- **Contract impact.** `json-output.md` documents the optional field;
  the acceptance entry carries a `meta-impact` note for aicogito, whose
  adapter ignores unknown fields today and may later read the tree. The
  contract names `claimr-json-output` and `claimr-token-api`, suggested
  since CLM-0015, remain to be added to aivolution-meta's vocabulary.

- **One task** implements the record: recording, the two renderers, the
  flag and toggle, `examples/explain.claimr` with a golden `.answers`
  produced under `--explain`, the JSON test, and the docs (`evaluator.md`,
  `json-output.md`, the REPL record, `answer-soundness.md`).

## Sources

- Incubation note `2026-11-08-explanations-of-answers.md` (graduated by
  this record).
- B. Dutertre, L. de Moura, *A Fast Linear-Arithmetic Solver for
  DPLL(T)*, CAV 2006, section on explanations, for the deferred failure
  case.
