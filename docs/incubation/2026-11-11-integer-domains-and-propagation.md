# Integer domains and propagation

Captured 2026-11-11. Prompted by the N-queens example (CLM-0034,
`examples/queens.claimr`, `docs/reference/benchmarks.md`): the standard
constraint-programming showcase fits claimr in twenty lines and runs a
dozen queens in well under a second, and the same run shows exactly what
claimr does not have.

What exists. Numbers are exact rationals; a numeric variable is a solver
variable with bounds in the simplex; `!=` on numbers is a disequation the
store decides the moment both sides are determined (CLM-0024 keeps the
candidates incrementally). Nothing is an integer by constraint: `{ 1 <= Q,
Q <= 8 }` admits `Q = 1/2`, and a disequation prunes nothing until a value
arrives. So the queens program must take its values from an explicit list
(`range/3`, `member/2`), and the search is backtracking with constraint
checking: a value is rejected as soon as it contradicts a posted
disequation, never before it is tried. Prolog III took the same position
on purpose — integers are "a special case" of rationals, there is no
integrality solver, only enumeration (`enum`) — and the exact-arithmetic
record (2026-08-17) and the evaluator record deferred integer constraints
with the words "a finite-domain solver would be its own component and
decision".

The idea: a finite-domain layer in the CLP(FD) tradition. Integer-valued
variables with a finite domain (`Q in 1..N`, or a set); the global
constraints that carry the common models (`all_different`, at least);
propagation that narrows domains as constraints are posted — bounds
consistency for linear constraints, arc consistency or Régin's matching for
`all_different` — so that a value is excluded before any search reaches it;
and labeling as a built-in (`label(Qs)`, with the usual variable- and
value-ordering heuristics), so the search itself becomes a one-line
declaration instead of `member/2` over an explicit list.

Why it would matter. The queens benchmark is the measuring stick: with
domains and propagation the first solution for 8, 10, 12 queens is a
handful of propagation steps and a few backtracks, where today each
candidate value is a resolution step through `member/2` and a constraint
check; and the models that matter for the integrity-checker use —
scheduling, assignment, counting — are integer models.

Open questions, none settled here:

- A separate solver component beside the simplex, or integrality inside
  it? Bounds from the simplex are rational; an FD variable's domain is a
  finite integer set; the two meet when an FD variable appears in a linear
  constraint. CLP(Q) + CLP(FD) systems keep them separate and bridge.
- Surface syntax inside `{ … }`: `X in 1..N`, `all_different(Qs)`, and
  `label/1` as a goal — and whether the groundness rule of CLM-0022 has an
  analogue for labeling inside a condition.
- Answers: a domain is a residual constraint (`Q in 1..8` printed when the
  search did not label), which the projection and the JSON vocabulary must
  carry.
- Explanations (CLM-0023, CLM-0031): a propagation step is a step too, and
  a conflict set over propagated domains needs the same replay oracle.
- Soundness: propagation must only remove values no solution has
  (`answer-soundness` invariant 3, nothing satisfiable dropped), which is
  the standard correctness property of a propagator and testable by the
  queens solution counts.

Adjacent work to connect with: `docs/design/2026-08-17-exact-rational-arithmetic.md`
(integer constraints deferred), `docs/design/2026-08-17-evaluator.md` D5,
`docs/architecture/components/solver.md`, the queens benchmark
(`tests/benchmarks.rs`), and the integrity-checker experiment whose models
would be the first consumers.
