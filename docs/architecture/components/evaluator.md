---
type: architecture-component
last-verified: 2026-11-09
decisions:
  - docs/design/2026-08-17-evaluator.md
  - docs/design/2026-08-17-arithmetic-in-terms.md
  - docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md
  - docs/design/2026-11-09-lists-as-a-constraint-domain.md
  - docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md
  - docs/design/2026-11-09-negation-disjunction-and-if-then-else.md
  - docs/design/2026-11-09-explanations-of-answers.md
  - docs/design/2026-11-09-aggregation-list-comprehensions-and-sum.md
  - docs/design/2026-11-09-numbers-numeric-variables-and-attribute-terms.md
aspects: [answer-soundness, exact-arithmetic]
---

# evaluator

`src/eval/` — runs Claimr programs: compiles the parsed AST into a runtime
program, resolves queries by SLD resolution over rational trees, and renders
answers in solved form. Public surface (re-exported from `claimr`):
`Program::compile` / `Program::compile_spanned`, `Program::queries`,
`Program::solve` → `Solutions` (a lazy iterator of `Answer`s;
`with_explanations()` makes each answer carry its proof tree, CLM-0023),
`Answer`, `ProofNode`, `Resolution`, `EvalError`.

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
  `post_eq_goal` / `post_dif_goal` (numeric when either operand denotes a
  number, else tree — CLM-0026) and `post_rel` (`< > <= >=`). A numeric
  variable the store determines is bound to its number, so head
  unification, indexing and every later goal see the number.
- **`compile.rs`** — `ast::Clause` → clause templates with pre-numbered
  variables (structure copying at instantiation; every occurrence of the
  anonymous variable `_` gets its own number, CLM-0020) indexed by predicate
  (functor, arity), each predicate carrying a **first-argument index**
  (CLM-0021): the principal functor of a clause's first argument — a
  constant, a number, a functor with its arity — maps to the positions of
  the clauses with that key merged with the variable-first clauses, in
  program order; `{c} => head.` desugars to `head :- {c}.`; constraint
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
  concatenation (*List concatenation* there). Lowering and instantiation
  walk terms with an explicit stack (CLM-0019), as do the clause printer
  and the drop of both term types, so the depth of a literal — a nested
  term, or a long list, which lowers to a chain as deep as it is long —
  costs no Rust stack: a 100 000-deep term and a 200 000-element list run
  on a 1 MiB thread. The one remaining recursion on the AST side is the
  derived `Debug` that `--parse` prints. The control constructs of
  CLM-0022 are templates too (`TGoal::Or`, `TGoal::IfThenElse`,
  `TGoal::Not`, with nested goal lists; recursion follows the source
  nesting only); after a body is lowered, `resolve_conditions` computes
  each condition's **external variables** — those also occurring in the
  clause outside the whole construct — and stores them with the construct's
  and the clause's text in a `CondCheck`, for the machine's groundness rule
  and its diagnostic. Only bodies with a construct pay for the clause text.
  Every clause keeps its source position and its text as written
  (CLM-0023), for explanations to cite. A **list comprehension**
  `<T | Body>` (CLM-0025) is lowered to a `TGoal::Collect` emitted before
  the goal it sits in, binding a fresh clause variable that replaces it in
  the goal; a comprehension in a template is collected at the end of the
  enclosing body, once per solution; the external-variable analysis treats
  the template's and the body's variables as the construct's, with the
  rest of the enclosing goal as outside; a comprehension in a head or a
  constraint fact is `EvalError::ComprehensionInHead`, positioned.
- **`machine.rs`** — the SLD machine: goals left to right, clauses in program
  order, chronological backtracking through choice points and the trail. A
  call whose first argument is bound visits only the clauses the index
  admits (CLM-0021; an unbound first argument, numeric or not, means all
  clauses), and a choice point holds that candidate list and the next
  position — none is pushed when one candidate remains. A join over fact
  tables keyed by an id is therefore linear in the facts.
  A call whose key is a **built-in** (`Program.builtins`: `atom_chars/2`,
  `number/1`, `atom/1`, `list/1`, CLM-0017) posts its constraint through
  the store and takes no choice point; every other call looks up clauses.
  **Body goals are instantiated lazily** (CLM-0013): at clause entry the
  clause's variables are allocated once into a shared frame and the head is
  built from it for unification; each body goal's terms are built from the
  frame when the goal reaches the front of the list, so arithmetic over
  values bound by earlier goals folds instead of defining solver rows.
  A query's variables are its frame, allocated at the start, and its goals
  are pending on it like a clause body's (since CLM-0022; the answer's
  variables are the named ones, `_` is not an answer variable and prints
  nothing of its own); **iterative** (explicit goal list and choice-point
  stack — resolution depth never uses the Rust stack). An undefined
  predicate has no clauses and fails.
  **Control constructs** (CLM-0022, design record
  `2026-11-09-negation-disjunction-and-if-then-else.md`): a choice point
  either retries clauses or resumes with a goal list (`Retry::Goals`: the
  second alternative of `A ; B`, the else branch of `C -> T ; E`, the
  continuation of `\+ G`). Opening a condition first applies the
  **groundness rule** — every external variable's term, dereferenced, holds
  no unbound variable and every numeric variable in it is determined by the
  solver (`Store::is_ground`, exact) — and otherwise stops the query with
  `EvalError::NonGroundCondition`; it then records the choice-point height,
  pushes the else branch (if any) as a goal-list choice point, and runs the
  condition's goals followed by a `Commit` goal. Reaching the commit
  finalizes the store as for an answer (a non-solution backtracks into the
  condition's alternatives; a non-linear residue stops the query as it would
  at an answer), truncates the choice-point stack to the recorded height,
  and continues with the then branch — or, for a negation, fails. The
  store, the solver and the answer printer are untouched by the constructs.
  **Collections** (CLM-0025): opening a `Collect` applies the groundness
  rule, pushes a `Retry::Finish` choice point holding the accumulator and
  runs the body followed by `Goal::CollectOne`; reaching that goal
  finalizes the store exactly, copies the template's instance out of the
  solution as a closed term (`Store::snapshot`: constants, numbers, a
  determined numeric variable as its value, compounds; an unbound variable
  or a cycle stops the query with `EvalError::NonGroundSolution`) and
  fails, so the search moves to the next solution; exhausting the body
  lands on the finishing choice point, which rebuilds the list from the
  copies on the restored heap, unifies it with the comprehension's variable
  and continues. The built-in `sum/2` posts `Store::post_sum`.
  **Explanations** (CLM-0023, design record
  `2026-11-09-explanations-of-answers.md`): on request
  (`Solutions::with_explanations`, the CLI's `--explain`, the REPL's
  `:explain`) the machine keeps a proof arena — one record per successful
  step: the clause that resolved a call (with the call's address), a
  constraint goal's two sides and operator, a built-in's call, a committed
  condition, a condition found without a solution — each under the record
  of the clause whose body it belongs to; every pending goal carries that
  parent. A choice point remembers the arena's length and backtracking
  truncates to it, as the trail does for bindings; a commit keeps the
  condition's records; an else branch or a negation's continuation records
  its `no-solution` leaf when taken. Off, nothing is recorded. At an
  answer the arena's goal instances are rendered by the answer's own
  printer (so `_N` names agree) and the tree is assembled bottom-up without
  recursion.
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
  `atom_chars(A, L)` (CLM-0017), and a pending sum as `sum(L, S)`
  (CLM-0025). An `Answer` may carry an `explanation`
  (CLM-0023): `ProofNode`s with the goal instance, the `Resolution`
  (`Fact`, `Rule`, `Constraint`, `Builtin`, `Committed`, `NoSolution`,
  `Collected` — a comprehension with the list it produced), the
  cited clause's position and text, and subgoals; `explanation_lines()`
  renders it as the indented tree the CLI prints, iteratively.

The CLI (`cli` component) loads a file and runs its queries in order,
`--limit N` capping answers per query, `--explain` printing each answer's
proof tree under it (and adding the `explanation` field under `--json`); a
runtime error stops it with exit 1.
The REPL (`src/repl.rs`, design record `2026-08-18-repl-interaction-model.md`)
recompiles the session program on every change and per query (programs are
small; an incremental compile is a later optimisation).

## Not yet (later stages)

Non-linear constraints beyond delayed products; integer/finite-domain
constraints; further built-ins; a delayed (`freeze`-style) goal, which would
let a negation wait for its external variables; indexing on other arguments;
heap and tableau garbage collection beyond trail-driven undo; tail-call
optimisation. The cut is deliberately absent: if-then-else and negation
cover its uses without its non-local effect (the design record of
CLM-0022).
