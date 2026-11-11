---
date: 2026-11-11
type: task
status: in-progress
affects:
  - docs/architecture/components/evaluator.md
  - docs/reference/json-output.md
components: [evaluator]
aspects: []
design:
  - docs/design/2026-11-09-explanations-of-answers.md
  - docs/design/2026-11-11-explanations-of-failure.md
tags: [CLM-0032, explanations, rendering, constraints, bugfix]
---

# CLM-0032: A constraint goal in an explanation renders as written, with its bindings

## Premise

A constraint node of a proof tree (CLM-0023) and the failed leaf of a
failure tree (CLM-0031) render the goal from the two heap terms the
machine built to post it. An arithmetic term is lowered to a fresh numeric
variable when it is built (`X - Y` becomes `_1` with the row `_1 = X - Y`),
so a goal whose arithmetic is over variables still unbound when it is
rendered shows the lowered variable, not the expression: `{ X - Y = 1 }`
failing before `X` and `Y` are known prints `{ _1 = 1 }`, and an answer
`Z = X + 1` explains its goal as `{ Z = _1 }`. CLM-0031's findings named
this and called rendering the template instead a refinement for both.
The records say what a constraint node is — "the instance that was
posted" — without saying how arithmetic in it prints; this task fixes the
rule: **the goal as written, with the derivation's bindings for its
variables, arithmetic as written**.

## Shape of the implementation

**A template renderer.** `answer::render_template(store, tpl, vars)` turns a
`TTerm` template into an `ast::Expr` — a variable becomes `Expr::Var` of
its rendered binding, a symbol its name (quoted when needed, as the AST
prints it), a `'<>'` chain an `Expr::List` with its tail, `Len`, `Concat`,
`Neg` and `Arith` their AST forms — and prints it with the AST's own
iterative, precedence-aware `Display`, so `2 * (X - Y)` keeps its
parentheses and a list prints as `<a, b> . T`. The walk is iterative and
flattens a list spine in a loop (CLM-0019: a literal in a clause can be
long).

**Where it applies.** `render_proof` collects, for a constraint record and
for a failed constraint leaf, the heap addresses of the template's
variables (`compile::term_vars`, now crate-visible) instead of the two
built sides, renders them in the one `render_answer` pass that names every
`_N` of the tree consistently, and assembles `{ L op R }` from the two
templates. `ProofKind::Constraint` and `FailedGoal::Constraint` therefore
keep only the goal template and the frame; the operator comes from the
template. Nothing changes in what is posted or decided.

**What changes visibly.** Only goals whose arithmetic was not folded:
`{ 5 > 3 }` stays `{ 5 > 3 }` (its variables are bound), `{ cv1 = cv1 }`
stays. `{ L = |"abc"| }` in `examples/explain.claimr` becomes
`{ 3 = |<a, b, c>| }` — the length as written over its list, the variable
as its value — and the golden file follows. The failure golden is
unchanged (every variable in it is bound at the leaf).

## Action items

1. `render_template` in `answer.rs`; `term_vars` crate-visible.
2. `render_proof`: the two constraint sites; the slimmer `ProofKind::Constraint`
   and `FailedGoal::Constraint`; `constraint()` without the sides.
3. Tests: a failing goal over unbound variables prints as written
   (`{ X - Y = 1 }`), an answer's goal with an arithmetic side (`{ Z = X + 1 }`),
   a length over a list (`{ 3 = |<a, b, c>| }`), a list with a tail, a
   quoted atom and a nested arithmetic that needs parentheses; the goldens.
4. Docs: `evaluator.md`, `json-output.md` (`constraint`: the goal as written
   with the bindings); this plan.

## Findings

(filled in as the work proceeds)

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test`; `examples/explain.answers` changes in the one line
      above, every other golden byte-identical.
- [ ] `aivolution lint .` clean; the two `affects` docs current.
