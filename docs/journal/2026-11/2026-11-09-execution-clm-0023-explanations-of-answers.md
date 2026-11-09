---
date: 2026-11-09
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0023-explanations-of-answers.md
design:
  - docs/design/2026-11-09-explanations-of-answers.md
meta-impact:
  note: "Under --explain, each answer object in the --json output carries an optional `explanation` field (a proof tree of {goal, by, line?, column?, clause?, subgoals}); documents produced without the flag are byte-identical to before. aicogito's reasoning adapter reads documents through generic field lookups and ignores the field; it may read the tree later. The suggestion from CLM-0015 stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0023 executed: explanations of answers

- **When:** 2026-11-09 20:30:40

## Context

The record accepted earlier today (PR #59) makes an answer able to carry
its proof tree on request, so that a finding of the integrity checker can
cite the rule and the facts that produced it. The implementation followed
as one task: recording in the machine, rendering in the answer, the flag,
the REPL toggle and the JSON field.

## Details

**Clauses keep their source.** `TClause` gains `span` (from
`compile_spanned`) and `text` (the clause as written), set for every
clause; both are bounded by the program's size.

**The arena.** `Solutions` holds an optional `Vec<ProofRec>`: a record is
a parent id and a kind — the clause that resolved a call (predicate key,
clause position and the call's address), a constraint goal's two sides and
operator, a built-in's call, a committed condition, or a condition found
without a solution (both as the construct's text, from the CLM-0022
`CondCheck`). Every pending goal carries `under`, the record of the clause
whose body it belongs to; the query's goals carry `None` and become the
roots. `try_clauses` records on a successful head unification and puts
the body goals under the new record; constraint goals and built-ins record
on success; an if-then-else records its committed node when opened and
runs the condition under it, with the then branch under the construct's
own parent; an else branch and a negation's continuation are goal-list
choice points that carry a note, the `NoSolution` leaf to record when
taken. A choice point remembers the arena's length and backtracking
truncates to it, as the trail does for bindings, so an abandoned
derivation leaves no record; a commit truncates choice points and keeps
the arena. With explanations off the arena is `None` and `record` is a
no-op.

**Rendering.** `render_answer` takes the arena's goal addresses as extra
terms and renders them with the answer's own printer after the answer's
names are fixed, so `_N` agrees between `X = f(_1)` and `p(f(_1))`; a
cyclic node first seen in a goal gets its equation too. The tree is
assembled bottom-up — children have higher ids than their parent, so a
reverse pass over the arena completes every node before moving it into
its parent — and `ProofNode` drops iteratively, as terms do since
CLM-0019. `Answer::explanation_lines` renders the human form with an
explicit stack: two spaces per level, the goal, the resolution, the cited
position, and a rule's text. The JSON writer nests the nodes under
`subgoals` with its own explicit stack.

**Surface.** `--explain` for batch runs, combinable with `--json`;
`:explain` toggles in the REPL and `:help` lists it; `with_explanations()`
on `Solutions`; `ProofNode` and `Resolution` are public. While stepping,
the tree follows the answer and the `;` or `.` marker takes its own line.
The REPL compiles its session without positions, so its citations give
the kind and a rule's text and no `line:column`; a session's clauses have
no stable positions anyway. The golden runner gained an optional
`examples/<name>.args` sibling for flags, which `examples/explain.claimr`
uses.

**What the deep test taught.** The first version counted down with
`M = N - 1` and timed out: 13 s for 1 000 steps and 55 s for 2 000 on
`main`, with or without explanations. Each step makes `M` a numeric
variable with a solver variable fixed to a constant, and every later
post's cheap determination pass walks all of them. That is a pre-existing
cost, penciled as the incubation note
`2026-11-09-bind-numeric-constants-directly.md` in this change, not fixed
here. The second version recursed over a 20 000-deep term and was
quadratic in text: an explanation prints every goal in full, so a goal at
depth k carries a term of depth 20 000 − k, and the indented human form is
O(depth) per line besides. Both are properties of the format, not of the
implementation. The test now walks a 20 000-fact chain, whose goal
instances stay small, on a 1 MiB thread, and renders the human form at a
depth of 500.

**Verification.** 158 tests: the tree's shape and citations over the
integrity example, the control constructs and built-ins, names agreeing
with the answer, abandoned derivations leaving no record, the soundness
property (the cited clauses alone, as a program, give the same answer),
the deep derivation, a REPL test stepping through explained answers and
toggling back, and a JSON test for the field's presence under the flag
only, plus the deep serialisation. Formatting, clippy with warnings denied
and the docs linter are clean; every existing golden file is byte-identical.

**Pre-write checks.** The three docs in the task's `affects` are current:
`json-output.md` documents the field and its vocabulary, `evaluator.md`
the arena, the renderer and the public types, `answer-soundness.md`
invariant 6 with its tests. `answer-soundness` gained an invariant and
changed none: recording takes no decision. Cross-repo impact is the
`meta-impact` note above. The REPL design record is not amended, since
design records are append-mostly after acceptance; the architecture
README's `cli` row lists `:explain` with the other commands.

## Links

- Task: `docs/implementation/2026-11-09-clm-0023-explanations-of-answers.md`
- Design record: `docs/design/2026-11-09-explanations-of-answers.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-explanations-of-answers.md`
- Incubation note from the deep test:
  `docs/incubation/2026-11-09-bind-numeric-constants-directly.md`
- Commits `8599bda` (plan, ticket claim), `e254057` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/60
- Views updated: `docs/reference/json-output.md`,
  `docs/architecture/components/evaluator.md`,
  `docs/architecture/aspects/answer-soundness.md`,
  `docs/architecture/README.md`, `README.md`, `CLAUDE.md`
