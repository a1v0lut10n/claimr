---
type: architecture-component
last-verified: 2026-11-09
decisions:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
  - docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md
  - docs/design/2026-11-09-lists-as-a-constraint-domain.md
  - docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md
aspects: [answer-soundness, exact-arithmetic]
---

# evaluator

`src/eval/` — runs Claimr programs: compiles the parsed AST into a runtime
program, resolves queries by SLD resolution over rational trees, and renders
answers in solved form. Public surface (re-exported from `claimr`):
`Program::compile` / `Program::compile_spanned`, `Program::queries`,
`Program::solve` → `Solutions` (a lazy iterator of `Answer`s), `EvalError`.

## Current state (evaluator stages 2–4 — CLM-0005, CLM-0006, CLM-0007)

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
  and re-checked when any is bound or determined; each check records the
  would-bind pairs, which give the disequation's **reduced form** for
  printing (`Z != W` for `f(Z) != f(W)`). `post_eq` / `post_dif` are
  transactional. Since CLM-0017 the store carries a clone of the program's
  **symbol table** (`Store::with_symbols`), so the atom that `atom_chars`
  spells can be interned at wake time; and it holds the delayed built-in
  constraints: **type relations** `atom(X)` / `list(X)` (pending on an
  unbound variable, decided on binding, failing on a numeric variable or
  on two kinds for one variable) and **`atom_chars(A, L)`** (decided when
  `A` is a constant or `L` a complete list of one-character constants,
  else pending on `A`, `L`'s open tail and its unbound elements). A
  variable held by either cannot become numeric (`unknown_of` fails).
  `number(X)` is `post_number`: a tree fails, an unbound variable becomes
  numeric. The store's numeric half — solver variables for numeric heap
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
  by `build_with` (a string literal is already the `'<>'` chain of its
  characters in the AST, CLM-0017): a node whose operands are all numbers at that moment
  folds to a `Num` cell (CLM-0013, no solver variable), any other node
  posts its defining constraint immediately (part of the clause's
  constraint system, as in Prolog III's abstract machine) and fails the
  step if that is unsatisfiable. `Number` literals unify structurally
  (exact equality). Lists (CLM-0014) lower to chains of `'<>'(H, T)` cells
  ending in the constant `'<>'` or in the written tail, so `<a, b> . T` and
  a hand-spelled `'<>'(a, '<>'(b, T))` are one term; a clause head named
  `'<>'` is rejected (`EvalError::ReservedName`, the one compile error that
  carries a source position, joined by `ReservedBuiltin` for the four
  built-in names and arities of `docs/reference/builtins.md`). `|L|` (CLM-0015) stays in the template as
  `TTerm::Len` and is lowered at instantiation by `Store::post_len`: a
  complete list folds to its length, an open one yields a numeric variable
  (see [`solver.md`](solver.md), *List lengths*). `X . Y` (CLM-0016) is
  `TTerm::Concat`, lowered by `Store::post_concat`: the chain when `X`'s
  spine is complete, else a fresh list variable with a pending
  concatenation (*List concatenation* there). Lowering recurses once per
  nesting level; its per-level stack cost in debug builds is what bounds
  the depth of literal terms, not the machine.
- **`machine.rs`** — the SLD machine: goals left to right, clauses in program
  order, chronological backtracking through choice points and the trail.
  A call whose key is a **built-in** (`Program.builtins`: `atom_chars/2`,
  `number/1`, `atom/1`, `list/1`, CLM-0017) posts its constraint through
  the store and takes no choice point; every other call looks up clauses.
  **Body goals are instantiated lazily** (CLM-0013): at clause entry the
  clause's variables are allocated once into a shared frame and the head is
  built from it for unification; each body goal's terms are built from the
  frame when the goal reaches the front of the list, so arithmetic over
  values bound by earlier goals folds instead of defining solver rows.
  Query goals are built eagerly (their variables are the answer's);
  **iterative** (explicit goal list and choice-point stack — resolution depth
  never uses the Rust stack). An undefined predicate has no clauses and fails.
  `Solutions::with_interrupt(Arc<AtomicBool>)` installs a flag polled every
  256 steps (the REPL's Ctrl-C); `Solutions::interrupted()` reports a stop
  through it; `Solutions::may_continue()` tells a caller whether asking for
  another answer could still find one (choice points remain) — what the
  REPL's `;` prompt is based on.
  A step is taken only if head unification, the clause's arithmetic
  definitions and any constraint goals reached leave the store satisfiable;
  before an answer is yielded `Store::finalize` runs (exact determination
  and disequation checks; a non-linear residue stops the query with
  `EvalError::NonLinear`, exposed by `Solutions::error`).
- **`project.rs`** — answer projection and simplification (stage 4): the
  visible numeric store as a residual system over alias-class roots (bounds
  with non-public definitions expanded, definitions of public defined
  variables, pending numeric disequations), restricted to constraints
  connected to public variables; internal variables eliminated by Gaussian
  substitution (equalities) then Fourier–Motzkin (inequalities, cheapest
  variable first, within `FM_BUDGET` = 256 constraints — beyond it the
  variable survives and is named `_N`); disequations follow the
  substitution and are dropped only when unlinked to anything public;
  normalisation to integer variable coefficients, duplicate removal, and
  exact removal of every inequality entailed by the rest (a fresh
  `Simplex` per candidate); equalities oriented to solved form.
- **`answer.rs`** — answers in **solved form**: `X = t` per bound query
  variable (in variable order; a determined numeric variable prints its
  value, also inside terms), aliases `A = B`, then pending tree
  disequations reachable from the query variables in reduced form when
  there is one (internal-only disequations are projected away — sound over
  an infinite universe), then equations for named cyclic nodes (`X = f(X)`,
  `_1 = …`), then the projected numeric system: fixed public attribute
  values, equalities `Y = X + 1`, inequalities (`X > Y`, `X >= 2`,
  `X + Y <= 10`; lower bounds first), numeric disequations (`X != 3`), all
  in a deterministic order; `true` when nothing remains. Public numeric
  variables are the numeric variables reachable from the query and the
  attribute terms the query created *or looked up*. Iterative printer; a
  `'<>'/2` spine prints in list notation (`<1, 2, 3>`, `<1, 2> . T`, `<>`),
  stopping at a cyclic cell so a cyclic list is an equation (`X = <1> . X`).
  The length of a reachable unbound list variable is a public numeric
  variable named `|T|` (CLM-0015); its implicit bound `|T| >= 0` is not
  printed. A pending concatenation that mentions something the query can
  see prints as `Z = X . Y` among the constraints (CLM-0016), and the
  length relation it implies is dropped when it appears as such. Pending
  built-ins print in goal form after them: `atom(X)`, `list(X)`,
  `atom_chars(A, L)` (CLM-0017).

The CLI (`cli` component) loads a file and runs its queries in order,
`--limit N` capping answers per query; a runtime error stops it with exit 1.
The REPL (`src/repl.rs`, design record `2026-08-18-repl-interaction-model.md`)
recompiles the session program on every change and per query (programs are
small; an incremental compile is a later optimisation).

## Not yet (later stages)

Non-linear constraints beyond delayed products; integer/finite-domain constraints; a
REPL; cut, negation, disjunction, built-ins, first-argument indexing, heap
and tableau garbage collection beyond trail-driven undo, tail-call
optimisation.
