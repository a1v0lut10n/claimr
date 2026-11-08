# Negation as failure

Captured 2026-11-08. Prompted by using claimr to check integrity rules over a knowledge
base. Constraint-violation rules need no negation: `violation(C) :- …, { K > N }.`
succeeds exactly when bound values break a constraint. But absence rules do, and they are
most of a lint:

- "a model file that has no residual-error claim";
- "an open claim that no closure closes";
- "a record without a label".

claimr's grammar has no `\+`/`not`. `!=` is a constraint between terms, not the failure
of a goal.

The idea is `\+ Goal` (or `not(Goal)`): it succeeds when `Goal` has no solution, and binds
nothing. The design questions, which matter more than the syntax:

- **Soundness with constraints.** Negation of a goal that leaves constraints in the store
  is the hard case in constraint logic programming. The usual answers are to allow `\+`
  only on goals that are ground when called (a runtime error otherwise, in the spirit of
  how claimr treats non-linear residues), or constructive negation, which is much harder.
- **Floundering.** A non-ground call to `\+` should be an error, never a silently wrong
  answer, to keep the `answer-soundness` aspect.
- **Stratification,** if tabling arrives: well-founded or stratified semantics for
  negation through recursion.

The ground-only rule would already serve the absence rules above, whose arguments are all
bound by the time the negated goal runs.

Relevant: `docs/architecture/aspects/answer-soundness.md`, `src/eval/machine.rs`.
