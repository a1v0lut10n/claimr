---
date: 2026-08-17
type: design
status: accepted
components: [evaluator, solver, parser, cli]
aspects: [answer-soundness, exact-arithmetic, grammar-authority]
tags: [evaluator, solver, resolution, constraints, prolog-iii]
---

# The evaluator: SLD resolution over rational trees with an exact linear constraint store

## Context

Claimr has a parser (rustemo-generated, `src/parser/claimr.rustemo`) and an
exact numeric type (`Number`, arbitrary-precision rationals — see
`docs/design/2026-08-17-exact-rational-arithmetic.md`), but nothing that runs
a program. The catalyst note `docs/catalyst/2026-08-17-evaluator.md` states
the idea — resolution plus an incremental constraint store, with residual
constraints as first-class parts of an answer — and lists the questions this
record must settle: the runtime term model, the meaning of `{c} => head.`,
search strategy and occurs check, the solver, and where answers surface.

The reference design is Prolog III (Colmerauer, *An Introduction to
Prolog III*, CACM 1990). The relevant facts from the paper:

- Its **abstract machine** state is `(W, t0 t1…tn, S)` — variables of
  interest, remaining goals, constraint system. Applying a rule
  `s0 → s1…sm, R` yields `(W, s1…sm t1…tn, S ∪ R ∪ {t0 = s0})`, and the
  transition is allowed *only if the new system has a solution*.
  Unification is not a separate mechanism: it is the equation `t0 = s0`
  added to the store.
- The **domain** is finite-or-infinite (rational) trees, with real numbers,
  Booleans, characters and identifiers as leaves; "the set of nodes of the
  tree can be infinite". Constraints on trees are `=` and `≠`.
- Numeric relations (`<`, `≤`, `=`, `≠`, linear arithmetic) are defined only
  on numbers; when an operand is not a number "the result … is [not]
  defined" and the constraint has no solution — failure, not an error.
- **Answers** are the store in **solved form**: "a solvable system such
  that, for every variable x, the solution of S on {x} is explicitly given,
  whenever this solution is unique." Every state with no goals left is
  simplified and "presented as an answer".
- Control is Prolog's: goals left to right, rules in program order, a
  two-stack machine "explor[ing] the search space … via backtracking";
  arithmetic in infinite precision.

Where claimr differs from Prolog III today: its constraint blocks
`{ … }` admit **compound terms as numeric operands** (`{ age(socrates) > 70 }`,
`eligible(X) :- { age(X) >= 18 }.`), it has an `Implication` clause form
`{c} => head.`, it has `ConstraintFact` clauses at top level, and its
constraint expressions have **no arithmetic yet** (`expr relop expr` only).
Each of these needs a meaning.

Two further sources cover the layer the CACM paper leaves implicit —
Prolog III's kernel is Prolog II's kernel with the Boolean and numeric
solvers added: Colmerauer, *Prolog and Infinite Trees* (1982), the
theoretical model of equations over infinite trees (solved form via
compaction, variable elimination, anteposition, confrontation, splitting),
which states that termination of occurs-check-free unification "is an open
problem which is not discussed here"; and Van Caneghem, *L'anatomie de
Prolog* (InterÉditions 1986, from his 1984 thèse *L'Anatomie de Prolog II*),
the implementation account that closes that gap — bind-before-descend
unification, `dif` by trial unification and suspension, `geler` (freeze) as
the suspension mechanism, and the two-stack machine with a restoration
stack (trail).

## Decision

### D1 — Runtime term model: rational trees, distinct from the parse AST

The evaluator works on a runtime `Term` type, not on `ast::Expr`:

- `Term::Var(VarId)` — a logic variable, freshly numbered per clause
  instance (clauses are renamed apart on each use, as in the abstract
  machine).
- `Term::Number(Number)` — an exact rational.
- `Term::Const(Symbol)` — a nullary identifier (`socrates`); the parser's
  `Expr::Ident`.
- `Term::Compound(Symbol, Vec<Term>)` — `f(t1,…,tn)`; the parser's
  `Expr::Atom`.

Terms are **rational (possibly infinite) trees**, as in Prolog II/III:
unification has **no occurs check** and terminates on cyclic structures by
Van Caneghem's rule — when two compound terms with the same functor and
arity meet, **bind one node to the other (trailed) *before* descending into
the arguments**, so a revisit of the same pair finds them already identical
and stops (this is how "confrontation" in Colmerauer's model is
implemented). Cyclic terms need no special printer: an answer is a system of
equations in solved form (D7), which is the finite representation of a
rational tree (`X = f(X)`). This is the reference design's core domain;
retrofitting it later would change the meaning of programs, whereas adopting
it now costs only care in one algorithm.

Symbols are interned; a compiled clause holds terms plus its variable count
so instantiation is a bump of the variable base — structure copying, as in
Marseille's Prolog II, rather than Edinburgh-style structure sharing.

### D2 — Resolution: SLD, depth-first, left-to-right, chronological backtracking

Goals are selected left to right, clauses tried in program order, with
chronological backtracking realised by a **trail** (address–value undo
records — the *pile de restauration* of Van Caneghem's two-stack machine,
which Prolog III's kernel kept) over the bindings *and the constraint
store*. This is Prolog III's control and the one every Prolog programmer
expects; fair or breadth-first search is not a goal.

A resolution step **succeeds only if the constraint store remains
satisfiable** after adding the head/goal equation and the clause's
constraints. Unification is executed by the store's equality machinery
(D5), so bindings and numeric constraints are never out of step
(`answer-soundness`).

### D3 — Program clauses and their meaning

- `Fact` / `Rule` / `ConstraintRule` — as in Prolog III: `head :- goals`,
  where a `{ … }` goal adds constraints to the store at that point in the
  body (left-to-right; constraints are checked as early as they appear).
- **`Implication` `{c} => head.` is syntactic sugar for the constraint rule
  `head :- {c}.`** — a rule usable only when `c` is consistent with the
  store, exactly as the README describes ("syntactic sugar"). It is
  desugared when the program is compiled; the AST variant stays. The
  alternative — a forward-chaining production rule that *asserts* `head`
  whenever the store entails `c` — would need a second engine (forward
  inference, entailment triggers) with different semantics; rejected for
  now, and if wanted later it gets its own syntax and record.
- **`ConstraintFact` `{c}.` is a global constraint**: all constraint facts
  are added to the **initial store** when the program is loaded, before any
  query runs. If they are jointly unsatisfiable, loading fails with a
  diagnostic (the program has no models).
- `Query` clauses in a program file are **run in order at load time**, each
  printing all its answers (D7). The same path serves a future REPL.

### D4 — Numeric relations and *attribute terms*

`<`, `>`, `<=`, `>=` are **numeric relations** on the exact rationals. Their
operands are:

- a `Number` — itself;
- a `Var` — the variable becomes *numeric-typed*: it may only ever be bound
  to a number (binding it to any other tree makes the store unsatisfiable,
  as Prolog III's `+` "constrain[s] x and y to denote numbers");
- any other term (`age(socrates)`, `age(X)`, or a bare constant `x`) — an
  **attribute term**: it denotes an *unknown rational* named by that term.
  Attribute terms are congruent under equality: two attribute terms whose
  trees are equal (after bindings) denote the same unknown, so a constraint
  on `age(X)` applies to `age(socrates)` once `X = socrates`. The store
  keeps one solver variable per distinct attribute term and merges them
  (adds an equation) whenever unification makes two such terms equal.

This is what the README's flagship examples mean, and it is a conservative
extension of Prolog III (uninterpreted functions from trees to Q). Without
it, `{ age(socrates) > 70 }` would be unsolvable and the language's own
examples meaningless. Attribute terms are *not* goals: `age(socrates)` in
`{ … }` never calls a predicate `age/1`.

`=` and `!=` are **general**: on trees `=` is unification (an equation in
the store) and `!=` a **disequation** — Prolog II's `dif`, Prolog III's
`≠` — decided by **trial unification** (Colmerauer's criterion, Van
Caneghem's implementation): unify the two sides speculatively; if that
*fails*, the disequation is satisfied and dropped; if it succeeds *without
binding any variable*, the sides are already equal and the store is
unsatisfiable; otherwise undo the trial bindings and **suspend** the
disequation on the variables that would have been bound, re-running the
check when any of them is bound. When both sides are numeric (numbers,
numeric variables, attribute terms) `=`/`!=` are the numeric
equality/disequality of D5, where a determined numeric variable counts as
bound.

### D5 — The constraint store: linear arithmetic over Q, exact, incremental

The store holds, over exact rationals (`exact-arithmetic`):

- **tree equations** (bindings via union-find, cycle-safe) and **tree
  disequations** (delayed);
- **linear equations** over numeric variables (numeric-typed `Var`s and
  attribute-term unknowns), kept in Gauss–Jordan solved form;
- **linear inequalities**, kept feasible by an **incremental simplex** in
  the CLP(Q) tradition (Prolog III; Holzbaur's CLP(Q,R)) with a cycle-free
  pivoting rule (Bland's rule), used both for satisfiability and to **detect
  determined variables** — a numeric variable with a single possible value
  is bound to it, so tree unification, disequations and answers see it;
- **numeric disequations**, delayed until decidable (both sides determined,
  or the equality is entailed — over Q a single disequation never restricts
  a non-degenerate solution set, so this is exact).

**One suspension mechanism** — Prolog II's `geler` (freeze): each unbound
variable carries a wake-up list, binding it fires the list, and the trail
undoes both — underlies everything that must be re-examined when a variable
is bound: tree disequations (D4), **attribute-term congruence** (binding a
variable inside `age(X)` wakes a re-canonicalisation that merges the term's
solver variable with that of the now-equal term), and numeric-typing checks.
It is built once, in the store, and later gives a `freeze`-style control
primitive for free if the language ever wants one.

Every store operation is undoable via the trail (D2). Satisfiability is
checked exactly at every step; there is no floating point anywhere
(`exact-arithmetic`), and no answer is ever reported from an unsatisfiable
store (`answer-soundness`).

Non-linear constraints (a variable times a variable) are **out of scope**;
if ever wanted, Prolog III's "approximated multiplication" (delay until one
factor is determined) is the model — a separate record.

### D6 — Grammar prerequisite: arithmetic in constraint expressions

The constraint language is useless without `+`, `-`, `*`, `/` on numeric
operands: `{ X + Y = 10, X - Y = 2 }`. This is a **grammar-first** change
through `claimr.rustemo` (`grammar-authority`): additive and multiplicative
operators with the usual precedence and left associativity, unary minus,
parenthesised expressions, and division `/` — exact over Q, so `1/3` is a
constant, per the exact-arithmetic record. **Linearity is a semantic
check** by the solver (multiplication and division require one operand to
be a constant at solve time), not a grammar restriction. Comment syntax
(`% …` to end of line, Prolog's) is added at the same time; it costs one
`Layout` rule.

### D7 — Answers: the store projected onto the query, in solved form

An answer is the store **restricted to the query's variables and the
attribute terms reachable from them**, simplified to Prolog III's solved
form: every uniquely determined variable shown as `X = value`; remaining
constraints shown explicitly (`Y > 2`, `age(socrates) > 70`); numbers
printed as exact integers or lowest-term fractions. `true` if nothing is
left. Answers are produced one at a time on backtracking; a query with no
answers prints `false`. Printing goes through a term/constraint printer
shared by the CLI and a future REPL. Projection/elimination of internal
variables (Prolog III noted the lack of it as a shortcoming) is done at
least for variables that occur in no remaining constraint; full
Fourier–Motzkin elimination is a later improvement.

### D8 — Layout of the code

- `src/eval/` (`evaluator` component): symbols, terms, compiled program,
  the resolution machine, answers.
- `src/solver/` (`solver` component): the store, union-find and disequations,
  the linear solver.
- The parser stays as is; a `compile` step lowers `ast::Clause` into the
  runtime program (desugaring D3).
- CLI: `claimr file.claimr` loads the program (constraint facts into the
  initial store), runs its queries in order, prints answers; exit code
  reflects load errors only.

## Consequences

- Programs behave as a Prolog programmer expects (D2) with constraints that
  are exact and sound (D5, aspects `exact-arithmetic`, `answer-soundness`).
  The README's examples acquire a precise meaning (D3, D4).
- Two invariants become testable rules: no resolution step is taken on an
  unsatisfiable store, and every printed answer's constraints are
  satisfiable in Q — the `answer-soundness` aspect doc will state them.
- Rational trees (D1) rule out the occurs check as a design lever and
  require the bind-before-descend discipline in unification; the payoff is
  fidelity to the reference design, stable semantics, and answers that
  print cyclic terms for free via the solved form.
- Attribute terms (D4) are claimr's own extension; their congruence handling
  is the trickiest part of the store and needs targeted tests.
- The grammar grows (D6) before the store can be exercised on anything
  interesting; that is the first implementation task, and it goes through
  `claimr.rustemo`.
- Staged implementation, each stage its own task and journal entry:
  1. **Grammar: arithmetic in constraint expressions and comments** (D6).
  2. **Terms, unification and SLD resolution** — pure programs, tree `=`
     and `!=`, queries answered from the CLI (D1, D2, D3, D7 without numeric
     constraints).
  3. **Constraint store and linear solver** — numeric typing, attribute
     terms, equations, inequalities (simplex), disequations, determined
     variables (D4, D5).
  4. **Answer projection and printing in solved form** (D7), and a REPL as
     a follow-up decision.
- Registered in `docs/architecture/README.md`: components `evaluator`,
  `solver`; aspect `answer-soundness`.
