---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, solver, cli]
aspects: [grammar-authority, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0016-list-concatenation.md
design:
  - docs/design/2026-11-09-lists-as-a-constraint-domain.md
meta-impact:
  note: "The --json constraint strings may now carry a pending list concatenation, `Z = X . Y` (document shape unchanged); no new token for the highlight API. aicogito consumes the JSON. No contract is named yet — the suggestion stands: add claimr-json-output and claimr-token-api to the vocabulary."
---

# CLM-0016 executed: `X . Y` is a concatenation constraint; the lists record is complete

- **When:** 2026-11-09 11:26:10

## Context

Stages 1 and 2 (CLM-0014, CLM-0015) gave lists their notation and their
length in the store. Stage 3 of the accepted record is Prolog III's
approximated concatenation, `z =. x • y`: the length relation
`|z| = |x| + |y|` holds at once and the concatenation itself completes
"only if the lists a1, …, ak–1 are all true lists", that is, once `|x|` is
known. It is the stage that gives the paper's idioms — last-element
access, the two-clause `reverse`, prefix tests as constraints, the
periodic-sequence query — and it closes the record. PR #43 was merged
before this entry was confirmed, so the entry comes in its own PR.

## Details

**Grammar.** `Expr '.' Expr`, right-associative with priority 0, replaces
the literal-left production of stage 1; `Tuple` stays its own rule so the
action sees a literal tuple as a unit and still builds the chain. The
`--dot` automaton was read for the decisive states: after `Expr . Expr`
the parser shifts on `.` and on every arithmetic operator, a tuple and a
sum reduce before `.`, and `-X . Y` is `(-X) . Y`. The terminator and the
operator remain one token in two roles that never meet in a state.

**Store.** `Store::post_concat` walks the left operand's spine. Complete
after `k` cells: the result is the chain of those heads ending in `Y`, no
constraint (the operation face; also the head pattern
`reverse(<E> . X, Y . <E>)` when `Y` is known). Open in a variable `T`:
the result is a fresh list variable `Z`; `|Z| = |X| + |Y|` is posted
through the stage 2 length entries of all three (a non-list `Y` fails
here, and a length that becomes determined expands its list, per stage
2); a **concatenation entry** is registered, suspended on `T`. When `T` is
bound the spine is re-walked: complete → `Z` is unified with the chain
and the entry closes; still open → it waits on the new tail; cyclic or
non-list → failure. A length determined by the simplex reaches the entry
through stage 2's expansion of `T`, which binds it. Concatenations are
never searched (Prolog III's `enum` is a program's business; `append/3`
stays). Unlike a pending product, a concatenation still pending at answer
time is a sound residual and prints as `Z = X . Y`; the length relation it
implies is dropped from the printed numeric system when it appears as
such over three unbound list variables.

**Verification.** 126 tests pass, among them
`list_concatenation_is_a_constraint` (19 queries),
`list_concatenation_bounds_the_search` (the two-clause `reverse` in both
directions, palindromes with the length first), the regression test
below, a parse test for associativity and precedence, and a JSON test for
a pending residual. `examples/concat.claimr` has golden answers checked by
hand, including `{ |Z| = 10, <1, 2, 3> . Z = Z . <2, 3, 1> }` solved to
`<1, 2, 3, 1, 2, 3, 1, 2, 3, 1>`. Formatting, clippy with warnings denied
and the docs linter are clean; every earlier golden file is
byte-identical.

**A stage 2 soundness bug, found and fixed.** The reversed
`reverse(X, <1, 2, 3>)` gave its answer and then ran on. The trace showed
one more pending concatenation per level with no failure: the pruning
bound `|Y| = -1` never reached the simplex. Cause: `recheck_len` treated a
length entry whose list variable had been bound to *another unbound
variable* as "still unbound" and waited for a determination, so
`|W| = |Y|` was never posted. The fix distinguishes "still the same
unbound variable" (`deref(list) == list`, only the length can have
changed) from "aliased" (the spine case, `|list| = |d|` through `d`'s
entry). This was reachable in CLM-0015 alone: `{ |L| = N, L = M,
|M| = 2 }` answered without binding `L`. `answer-soundness` is therefore
strengthened by this task, not merely kept; the regression test covers
both orientations of the binding.

**Two more findings.** Since `X .` can begin a concatenation, a clause
missing its `)` before the terminator (`human(X.`) is now reported at the
token after the `.`; two position tests were updated and the EBNF view
says so. And the 10 000-deep literal in the machine's stack test
overflowed its 64 MiB thread after the new lowering arm: lowering costs
about 6 KB of stack per nesting level in debug builds, most of it
iterator-adapter frames in `Lowerer::atom`, whose `collect` is now a
loop; the phase is back to the margin it had on `main` (between 32 and 64
MiB at that depth). An iterative lowering is a follow-up if deeper
literals matter.

**Pre-write checks.** The five docs in the task's `affects` are current:
the EBNF view and the README carry the production and a paragraph on
concatenation, `evaluator.md` the lowering and the printing, `solver.md` a
*List concatenation* bullet and the alias rule under *List lengths*,
`json-output.md` the note on constraint strings. `grammar-authority` held
in order (grammar, actions, example, EBNF view, README view).
`answer-soundness`: see above. Cross-repo impact is the `meta-impact`
note.

**The record is complete.** All three stages of
`lists-as-a-constraint-domain` are implemented; the record's status stays
`accepted` (it is append-mostly), and the three plans carry the
interpretations made along the way. The strings record
(`strings-as-character-lists-and-type-relations`) is the next accepted
design to implement, and the anonymous variable `_` the next incubation
note to write.

## Links

- Task: `docs/implementation/2026-11-09-clm-0016-list-concatenation.md`
- Design record: `docs/design/2026-11-09-lists-as-a-constraint-domain.md`
- Previous stages: `docs/journal/2026-11/2026-11-09-execution-clm-0014-list-notation.md`,
  `docs/journal/2026-11/2026-11-09-execution-clm-0015-list-length.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-fold-arithmetic-lists-strings.md`
- Commits `ae2fae1` (plan, ticket claim), `99982ae` (implementation),
  merged as `1e79d0c`
- PR: https://github.com/a1v0lut10n/claimr/pull/43
- Views updated: `docs/reference/grammar.md`, `docs/reference/json-output.md`,
  `README.md`, `docs/architecture/components/evaluator.md`,
  `docs/architecture/components/solver.md`
