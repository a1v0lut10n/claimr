# Bind a variable equated to a number directly

Captured 2026-11-09. Prompted by a deep-derivation test for the explanations
task (CLM-0023): the countdown

```
down(0).
down(N) :- { N > 0, M = N - 1 }, down(M).
?- down(2000).
```

takes 13 s for 1 000 steps and 55 s for 2 000 (debug build), with or without
explanations. The arithmetic folds (CLM-0013): `N - 1` is a number by the time
the goal is reached. But `M = <number>` with `M` an unbound variable goes
through `post_eq_goal`'s numeric path, which makes `M` a numeric variable with
a solver variable fixed to the constant, and every later post's cheap
determination pass (`numeric_consequences(false)`) walks all numeric
variables. Twenty thousand steps leave twenty thousand fixed solver variables
behind, so the run is quadratic.

The idea: when one side of a numeric equation is a plain unbound variable
(not numeric, no attribute) and the other a number, bind the variable to the
`Num` cell as a tree binding, exactly as unification would for a literal.
`M = 2` then means what `M = f(a)` means, and the solver never hears of it.
A variable that is already numeric (it appears in an inequality) keeps the
current path. Prolog III's semantics are the same either way: a number is a
leaf of the tree domain.

Adjacent work to connect with:

- `docs/design/2026-11-09-fold-ground-arithmetic-before-the-solver.md`
  (CLM-0013), which removed the solver rows for ground arithmetic but left
  this equation on the solver path;
- `docs/design/2026-11-09-truncate-the-tableau-on-undo.md` (CLM-0018), the
  other growth source, now gone;
- the `numeric_consequences` candidate scan in `src/eval/store.rs`, whose
  cost per post is the number of live numeric variables.

## Addendum, 2026-11-09 (CLM-0024)

The quadratic cost went away without this idea: CLM-0024 keeps the
determination pass's candidates incrementally and, in `unknown_of`,
substitutes a determined variable's value, so `N - 1` over a determined `N`
folds and defines no row. The countdown runs 16 000 steps in 0.05 s. What
remains of this note is a semantic question, not a performance one. A probe
of the current behaviour:

```
?- { M = 2 }, { M = f(a) }.          M = 2, f(a) = 2
p(2). ?- p(M), { M = f(a) }.          false
?- { X = age(bob) }, { X = 31 }.      X = age(bob), age(bob) = 31
?- { X = 31 }, { X = age(bob) }.      X = 31, age(bob) = 31
```

A number acquired through `{ M = 2 }` is a numeric variable, and a compound
equated to it later becomes an attribute term (D4); a number acquired by
unification is a tree leaf, and the same equation fails. Binding directly
would make the first pair agree (both fail) — arguably the Prolog III
reading, a number being a leaf — but would also make the second pair
disagree by goal order unless `is_numericish` looked through variables
bound to numbers, which changes `p(M), { M = f(a) }` the other way. Either
choice is a design record on what `=` means between a number and a tree in
numeric position, with `answer-soundness` (no goal-order dependence) as
the constraint.
