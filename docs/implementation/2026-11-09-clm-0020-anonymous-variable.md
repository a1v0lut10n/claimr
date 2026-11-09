---
date: 2026-11-09
type: task
status: in-progress
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

1. **Grammar**: the `Var` terminal's regex gains the alternative `_`
   (`/[A-Z][A-Za-z0-9_]*|_/`); no production changes, so the AST carries
   `Expr::Var("_")` and the action is untouched. `_foo` stays an error: it
   lexes as `_` followed by an identifier the grammar does not expect.
2. **Compile** (`VarMap::get`): the name `_` allocates a fresh index on
   every occurrence and is never indexed by name; the clause's variable
   count includes them.
3. **Query variables** (`Solutions::new`): `_` entries are dropped from
   the answer's variable list, so they print no equation and name
   nothing; the heap cells they bind stay reachable through named
   variables and print as internal (`_1`).
4. **Highlight**: a lone `_` lexes as `Variable`; coverage example added.
5. **Examples**: `lists.claimr` and `strings.claimr` take the records'
   spelling for don't-care positions; queries with `_` added; golden
   answers regenerated (unchanged where no query changed).
6. **Tests**: machine tests (distinct per occurrence, hidden in answers,
   reachable-through-named prints as internal, a constraint on `_` alone
   projects away, clause echo prints `_`); parse test (`_` is
   `Expr::Var("_")`, `_foo` fails); REPL `:list` echo.
7. **Docs**: `grammar.md` (lexical rule and a paragraph), `evaluator.md`
   (numbering and query variables); README if its grammar block names
   variables; this plan.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean.
- [ ] `aivolution lint .` clean; the `affects` docs current.
