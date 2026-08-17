---
date: 2026-08-18
type: journal
components: [evaluator, solver, cli]
aspects: [answer-soundness, exact-arithmetic]
tasks: [docs/tasks/2026-08/2026-08-18-clm-0007-answer-simplification.md]
design: [docs/design/2026-08-17-evaluator.md]
---

# Answer projection and simplification (CLM-0007)

## Context
Stage 4 of the evaluator design: answers as the store projected onto the
query and simplified — Prolog III's solved form, plus the elimination of
useless variables that Prolog III itself lacked.

## Details
- `src/eval/project.rs`: residual system over alias-class roots (bounds
  with non-public definitions expanded, public definitions, numeric
  disequations), connectivity to public variables, Gaussian elimination
  of internal variables, Fourier–Motzkin (cheapest first, budget 256 →
  named survivors), integer-coefficient normalisation, dedupe, exact
  entailment-based redundancy removal, solved-form orientation.
- `answer.rs` renders from the projected system; tree disequations print
  in reduced form from the would-bind pairs recorded by `check_dif`;
  deterministic ordering (equalities, then inequalities lower-first, then
  disequations). Public = query-reachable numeric variables + attribute
  terms the query created or looked up.
- Fixed the stage-3 projection bug that dropped a disequation linked to a
  public variable (`q(A)` → `A != 3`).
- 87 tests; goldens `dif` and `nested_terms` updated deliberately;
  clippy clean. Docs: evaluator/solver components, answer-soundness
  aspect, README.

## Links
- Task: docs/tasks/2026-08/2026-08-18-clm-0007-answer-simplification.md
