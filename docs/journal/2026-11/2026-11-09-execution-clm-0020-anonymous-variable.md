---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0020-anonymous-variable.md
design:
  - docs/design/2026-11-09-anonymous-variable.md
---

# CLM-0020 executed: the anonymous variable `_`

- **When:** 2026-11-09 15:02:02

## Context

The record accepted earlier today gives claimr the Prolog-family `_`,
which the lists and strings records had assumed in their examples
(`member(X, <X> . _)`, `{ Cs = Prefix . _ }`) and which the grammar
rejected when those records were implemented. This task is the smallest
of the day: one terminal, one rule in the compile step's variable
numbering, one filter on a query's answer variables.

## Details

**Grammar.** The `Var` terminal is `/(?:[A-Z][A-Za-z0-9_]*|_)/`. The
alternation has to be grouped: the generator anchors the pattern, and the
first attempt, an ungrouped `|_`, let the second alternative match an
underscore anywhere later in the input, so every example broke at its
first `, L`. No production changes; the AST carries `Expr::Var("_")`,
which no other spelling can produce, and the action is untouched. `_foo`
lexes as `_` followed by `foo` and is reported as an unexpected identifier
at `foo`, as the record wants: `_`-prefixed names are not names.

**Compile and machine.** `VarMap::get` allocates a fresh clause-variable
index for every occurrence of `_` and never indexes it by name, so two
`_` in one clause are two variables. `Solutions::new` drops `_` entries
from the answer's variable list: an anonymous query variable prints no
equation and names nothing, while whatever it binds prints through the
named variables that reach it (`?- { X = f(_) }.` answers `X = f(_1)`),
and a constraint on anonymous variables alone projects away like any
internal one (`?- { _ > 3 }.` answers `true`). The clause printer prints
`_` where it was written; the highlight lexer yields a `Variable` for a
lone `_`, no new token kind.

**Examples.** `lists.claimr` (`member`, `length`) and `strings.claimr`
(`has_prefix`, `scheme`) take the records' spelling; three `_` queries
join the lists example. The strings answers are unchanged, the lists
answers gain the three new queries only.

**Verification.** 138 tests pass: `the_anonymous_variable_is_fresh_and_unnamed`
(11 queries), a parse test for the AST shape, the round trip and the
rejection of `_foo`, and a REPL test for `:list` and a query with `_`.
Formatting, clippy with warnings denied and the docs linter are clean.

**Pre-write checks.** The EBNF view and `evaluator.md`, the two docs in
the task's `affects`, are current (lexical rule and paragraph; numbering
and query variables). `grammar-authority` held in order: terminal,
examples, EBNF view, highlight; the README's grammar block has no lexical
rules and is unchanged. `answer-soundness` is unaffected: `_` changes what
is named, not what is derived or printed about named variables. No
cross-repo impact: the token API gains no kind.

## Links

- Task: `docs/implementation/2026-11-09-clm-0020-anonymous-variable.md`
- Design record: `docs/design/2026-11-09-anonymous-variable.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-anonymous-variable.md`
- Commits `d7d3f26` (plan, ticket claim), `ba48b7f` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/53
- Views updated: `docs/reference/grammar.md`,
  `docs/architecture/components/evaluator.md`
