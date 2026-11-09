# List length and concatenation as constraints (Prolog III tuples)

Captured 2026-11-09, while deciding list notation
(`docs/design/2026-11-09-list-notation.md`). That record adopts Edinburgh
brackets over cons cells and defers the part of Prolog III's list design
that is not notation: lists as a constraint domain.

In Prolog III (Colmerauer, CACM 1990) the length of a list is a real in the
store, `|z| = 10` is a linear constraint, and concatenation
`z =. x • y` is a delayed constraint that asserts `|z| = |x| + |y|` at once
and completes when all operands but the last have known length. That buys
things cons lists cannot do without walking: reading the last element
(`y =. x • <n>`), a two-clause `reverse` via palindromes, a context-free
recognizer that is a transcription of the grammar, and sequence puzzles
posed as `{ |z| = 10, <1,2,3> • z =. z • <2,3,1> }`.

The idea is to add exactly that on top of the chosen notation:

- a length constraint, say `{ length(L) = N }` or a `|L|` term, that
  attaches a numeric variable to a list variable and is kept consistent
  with the cons structure as it is built or unified;
- a concatenation constraint over cons lists with Prolog III's delaying
  rule, waking when a prefix's length becomes known;
- Prolog III's improper lists (known prefix, unknown-length tail) only if
  the above needs them; Edinburgh `[a | T]` already expresses an unknown
  tail of unknown length.

Open: how the length variable interacts with the simplex (one variable per
list variable, trailed with the store), how to wake on partial
instantiation of the spine, and whether `append/3` should become this
constraint or stay a predicate beside it.

Relevant: `docs/architecture/components/solver.md` (attribute terms,
delayed products: the mechanism is the same shape),
`docs/design/2026-08-17-evaluator.md`.
