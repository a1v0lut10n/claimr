---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0022-negation-disjunction-if-then-else.md
design:
  - docs/design/2026-11-09-negation-disjunction-and-if-then-else.md
---

# CLM-0022 executed: negation, disjunction and if-then-else in bodies

- **When:** 2026-11-09 19:22:12

## Context

The record accepted earlier today (PR #56) gives bodies ISO Prolog's
control constructs under a rule that keeps them sound over the constraint
store: a condition's external variables must be ground when the construct
is reached, or the query stops. The implementation followed at once as one
task, since negation is if-then-else with a failing then-branch and the
example exercises all three constructs together.

## Details

**Grammar.** Three terminals: `;` and `->` as strings, `\+` as the regex
terminal `NotOp`, so its action receives the token text. The rules are
`Body: Alt ';' Body | Alt`, `Alt: Conj '->' Conj | Conj`,
`Conj: Goal+[Comma]`, and `Goal` gains `NotOp Goal` and `'(' Body ')'`. A
`Goal` production yields a goal *list*: one goal, or a parenthesised
conjunction's goals, which the `Conj` action splices, so parentheses leave
no trace and no AST node is spent on grouping. `rcomp` reports 104 states
and no conflict; the new rules carry no priorities, so none could be
silenced (the CLM-0014 lesson, checked). The `->` versus `-` question
does not arise: no parser state expects both.

**AST and printer.** `Goal::Or(Vec, Vec)`, `Goal::IfThenElse { cond, then,
els: Option }`, `Goal::Not(Vec)`. The printer has `fmt_body` (an
alternative, then `;` and a body) and `fmt_conj` (goals joined by `, `,
a disjunction or if-then-else inside one parenthesised), so `:list` and
`--parse` print the ISO precedence back exactly where the grammar needs
parentheses; nine forms round-trip in the parse test, and redundant
parentheses vanish. `ConstraintRule` classification looks through the
constructs (`body_has_constraint`).

**Compile.** The constructs are templates, `TGoal::Or`,
`TGoal::IfThenElse`, `TGoal::Not`, with nested goal lists; lowering
recurses on the source nesting only. After a body is lowered,
`resolve_conditions` computes each condition's external variables, those
also occurring in the clause outside the whole construct, by set algebra
over the head's variables and the sibling goals' variables at each level,
and stores them with the construct's and the clause's text in a
`CondCheck`. Only bodies with a construct render the clause text, so fact
tables pay nothing. Queries get the same pass with no head.

**Machine.** A choice point now carries a `Retry`: the clause retry of
before, or a goal list (the second alternative, the else branch, a
negation's continuation). Opening a condition applies the groundness rule
through `Store::is_ground`, an iterative walk with a visited set for
cyclic terms in which a numeric variable counts as ground only when the
solver determines it exactly, and otherwise sets
`EvalError::NonGroundCondition`; it then records the choice-point height,
pushes the else branch as a goal-list choice point, and runs the
condition's goals followed by `Goal::Commit { height, then }`. Reaching
the commit finalizes the store as for an answer; a non-solution backtracks
into the condition's alternatives, a non-linear residue stops the query as
it would at an answer; then the stack is truncated to the height and the
then branch runs, or, for a negation, the step fails. Query goals are now
pending goals over a frame allocated at the start, exactly like a clause
body's, so a query body is handled by the same code; the eager
instantiation path and `Goal::Built` are gone, and every existing golden
file is byte-identical.

**The finalize at the commit matters.** `{ X + Y = 2, X - Y = 0, X != 1 }`
as a condition: `X` is determined through the tableau, not by its bounds,
so only the exact probe sees the disequation violated. With the finalize
the else branch is taken (`Z = no`); without it the then branch would run
and the query would end in `false`. The test pins the correct outcome.

**Diagnostic.** ``condition `\+ { X = a }` reached with `X` unbound, in
`r(X) :- \+ { X = a }.`; a variable shared with the clause outside the
construct must be bound before it (bind it earlier, or keep it local to
the construct)``. The classic wrong positive `s :- \+ r(X)` is this error,
not `true`; the Prolog spelling of the curated-or-harvested idiom with the
result in the condition errors when called with the result unbound; a
numeric variable with bounds but no value, and a variable bound to a term
with an unbound variable, are not ground. Variables shared only with the
then or else branch or with the other alternative are exempt, as the
record says.

**Example and tests.** `examples/control.claimr` runs 16 queries: the two
absence rules, the curated-or-harvested idiom, a sign function as a chain
of if-then-else, disjunction, if-then without else, `\+` over a constraint
block and a parenthesised body, double negation, and the finalize case.
150 tests pass: seven machine tests (disjunction order, commit, negation,
finalize before commit, the error and its exemptions, `may_continue`), the
parse round trips, a REPL test that lists the constructs, steps a
disjunction with `;` and recovers from the error, a JSON test showing the
error is the existing query error document, and the highlight drift alarm
over the new terminals. Formatting, clippy with warnings denied and the
docs linter are clean.

**Pre-write checks.** The three docs in the task's `affects` are current:
`grammar.md` has the body rules and the control paragraph with the
groundness rule and both idioms, `evaluator.md` the compile and machine
paragraphs (its *Not yet* list loses negation and disjunction, names the
delayed goal, and declares the cut deliberately absent), and
`answer-soundness.md` invariant 5 with its tests and `last-verified`
bumped. `grammar-authority` held in order: grammar, actions, example, EBNF
view, README, highlight. `answer-soundness` gained an invariant and
changed none. No cross-repo impact: the error reaches the JSON output as
the existing `"outcome": "error"` document and the token API gains no
kind.

## Links

- Task: `docs/implementation/2026-11-09-clm-0022-negation-disjunction-if-then-else.md`
- Design record: `docs/design/2026-11-09-negation-disjunction-and-if-then-else.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-negation-disjunction-and-if-then-else.md`
- Commits `14d3c7f` (plan, ticket claim), `5666375` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/57
- Views updated: `docs/reference/grammar.md`, `README.md`,
  `docs/architecture/components/evaluator.md`,
  `docs/architecture/aspects/answer-soundness.md`
