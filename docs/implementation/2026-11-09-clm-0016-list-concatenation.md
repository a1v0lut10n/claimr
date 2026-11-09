---
date: 2026-11-09
type: task
status: done
affects:
  - docs/reference/grammar.md
  - docs/reference/json-output.md
  - docs/architecture/components/evaluator.md
  - docs/architecture/components/solver.md
  - README.md
components: [grammar, parser, evaluator, solver, cli]
aspects: [grammar-authority, answer-soundness]
design: [docs/design/2026-11-09-lists-as-a-constraint-domain.md]
tags: [CLM-0016, lists, concatenation, solver, prolog-iii]
---

# CLM-0016: List concatenation `X . Y` (lists as a constraint domain, stage 3)

## Premise

Stages 1 and 2 (CLM-0014, CLM-0015) gave the notation and the length.
Stage 3 of the accepted record is Prolog III's approximated concatenation:
`X . Y` with a non-literal left operand denotes a list `Z` with
`|Z| = |X| + |Y|` posted at once, and the concatenation itself completes,
by elementwise unification, as soon as `|X|` is known — by instantiation of
`X`'s spine or by the simplex. It is never searched; `append/3` stays the
nondeterministic predicate. This gives the paper's idioms: last-element
access, the two-clause `reverse`, prefix tests as constraints, and the
periodic-sequence query `{ |Z| = 10, <1, 2, 3> . Z = Z . <2, 3, 1> }`.

## Scope

In: the general `Expr '.' Expr` production replacing the literal-left one;
`Expr::Concat` with its display, the literal-left case still building a
chain in the action; `TTerm::Concat` lowered by `Store::post_concat`; the
pending-concatenation entries in the store, their trailing, waking and
completion; a complete left operand building the chain at once; pending
concatenations printed as `Z = X . Y`; the five doc views; an example
with the paper's idioms.

Out: searching a concatenation (Prolog III's `enum`); strings and
`atom_chars/2` (the strings record); tableau truncation.

## Action items

1. **Grammar**: `| left=Expr '.' right=Expr {ConcatExpr, right, 0}`
   replaces `tuple=Tuple '.' tail=Expr`; `Tuple` stays its own rule so the
   action sees the tuple as a unit. Automaton (rustemo 0.9.1, 59
   productions, 92 states): after `Expr . Expr` the parser shifts on `.`
   and on every arithmetic operator; a tuple and a sum reduce before `.`;
   `-X . Y` is `(-X) . Y`. **Done.**
2. **AST and actions**: `Expr::Concat(Box<Expr>, Box<Expr>)`, Display
   `X . Y` at precedence 0 with the left operand parenthesised when it is
   itself a `.` form. `expr_concat_expr` keeps the stage 1 normalisations
   for a literal left tuple. **Done.**
3. **Compile**: `TTerm::Concat`; `build_with` lowers it with
   `Store::post_concat(x, y)`. **Done.**
4. **Store**: `Concat { z, x, y, pending }` entries in `concats`,
   `Waker::Concat`, `Undo::ConcatPending`, the count in `Mark`.
   `post_concat`: complete `X` → `chain(spine_heads(X), Y)`; open in `T`
   → fresh `Z`, `|Z| = |X| + |Y|` through `post_len` on all three and
   `assert_lin`, entry suspended on `T`, `settle`; bad → fail.
   `recheck_concat`: complete → close and `unify(Z, chain)`; open in `U` →
   re-suspend; bad → fail. `pending_concats()` for the printer. **Done.**
5. **Answers**: pass 7 of `render_answer` prints each pending concatenation
   that mentions a reachable variable as `Z = X . Y` among the
   constraints; `render_numeric` drops an equality that is exactly the
   implied `|Z| = |X| + |Y|` over three unbound list variables
   (`Store::length_svar`); with a complete operand the relation carries a
   constant and stays, which is only redundancy. **Done.**
6. **Example and tests**: `examples/concat.claimr` with golden answers
   (`last`, `has_prefix`, two-clause `reverse` both ways, palindromes with
   the length first, the periodic sequence, pending residuals, failures).
   Machine tests `list_concatenation_is_a_constraint` (19 queries),
   `list_concatenation_bounds_the_search`,
   `a_list_variable_aliased_to_another_shares_its_length` (regression,
   below); parse test `concatenation_is_right_associative_and_loosest`;
   JSON test `pending_concatenations_are_constraint_strings`. **Done.**
7. **Docs**: `grammar.md`, README grammar section, `evaluator.md`,
   `solver.md` (*List concatenation* bullet, the alias rule under *List
   lengths*), `json-output.md`; plan to what was built. **Done.**

## Found on the way

- **A stage 2 soundness bug.** `recheck_len` treated a length entry whose
  list variable had been bound to *another unbound variable* as "still
  unbound", so `|W| = |Y|` was never posted. It surfaced as the reversed
  `reverse(X, <1, 2, 3>)` not terminating after its answer: the pruning
  bound `|Y| = -1` never reached the simplex, and the derivation went on
  with one more pending concatenation per level. The fix distinguishes
  "still the same unbound variable" (`deref(list) == list`, only the
  length can have changed) from "aliased" (the spine case, `|list| = |d|`
  through `d`'s entry). The regression test covers both orientations of
  the binding. This was reachable in CLM-0015 alone (`{ |L| = N, L = M,
  |M| = 2 }` answered without binding `L`) and is fixed here.
- **Diagnostics position.** Since `X .` can begin a concatenation, a
  clause missing its `)` before the terminator (`human(X.`) is reported
  at the token after the `.`; two position tests updated, the EBNF view
  says so. This is the one place where the terminator and the operator
  being one token shows.
- **Stack margin of deep literals.** The 10 000-deep term in
  `deep_derivations_do_not_touch_the_rust_stack` overflowed its 64 MiB
  thread after the new lowering arm: lowering costs about 6 KB of stack
  per nesting level in debug builds, most of it iterator-adapter frames in
  `Lowerer::atom`. Replacing that `collect` with a loop brought the phase
  back under 64 MiB (it needs between 32 and 64 MiB at that depth, as on
  `main`). Lowering and Display still recurse per level; an iterative
  lowering is a follow-up if deeper literals matter.
- **Generator order.** `palindrome(P), { |P| = 3 }` enumerates palindromes
  forever before the constraint is reached, as any Prolog would; with the
  constraint first the query has one answer. The example is written that
  way; it is the ordinary discipline, not a defect.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (126 tests); every previous golden file
      byte-identical.
- [x] `aivolution lint .` clean; the five `affects` docs current.
