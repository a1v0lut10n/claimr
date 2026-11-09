---
date: 2026-11-09
type: task
status: in-progress
affects:
  - docs/architecture/components/evaluator.md
components: [evaluator]
aspects: [answer-soundness]
design: []
tags: [CLM-0021, indexing, machine, performance]
---

# CLM-0021: First-argument clause indexing

## Premise

Graduated from the incubation note of 2026-11-08 (moved here). Clauses are
grouped by predicate, but within a predicate the machine tries every
clause in order: a call whose first argument is bound, `n(C, N)` with `C`
known, still builds and unifies the head of every `n` clause. A join over
fact tables keyed by an id,

```
violation(C) :- k(C, K), n(C, N), { K > N }.
```

costs O(F²) over F facts with no arithmetic involved. Fact tables keyed by
an id are exactly the shape of the integrity-checker data, and the join is
how rules over them are written. With the solver's row growth gone
(CLM-0013, CLM-0018), this is the cost that dominates such queries.

## Decision (no design record: an implementation choice with no semantic
effect)

Each predicate's clauses are indexed by the principal functor of their
first argument: a constant, a number, or a functor with its arity. A call
whose first argument is bound to one of those visits only the clauses
with that key together with the clauses whose first argument is a variable
(or an arithmetic term, whose value is not known statically), **in program
order**, so clause order — the language's semantics — is untouched. A call
with an unbound first argument, or a nullary predicate, visits every
clause as before. A clause that the index skips could not have unified: a
constant, a number and a compound never unify with one another, and two
constants, two numbers or two compounds unify only if their keys agree.
When one candidate remains, no choice point is pushed, which also cuts
memory and undo work on deterministic lookups.

## Action items

1. **Compile** (`src/eval/compile.rs`): `Pred { clauses, all, by_key,
   var_only }` per predicate; `ArgKey` (`Const`, `Num`, `Struct(f,
   arity)`) from a head's first argument, `None` for a variable or an
   arithmetic term; `by_key` lists merge the key's clauses with the
   variable-first clauses in program order; `Program::candidates(key,
   first: Option<ArgKey>) -> &[usize]`.
2. **Machine** (`src/eval/machine.rs`): the call's first argument's key
   from the heap (an unbound variable, numeric or not, means "all");
   `try_clauses` walks a candidate slice from a position; `ChoicePoint`
   stores the slice (`&'p [usize]`) and the next position instead of the
   key and clause index; a choice point is pushed only when a candidate
   remains.
3. **Tests**: clause order preserved under indexing (mixed constant,
   variable and compound first arguments, answers in program order);
   numbers as keys (exact: `p(1/2)` matches `p(0.5)`); a call with an
   unbound first argument sees every clause; a determined numeric variable
   as first argument is treated as unbound and still matches; choice
   points not pushed for deterministic lookups (through
   `Solutions::may_continue`); every golden example unchanged.
4. **Measurement**: the join above over 1 820, 3 640, 7 280 and 14 560
   facts, before and after.
5. **Docs**: `evaluator.md` (`compile.rs` index, `machine.rs` lookup; the
   *Not yet* list loses first-argument indexing); this plan.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every golden file byte-identical.
- [ ] The join scales linearly.
- [ ] `aivolution lint .` clean; `evaluator.md` current.
