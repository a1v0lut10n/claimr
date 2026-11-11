---
date: 2026-11-11
type: journal
components: [evaluator]
aspects: []
tasks:
  - docs/implementation/2026-11-11-clm-0032-constraints-as-written.md
design:
  - docs/design/2026-11-09-explanations-of-answers.md
  - docs/design/2026-11-11-explanations-of-failure.md
meta-impact:
  note: "Under `--explain`, the `goal` string of a `constraint` node (an answer's `explanation`, a failure's `branches` and `conflict`) is now the goal as written with the derivation's bindings — arithmetic, lengths and lists in source form (`{ Z = X + 1 }`, `{ 3 = |<a, b, c>| }`) where a lowered numeric variable (`{ Z = _1 }`) or a folded value (`{ 3 = 3 }`) printed before. Document shapes unchanged; documents without the flag byte-identical. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0032 executed: a constraint goal in an explanation renders as written

- **When:** 2026-11-11 20:46:35

## Context

CLM-0031's findings named it: a constraint goal whose arithmetic was over
variables still unbound when it was rendered showed the lowered numeric
variable, `{ _1 = 1 }` for `{ X - Y = 1 }`, in a failure leaf and in an
answer's proof tree alike, because both rendered the goal from the two
heap terms built to post it. Taken as a bugfix-sized ticket before 0.5.0,
so the release ships explanations that read as the program was written.

## Details

**What was built.** `answer::render_template` renders a clause template
with the derivation's bindings: each variable becomes an `ast::Expr::Var`
carrying its rendered binding, a symbol its name, a `'<>'` chain a list
with its tail (the spine flattened in a loop, CLM-0019), and `Neg`,
`Arith`, `Len` and `Concat` their AST forms; the AST's own iterative,
precedence-aware `Display` then prints it, so `2 * (3 - 1)` keeps its
parentheses, `-B` its sign and `'x y'` its quotes — one renderer, the
parser's, rather than a second that would drift from it. `render_proof`
collects the templates' variables (`compile::term_vars`, now
crate-visible) instead of the two built sides, renders them in the one
pass that names every `_N` of a tree consistently, and assembles
`{ L op R }`; `ProofKind::Constraint` and `FailedGoal::Constraint` keep
only the goal and the frame. Nothing posted or decided changes.

**Verified.** 212 tests pass; two new in `constraints_as_written`: a
failed goal over unbound variables prints as written (`{ X - Y = 1 }`
with its conflict `{ X - Y = 0 }`), and an answer's goals keep their
arithmetic (`{ Z = X + 1 }`), lengths (`{ 3 = |<a, b, c>| }`), lists
(`{ N = |<a, b> . T| }`), parentheses (`{ 2 * (3 - 1) = 4 }`) and quoting
(`{ 'x y' = 'x y' }`). Two expectations moved, both for the better: the
explain golden's `{ 3 = 3 }` is `{ 3 = |<a, b, c>| }` and a
comprehension's count `{ 2 = 2 }` is `{ 2 = |<1, 2>| }` — the goal says
what was measured, with the value in the place of its variable. The
failure golden diffed clean against a regeneration; every other golden
is byte-identical. Fmt, clippy and lint clean.

**Findings.** A variable bound to a value still shows the value, not its
name: `{ X = 1 }` with `X` bound prints `{ 1 = 1 }`, as `{ cv1 = cv1 }`
did under CLM-0023; the rule is the goal as written with the bindings,
and a binding to a number is a number. Showing the source name alongside
would be a different rule, not taken.

**Pre-write checks.** The task's two `affects` docs were updated in the
PR: `evaluator.md` (the record's template and frame, `render_template`)
and `json-output.md` (`constraint`: the goal as written with the
bindings). No aspect invariant is touched: invariant 6 of
`answer-soundness` says an explanation cites the posted constraints,
which it still does, now spelled as written. Cross-repo impact: the
`goal` strings, noted above.

## Links

- Task: `docs/implementation/2026-11-11-clm-0032-constraints-as-written.md`
- Where it was found: `2026-11-11-execution-clm-0031-failure-tree.md`, Findings
- PR #82: commits e27b8c9 (plan), 8a9dd06 (fix)
