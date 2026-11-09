---
date: 2026-11-09
type: task
status: done
affects:
  - docs/reference/grammar.md
  - docs/architecture/components/evaluator.md
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
design: [docs/design/2026-11-09-anonymous-variable.md]
tags: [CLM-0020, anonymous-variable, grammar, variables]
---

# CLM-0020: The anonymous variable `_`

## Premise

The accepted record gives claimr the Prolog-family `_`: one terminal,
admitted wherever a variable is, a fresh clause variable per occurrence,
not an answer variable in a query, printed as written. The lists and
strings records' examples (`member(X, <X> . _)`, `{ Cs = Prefix . _ }`)
become valid as written.

## Action items

1. **Grammar**: `Var: /(?:[A-Z][A-Za-z0-9_]*|_)/`. The alternation must be
   grouped: the generator anchors the pattern, and an ungrouped `|_` made
   the second alternative match an underscore anywhere later in the input
   (every example broke). No production changes; the AST carries
   `Expr::Var("_")`; `_foo` lexes as `_` then `foo` and is reported as an
   unexpected identifier at `foo`. **Done.**
2. **Compile** (`VarMap::get`): `ANONYMOUS` allocates a fresh index on
   every occurrence and is never indexed by name. **Done.**
3. **Query variables** (`Solutions::new`): `_` entries are dropped from the
   answer's variable list. **Done.**
4. **Highlight**: a lone `_` lexes as `Variable`; coverage example added.
   **Done.**
5. **Examples**: `lists.claimr` (`member`, `length`) and `strings.claimr`
   (`has_prefix`, `scheme`) take the records' spelling; three `_` queries
   added to the lists example; the strings answers are unchanged, the
   lists answers gain the three new queries only. **Done.**
6. **Tests**: `the_anonymous_variable_is_fresh_and_unnamed` (11 queries),
   `the_anonymous_variable_parses_and_prints_as_written`,
   `the_anonymous_variable_echoes_and_is_not_answered` (REPL). 138 tests.
   **Done.**
7. **Docs**: `grammar.md` (lexical rule and paragraph), `evaluator.md`
   (numbering, query variables); the README's grammar block has no lexical
   rules and is unchanged. **Done.**

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (138 tests).
- [x] `aivolution lint .` clean; the `affects` docs current.
