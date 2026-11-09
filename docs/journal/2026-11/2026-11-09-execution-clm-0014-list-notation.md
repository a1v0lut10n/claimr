---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, cli]
aspects: [grammar-authority, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0014-list-notation.md
design:
  - docs/design/2026-11-09-lists-as-a-constraint-domain.md
---

# CLM-0014 executed: list notation — `<>`, `<a, b, c>`, `<a, b> . T`

- **When:** 2026-11-09 09:46:42

## Context

The accepted record `lists-as-a-constraint-domain` delivers Prolog III
lists in three stages; stage 1 is the notation alone, with nothing below
the parser changing except the printer. It was started directly after
CLM-0013 because it is self-contained and because the data with
multi-valued fields that motivated the record (`aliases(p1, <'alpha',
'beta'>)` instead of one fact per value) needs only this stage.

## Details

**Grammar.** `src/parser/claimr.rustemo` gains a `Tuple` rule
(`'<' Args? '>'`) and two `Expr` productions, `Tuple` and
`Tuple '.' Expr` with priority 0 and right associativity, so `.` binds
looser than every arithmetic operator (`<a> . T + 1` is `<a> . (T + 1)`)
and chains to the right. No new terminal: `'.'` resolves to the existing
`Dot`, and `<` `>` to `Lt` `Gt`. A term is never followed by the clause
terminator and a relational operator never occurs inside a term, so the
two roles of each token never meet in one LR state.

**A silent conflict in the record's spelling.** The record writes the
stage 1 production as `'<' Args '>' '.' Expr` beside `'<' Args? '>'`. In
that form the state after `Args` holds a reduce of `Args?` (for the list)
and a shift of `>` (for the concatenation): a shift/reduce conflict that
rustemo resolves by production priority without reporting it, and since
the list production has the default priority 10 against the
concatenation's 0, the reduce wins and the `.` branch is unreachable.
`append(<H> . T, L, R)` failed to parse with no diagnostic from the
generator. The record's earlier zero-conflict checks were on the general
`Expr '.' Expr` form, which does not have this state. Factoring the tuple
into its own rule moves the choice to after `>`, where the two items
differ only by whether `.` follows. The lesson, written into the plan for
stages 2 and 3: once productions carry priorities, a zero-conflict report
from `rcomp` means little; the `--dot` automaton must be read for the
states that matter.

**AST and display.** `Expr::List { items, tail }` with a Display that
prints `<>`, `<a, b, c>` and `<a, b> . T`, parenthesising the tail form
under arithmetic. The action normalises `<> . T` to `T` and `<a> . <b> . T`
to the single chain `<a, b> . T`, which is the same term and what the
answer printer would print, so query echoes, `--parse` and the REPL's
`:list` agree with answers.

**Lowering and the reserved name.** A list lowers to a chain of
`'<>'(H, T)` cells ending in the constant `'<>'` or the written tail
(`LIST_LABEL` in `compile.rs`), so a hand-spelled `'<>'(a, '<>')` is the
same term as `<a>`. A clause whose head is named `'<>'` is rejected with
the new `EvalError::ReservedName`, which carries the clause's span: the
first compile error with a source position, through the `span()` hook
that had been waiting for one, so the CLI prints `file:line:col:`.

**Printing.** `Printer::push_list` in `answer.rs` walks a `'<>'/2` spine
and queues `<1, 2, 3>` when it ends in `'<>'`, `<1, 2> . T` otherwise; the
bare constant prints `<>`; `'<>'` with another arity stays an ordinary
compound. The walk takes the first cell unconditionally and stops at any
later cyclic cell, which is also what stops a cycle back to the first
cell, so `X = <1> . X` prints as that equation. The first version
admitted the first cell again and looped on exactly that query; the
cyclic case is now in the machine tests.

**Example and tests.** `examples/lists.claimr` is rewritten in the
notation with the same `append`, `member`, `reverse` and `length` and new
queries for a partial list, chained tuples, a cyclic list,
`'<>'(a, '<>') = <a>` and nested lists; its golden answers were
regenerated and checked by hand, and are the previous answers in the new
notation. New tests: `lists_print_in_their_notation` and
`the_list_constructor_cannot_be_defined_as_a_predicate` (machine),
`list_notation_parses_to_list_terms` (AST shape, `.` against `+`, Display
round trip) and `lists_echo_in_their_notation` (REPL). 117 tests pass;
every golden file other than `lists.answers` is byte-identical.

**Deviations from the record**, recorded in the plan since the record is
append-mostly after acceptance: the production spelling above; the
record's examples use the anonymous variable `_`, which the grammar does
not have, so the example uses named variables (`_` deserves its own
incubation note: one terminal, a fresh variable per occurrence, hidden in
answers); and the printer renders `<1> . 2`, a chain ending in a non-list,
faithfully rather than only "a variable tail".

**Pre-write checks.** The three docs in the task's `affects` are current:
`docs/reference/grammar.md` and the README grammar section carry the
productions and a paragraph on lists, `evaluator.md` describes the
lowering and the printing. `grammar-authority` held in order: grammar,
actions, example, EBNF view, with the README view following.
`answer-soundness` is untouched, only the rendering of terms changed. No
cross-repo impact: no new token for the highlight API, and the JSON
documents keep their shape (answer strings now spell list terms in the
notation, as any term rendering follows the printer).

## Links

- Task: `docs/implementation/2026-11-09-clm-0014-list-notation.md`
- Design record: `docs/design/2026-11-09-lists-as-a-constraint-domain.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-fold-arithmetic-lists-strings.md`
- Previous execution: `docs/journal/2026-11/2026-11-09-execution-clm-0013-fold-ground-arithmetic.md`
- Commits `a493e3c` (plan, ticket claim), `a221539` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/41
- Views updated: `docs/reference/grammar.md`, `README.md`,
  `docs/architecture/components/evaluator.md`
