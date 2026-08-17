---
date: 2026-08-17
type: design
status: accepted
components: [parser]
aspects: [exact-arithmetic]
tags: [numbers, rationals, prolog-iii, evaluator]
---

# Numbers are exact arbitrary-precision rationals; no floats, no separate real type

## Context

The parser represents numeric literals as `Expr::Number(f64)`. Before the
evaluator and constraint solver exist (see
`docs/catalyst/2026-08-17-evaluator.md`), the numeric domain has to be
settled, because it shapes the solver, the term model, printing, and the
first grammar extensions.

A linear constraint solver must answer exact questions — is this variable
now determined? does the store entail `X = Y`? is this disequality
violated? — and those are equality tests on numbers. With floating point they
become meaningless after a few pivots: CLP(R) is documented as unsound for
this reason, and SWI-Prolog ships `clpq` (exact) alongside `clpr`
(approximate) because one type cannot serve both. Residual constraints in
answers are only trustworthy if the arithmetic behind them is exact.

Prolog III, claimr's reference design (Colmerauer, *An Introduction to
Prolog III*, CACM 1990), takes the domain to be the **perfect reals** —
"not floating point numbers" — but "the machine will compute with rational
numbers only", justified by a property of its constraint language: "if a
variable is sufficiently constrained to represent a unique real number then
this number is necessarily a rational number." Irrationals cannot be written
as constants and exist only semantically, as values under-constrained
variables may take. Integers are "a special case" of rationals; there is no
integrality solver, only enumeration. Its numerical module is an
incremental simplex over "fractions whose numerators and denominators are
unbounded integers".

The wish expressed for claimr is the same level of purism. Read against the
source, that purism is *one exact representation* (rationals) with reals as
the semantic domain — not two representations.

Alternatives considered:

- **`f64` throughout** (status quo). Simple and fast; makes the constraint
  store unsound and answers approximate. Rejected.
- **Rationals plus a distinct floating "real" type.** With linear rational
  constraints the real type would never be populated by the solver, and any
  float that enters the store poisons its exactness. Rejected.
- **Rationals plus interval reals** (Prolog IV / CLP(BNR) style). Sound, but
  only needed for non-linear constraints, which are out of scope. Deferred,
  not rejected — it is the extension path if non-linear ever arrives.
- **Distinct integer type.** Not needed for representation (denominator 1
  covers it) and integrality *constraints* are a separate solver (finite
  domains). Deferred as in Prolog III.

## Decision

- **Every number in the language is an exact rational of arbitrary
  precision.** Integers are rationals with denominator 1. There is **no
  floating-point type** anywhere in the language core (AST, terms, solver,
  printing).
- **Decimal literals denote exact rationals**: `18.5` is 37/2, `0.1` is
  1/10. Literals are converted from their source text exactly. Once
  arithmetic exists, `1/3` is exact division; no rational-literal syntax is
  introduced.
- **No separate real representation.** Reals are the semantic domain of
  variables; an irrational is never a value. Non-linear constraints, if ever
  wanted, are a new decision (interval reals), never bare floats.
- **Integer constraints are deferred**; a finite-domain solver would be its
  own component and decision.
- **Printing**: integers print as integers, other rationals as fractions in
  lowest terms (`33/32`), as Prolog III did.
- **Implementation**: a `Number` newtype in the crate wrapping
  `num_rational::BigRational` (`num-bigint` backed; pure Rust, MIT/Apache).
  The backend is an implementation detail behind the newtype so
  `rug`/`malachite` or a small-integer fast path can replace it without
  touching the language.

## Consequences

- `Expr::Number` changes from `f64` to `Number` — a breaking AST change at
  0.1. The AST becomes `Eq + Hash`, which the evaluator's term handling
  benefits from.
- Arithmetic is slower than machine floats (bignum on every operation).
  Acceptable at claimr's scale; the newtype leaves room for a fast path.
- Number formatting must render fractions; the CLI/REPL and any future
  answer printer follow the printing rule above.
- The `exact-arithmetic` aspect is registered in
  `docs/architecture/README.md`; every future numeric feature (arithmetic
  expressions, the solver, printing) is bound by it.
- Tracked by task `docs/tasks/2026-08/2026-08-17-clm-0003-exact-rational-numbers.md`.
