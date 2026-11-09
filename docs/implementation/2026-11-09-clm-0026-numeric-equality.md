---
date: 2026-11-09
type: task
status: done
affects:
  - docs/architecture/components/solver.md
  - docs/architecture/components/evaluator.md
  - docs/architecture/aspects/answer-soundness.md
components: [evaluator, solver]
aspects: [answer-soundness, exact-arithmetic]
design:
  - docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md
tags: [CLM-0026, numbers, numeric-variables, attribute-terms, equality, store]
---

# CLM-0026: Numbers, numeric variables and attribute terms — where `=` is numeric

## Premise

The accepted record `2026-11-09-numbers-numeric-variables-and-attribute-terms.md`
closes a drift between the evaluator record (D4, D5) and the store with
three rules: (1) a determined numeric variable is bound to its number;
(2) `=` / `!=` in a constraint block is numeric when either side denotes a
number — a literal, an arithmetic result, a variable bound to a number, or
a numeric variable; (3) unification never creates an attribute term — a
numeric variable meeting a compound or constant fails. Numbers inside
structures stay leaves (rule 4, unchanged).

## Action items

1. **Store, rule 1** (`src/eval/store.rs`): the determination pass
   (`numeric_consequences`, cheap and exact) fixes the solver variable and
   then binds the heap variable to a fresh number cell; `bind` of a numeric
   variable meeting a number fixes and binds likewise, so a trial
   unification records the binding. The `fired` set and `Undo::Fired`
   become redundant (a bound variable is no candidate) and go; the
   candidate set is exactly the unbound numeric variables. **Done.**
2. **Store, rule 2**: `is_numericish` dereferences — a number cell, or an
   unbound numeric variable, at the end of the binding chain. **Done.**
3. **Store, rule 3**: `bind` of a numeric variable meeting a constant or a
   compound returns `false`. **Done.**
4. **Tests**: machine tests for every line of the record's probe in both
   goal orders, the integrity-checker shape (`person(alice, A),
   { A = age(alice) }` against `{ age(alice) = 30 }`), `{ X > 3 }, X = foo`
   failing, and the store's candidate test without `fired`; every golden
   file checked (any that relied on head unification creating an attribute
   term is corrected and the correction noted here). **Done**: 123 unit
   tests; every golden file byte-identical — no example relied on the old
   path. Two existing tests asserted the old behaviour and now assert the
   record's: `{ X > 3 }, same(X, foo)` fails, and `{ V = 1/2 }, p(V, N)`
   indexes `V` as the number (two answers instead of seven).
5. **Docs**: `solver.md` (*Semantics as implemented*: the `=`/`!=` rule,
   the numeric-variable bullet, the stated consequence of D4, the
   candidates), `evaluator.md` and `answer-soundness.md` (the record in
   `decisions`; the stated property that a block relation's meaning depends
   on what its operands denote — invariant 7), this plan. **Done.**

## Findings

- **`{ V = a }` with `V` bound to a number** is a numeric equation by
  rule 2 in which the constant `a` names an attribute term, exactly as
  `{ 1/2 = a }` does today: `p(V, N), { V = a }` over `p(1/2, 6)` now
  answers `V = 1/2, N = 6, a = 1/2` where it used to fail by tree
  unification. The record's item 5 defers whether a bare constant should
  be admitted as an attribute term at all; this is the place it shows.
- **Block first, number second.** `{ A = age(alice) }, person(alice, A)`
  stays `false`: an unbound `A` meets a tree, so by rule 2 this is tree
  unification and the head's `30` then fails against `age(alice)`. The
  record's sentence that the integrity-checker shape succeeds "in either
  goal order" holds for `person(alice, A), { A = age(alice) }` and for
  `{ A = 30 }, { A = age(alice) }`, not for the block-first order, which
  its own rules make tree equality. A program wanting the block first says
  that `A` is a number: `number(A), { A = age(alice) }, person(alice, A)`
  answers `A = 30, age(alice) = 30`. Accepted as the phrasing; the record
  stands as written.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every golden file byte-identical.
- [x] The probe of the record answers as its *Behaviour that changes* says,
      in both goal orders (`numeric_equality_follows_what_the_operands_denote`).
- [x] `aivolution lint .` clean; the three `affects` docs current.
