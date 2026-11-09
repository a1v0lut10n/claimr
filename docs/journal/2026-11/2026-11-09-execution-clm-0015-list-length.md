---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, solver, cli, highlight]
aspects: [grammar-authority, exact-arithmetic, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0015-list-length.md
design:
  - docs/design/2026-11-09-lists-as-a-constraint-domain.md
meta-impact:
  note: "The highlight token API now emits `|` as an operator and the --json constraint strings may carry list-length terms such as `|T| >= 1` (document shape unchanged); aicogito consumes both. No contract is named yet — the suggestion from the acceptance entry stands: add claimr-json-output and claimr-token-api to the vocabulary."
---

# CLM-0015 executed: `|L|` is a numeric term, list lengths live in the store

- **When:** 2026-11-09 10:16:50

## Context

Stage 1 (CLM-0014) gave lists their notation. Stage 2 of the accepted
record connects them to the numeric store: Prolog III's "the length `|a|`
of the list a is thus the real n+α", so that `{ |Z| = |X| + |Y| }`,
`{ |L| > 2 }` and `{ |L| = N / 2 }` are ordinary linear constraints and
residual constraints on lengths print as such. The integrity-checker use
that motivated the records needs `{ |Keys| >= 2 }` as a rule.

## Details

**Grammar.** Terminal `Bar: '|'` and the production `'|' Expr '|'` as an
atomic term. After CLM-0014's silent-conflict lesson the `--dot` automaton
was read rather than the conflict count: `Bar` appears only as a shift,
the tuple-dot decision inside `|…|` still falls after `>`, and no action
was removed by priority resolution (59 productions, 92 states). The bars
nest syntactically (`||L||` parses) and fail semantically, since the inner
length is a number, not a list.

**Store.** `Store::post_len` follows the `'<>'` spine of its operand. A
complete list folds to the number of its cells, with no solver variable,
as CLM-0013 does for ground arithmetic. A spine ending after `k` cells in
an unbound non-numeric variable `T` is `k + |T|`, where `|T|` is a
**length entry** registered once per list variable (`len_index`), so every
occurrence of `|T|` is one unknown: a numeric heap variable with `>= 0`
asserted at creation, suspended on `T` and on itself. A cycle or a
non-list (a number, another constant or compound, a numeric variable)
fails. When `T` is bound the spine is re-walked and `|T| = k` or
`|T| = k + |U|` is asserted through `U`'s entry, and the entry closes.
When `|T|` becomes determined while `T` is still unbound, the value must
be a non-negative integer `n` or the branch fails, and `T` is unified with
`<_1, …, _n>`. Determination is detected cheaply in `settle` and exactly
in `finalize`, like every numeric variable, so `{ |L| + |M| = 3, |L| =
|M| }` fails at answer time on the non-integer 3/2. Entries, the index
and the pending flags are trailed; `Mark` carries the entry count. The
list label is now interned first in every symbol table (`symbol::LIST`),
so the store recognises and builds list cells without the table.

**Answers.** The length of a reachable unbound list variable is a public
numeric variable named `|T|`, beside heap numeric variables and attribute
terms, so `{ L = <1> . T, |L| = N }` answers `L = <1> . T, N = |T| + 1` and
`{ |L| > 2 }` answers `|L| > 2`. The implicit bound `|T| >= 0` belongs to
the notation and is filtered from the printed inequalities. The JSON
constraint strings carry the same text; the document shape is unchanged.

**Verification.** 121 tests pass, among them `list_lengths_are_numeric_terms`
(19 queries: ground fold, residuals, partial lists, determination to a
tuple, fractional and negative lengths, cycles, non-lists including
`||L||`, length arithmetic), `list_lengths_follow_backtracking`, the parse
test for `|L|` and nested bars, and a JSON test for a length residual.
`examples/lengths.claimr` has golden answers checked by hand; every
earlier golden file is byte-identical. Formatting, clippy with warnings
denied and the docs linter are clean.

**Measurement**, release build, `violation(C) :- kn(C, Keys), { |Keys| <
2 }.` over facts with one to three keys, all answers:

| Facts  | complete lists `<k1, k2>` | open lists `<k1, k2> . T` |
|--------|---------------------------|---------------------------|
| 1 820  | 0.02 s                    | 0.04 s                    |
| 3 640  | 0.03 s                    | 0.11 s                    |
| 7 280  | 0.06 s                    | 0.33 s                    |
| 14 560 | 0.12 s                    | 1.14 s                    |

Complete lists fold and scale linearly. Open lists under search create a
length entry and a defined variable per candidate, rows persist across
backtracking, and the time grows about quadratically, as the record
anticipated. The tableau truncation deferred by the folding record is the
fix when that matters.

**Interpretations of the record**, written into the plan since the record
is append-mostly: the record is silent on the list whose length becomes
determined, and this task binds it to a tuple of fresh variables, Prolog
III's solved form and what stage 3 needs to complete a concatenation; the
spine is walked whole on each binding event rather than one cell at a
time (the same system, fewer rows); reasoning on lengths stays linear
over the rationals, so `{ |L| > 2 }` is kept as written rather than
tightened to `>= 3`.

**Pre-write checks.** The five docs in the task's `affects` are current:
the EBNF view and the README carry the production and a paragraph on
length, `evaluator.md` the lowering and the printing, `solver.md` a *List
lengths* bullet and the backtracking note with the measurement,
`json-output.md` the note on constraint strings. `grammar-authority` held
in order (grammar, actions, example, EBNF view, README view).
`exact-arithmetic` holds: lengths are exact rationals in the simplex and
integrality is checked exactly. `answer-soundness` holds: every printed
length system is the store's own, no non-integer or negative length
survives to an answer, and a determined length is realised as a list of
that length. Cross-repo impact is the `meta-impact` note above.

## Links

- Task: `docs/implementation/2026-11-09-clm-0015-list-length.md`
- Design record: `docs/design/2026-11-09-lists-as-a-constraint-domain.md`
- Previous stage: `docs/journal/2026-11/2026-11-09-execution-clm-0014-list-notation.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-fold-arithmetic-lists-strings.md`
- Commits `7bedc3a` (plan, ticket claim), `5e3b15f` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/42
- Views updated: `docs/reference/grammar.md`, `docs/reference/json-output.md`,
  `README.md`, `docs/architecture/components/evaluator.md`,
  `docs/architecture/components/solver.md`
