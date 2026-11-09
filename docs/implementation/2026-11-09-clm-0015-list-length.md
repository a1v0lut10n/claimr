---
date: 2026-11-09
type: task
status: done
affects:
  - docs/reference/grammar.md
  - docs/reference/json-output.md
  - docs/architecture/components/evaluator.md
  - docs/architecture/components/solver.md
  - README.md
components: [grammar, parser, evaluator, solver, cli, highlight]
aspects: [grammar-authority, exact-arithmetic, answer-soundness]
design: [docs/design/2026-11-09-lists-as-a-constraint-domain.md]
tags: [CLM-0015, lists, length, solver, prolog-iii]
---

# CLM-0015: List length `|L|` (lists as a constraint domain, stage 2)

## Premise

Stage 1 (CLM-0014) gave the notation. Stage 2 of the accepted record
connects lists to the numeric store: `|L|` is a numeric term admissible
wherever a term is, its value lives in the simplex like any number, so
`{ |Z| = |X| + |Y| }`, `{ |L| > 2 }` and `{ |L| = N / 2 }` are ordinary
linear constraints and residual constraints on lengths print as such. A
length that becomes determined must be a non-negative integer or the
branch fails; `|L|` on a cyclic chain or a non-list fails.

## Scope

In: the `|` terminal and one `Expr` production; `Expr::Len` with its
display; lowering to a store entry per list variable with congruence
(two `|T|` of the same variable are one unknown); propagation along the
spine as the list is bound; the non-negativity bound at creation and the
integer check at determination; the list of a determined length becoming
a tuple of fresh variables; `|L|` in answers and in the JSON constraint
strings; highlight; example; the five doc views.

Out: general concatenation `X . Y` (stage 3); strings; any search on
lengths (Prolog III's `enum`); tableau truncation (the record notes this
stage adds solver rows per list variable under search and may bring the
deferred truncation forward; measured, not addressed, here).

## Action items

1. **Grammar**: terminal `Bar: '|';` and `| '|' inner=Expr '|' {LenExpr}`
   in `Expr`, default priority. The `--dot` automaton (rustemo 0.9.1, 59
   productions, 92 states) has `Bar` only as a shift, and the tuple-dot
   decision inside `|…|` still falls after `>`; no action was removed by
   priority resolution. **Done.**
2. **AST and actions**: `Expr::Len(Box<Expr>)`, Display `|e|` at atomic
   precedence; `expr_len_expr`. **Done.**
3. **Compile**: `TTerm::Len(Box<TTerm>)`; `build_with` lowers it with
   `Store::post_len`. The list label is now interned first in every symbol
   table (`symbol::LIST`, `LIST_LABEL` moved to `symbol.rs`) so the store
   recognises and builds list cells without the table. **Done.**
4. **Store** (`src/eval/store.rs`): `Length { list, n, svar, pending }`
   entries in `lengths`, `len_index` from an unbound list variable to its
   entry, `Waker::Len`, `Undo::LengthPending` / `Undo::LenIndex`, the count
   in `Mark`. `walk_spine` classifies a term as `Nil(k)`, `Open(k, var)` or
   `Bad`. `post_len`: `Nil` folds to a number, `Open` is the entry's heap
   variable (`k = 0`) or a `define_fresh` of `k + |T|`, `Bad` fails.
   `length_entry` asserts `>= 0` and suspends on the variable and on the
   length. `recheck_len`: list still unbound and length determined to a
   non-negative integer `n` → unify with `<_1, …, _n>` (anything else
   fails); list bound → `|T| = k` or `|T| = k + |U|` asserted and the entry
   closed; cycle or non-list fails. **Done.**
5. **Answers**: `public_names` names the length of a reachable pending
   entry `|L|`; survivors that are lengths print the same way; the
   implicit `|L| >= 0` (a single length variable, coefficient 1, non-strict,
   constant 0) is filtered from the printed inequalities. **Done.**
6. **Highlight**: `|` in the operator set. **Done.**
7. **Example and tests**: `examples/lengths.claimr` with golden answers;
   machine tests `list_lengths_are_numeric_terms` (19 queries: ground fold,
   residuals, partial list, determination to a tuple, fractional and
   negative lengths, cycle, non-lists including `||L||`, length arithmetic,
   exact-time determination to 3/2 failing) and
   `list_lengths_follow_backtracking`; parse test
   `length_bars_parse_as_a_numeric_term`; JSON test
   `length_residuals_are_constraint_strings`. **Done.**
8. **Docs**: `grammar.md`, README grammar section, `evaluator.md`,
   `solver.md` (new *List lengths* bullet, backtracking note with the
   measurement), `json-output.md` (constraint strings may carry `|T|`).
   **Done.**
9. **Measurement** (release build, `violation(C) :- kn(C, Keys),
   { |Keys| < 2 }.`, one to three keys per fact, all answers):

   | Facts  | complete lists `<k1, k2>` | open lists `<k1, k2> . T` |
   |--------|---------------------------|---------------------------|
   | 1 820  | 0.02 s                    | 0.04 s                    |
   | 3 640  | 0.03 s                    | 0.11 s                    |
   | 7 280  | 0.06 s                    | 0.33 s                    |
   | 14 560 | 0.12 s                    | 1.14 s                    |

   Complete lists fold and scale linearly. Open lists create a length
   entry and a defined variable per candidate, rows persist across
   backtracking, and the time grows about quadratically, as the record
   anticipated. Usable at these sizes; the tableau truncation deferred by
   `2026-11-09-fold-ground-arithmetic-before-the-solver.md` is the fix
   when it is not. **Done.**

## Notes against the design record

- The record does not say what happens to the list when its length is
  determined. This task binds it to a tuple of fresh variables, Prolog
  III's solved form for a list of known length, which is also what stage
  3 needs to complete a concatenation once `|X|` is known. A very large
  determined length allocates that many cells, as in Prolog III.
- The record says binding `L` to a cell "posts `|L| = 1 + |T|`". The
  implementation walks the whole spine at once and posts one equation
  per binding event, which is the same system with fewer rows.
- Reasoning on lengths is linear over the rationals, as the record states:
  `{ |L| > 2 }` is kept as written, not tightened to `>= 3`.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (121 tests); all previous golden files
      byte-identical.
- [x] `aivolution lint .` clean; the five `affects` docs current.
