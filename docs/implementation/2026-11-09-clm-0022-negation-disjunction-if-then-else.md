---
date: 2026-11-09
type: task
status: done
affects:
  - docs/architecture/components/evaluator.md
  - docs/architecture/aspects/answer-soundness.md
  - docs/reference/grammar.md
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
design:
  - docs/design/2026-11-09-negation-disjunction-and-if-then-else.md
tags: [CLM-0022, negation, disjunction, if-then-else, control, grammar, machine]
---

# CLM-0022: Negation, disjunction and if-then-else in bodies

## Premise

The accepted record `2026-11-09-negation-disjunction-and-if-then-else.md`
gives bodies the ISO control constructs `A ; B`, `C -> T ; E`, `C -> T`
and `\+ G`, with no cut, and the groundness rule: a condition's external
variables (those also occurring in the clause outside the construct) must
be ground when the construct is reached, or the query stops with a runtime
error. The record fixes the semantics; this task is the implementation,
all three constructs in one change since negation is if-then-else with a
failing then-branch.

## Action items

1. **Grammar** (`src/parser/claimr.rustemo`): terminals `;`, `->`, `\+`;
   `Body: Alt ';' Body | Alt`, `Alt: Conj '->' Conj | Conj`,
   `Conj: Goal+[Comma]`, `Goal: Atom | '{' ConstraintExpr '}' | '\+' Goal
   | '(' Body ')'`. The automaton is checked with `rcomp --force --dot`
   for priority-silenced conflicts (lesson of CLM-0014). **Done** — `;` and
   `->` are string terminals, `\+` a regex terminal; 104 states, no
   conflict, and the new rules carry no priorities, so none could be
   silenced.
2. **AST and printer** (`src/ast.rs`): `Goal::Or(Vec<Goal>, Vec<Goal>)`,
   `Goal::IfThenElse { cond, then, els: Option<_> }`, `Goal::Not(Vec<Goal>)`;
   parentheses leave no trace (a grouped conjunction splices into its
   parent); the printer puts parentheses exactly where the grammar needs
   them, so `:list` and `--parse` round-trip. `Clause::ConstraintRule`
   classification looks through the constructs. **Done**
   (`body_has_constraint`, `fmt_body` / `fmt_conj`).
3. **Actions** (`src/parser/claimr_actions.rs`): a `Goal` production yields
   a goal list (one goal, or a grouped body's goals), `Conj` splices,
   `Alt`/`Body` build the constructs; no `let … else` (CLM-0017). **Done.**
4. **Compile** (`src/eval/compile.rs`): `TGoal::Or`, `TGoal::IfThenElse`,
   `TGoal::Not` templates with nested goal lists; after lowering a body, a
   pass computes each condition's external variables (indices with their
   source names) and keeps the construct's and the clause's source text
   for the diagnostic. Query bodies get the same treatment. **Done**
   (`CondCheck`, `resolve_conditions`; only bodies with a construct render
   the clause text).
5. **Machine** (`src/eval/machine.rs`): a choice point kind that resumes
   with a goal list (`Alt`: the else branch or the second alternative); a
   `Commit` goal carrying the choice-point height and the then-branch (or
   `fail` for negation); reaching it finalizes the store as for an answer
   (on failure: backtrack into the condition), then truncates the
   choice-point stack to the height and continues. The groundness check
   walks the external variables' dereferenced terms (a visited set for
   cyclic terms; a numeric variable is ground when the solver determines
   it) and raises `EvalError::NonGroundCondition`. Query goals become
   pending goals over an eagerly allocated frame, so a query body is
   handled like a clause body. **Done** (`Retry::Clauses` / `Retry::Goals`,
   `Goal::Commit { height, then }`, `Store::is_ground`, `check_condition`,
   `open_construct`). Query goals were built eagerly before; every golden
   file is byte-identical with the frame.
6. **Error** (`src/eval/error.rs`): `NonGroundCondition { construct, variable,
   clause }`, a runtime error like `NonLinear` (no position), whose message
   says to bind the variable before the construct. **Done.**
7. **Highlight** (`src/highlight.rs`): `\+`, `->` operators; `;` punctuation;
   the coverage test's terminal scan handles the new literals. **Done.**
8. **Example** `examples/control.claimr` + `.answers`: the absence rules,
   the curated-or-harvested idiom, disjunction, if-then without else,
   nested forms, `\+` over a constraint block, a query-level construct.
   **Done** (16 queries).
9. **Tests**: machine tests for each construct, clause order under
   disjunction, commit discarding alternatives, finalize-before-commit
   (a condition whose solution a pending disequation rejects), nested
   negation soundness (`s :- \+ r(X)` is an error, not `true`), the
   groundness rule's exemptions (local, then-shared, else-shared
   variables), the error text, `may_continue`; parse tests for precedence
   and round trip; a REPL `:list` test; a JSON error test. **Done**: 150
   tests (seven machine tests, a parse test over nine round-trip forms, a
   REPL test that also steps a disjunction and recovers from the error, a
   JSON test).
10. **Docs**: EBNF view and README grammar block; `evaluator.md` (compile
    and machine paragraphs; the *Not yet* list loses negation and
    disjunction, cut becomes deliberately absent); `answer-soundness.md`
    gains invariant 5 (conditions decided on ground external variables
    only); `builtins.md` unchanged; this plan. **Done.**

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every existing golden file byte-identical.
- [x] `rcomp --force --dot` automaton inspected: no silenced conflict
      involving `;`, `->`, `\+`, `(`.
- [x] `aivolution lint .` clean; the three `affects` docs current.
