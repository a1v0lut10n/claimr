---
type: architecture-component
last-verified: 2026-08-18
decisions:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
aspects: [answer-soundness, exact-arithmetic]
---

# evaluator

`src/eval/` — runs Claimr programs: compiles the parsed AST into a runtime
program, resolves queries by SLD resolution over rational trees, and renders
answers in solved form. Public surface (re-exported from `claimr`):
`Program::compile` / `Program::compile_spanned`, `Program::queries`,
`Program::solve` → `Solutions` (a lazy iterator of `Answer`s), `EvalError`.

## Current state (evaluator stages 2–3 — CLM-0005, CLM-0006)

- **`symbol.rs`** — interned functor/constant names.
- **`store.rs`** — the tree part of the constraint store: a cell heap
  (`Var` / `Const` / `Num` / `Struct`), the **trail** (address–value undo
  records; `Mark` + `undo_to` restore any earlier state exactly), the
  **suspension** mechanism (each unbound variable carries a wake-up list;
  binding fires it; the trail undoes it), **unification over rational
  trees** with no occurs check — compound/compound pairs bind-before-descend
  so cyclic terms terminate — and **disequations** (`dif`) decided by trial
  unification: fail ⇒ satisfied and dropped; success without a new binding ⇒
  violated; otherwise suspended on the variables that would have been bound
  and re-checked when any is bound or determined. `post_eq` / `post_dif` are
  transactional. The store's numeric half — solver variables for numeric heap
  variables, attribute terms, numeric disequations, delayed products,
  `finalize` before answers — is documented in
  [`solver.md`](solver.md); the goal-level entry points are
  `post_eq_goal` / `post_dif_goal` (numeric or tree by operand) and
  `post_rel` (`< > <= >=`).
- **`compile.rs`** — `ast::Clause` → clause templates with pre-numbered
  variables (structure copying at instantiation) indexed by predicate
  (functor, arity); `{c} => head.` desugars to `head :- {c}.`; constraint
  facts form the **initial store**, validated satisfiable at compile time;
  `?-` clauses become `Query`s in source order. Arithmetic stays in the
  templates (`TTerm::Neg` / `TTerm::Arith`) and is lowered at instantiation
  by `build`, which posts each node's defining constraint immediately (part
  of the clause's constraint system, as in Prolog III's abstract machine)
  and fails the step if that is unsatisfiable. `Number` literals unify
  structurally (exact equality).
- **`machine.rs`** — the SLD machine: goals left to right, clauses in program
  order, chronological backtracking through choice points and the trail;
  **iterative** (explicit goal list and choice-point stack — resolution depth
  never uses the Rust stack). An undefined predicate has no clauses and fails.
  A step is taken only if head unification, the clause's arithmetic
  definitions and any constraint goals reached leave the store satisfiable;
  before an answer is yielded `Store::finalize` runs (exact determination
  and disequation checks; a non-linear residue stops the query with
  `EvalError::NonLinear`, exposed by `Solutions::error`).
- **`answer.rs`** — answers in **solved form**: `X = t` per bound query
  variable (in variable order; a determined numeric variable prints its
  value, also inside terms), aliases `A = B`, then pending tree
  disequations reachable from the query variables (internal-only
  disequations are projected away — sound over an infinite universe), then
  equations for named cyclic nodes (`X = f(X)`, `_1 = …`), then the numeric
  store the query can see (see [`solver.md`](solver.md#answer-rendering-stage-3-form));
  `true` when nothing remains. Iterative printer.

The CLI (`cli` component) loads a file and runs its queries in order,
`--limit N` capping answers per query; a runtime error stops it with exit 1.

## Not yet (later stages)

Answer projection / variable elimination beyond "what the query can see"
and disequation simplification to reduced form (stage 4); non-linear
constraints beyond delayed products; integer/finite-domain constraints; a
REPL; cut, negation, disjunction, built-ins, first-argument indexing, heap
and tableau garbage collection beyond trail-driven undo, tail-call
optimisation.
