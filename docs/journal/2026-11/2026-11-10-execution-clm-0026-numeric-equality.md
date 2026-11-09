---
date: 2026-11-10
type: journal
components: [evaluator, solver]
aspects: [answer-soundness, exact-arithmetic]
tasks:
  - docs/implementation/2026-11-09-clm-0026-numeric-equality.md
design:
  - docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md
meta-impact:
  note: "No output shape changes. The language's answers change for programs that relied on head unification turning a numeric variable into an attribute term (`{ M = 2 }, q(M)` with `q(f(a))` now fails, as `q(2)` does) and for a stored number meeting its attribute term in a block (`person(alice, A), { A = age(alice) }` now succeeds); aicogito's integrity rules should be rerun against this build."
---

# CLM-0026 executed: numbers, numeric variables and attribute terms

- **When:** 2026-11-10 00:03:27

## Context

The record accepted yesterday (PR #66) closes a drift between the
evaluator record's D4 and D5 and the store: a numeric variable with a
single value was kept as an unbound cell with coinciding bounds, a
block `=` counted a variable bound to a number as a tree, and
unification turned a numeric variable meeting a compound into an
attribute term. The three rules of the record were implemented as one
task in the store, with the machine, the compile step and the printer
untouched.

## Details

**Rule 1 — a determined numeric variable is bound to its number.** The
determination pass (`numeric_consequences`, cheap and exact) fixes the
solver variable and then binds the heap variable to a fresh number cell,
trailed like any binding; `bind` of a numeric variable meeting a number
fixes and binds the same way, so a trial unification for `dif` records
the binding where it used to see "no change" and compare values. From
the binding on the variable is the number: a tree leaf for unification
and first-argument indexing, a number for the block rule, its value for
arithmetic and printing — the printer's own value lookup is now a
fallback that finalization never reaches. The `fired` set and
`Undo::Fired` of CLM-0024 are gone: a bound variable is no determination
candidate, so the candidate set is exactly the unbound numeric variables.

**Rule 2 — a block `=`/`!=` is numeric when either side denotes a
number.** `is_numericish` dereferences: a number cell at the end of the
chain, by whatever route it got there, or an unbound numeric variable.
The old test excluded "a plain variable that happens to be bound to a
number tree" on purpose; the record reverses that, and
`person(alice, A), { A = age(alice) }` over `person(alice, 30)` now
answers `A = 30, age(alice) = 30`.

**Rule 3 — unification never creates an attribute term.** The
`Const`/`Struct` arm of `bind` for a numeric variable returns false
(D4 as written) instead of registering the term and equating unknowns.
`{ X > 3 }` followed by a head binding `X` to `foo` fails; `foo` bound
first and then `{ X > 3 }` still answers `foo > 3`, the constant being a
direct operand of a block relation.

**What changed and what did not.** Every golden example is
byte-identical: none relied on the removed path. Two unit tests asserted
the old behaviour and now assert the record's — `attribute_terms_and_congruence`
(`{ X > 3 }, same(X, foo)` fails) and
`first_argument_indexing_preserves_clause_order_and_answers`
(`{ V = 1/2 }, p(V, N)` indexes `V` as the number: two answers instead of
seven; and `p(V, N), { V = a }` gains the answer `V = 1/2, N = 6, a = 1/2`,
the constant `a` naming an attribute term as `{ 1/2 = a }` already did).
The new test `numeric_equality_follows_what_the_operands_denote` runs
the record's probe in both goal orders, nineteen queries, with the
integrity-checker shape, the determination-binds-the-variable cases
(`{ 2 * X = 5 }` then a head with `5/2` succeeds, with `2` fails) and a
disequation decided through two bound variables.

**Two findings.** The constant-as-attribute-term answer above is where
the record's deferred item 5 shows in practice. And
`{ A = age(alice) }, person(alice, A)` stays `false`: with the block
first, an unbound `A` meets a tree, which rule 2 makes tree unification,
and the head's `30` then fails against `age(alice)`. The record's "in
either goal order" holds for the two probe lines it names, not for this
order; widening rule 2 to compounds would cost either tree equations in
blocks or the independence of `=` from world facts. The accepted
phrasing is to say that `A` is a number first:
`number(A), { A = age(alice) }, person(alice, A)` answers
`A = 30, age(alice) = 30`. The record stands as written.

**Verification.** 123 unit tests plus the integration and golden suites;
formatting, clippy with warnings denied and the docs linter (79 files)
clean.

**Pre-write checks.** The three docs in the task's `affects` are current:
`solver.md` (the `=`/`!=` rule, the numeric-variable bullet replacing
the "stated consequence of D4", the candidates, the trailed sets),
`evaluator.md` (the goal-level entry points, a determined variable bound
to its number), `answer-soundness.md` (invariant 7: a block relation
means what its operands denote, with the tests). The record is in the
`decisions` of all three. `answer-soundness` is strengthened, not
changed: a determined variable has its value in every solution, so
binding it changes no solution set. `exact-arithmetic` held: the bound
number is the exact value the solver fixed. Cross-repo impact is the
`meta-impact` note.

## Links

- Task: `docs/implementation/2026-11-09-clm-0026-numeric-equality.md`
- Design record: `docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-numbers-numeric-variables-and-attribute-terms.md`
- Commits `71b0899` (plan, ticket claim), `105d4ec` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/67
- Views updated: `docs/architecture/components/solver.md`,
  `docs/architecture/components/evaluator.md`,
  `docs/architecture/aspects/answer-soundness.md`
