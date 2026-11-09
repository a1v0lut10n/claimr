---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, solver, cli, highlight]
aspects: [grammar-authority, exact-arithmetic, answer-soundness]
design:
  - docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md
  - docs/design/2026-11-09-lists-as-a-constraint-domain.md
  - docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md
meta-impact:
  note: "Implementing the lists and strings records will add residual-constraint kinds to the --json output (new contract version) and new tokens (< > . | and string literals) to the highlight token API that aicogito consumes; no contract is named yet — suggest adding claimr-json-output and claimr-token-api to the vocabulary."
---

# Three design records accepted: ground arithmetic folding, lists as a constraint domain, strings and type relations

- **When:** 2026-11-09 03:12:39

## Context

Running claimr as an integrity checker over a knowledge base (about 2 500
claims, about 10 500 facts) on 2026-11-08 produced nine incubation entries
(CLM-0012, PR #34). One graduated to catalyst with its mechanism confirmed in
the code and its measurements reproduced (PR #37); it and two of the
incubation entries then graduated to design records (PR #38), written
against the primary source, Colmerauer's *An Introduction to Prolog III*
(CACM 33(7), 1990), with the standing instruction to stay true to the
original Prolog III design where new language features are concerned.

## Details

Accepted the three records (status `proposed` → `accepted`):

**Fold ground arithmetic before it reaches the solver.** When every
operand of an arithmetic term is a number at instantiation, `post_arith`
computes the exact rational and binds a number cell instead of defining a
solver variable and row. Division by zero still fails. The arithmetic-in-
terms record's "folding happens when the evaluator compiles a clause" is
read as "at instantiation", a refinement, not a supersession. Evidence:
`{ 10 * K > 10 * N }` over 1 820 and 3 640 facts took 1.88 s and 7.41 s
against 0.02 s and 0.04 s for `{ K > N }`, identical answers; the cause is
`define_fresh` creating a row even for two constant operands, and
`Simplex::undo_to` restoring bounds only, so every failed trial's rows
persist and `update` walks them all. Truncating the tableau on undo is
deferred to its own record.

**Lists as a constraint domain.** Lists in Prolog III's sense, in three
stages under one record. Representation: chains of `'<>'(H, T)` cells
ending in `'<>'`, the paper's single list label; cyclic chains are legal
terms with no finite length; unknown tails are variables with
undetermined lengths, in place of the paper's irrational α. Stage 1,
notation: `<>`, `<a, b, c>`, and `<a> . T` as the cons pattern, which is
Prolog III's literal-left `•` operation. Stage 2, length: `|L|` as a
numeric term attached to the list variable, living in the simplex,
integer when determined or the branch fails. Stage 3, concatenation:
`X . Y` as a pending constraint that posts `|Z| = |X| + |Y|` at once and
completes when the left length is determined; never searched, as in the
paper, with `append/3` kept for that.

Alternatives weighed: Edinburgh `[a, b]` / `[H | T]` as sugar over
`cons`/`nil` with the constraint domain deferred, which was the first
draft. Rejected on two grounds after review. The parser argument against
angle brackets (ambiguity with the relational operators) was tested with
the project's generator (rustemo 0.9.1, `rcomp`, LALR-Pager) and found
false: tuple, `|L|` and `.` concatenation productions compile with zero
conflicts, because relational operators occur only in `Constraint` and the
clause terminator and the concatenation operator are one `Dot` terminal
whose role the LR state decides. And the length and concatenation
constraints are the part of Prolog III's list design that adds value to a
constraint language, so deferring them was the wrong default. Angle
brackets over square ones is a spelling choice: they read as Prolog III
and keep `|L|` legible. `.` over `++` for concatenation: the paper's own
operator, with its footing in formal language theory; the only objection
had been its look beside the terminator.

**Strings as character lists, and type relations as constraints.**
`"abc"` denotes `<a, b, c>`, the paper's `"c1...cm" = <'c1', ..., 'cm'>`,
with characters as one-character atoms because `'c'` is already claimr's
quoted-atom spelling. One bridge, `atom_chars/2`, behaves as a delaying
constraint. `number/1`, `atom/1` and `list/1` are constraints that
suspend, following Prolog III's `num`, `id` and `list` relations; there is
no `var/1`, which dissolves the question of `var` on a constrained
variable. The ISO atom built-ins proposed in incubation are dropped:
prefix tests are concatenation constraints, lengths are `|Cs|`, and
scheme extraction keeps `append` because its left length is unknown and
Prolog III does not search that either.

Pre-write checks: no task closes with this entry. The aspect invariants
hold unchanged, since no code moved; the lists record extends
`exact-arithmetic` to lengths (integer on determination, never
approximated) and keeps `answer-soundness` by never searching an
unknown-left concatenation. Cross-repo impact is forward-looking and
carried in `meta-impact` as a bare note, since the contract vocabulary
names nothing yet.

Next: each record becomes a task with its own ticket. The folding fix and
stage 1 of the lists record can start at once.

## Links

- PR #34 (incubation batch, CLM-0012), PR #37 (catalyst graduation),
  PR #38 (the three records, including the `<`/`>` correction and the
  `.` operator).
- Primary source: Colmerauer, *An Introduction to Prolog III*, CACM 33(7),
  1990, as cited in `docs/catalyst/2026-08-17-evaluator.md`.
- Related records: `docs/design/2026-08-17-arithmetic-in-terms.md`,
  `docs/design/2026-08-17-evaluator.md`,
  `docs/design/2026-09-10-quoted-atoms.md`.
- Component doc to update when the folding fix lands:
  `docs/architecture/components/solver.md` (*Backtracking*).
