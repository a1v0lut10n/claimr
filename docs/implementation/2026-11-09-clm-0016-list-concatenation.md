---
date: 2026-11-09
type: task
status: in-progress
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
   replaces `tuple=Tuple '.' tail=Expr`. Automaton check (rustemo 0.9.1,
   59 productions, 92 states): after `Expr . Expr` the parser shifts on
   `.` (right associative) and on every arithmetic operator (`.` is
   loosest); a tuple and a sum reduce before `.`; unary minus binds
   tighter (`-X . Y` is `(-X) . Y`). The terminator never meets the
   operator: an `Expr` is still never followed by the clause's `.`.
2. **AST and actions**: `Expr::Concat(Box<Expr>, Box<Expr>)`, Display
   `X . Y` at precedence 0, left operand parenthesised when it is itself
   a concatenation or a tailed list. The action keeps the stage 1
   normalisations when the left operand is a literal tuple: `<a> . T` is
   `List { items, tail }`, `<a> . <b> . T` one chain, `<> . T` is `T`.
3. **Compile**: `TTerm::Concat`; `build_with` lowers it with
   `Store::post_concat(x, y)`, which returns the address of `Z`.
4. **Store**: `post_concat` walks `X`'s spine. Complete after `k` cells:
   build the chain of those `k` heads ending in `Y` and return it, no
   constraint (the operation face; this is also the head pattern
   `reverse(<E> . X, Y . <E>)` when `Y` is known). Cycle or non-list:
   fail. Open after `k` cells in `T`: `Z` is a fresh variable; post
   `|Z| = |X| + |Y|` through `post_len` on all three (so a non-list `Y`
   fails here and a length that becomes determined expands its list, per
   stage 2); register `Concat { z, x, y, pending }` suspended on `T`
   through a new `Waker::Concat`. When `T` is bound: re-walk `X`; complete
   → unify `Z` with the chain of `X`'s heads ending in `Y` and close the
   entry; still open in `U` → re-suspend on `U`; bad → fail. Determination
   of `|X|` by the simplex reaches here through stage 2's expansion of
   `T`, which binds it. `finalize` leaves pending concatenations alone
   (unlike products, they are a sound residual). Entries and flags are
   trailed; `Mark` carries the count.
5. **Answers**: a pending concatenation whose terms mention a variable
   reachable from the query prints as the residual `Z = X . Y` in the
   constraint list (`Z`, `X`, `Y` rendered as terms, so `<a> . T` shows as
   such). The length relation `|Z| = |X| + |Y|` it implies is dropped from
   the printed numeric system when it matches exactly; otherwise it
   stays, which is only redundancy.
6. **Example and tests**: `examples/concat.claimr` with golden answers:
   `last/2`, `has_prefix/2`, the two-clause `reverse/2` both ways (the
   reversed direction terminates because the length bound prunes the
   search), the periodic sequence, a pending concatenation in an answer,
   non-list and cycle failures. Machine tests for the same and for undo
   across backtracking; parse test for associativity and precedence and
   the display; JSON test for a residual concatenation.
7. **Docs**: `grammar.md`, README grammar section, `evaluator.md`,
   `solver.md` (concatenation entries beside lengths), `json-output.md`;
   plan to what was built.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; all previous golden files byte-identical.
- [ ] `aivolution lint .` clean; the five `affects` docs current.
