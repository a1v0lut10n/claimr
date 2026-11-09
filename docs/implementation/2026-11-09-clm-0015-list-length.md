---
date: 2026-11-09
type: task
status: in-progress
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
   in `Expr`, default priority (it is an atomic form like `( … )`). Check
   the `--dot` automaton for the states that shift or reduce on `Bar`:
   the lesson of CLM-0014 is that a zero-conflict report is not enough
   once productions carry priorities.
2. **AST and actions**: `Expr::Len(Box<Expr>)`, Display `|e|` at atomic
   precedence; `expr_len_expr`.
3. **Compile**: `TTerm::Len(Box<TTerm>)`; `build_with` lowers it with
   `Store::post_len`, which returns a heap address like `post_arith`.
4. **Store** (`src/eval/store.rs`), the substance:
   - `Length { list: Addr, n: Addr, svar: SVar, pending: bool }` entries
     in `lengths`, trailed like products (count in `Mark`, pending flag
     in `Undo`), plus `len_index: HashMap<Addr, usize>` from an unbound
     list variable to its entry so that every `|T|` of the same variable
     is the same unknown (trailed like the attribute index).
   - `post_len(a)`: walk the `'<>'/2` spine from `a` counting cells,
     with a visited set. Spine ends in `'<>'`: the length is ground and
     **folds to a number** (no solver variable, as CLM-0013 does for
     arithmetic). Ends in an unbound non-numeric variable `T`: the
     result is `k + |T|`, where `|T|` is the entry's numeric heap
     variable (created with `svar >= 0` asserted and `Waker::Len`
     suspended on `T` and on `n`), as a `define_fresh` when `k > 0` and
     the entry's own heap variable when `k = 0`. Ends in anything else
     (a number, another constant, another compound, a numeric variable)
     or revisits a cell: fail.
   - `Waker::Len(id)` fired by binding `T`: walk from `T`'s new value;
     `'<>'` after `k` cells asserts `|T| = k`; an unbound variable `U`
     after `k` cells asserts `|T| = k + |U|` through `U`'s entry; a
     cycle or non-list fails. The entry is then no longer pending.
   - `Waker::Len(id)` fired by the determination of `n`: the value must
     be a non-negative integer, else fail; if `T` is still unbound, bind
     it to `<_1, …, _n>` with fresh variables — Prolog III's solved form
     for a list of known length, and what stage 3 needs to complete a
     concatenation. Determination is detected cheaply in `settle` and
     exactly in `finalize`, like every other numeric variable.
5. **Answers** (`src/eval/answer.rs`): the length of a reachable unbound
   list variable is a public numeric variable named `|L|` (rendered with
   the variable's answer name), beside heap numeric variables and
   attribute terms; survivors that are lengths print the same way.
   `{ |L| > 2 }` answers `|L| > 2`; `{ L = <1> . T, |L| = N }` answers
   `N = |T| + 1`; `{ |L| = 3 }` answers `L = <_1, _2, _3>`.
6. **Highlight**: `|` joins the operator set; the terminal-coverage test
   then passes.
7. **Example and tests**: `examples/lengths.claimr` with golden answers
   (ground fold, partial list, residual bounds, length arithmetic,
   determination to a tuple, non-integer and negative failures, cyclic
   and non-list failures, congruence of two `|T|`); machine tests for
   the same and for undo across backtracking; parse test for `|L|` in
   arguments and constraints and its display; a JSON test that a length
   residual appears in `constraints`.
8. **Docs**: `grammar.md`, README grammar section, `evaluator.md`
   (lowering), `solver.md` (the length entries beside attribute terms and
   products, backtracking note), `json-output.md` (constraint strings may
   carry `|L|` terms: a contract note, no shape change); plan to what was
   built.
9. **Measurement**: the CLM-0013 benchmark shape with a list field,
   `{ |Keys| >= 2 }` over a few thousand facts, to see whether the
   row-persistence cost the record anticipates shows up now.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; all previous golden files byte-identical.
- [ ] `aivolution lint .` clean; the five `affects` docs current.
