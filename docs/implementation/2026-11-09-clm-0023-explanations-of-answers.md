---
date: 2026-11-09
type: task
status: done
affects:
  - docs/architecture/components/evaluator.md
  - docs/architecture/aspects/answer-soundness.md
  - docs/reference/json-output.md
components: [evaluator, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-09-explanations-of-answers.md
tags: [CLM-0023, explanations, proof-tree, json-output, cli, repl]
---

# CLM-0023: Explanations of answers

## Premise

The accepted record `2026-11-09-explanations-of-answers.md`: an
explanation is the proof tree of an answer, recorded by the machine on
request (`--explain`, `:explain`, `Solutions::with_explanations()`),
citing clauses by source position and text, constraint goals as the posted
instances, built-ins, committed conditions and conditions without a
solution; rendered as an indented tree under the answer and as an optional
`explanation` field in the JSON answer object. Off, nothing is recorded.

## Action items

1. **Compile** (`src/eval/compile.rs`): `TClause` gains `span: Option<Span>`
   and `text: String` (the clause as written), set for every clause. **Done.**
2. **Answer** (`src/eval/answer.rs`): public `ProofNode { goal, by, line,
   column, clause, subgoals }` and `Resolution { Fact, Rule, Constraint,
   Builtin, Committed, NoSolution }`; `Answer.explanation:
   Option<Vec<ProofNode>>`; the answer renderer also renders the
   explanation's goal instances with the *same* printer, so `_N` names
   agree; `Answer::explanation_lines()` renders the human tree
   iteratively; `ProofNode` drops iteratively too. **Done.**
3. **Machine** (`src/eval/machine.rs`): an optional arena of proof records
   (`parent`, kind, addresses) on `Solutions`; every pending goal carries
   the node it belongs under; `try_clauses`, constraint goals, built-ins
   and the constructs record on success; choice points record the arena
   length and backtracking truncates to it; a commit keeps the condition's
   nodes; the else branch and a negation's continuation record a
   `NoSolution` leaf when taken. The tree is built bottom-up without
   recursion. **Done** (`ProofRec`/`ProofKind`, `Under` on every pending
   goal, `ChoicePoint.proof_len`, `Retry::Goals { note }`).
4. **JSON** (`src/json_out.rs`): `explanation` in the answer object when
   present, nodes nested under `subgoals`, serialised iteratively. **Done.**
5. **CLI and REPL**: `--explain` (with or without `--json`); `:explain`
   toggle and a `:help` line; stepping prints the tree under the answer and
   the `;`/`.` marker on its own line. **Done.** The REPL compiles its
   session without positions, so its citations carry the kind and a rule's
   text, no `line:column`.
6. **Golden runner** (`tests/run_examples.rs`): an optional
   `examples/<name>.args` file supplies flags; `examples/explain.claimr`
   runs under `--explain`. **Done** (16 queries' worth of trees over seven
   queries).
7. **Tests**: tree shape and citations for rules, facts, constraints,
   built-ins, committed and failed conditions, negation, disjunction;
   backtracking discards a discarded derivation's nodes; off by default;
   names agree with the answer's; a deep derivation renders on a small
   stack; the soundness property (the cited clauses, run alone, reproduce
   the answer); REPL and JSON tests. **Done**: 158 tests. The deep test
   uses a 20 000-fact chain, since an explanation prints every goal in
   full (a recursion over a deep term is quadratic in text by nature, and
   so is the indented form in depth), and because a numeric countdown
   `M = N - 1` is quadratic on `main` already — each step makes `M` a solver
   variable fixed to a constant that every later post rescans (13 s for
   1 000 steps, 55 s for 2 000, debug build); that finding is penciled as
   an incubation note, not fixed here.
8. **Docs**: `json-output.md`, `evaluator.md`, `answer-soundness.md`
   (invariant 6), the architecture README's `cli` row, README, CLAUDE.md
   (`.args`), this plan. The REPL design record is not amended: design
   records are append-mostly after acceptance (the schema), so the record's
   "amended in place" consequence is served by the `cli` row and `:help`
   instead. **Done.**

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every existing golden file byte-identical.
- [x] `aivolution lint .` clean; the three `affects` docs current.
