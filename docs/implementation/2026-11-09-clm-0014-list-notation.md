---
date: 2026-11-09
type: task
status: in-progress
affects:
  - docs/reference/grammar.md
  - docs/architecture/components/evaluator.md
  - README.md
components: [grammar, parser, evaluator, cli]
aspects: [grammar-authority, answer-soundness]
design: [docs/design/2026-11-09-lists-as-a-constraint-domain.md]
tags: [CLM-0014, lists, grammar, printing, prolog-iii]
---

# CLM-0014: List notation (lists as a constraint domain, stage 1)

## Premise

Lists work today only as hand-spelled terms (`nil`, `cons(H, T)`,
`examples/lists.claimr`). The accepted design record
`2026-11-09-lists-as-a-constraint-domain.md` makes lists a constraint
domain in three stages; this task is stage 1, the notation: `<>`,
`<a, b, c>` and the literal-left concatenation `<a, b> . T`, printed back
in the same form. Nothing below the parser changes except the printer; the
length (`|L|`) and general concatenation (`X . Y`) stages follow under
their own tickets.

## Scope

In: the two `Expr` productions, the AST variant and its display, the
lowering to `'<>'` chains, list-aware printing in answers and clause
echoes, the reserved spelling `'<>'`, the example and its golden answers,
the EBNF view and the README grammar section, the evaluator component doc.

Out: `|L|`, non-literal `X . Y`, any store or solver change, strings,
`append`/`member`/`reverse` as built-ins (they stay library predicates),
`src/highlight.rs` (no new terminal; its lexer names tokens, not roles),
the JSON output (a list is the compound it is).

## Action items

1. **Grammar** (`src/parser/claimr.rustemo`): two `Expr` productions and
   no new terminal:

       | '<' items=Args? '>'               {ListExpr}
       | '<' items=Args '>' '.' tail=Expr {ListTailExpr, right, 0}

   Priority 0 puts `.` below `+ -` (1), so `<a> . T + 1` is
   `<a> . (T + 1)`; right associativity gives `<a> . <b> . T`. Checked
   with the project's generator before starting (rustemo 0.9.1 `rcomp`,
   default tables): 57 productions, 90 states, zero conflicts. `'.'`
   resolves to the existing `Dot` terminal; an `Expr` is never followed by
   the clause terminator, so the two roles never meet in one state.
2. **AST** (`src/ast.rs`): `Expr::List { items: Vec<Expr>, tail:
   Option<Box<Expr>> }`. Display prints `<>`, `<a, b, c>` and
   `<a, b> . T`; the tail form has precedence 0 and is parenthesised as an
   arithmetic operand (`(<a> . T) + 1`), the tail itself prints at
   precedence 0. Query echoes (`Query::text`), `--parse` and the REPL's
   `:list` follow from this.
3. **Actions** (`src/parser/claimr_actions.rs`): `expr_list_expr`,
   `expr_list_tail_expr` build the variant.
4. **Lowering** (`src/eval/compile.rs`): `Expr::List` becomes a chain of
   `TTerm::Compound('<>', [h, t])` ending in `TTerm::Const('<>')` or the
   lowered tail. `'<>'(a, '<>')` written by hand is therefore the same
   term as `<a>`. A clause whose head is named `'<>'` is rejected at
   compile time with a new `EvalError::ReservedName` carrying the clause's
   span (the record reserves the spelling; this is the first `EvalError`
   with a position, through the existing `span()` hook).
5. **Printing** (`src/eval/answer.rs`): the iterative renderer walks a
   `'<>'/2` spine, printing `<1, 2, 3>` when it ends in `'<>'`,
   `<1, 2> . T` when it ends in anything else, `<>` for the bare constant.
   The walk stops at a cyclic node (back-edge target) so `X = <1> . X`
   prints as that equation, the printer's existing finite form; `'<>'`
   with another arity is an ordinary compound.
6. **Example and tests**: `examples/lists.claimr` rewritten in the
   notation (`append`, `member` with the cons pattern `<X> . _`, `reverse`,
   `length` with Peano numerals as before) plus queries for a partial list
   with a variable tail, a cyclic list, and `'<>'(a, '<>') = <a>`; its
   `.answers` regenerated and checked by hand. Machine tests in
   `src/eval/tests.rs` for printing (`<1, 2> . T`, `<>`, cyclic, hand-spelled
   `'<>'` cells) and for `ReservedName`. Parse test for the AST shape and
   the precedence of `.` against `+`. REPL test for the echo if the
   existing harness makes it cheap.
7. **Docs**: `docs/reference/grammar.md` and the README grammar section
   gain the two productions and a paragraph on lists; `evaluator.md`
   (`compile.rs` lowering, `answer.rs` printing) updated with
   `last-verified` bumped and the design record in `decisions`.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean.
- [ ] Every existing golden example unchanged except `lists.answers`, whose
      new content is the same answers in the new notation.
- [ ] `aivolution lint .` clean; the three `affects` docs current.
