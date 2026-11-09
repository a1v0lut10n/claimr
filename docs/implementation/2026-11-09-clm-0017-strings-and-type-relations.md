---
date: 2026-11-09
type: task
status: done
affects:
  - docs/reference/grammar.md
  - docs/reference/json-output.md
  - docs/reference/README.md
  - docs/architecture/components/evaluator.md
  - README.md
components: [grammar, parser, evaluator, cli, highlight]
aspects: [grammar-authority, answer-soundness]
design: [docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md]
tags: [CLM-0017, strings, atoms, built-ins, type-relations, prolog-iii]
---

# CLM-0017: Strings as character lists, `atom_chars/2`, and the type relations

## Premise

The accepted record makes a string literal a list of one-character atoms
(`"abc"` is `<a, b, c>`), gives one bridge between an atom and its
characters, `atom_chars(A, L)`, as a delayed constraint, and makes the
type relations `number/1`, `atom/1`, `list/1` constraints with Prolog
III's semantics. No `var/1`, no ISO atom built-ins: prefix, length and
scheme of an identifier are list programming over its characters, which
the three list stages (CLM-0014 to CLM-0016) now support. These are the
evaluator's first reserved predicate names.

## Scope

In: the `String` terminal and `Expr` alternative; the action building the
character list; `atom_chars/2`, `number/1`, `atom/1`, `list/1` dispatched
by the machine before clause lookup; delayed type constraints and delayed
`atom_chars` in the store, printed as residuals in goal form; the reserved
heads as compile errors with their span; the symbol table carried by the
store; highlight's `String` token; a built-ins reference page; the
example; the doc views.

Out: `var/1`; `atom_prefix`, `atom_concat`, `atom_length`;
case-insensitive comparison; a character type.

## Action items

1. **Grammar**: terminal `StringLit: /"(?:[^"\\]|\\.)*"/;` and
   `| string=StringLit {StringExpr}`. Named `StringLit`, not `String`:
   rustemo generates a type alias per terminal, and one called `String`
   shadows the standard type in the actions module. The automaton gains
   only shifts (60 productions, 93 states). **Done.**
2. **Action**: `string_lit` unescapes with the quoted atoms' rule (now a
   shared `unescape`) and `expr_string_expr` builds `Expr::List` of
   one-character `Expr::Ident`s; `""` is `<>`. No AST change; `"ab"` and
   `<a, b>` are one term and print as the list, query echoes included.
   **Done.**
3. **Symbols in the store**: `Store::with_symbols(Symbols)`; `Store::new`
   is gone (tests use `Store::default`). The machine and the compile-time
   validation give the store a clone of the program's table; the printer
   reads `store.symbols`. **Done.**
4. **Built-ins**: `compile::Builtin`, `BUILTINS` (name, arity, kind),
   `Program.builtins` interned at compile; `Solutions::run_builtin` runs
   one deterministically before clause lookup. `EvalError::ReservedBuiltin
   { name, arity, span }` for a user head with a built-in name and arity;
   another arity is an ordinary predicate (`list/2`). The CLI now prints
   `file:line:col:` for a positioned `EvalError` (it printed `file:` and
   the message before — a CLM-0014 slip, corrected here). **Done.**
5. **Type relations**: `TypeKind`, `TypeCon`, `Waker::Type`,
   `Undo::TypePending`; `post_type` / `recheck_type` as planned, plus:
   two relations of different kinds on one variable fail at once, and
   `unknown_of` refuses to make a variable numeric while a type relation
   (or the atom side of an `atom_chars`) holds it, so `atom(X), { X > 3 }`
   fails as `{ X > 3 }, atom(X)` does. `post_number` as planned. **Done.**
6. **`atom_chars`**: `Chars`, `Waker::Chars`, `Undo::CharsPending`;
   `post_atom_chars` / `recheck_chars` as planned, spelling through the
   store's symbol table. **Done.**
7. **Answers**: pass 8 of `render_answer` prints pending type relations
   and `atom_chars` in goal form after the numeric constraints and the
   concatenations. **Done.**
8. **Highlight**: `TokenKind::String` for `"…"`, the coverage test's
   regex-terminal examples gain one. Additive change to the token API
   aicogito consumes. **Done.**
9. **Example and tests**: `examples/strings.claimr` with golden answers;
   machine tests `strings_are_character_lists`,
   `atom_chars_is_a_delayed_constraint` (20 queries),
   `type_relations_are_constraints` (25 queries),
   `built_in_names_cannot_be_defined`; parse test
   `string_literals_are_character_lists`; JSON test
   `goal_form_residuals_are_constraint_strings`. 132 tests in all.
   **Done.**
10. **Docs**: `docs/reference/builtins.md` (new, indexed from
    `docs/reference/README.md`), `grammar.md`, README grammar section,
    `evaluator.md`, `json-output.md`; this plan. **Done.**

## Notes against the design record

- `number(X)` on an unbound variable makes it numeric, as the record says;
  the known consequence of D4 then applies: `number(X), { X = a }` does
  not fail but makes `a` an attribute term and answers `a = X`. Prolog III
  would fail. The record accepts the typing as it stands; tightening it is
  a question for the numeric store, not for this task.
- The record lists `atom_chars` failing on "a non-atom element, an atom
  longer than one character"; the implementation also fails on a numeric
  variable as `A` and refuses to make `A` numeric afterwards, which the
  record implies (an atom is not a number) without stating.
- The record calls the printing of residuals "goal form"; the JSON
  `constraints` list carries them as strings, shape unchanged, as it does
  for lengths and concatenations.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (132 tests); every previous golden file
      byte-identical.
- [x] `aivolution lint .` clean; the five `affects` docs current.
