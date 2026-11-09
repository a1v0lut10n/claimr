# Built-ins on atoms and type tests

Captured 2026-11-08. Data loaded from outside (identifiers like `'doc:h3fa21c9d'`,
`'cl-2a04…'`) often needs tests on the text of an atom:

- does an id have a given prefix;
- what is the scheme part before `:`;
- are two atoms equal ignoring case.

Type tests matter too: is this term a number, an atom, or still a variable? The grammar
defines no named built-in predicates, so today all of this has to be decided before the
data reaches claimr, and fed in as facts.

The idea is a small, documented set of built-ins, each deterministic and sound:

- `atom_prefix(A, P)` and `atom_concat(A, B, C)` (for ground arguments);
- `atom_length(A, N)` (with `N` exact);
- `number(X)`, `atom(X)` and `var(X)`.

The type tests are the delicate ones under constraints. `var(X)` on a variable that is
constrained but unbound needs a definition: unbound, or "not determined"?

Strings as a separate type are probably not needed. Quoted atoms already carry arbitrary
text (`docs/design/2026-09-10-quoted-atoms.md`).
