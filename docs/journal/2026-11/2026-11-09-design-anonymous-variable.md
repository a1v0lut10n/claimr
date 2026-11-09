---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
design:
  - docs/design/2026-11-09-anonymous-variable.md
---

# Design record accepted: the anonymous variable `_`

- **When:** 2026-11-09 14:48:03

## Context

Implementing the lists record (CLM-0014) and the strings record
(CLM-0017) showed that their own examples, `member(X, <X> . _)` and
`{ Cs = Prefix . _ }`, do not parse: the grammar has no anonymous
variable, since `Var` requires an uppercase initial. The shipped examples
invent names for don't-care positions, which then appear in `:list` and
among a query's variables. The gap was noted in both plans, captured as
an incubation note the same day, graduated to a design record (PR #51),
and is accepted here. The incubation note is removed in this change, as
a graduation moves the content rather than copying it.

## Details

**Decision.** One terminal, `_`, admitted wherever a variable is and
carried in the AST as `Expr::Var("_")`, which no other spelling can
produce. Every occurrence is a fresh clause variable: the compile step's
variable map allocates a new index instead of looking the name up. In a
query, `_` is not an answer variable — it is not among the query
variables, prints no equation and names nothing — while anything
reachable from it that is also reachable from a named variable prints as
today, and a residual that mentions only variables reachable from `_` is
projected away like any internal one. The clause printer prints `_` where
it was written, so programs round-trip through `:list`, query echoes and
`--parse`.

**Left out, deliberately.** `_`-prefixed names such as `_Tail` remain
lexical errors. Several Prologs read them as named but hidden; that is a
separate question for a later record if a program wants it, not a
consequence of this one.

**Why.** Prolog III has `_` with exactly this meaning, the two records'
examples assume it, and a don't-care position should not need a name
that then leaks into listings and answers.

**Consequences.** The grammar path per `grammar-authority`: the terminal,
no new action (the variable alternative already yields its text), the
lists and strings examples rewritten in the records' spelling, the EBNF
view, the README, and the highlight lexer's variable rule with a coverage
example. The evaluator changes in the compile step's numbering and the
query's variable list only; the machine and the store are untouched, so
`answer-soundness` is unaffected — `_` changes what is named, not what is
derived.

**Pre-write checks.** No task closes with this entry. The two aspects
named are the record's own concern and hold on acceptance. No cross-repo
impact: the highlight token API gains no kind, `_` lexes as a variable.

**Next.** The implementation task claims the next ticket.

## Links

- Design record: `docs/design/2026-11-09-anonymous-variable.md`
  (status `proposed` → `accepted` in the same change)
- Removed in the same change: `docs/incubation/2026-11-09-anonymous-variable.md`
- Where the gap surfaced: `docs/implementation/2026-11-09-clm-0014-list-notation.md`,
  `docs/journal/2026-11/2026-11-09-execution-clm-0017-strings-and-type-relations.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/51
