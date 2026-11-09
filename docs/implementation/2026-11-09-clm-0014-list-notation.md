---
date: 2026-11-09
type: task
status: done
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

1. **Grammar** (`src/parser/claimr.rustemo`): two `Expr` productions, one
   `Tuple` rule and no new terminal:

       | tuple=Tuple               {ListExpr}
       | tuple=Tuple '.' tail=Expr {ListTailExpr, right, 0}
       Tuple: '<' items=Args? '>';

   Priority 0 puts `.` below `+ -` (1), so `<a> . T + 1` is
   `<a> . (T + 1)`; right associativity gives `<a> . <b> . T`. `'.'`
   resolves to the existing `Dot` terminal; an `Expr` is never followed by
   the clause terminator, so the two roles never meet in one state
   (rustemo 0.9.1 `rcomp`, default tables: 58 productions, 89 states).
   **Done**, with a correction to the record's spelling of the production,
   see *Deviation* below.
2. **AST** (`src/ast.rs`): `Expr::List { items: Vec<Expr>, tail:
   Option<Box<Expr>> }`. Display prints `<>`, `<a, b, c>` and
   `<a, b> . T`; the tail form has precedence 0 and is parenthesised as an
   arithmetic operand (`(<a> . T) + 1`), the tail itself prints at
   precedence 0. Query echoes (`Query::text`), `--parse` and the REPL's
   `:list` follow from this. **Done.**
3. **Actions** (`src/parser/claimr_actions.rs`): `expr_list_expr`,
   `expr_list_tail_expr`, `tuple_c1`. The tail action normalises:
   `<> . T` is `T`, and `<a> . <b> . T` is the one chain `<a, b> . T`
   (the same term, and what the answer printer prints). **Done.**
4. **Lowering** (`src/eval/compile.rs`): `Expr::List` becomes a chain of
   `TTerm::Compound('<>', [h, t])` ending in `TTerm::Const('<>')` or the
   lowered tail (`LIST_LABEL`). `'<>'(a, '<>')` written by hand is
   therefore the same term as `<a>`. A clause whose head is named `'<>'`
   is rejected at compile time with the new `EvalError::ReservedName`
   carrying the clause's span: the first `EvalError` with a position,
   through the existing `span()` hook, so the CLI prints `file:line:col:`.
   **Done.**
5. **Printing** (`src/eval/answer.rs`): `Printer::push_list` walks a
   `'<>'/2` spine and queues `<1, 2, 3>` when it ends in `'<>'`,
   `<1, 2> . T` when it ends in anything else; the bare constant prints
   `<>`. The walk takes the first cell unconditionally and stops at any
   later cyclic cell (back-edge target), which also stops a cycle back to
   the first cell, so `X = <1> . X` prints as that equation; `'<>'` with
   another arity is an ordinary compound. (The first version re-entered
   the first cell and looped; the cyclic case is now a test.) **Done.**
6. **Example and tests**: `examples/lists.claimr` rewritten in the
   notation (`append`, `member` with the cons pattern, `reverse`, `length`
   with Peano numerals as before) plus queries for a partial list, chained
   tuples, a cyclic list, `'<>'(a, '<>') = <a>` and nested lists; the
   `.answers` regenerated and checked by hand (same answers as before in
   the new notation). Machine tests `lists_print_in_their_notation` and
   `the_list_constructor_cannot_be_defined_as_a_predicate`; parse test
   `list_notation_parses_to_list_terms` (AST shape, `.` against `+`,
   Display round trip); REPL test `lists_echo_in_their_notation`.
   **Done.**
7. **Docs**: `docs/reference/grammar.md` and the README grammar section
   gain the productions and a paragraph on lists; `evaluator.md`
   (`compile.rs` lowering, `answer.rs` printing) updated with
   `last-verified` bumped and the design record in `decisions`. **Done.**

## Deviation from the design record

- The record spells the stage 1 production `'<' Args '>' '.' Expr` beside
  `'<' Args? '>'`. Written that way the generator has a shift/reduce
  conflict on `>` (reduce `Args?` for the list against shift `>` for the
  concatenation) that it resolves silently by production priority, so the
  `.` branch was unreachable: `append(<H> . T, L, R)` failed to parse with
  no diagnostic. The record's own generator check was for the general
  `Expr '.' Expr` form, which does not have this state. Factoring the
  tuple into its own rule puts the choice after `>`, where the two items
  differ only by whether `.` follows, and `.` is not in the follow set of
  a term. Lesson for stages 2 and 3: a zero-conflict report from `rcomp`
  is only meaningful when no production carries a priority; check the
  `--dot` automaton for the states that matter.
- The record's examples use the anonymous variable `_` (`member(X, <X> .
  _)`), which claimr's grammar does not have. The example uses named
  variables; `_` is a separate small feature worth its own incubation note
  (one terminal, a fresh variable per occurrence, not printed in answers).
- The record says a list "with a variable tail" prints `<1, 2> . T`; the
  printer does so for any non-`'<>'` tail, including a non-list
  (`<1> . 2`), which is the faithful rendering of that term.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (87 unit tests plus the integration suites).
- [x] Every existing golden example unchanged except `lists.answers`, whose
      new content is the same answers in the new notation.
- [x] `aivolution lint .` clean; the three `affects` docs current.
