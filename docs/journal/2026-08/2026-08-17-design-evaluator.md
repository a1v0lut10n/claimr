---
date: 2026-08-17
type: journal
components: [evaluator, solver]
aspects: [answer-soundness, exact-arithmetic]
design: [docs/design/2026-08-17-evaluator.md]
---

# Evaluator design accepted

## Context
The evaluator note graduated to catalyst with the numeric domain settled;
the remaining questions (term model, `=>`, search, solver, answers) needed
a decision before implementation.

## Details
Accepted `docs/design/2026-08-17-evaluator.md`: SLD resolution over
rational trees (no occurs check, bind-before-descend), trail over bindings
and store, `{c} => head.` as sugar for `head :- {c}.`, constraint facts as
the initial store, attribute terms as claimr's extension for compound
operands in numeric constraints, an exact incremental linear store
(Gauss–Jordan + simplex, delayed disequations by trial unification, one
`geler`-style suspension mechanism), grammar-first arithmetic and comments,
answers in Prolog III solved form. Anchored in Colmerauer 1990/1982 and
Van Caneghem 1986. Registered components `evaluator`, `solver` and aspect
`answer-soundness`. Four implementation stages planned.

## Links
- Catalyst note: docs/catalyst/2026-08-17-evaluator.md
