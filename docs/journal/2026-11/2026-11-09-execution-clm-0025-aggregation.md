---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, solver, highlight]
aspects: [grammar-authority, answer-soundness, exact-arithmetic]
tasks:
  - docs/implementation/2026-11-09-clm-0025-aggregation.md
design:
  - docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md
meta-impact:
  note: "The --json `constraints` strings now include the goal-form residual `sum(L, S)` for a pending sum, and the `explanation` nodes' `by` field gains the value `collected` (document shape unchanged in both cases); aicogito's reasoning adapter treats the strings as opaque and reads unknown fields as data. The suggestion from CLM-0015 stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# CLM-0025 executed: aggregation — list comprehensions and the sum relation

- **When:** 2026-11-09 22:13:22

## Context

The record accepted earlier today (PR #63) gives claimr all solutions of a
goal as a list comprehension `<T | Body>`, count as its length, and
`sum/2` as a linear built-in relation, under the groundness rule of
CLM-0022 with every solution finalized exactly and every instance ground.
The implementation followed as one task, the example needing both halves.

## Details

**Grammar and AST.** `Expr` gains `'<' template=Expr '|' body=Body '>'`;
no new terminal, `|` being the length bar. `Expr` and `Body` are mutually
recursive through it, and the automaton has no conflict; the bar after a
complete template inside an unclosed `<` can only open the body, and
`|<X | p(X)>|` nests as length bars do. The AST carries
`Expr::Comprehension { template, body }`, printed back as written, with
atomic precedence; its drop moves only the template into the worklist,
the body's goals dropping on their own.

**Lowering.** `Lowerer::term` meets a comprehension at build time and
replaces it by a fresh clause variable, queuing a
`TGoal::Collect { template, body, into, check }`; `Lowerer::body` emits
the queued collections before the goal they belong to, saving the queue
around nested bodies so an inner body does not swallow its enclosing
goal's collections. A comprehension inside a *template* is evaluated once
per solution, so its collection goes at the end of the enclosing body: the
first version queued it before the outer collection, the nested example
errored on the outer body's unbound variable, and the fix is a second
save-and-restore around the template. `no_collects` makes a comprehension
in a fact's or rule's head or in a constraint fact
`EvalError::ComprehensionInHead` with the clause's position; the
constraint of `{c} => head` is lowered as a body and admits one. The
external-variable analysis treats the template's and the body's variables
as the construct's, with the rest of the enclosing goal as outside, so
`{ N = |<X | p(X, N)>| }` is the runtime error of CLM-0022 naming `N`.

**Machine.** Opening a `Collect` applies the groundness rule, pushes a
`Retry::Finish` choice point holding the accumulator (an
`Rc<RefCell<Vec<TTerm>>>` shared with the goal that fills it) and runs the
body followed by `Goal::CollectOne`. Reaching that goal finalizes the
store exactly (a non-solution backtracks into the body, a non-linear
residue stops the query as at an answer), builds the template's instance
from the frame, copies it out with `Store::snapshot` — an iterative walk
yielding a closed term template of constants, numbers (a determined
numeric variable as its value, exactly) and compounds, `None` on an
unbound variable or a cycle, which stops the query with
`EvalError::NonGroundSolution` naming the comprehension and the instance
— and fails, so the search moves to the next solution. Exhausting the
body lands on the finish with the store restored: the list is rebuilt from
the copies as a `'<>'` chain on the heap, unified with the comprehension's
variable, and the rest continues. Explanations record one `collected`
leaf, the comprehension as written with the list it produced; the
solutions' own proofs are discarded with their derivations.

**`sum/2`.** The fifth built-in. `post_sum` walks the spine as `post_len`
does: a complete list posts `S − e1 − … − en = 0` through `unknown_of` on
each element (a number, a numeric variable, or an attribute term; a
variable held by a type relation fails), exact and solvable in either
direction; an open spine registers a pending `Sum` entry suspended on the
tail, re-checked on binding, undone with the trail; a pending one prints as
`sum(L, S)` after the other goal-form residuals.

**Verification.** 167 tests: `comprehensions_collect_all_solutions_in_order`
(order, pairs, empty, nested in a template, count, under negation, in a
condition, arithmetic in the template),
`comprehensions_apply_the_groundness_rules` (the external rule with the
rest of the goal as outside, an unbound and an undetermined instance, a
compound with a variable, a cycle, a determined variable collected, a
solution a disequation rejects, the head and constraint-fact errors, the
implication admitting one), `sum_is_a_linear_relation_over_a_list`
(exact total and average, solving for an element, the empty list, delayed
on an open tail and decided on binding, an attribute-term element, a
non-list, the reserved name), `a_comprehension_explains_as_one_collected_leaf`,
`a_large_collection_builds_on_a_small_stack` (20 000 solutions on a 1 MiB
thread), the parse round trips, and the JSON vocabulary test. Every
existing golden file is byte-identical; `examples/aggregation.claimr`
runs 16 queries. Formatting, clippy with warnings denied and the docs
linter are clean.

**Pre-write checks.** The six docs in the task's `affects` are current:
`grammar.md` (the tuple alternative and the comprehension paragraph),
`builtins.md` (five predicates, the `sum` row, the aggregation idioms),
`evaluator.md` (the lowering, the collection, the `Collected` resolution,
the pending sum in answers), `solver.md` (the sum relation),
`answer-soundness.md` (invariant 5 restated for the three constructs that
run a sub-search, with the new tests), `json-output.md` (`collected`,
`sum(L, S)`). `grammar-authority` held in order: grammar, actions,
example, EBNF view, README; the highlight lexer needed no change.
`answer-soundness`: the comprehension is a function of ground inputs and
collects nothing the store does not determine. `exact-arithmetic`: the sum
is an exact linear equation. Cross-repo impact is the `meta-impact` note.

## Links

- Task: `docs/implementation/2026-11-09-clm-0025-aggregation.md`
- Design record: `docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-aggregation-list-comprehensions-and-sum.md`
- Commits `87e7043` (plan, ticket claim), `9a2149c` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/64
- Views updated: `docs/reference/grammar.md`, `docs/reference/builtins.md`,
  `docs/reference/json-output.md`, `README.md`,
  `docs/architecture/components/evaluator.md`,
  `docs/architecture/components/solver.md`,
  `docs/architecture/aspects/answer-soundness.md`
