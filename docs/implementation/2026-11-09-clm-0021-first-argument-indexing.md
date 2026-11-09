---
date: 2026-11-09
type: task
status: done
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
   var_only }`; `ArgKey`; `arg_key` of a head's first argument (`None` for
   a variable or an arithmetic term); `Pred::push` keeps the key lists in
   program order and `Pred::finish` adds the variable-first clauses that
   preceded a key's first clause; `Program::candidates`. **Done.**
2. **Machine** (`src/eval/machine.rs`): `first_arg_key` from the heap (an
   unbound variable, numeric or not, is `None`); `try_clauses` walks a
   candidate slice; `ChoicePoint { candidates: &'p [usize], next }`; a
   choice point only when a candidate remains. **Done.**
3. **Tests**: `first_argument_indexing_preserves_clause_order_and_answers`
   (ten queries over mixed first arguments, including `p(0.5, N)` matching
   `p(1/2, 6)` and a determined numeric variable treated as unbound, where
   D4's attribute-term matches appear exactly as on `main`);
   `deterministic_lookups_leave_no_choice_point`. 140 tests; every golden
   file byte-identical. **Done.**
4. **Measurement** (release build, all answers, answer files byte-identical
   to `main`'s):

   | Facts per table | `main`  | indexed |
   |-----------------|---------|---------|
   | 1 820           | 0.71 s  | 0.03 s  |
   | 3 640           | 1.83 s  | 0.05 s  |
   | 7 280           | 7.13 s  | 0.09 s  |
   | 14 560          | 28.24 s | 0.18 s  |

   The join is linear now. **Done.**
5. **Docs**: `evaluator.md` (`compile.rs` index, `machine.rs` lookup, the
   *Not yet* list now names indexing on other arguments instead); this
   plan. **Done.**

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (140 tests); every golden file byte-identical.
- [x] The join scales linearly.
- [x] `aivolution lint .` clean; `evaluator.md` current.
