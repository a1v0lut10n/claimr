# Built-in predicates

The evaluator defines exactly five predicates: four from CLM-0017
([`../design/2026-11-09-strings-as-character-lists-and-type-relations.md`](../design/2026-11-09-strings-as-character-lists-and-type-relations.md))
and `sum/2` from CLM-0025
([`../design/2026-11-09-aggregation-list-comprehensions-and-sum.md`](../design/2026-11-09-aggregation-list-comprehensions-and-sum.md)).
Their names and arities are reserved: a program clause whose head is one of
them is a compile error reported with its position
(`EvalError::ReservedBuiltin`). Each posts a constraint and creates no
choice point; none of them is a test that can flounder silently. There is
no `var/1`, following Prolog III.

| Predicate | Meaning | When undecided |
|---|---|---|
| `atom_chars(A, L)` | `L` is the list of `A`'s characters, each a one-character atom (`atom_chars(hello, L)` gives `L = <h, e, l, l, o>`; `atom_chars(A, "abc")` gives `A = abc`). Fails when `A` is a number, a compound, `<>` or a numeric variable, when `L` is not a list, or when any known element of `L` is not a one-character atom. | Suspends on `A`, on `L`'s open tail and on its unbound elements; a pending one prints as `atom_chars(A, L)`. |
| `number(X)` | `X` is a number. A number succeeds; a constant, compound or list fails; an unbound variable becomes a numeric variable of the store (the typing arithmetic operands already receive), unless an `atom`/`list` relation or an `atom_chars` holds it, which fails. | Never pending: the constraint lives in the numeric store. |
| `atom(X)` | `X` is an atom: a nullary name, quoted or plain, other than `<>`. A numeric variable fails. | Suspends on `X` and is decided when `X` is bound; prints as `atom(X)`. |
| `list(X)` | `X` is `<>` or a list cell `'<>'(H, T)` — Prolog III's shallow `list` relation. A numeric variable fails. | Suspends on `X`; prints as `list(X)`. |
| `sum(L, S)` | `S` is the sum of the elements of the list `L`. On a complete list it posts the linear equation `S = e1 + … + en` over the elements' solver view — a number, a numeric variable, or an attribute term, as in every numeric position — exact over rationals and solvable in either direction (`sum(<1/3, 1/3, 1/3>, S)` gives `S = 1`; `sum(<1, X, 3>, 10)` gives `X = 6`; `sum(<>, S)` gives `S = 0`). An element held to be an atom or a list fails, as does a non-list or a cyclic `L`. | Suspends on an open tail and is decided when the spine completes; prints as `sum(L, S)`. |

Two relations of different kinds on one variable fail at once
(`atom(X), list(X)`), and a variable held to be an atom or a list cannot
become numeric (`atom(X), { X > 3 }` fails).

With a list comprehension (CLM-0025) `sum` aggregates over all solutions
of a goal, and the comprehension's length is a count:

```
total(T)     :- sum(<A | amount(_, A)>, T).
average(Avg) :- { L = <A | amount(_, A)> }, sum(L, T), { Avg = T / |L| }.
sources(X, N) :- { N = |<S | source(S, X)>| }.
```

`examples/aggregation.claimr` runs these.

Everything else about text is list programming over characters, with the
notation, length and concatenation of
[`../design/2026-11-09-lists-as-a-constraint-domain.md`](../design/2026-11-09-lists-as-a-constraint-domain.md):

```
has_prefix(Id, Prefix) :- atom_chars(Id, Cs), { Cs = Prefix . S }.
id_length(Id, N)       :- atom_chars(Id, Cs), { N = |Cs| }.
scheme(Id, Scheme)     :- atom_chars(Id, Cs), append(Scheme, <':'> . R, Cs).

?- has_prefix('doc:h3fa21c9d', "doc:").      % true
?- id_length('doc:h3fa21c9d', N).            % N = 13
?- scheme('doc:h3fa21c9d', S).               % S = <d, o, c>
```

`examples/strings.claimr` runs these.
