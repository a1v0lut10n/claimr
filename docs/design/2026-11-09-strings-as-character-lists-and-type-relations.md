---
date: 2026-11-09
type: design
status: proposed
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
tags: [strings, atoms, type-tests, built-ins, prolog-iii]
---

# Strings as character lists, and type relations as constraints

## Context

Data loaded from outside carries identifiers such as `'doc:h3fa21c9d'`
and `'cl-2a04…'` (quoted atoms, `2026-09-10-quoted-atoms.md`), and
integrity rules need to look inside them: does an id have a given prefix,
what is the scheme before the `:`, how long is it. Rules also need to ask
what kind of thing a term is: a number, an atom, a list. The grammar
defines no built-in predicates, so today all of this is decided before the
data reaches claimr and fed in as facts.

The incubation entry this record graduates from proposed ISO-style atom
built-ins (`atom_prefix/2`, `atom_concat/3`, `atom_length/2`) and type
tests (`number/1`, `atom/1`, `var/1`), and flagged `var/1` under
constraints as "the delicate one".

**Prolog III** takes a different line on both counts (Colmerauer, CACM
33(7), 1990):

- There is no string type. "The constant `"c1...cm"` designates the true
  list whose immediate daughters make up the sequence of characters
  `'c1' ... 'cm'`", so `"c1...cm" = <'c1', ..., 'cm'>`, and string
  concatenation is list concatenation. The context-free recognizer in the
  paper runs `Sform("aaabb")` with the same `•` as any list, and a mixed
  list prints as `<'M', 'a', 'x', 9/5, meters, 1'>`: characters, not a
  string.
- Type tests are constraints: "The relations `id`, `char`, `bool`, `num`,
  `irint`, `list` and `leaf` are used to specify that the tree a1 is an
  identifier, a character, a Boolean value, a real number, an integer or
  irrational number, a list, a label not of the form `<>α`." They delay
  like any other constraint. There is no `var`.

Characters in Prolog III are a leaf type written `'c'`, which is claimr's
quoted-atom spelling; a new leaf type would also need its own unification
and printing cases for little gain.

## Decision

1. **A string literal denotes a list of one-character atoms.**
   `"abc"` parses to `<a, b, c>`; `""` to `<>`, which is the paper's
   `"c1...cm" = <'c1', ..., 'cm'>` with characters as atoms. The literal
   uses the same backslash escapes as quoted atoms (`\"`, `\\`, any other
   `\c` is `c`). `'a'` and `a` are the same character, so `"doc:x"` is
   `<d, o, c, ':', x>`. No new term kind, no new leaf type. Depends on
   stage 1 of `2026-11-09-lists-as-a-constraint-domain.md`.
2. **Printing is list form, as in Prolog III.** `<a, b>` and `"ab"` are
   the same term, so the printer never guesses a string reading; a list of
   one-character atoms prints as `<a, b>`, as the paper prints
   `<'M', 'a', 'x', 9/5, meters, 1'>`.
3. **One bridge between an atom and its characters: `atom_chars(A, L)`.**
   It behaves as a delayed constraint, not a test: when `A` is a bound
   atom it unifies `L` with the character list; when `L` is a ground list
   of one-character atoms it unifies `A` with the atom they spell; when
   neither holds it suspends on the existing suspension machinery (as
   `dif` and delayed products do) and, if still pending at answer time, is
   reported as a residual constraint. It never flounders silently and
   never errors, which keeps `answer-soundness`. Any other shape of `L`
   (a non-atom element, an atom longer than one character) fails.
4. **Type relations, with Prolog III's semantics and Prolog-family
   names: `number(X)`, `atom(X)`, `list(X)`.** Each is a goal that posts a
   constraint:
   - `number(X)`: `X` is a number. Bound to a number, succeeds; bound to
     anything else, fails; unbound, `X` becomes a numeric variable in the
     store (the typing `post_arith` already applies to operands), and the
     constraint is implied by the answer's numeric part.
   - `atom(X)`: `X` is an atom (a nullary name, quoted or plain). Unbound,
     a delayed type constraint on `X` that fails any later binding to a
     number, compound or list, and prints as `atom(X)` if still pending.
   - `list(X)`: `X` is `<>` or a cell, the shallow label test Prolog
     III's `list` relation makes. Same delaying behaviour.
   These are the only built-in predicates introduced; they are reserved
   names in the evaluator, not grammar changes, since `Goal` already
   parses `number(X)`.
5. **No `var/1`, following Prolog III.** The question "is this variable
   still unbound" has no sound answer when the variable may be constrained,
   and the type relations plus `dif` cover the uses that motivated it.
6. **No `atom_prefix`, `atom_concat`, `atom_length`.** They are
   concatenation and length over character lists, as constraints once
   stages 2 and 3 of the lists record land and as `append`/`length`
   predicates from stage 1 (see Consequences). Case-insensitive comparison
   is left out: Prolog III has nothing for it, and a character-mapping
   built-in can be its own small record when a rule needs one.

## Consequences

- The integrity-rule idioms become list programming over the characters
  of an id. With the full lists record, a prefix test is a constraint that
  completes at once because the left operand's length is known, and a
  length is a number in the store:

  ```
  has_prefix(Id, Prefix) :- atom_chars(Id, Cs), { Cs = Prefix . _ }.
  id_length(Id, N)       :- atom_chars(Id, Cs), { N = |Cs| }.
  scheme(Id, Scheme)     :- atom_chars(Id, Cs), append(Scheme, <':'> . _, Cs).

  ?- has_prefix('doc:h3fa21c9d', "doc:").
  ?- scheme('doc:h3fa21c9d', S).
  ```

  `scheme` keeps `append`: the scheme's length is unknown, and Prolog III
  does not search a concatenation with an unknown left length either.
  From stage 1 alone, `has_prefix` and `id_length` are `append/3` and
  `length/2` as in `examples/lists.claimr`. Text operations are O(n) list
  walks rather than O(1) atom operations, which is fine for identifiers.
- Grammar path per `grammar-authority`: one `String` terminal with the
  quoted-atom escape regex over `"`, one `Expr` alternative, an action
  that builds the cons list, an example under `examples/`, the EBNF view,
  and `src/highlight.rs` (coverage test).
- The evaluator gains its first reserved predicate names: `atom_chars/2`,
  `number/1`, `atom/1`, `list/1`. A user clause whose head uses one of
  them is a compile error (`EvalError`), reported with its span, so that
  the reservation is never silent. `docs/reference/` gains a built-ins
  page listing exactly these four.
- Answers may now carry two new kinds of residual constraint,
  `atom_chars(A, L)` and `atom(X)` (and `list(X)`), printed in goal form
  after the numeric constraints. The JSON output needs a contract version
  note for the new residual kinds.
- `number/1` on an unbound variable has an observable effect on printing
  only through the numeric store, as today for a variable that appears in
  an arithmetic term.
