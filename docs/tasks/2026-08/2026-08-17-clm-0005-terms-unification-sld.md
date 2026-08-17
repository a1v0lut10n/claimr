---
date: 2026-08-17
type: task
status: planned
affects:
  - docs/architecture/README.md
  - README.md
  - CLAUDE.md
components: [evaluator, cli, parser]
aspects: [answer-soundness, grammar-authority]
design:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
tags: [CLM-0005, evaluator, unification, resolution, evaluator-stage-2]
---

# CLM-0005: Terms, unification over rational trees, and the SLD machine

## Objective

Stage 2 of the evaluator plan
([design record](../../design/2026-08-17-evaluator.md), D1, D2, D3, D7): make
claimr *run* pure programs — facts, rules, `=`/`!=` on trees, queries — with
answers printed in solved form from the CLI. Everything numeric (numeric
relations, arithmetic terms, attribute terms, the linear store) is stage 3
and must be rejected cleanly, not half-implemented.

## Context

The front end is complete (rustemo grammar with arithmetic and comments,
exact `Number`s). The evaluator record fixes the term model (rational trees,
no occurs check, bind-before-descend), control (SLD, depth-first,
left-to-right, program order, trail over bindings and store), the meaning of
`=>` (sugar) and constraint facts (initial store), `dif` by trial
unification with suspension (Prolog II's `geler` mechanism), and answers as
the store in solved form. This task builds exactly that skeleton so stage 3
can plug the numeric store into an engine that already backtracks,
suspends and prints.

## Deliverables

1. **`src/eval/` — the `evaluator` component**, an iterative machine (no
   Rust recursion for resolution depth; explicit goal stack and choice-point
   stack):
   - `symbol.rs`: interned symbols (functor/constant names).
   - `term.rs`: runtime terms — `Var(VarId)`, `Number(Number)`,
     `Const(Symbol)`, `Compound(Symbol, Vec<Term>)`; a term store/heap and
     variable cells with bindings.
   - `store.rs` (the tree part of the constraint store, D5): variable
     bindings, the **trail** (undo records for bindings, suspensions and
     any later solver state), and **suspension** — each unbound variable
     carries a wake-up list; binding fires it; the trail undoes it. Built
     once here, reused by stage 3.
   - `unify.rs`: unification over **rational trees**, no occurs check,
     **bind-before-descend** on compound/compound pairs (trailed) so cyclic
     terms terminate; `Number`s unify by exact equality; unification is a
     store operation (fails ⇒ store unsatisfiable ⇒ the step is not taken).
   - `dif.rs`: `!=` on trees by **trial unification**: fail ⇒ satisfied and
     dropped; success with no new binding ⇒ violated; otherwise undo and
     suspend on the would-be-bound variables, re-checking on wake-up.
   - `compile.rs`: `ast::Clause` → runtime program. Clauses hold terms with
     pre-numbered variables and a variable count so instantiation is a bump
     of the variable base (structure copying). `Implication` desugars to
     `head :- {c}.`; `ConstraintFact`s form the **initial store**;
     `Query` clauses are collected in order. `Expr::Ident` → `Const`,
     `Expr::Atom` → `Compound`, `Expr::Number` → `Number`. **Stage-2
     boundary**: any numeric relation (`< > <= >=`), any arithmetic term
     (`Neg`/`Binary`), and any `=`/`!=` whose operand is a `Number`-typed
     expression *other than a bare literal* is rejected at compile time
     with a positioned `EvalError::Unsupported("numeric constraints arrive
     in stage 3")` — bare `Number` literals unify structurally and are fine.
   - `machine.rs`: SLD resolution — goals left to right, clauses in program
     order (first-argument indexing optional, not required), chronological
     backtracking via choice points + trail; a resolution step succeeds only
     if unification and the clause's `{ … }` goals leave the store
     satisfiable (`answer-soundness`). An undefined predicate simply has no
     clauses (fails), as in Marseille Prolog; no existence error.
   - `answer.rs`: answers as the store restricted to the query's variables
     in **solved form** — `X = t` for bound variables (cyclic terms print as
     equations, `X = f(X)`), remaining disequations shown (`Y != a`),
     internal variables named `_1`, `_2`, …; `true` when nothing remains.
     A `Solutions` iterator yields answers on demand (backtracking between
     calls) so callers can stop early.
   - Public API in `src/lib.rs`: `Program::compile(&[Clause]) ->
     Result<Program, EvalError>`, `program.queries()`,
     `program.solve(&Query) -> Solutions`, `Answer: Display`, `EvalError`
     (`Unsupported`, `InitialStoreUnsatisfiable`, …) with positions where the
     AST carries them (add spans to `ast::Clause` if needed — the parser can
     supply them from rustemo's context; do the minimum).
2. **CLI** — `claimr <file>` now **runs** the program: load (parse, compile,
   initial store), then each `?-` query in order, printing the query and
   its answers one per line, `false` if none. `--parse` keeps today's AST
   dump. `--limit N` caps answers per query (default unlimited; documented
   as the escape hatch for infinite answer sets). Exit code: 0 on success,
   1 on load error (with the positioned message), 2 on usage/IO error.
3. **Tests**:
   - unification: symbols, numbers (exact: `0.5 = 1/2` *is not* tested here
     — `1/2` is arithmetic and stage 3; `0.50 = 0.5` is), compound,
     variable aliasing, failure cases, **cyclic**: `X = f(X)` then
     `X = f(f(X))` succeeds; `X = f(X), Y = f(Y), X = Y` succeeds;
     `X = f(X, a), X = f(X, b)` fails; trail undo restores state exactly.
   - dif: `X != a` then `X = a` fails; `X = a` then `X != a` fails;
     `X != Y` then `X = f(Z), Y = f(W)` suspends, then `Z = W` fails,
     alternatively `Z != W`… ; satisfied difs are dropped from answers.
   - resolution: family/ancestor programs (multiple answers, backtracking),
     recursion over `cons/nil` lists (append, member, reverse — no list
     syntax yet), rules with `{ X = f(Y) }` and `{ X != Y }` goals, `=>`
     sugar behaving as a rule, constraint facts populating the initial
     store, unsatisfiable initial store rejected, undefined predicate ⇒
     `false`, `--limit` on an infinite program (`nat(s(X)) :- nat(X).`),
     deep recursion (10⁵ steps) without stack overflow.
   - stage-2 boundary: programs with `<`, arithmetic, or attribute terms
     are rejected with `Unsupported` at load, with a position.
   - **golden runs**: `tests/run_examples.rs` executes every
     `examples/<name>.claimr` that has an `examples/<name>.answers` sibling
     and compares output; add `.answers` for new pure examples
     (`family.claimr`, `lists.claimr`, `dif.claimr`, `cyclic.claimr`);
     numeric examples get theirs in stage 3.
   - `cargo clippy --all-targets -- -D warnings` clean.
4. **Docs** — every path in `affects`, plus new architecture docs:
   - `docs/architecture/components/evaluator.md` (schema
     `architecture-component`, `decisions:` the two design records) — what
     exists after this task, incl. the stage-2 boundary; the `evaluator`
     row in `docs/architecture/README.md` drops *(planned)*.
   - `docs/architecture/aspects/answer-soundness.md` (schema
     `architecture-aspect`, `applies-to: [evaluator, solver]`) stating the
     two testable invariants: no resolution step on an unsatisfiable store;
     no printed answer whose store is unsatisfiable.
   - `README.md`: status ("evaluation: pure programs; constraint solving in
     progress"), Usage (`claimr file` runs; `--parse`, `--limit`), an
     answers example. `CLAUDE.md`: "Only the parser exists so far" → current
     state; commands (`cargo run -- examples/family.claimr`).

## Explicitly out of scope

- Numeric relations, arithmetic terms, attribute terms, the linear store,
  numeric typing — **stage 3** (rejected at load here, never silently
  ignored).
- Answer projection beyond "restrict to query variables + their
  disequations"; variable elimination — stage 4.
- REPL, `--interactive`, cut, negation, disjunction, if-then-else,
  built-ins, I/O, first-argument indexing, garbage collection of the term
  heap beyond trail-driven undo, tail-call optimisation.
- Any grammar change (none is needed; if one turns out to be, it goes
  grammar-first in its own task).

## Completion

Before setting `status: done`: every path in `affects` describes the
post-task state, the two new architecture docs exist and lint clean,
`cargo test`/clippy/`aivolution lint` are clean, and a journal entry links
back to this task via `tasks:`. Then freeze this file.
