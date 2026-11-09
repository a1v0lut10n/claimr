---
date: 2026-11-09
type: task
status: in-progress
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

1. **Grammar**: terminal `String: /"(?:[^"\\]|\\.)*"/;` and
   `| string=String {StringExpr}` in `Expr`. A string is a single token,
   so the automaton gains only shifts; checked in the `--dot` output.
2. **Action**: unescape as quoted atoms do (`\"`, `\\`, any other `\c`
   is `c`) and build `Expr::List` of one-character `Expr::Ident`s;
   `""` is `<>`. No AST change: `"ab"` and `<a, b>` are one term and
   print as the list (query echoes included).
3. **Symbols in the store**: `Store::with_symbols(Symbols)`; the machine
   and the compile-time validation give the store a clone of the
   program's table, the printer reads `store.symbols`. `atom_chars` can
   then intern the atom it spells when it wakes inside `settle`.
4. **Built-ins**: `Program.builtins: HashMap<PredKey, Builtin>` interned
   at compile; the machine's `RtGoal::Call` consults it before the clause
   lookup and runs the built-in deterministically (no choice point). A
   clause head with a reserved name and arity is `EvalError::
   ReservedBuiltin { name, arity, span }` at compile time; `'<>'` keeps
   `ReservedName`.
5. **Type relations in the store**: `TypeCon { var, kind: Atom | List,
   pending }` entries with `Waker::Type`. `post_type(a, kind)`: a bound
   term is decided at once (`atom`: a constant other than `'<>'`; `list`:
   `'<>'` or a `'<>'/2` cell); a numeric variable fails (it denotes a
   number); an unbound plain variable gets a pending entry suspended on
   it, re-checked on binding, re-suspended if bound to another variable.
   `post_number(a)`: a number or a numeric variable succeeds; a constant,
   compound or list fails; an unbound plain variable with a pending type
   constraint fails, otherwise it becomes numeric through `unknown_of`.
6. **`atom_chars` in the store**: `Chars { a, l, pending }` entries with
   `Waker::Chars`. Decided when `a` is a constant other than `'<>'` (its
   characters as a list of interned one-character atoms, unified with
   `l`) or when `l` is a complete list of one-character constants (the
   spelled atom, interned, unified with `a`). Fails when `a` is a number,
   a compound, `'<>'` or a numeric variable, when `l` is not a list, or
   when any known element of `l` is not a one-character atom. Otherwise
   suspended on `a`, on `l`'s open tail and on its unbound elements.
7. **Answers**: pending type constraints and `atom_chars` whose terms
   mention a reachable variable print in goal form after the numeric
   constraints: `atom(X)`, `list(X)`, `atom_chars(A, L)`.
8. **Highlight**: `TokenKind::String` for `"…"` with the quoted-atom
   escape rule; the coverage test's regex-terminal examples gain one.
   This is an additive change to the token API aicogito consumes.
9. **Example and tests**: `examples/strings.claimr` with golden answers
   (the record's `has_prefix`, `id_length`, `scheme`; `atom_chars` both
   ways, delayed and failing; each type relation on each kind of term,
   delayed, and in conflict with `number`); machine tests for the same
   and for undo; parse test for the literal and its escapes; a JSON test
   for a residual in goal form; a compile test for the reserved heads.
10. **Docs**: `docs/reference/builtins.md` (the four predicates, exactly),
    indexed from `docs/reference/README.md`; `grammar.md` and the README
    grammar section (string literal); `evaluator.md` (built-ins, type and
    `atom_chars` entries, symbols in the store); `json-output.md`
    (residuals in goal form); plan to what was built.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; all previous golden files byte-identical.
- [ ] `aivolution lint .` clean; the five `affects` docs current.
