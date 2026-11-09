---
date: 2026-11-09
type: journal
components: [grammar, parser, evaluator, cli, highlight]
aspects: [grammar-authority, answer-soundness]
tasks:
  - docs/implementation/2026-11-09-clm-0017-strings-and-type-relations.md
design:
  - docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md
meta-impact:
  note: "The highlight token API gains `TokenKind::String` (additive; a consumer matching the enum exhaustively must add an arm) and the --json constraint strings may carry built-ins in goal form, `atom(X)`, `list(X)`, `atom_chars(A, L)` (document shape unchanged). aicogito consumes both. No contract is named yet — the standing suggestion: add claimr-json-output and claimr-token-api to the vocabulary."
---

# CLM-0017 executed: strings are character lists, `atom_chars/2` and the type relations are the first built-ins

- **When:** 2026-11-09 12:24:05

## Context

The last of the three design records accepted on 2026-11-09. Identifiers
loaded from outside (`'doc:h3fa21c9d'`) need looking into — prefix,
length, scheme — and rules need to ask what kind of thing a term is. The
record follows Prolog III on both: no string type (`"c1...cm" = <'c1',
..., 'cm'>`), type relations as constraints, no `var`. With the three list
stages (CLM-0014 to CLM-0016) in place, text operations are list
programming over characters, and this task adds only the bridge and the
relations.

## Details

**Grammar.** One terminal, `StringLit`, with the quoted atoms' escape
rule, and one `Expr` alternative; the automaton gains only shifts (60
productions, 93 states). The terminal is not called `String`: rustemo
generates a type alias per terminal, and one named `String` shadows the
standard type in the actions module. The action builds `Expr::List` of
one-character `Expr::Ident`s, so there is no new AST node and `"ab"` and
`<a, b>` are one term, printed as the list in answers and query echoes
alike. The unescaping is now one function shared with quoted atoms.

**Built-ins.** `Program.builtins` maps the four reserved keys
(`atom_chars/2`, `number/1`, `atom/1`, `list/1`) to a `Builtin`; the
machine consults it before clause lookup and runs the built-in
deterministically, no choice point. A user clause with one of these heads
is `EvalError::ReservedBuiltin { name, arity, span }` at compile time;
another arity is an ordinary predicate. `docs/reference/builtins.md` lists
exactly the four.

**Store.** Two delayed constraint kinds beside lengths and concatenations.
*Type relations* (`TypeCon`, `Waker::Type`): a bound term is decided at
once (`atom`: a constant other than `'<>'`; `list`: `'<>'` or a `'<>'/2`
cell); an unbound plain variable gets a pending entry decided when it is
bound; a numeric variable fails; two relations of different kinds on one
variable fail at once. `number(X)`: a number or a numeric variable
succeeds, a tree fails, an unbound plain variable becomes numeric through
`unknown_of`. *`atom_chars`* (`Chars`, `Waker::Chars`): decided when the
atom is a constant other than `'<>'` (its characters interned as
one-character atoms and unified with the list) or when the list is a
complete list of one-character constants (the spelled atom interned and
unified); fails on a number, compound, `'<>'` or numeric variable as the
atom, on a non-list, or on any known element that is not a one-character
atom; otherwise suspended on the atom, the list's open tail and its
unbound elements. `unknown_of` refuses to make a variable numeric while a
type relation or the atom side of an `atom_chars` holds it, so
`atom(X), { X > 3 }` fails as `{ X > 3 }, atom(X)` does. To intern at wake
time inside `settle`, the store now carries a clone of the program's
symbol table (`Store::with_symbols`; `Store::new` is gone), and the
printer reads it from the store.

**Answers.** Pending type relations and `atom_chars` that mention a
reachable variable print in goal form after the numeric constraints and
the concatenations: `atom(X)`, `atom_chars(A, <a> . T)`. The JSON
constraint strings carry the same text.

**Highlight.** `TokenKind::String` for `"…"` with the escape rule; the
coverage test's regex-terminal examples gain one.

**CLI.** A positioned compile error printed `file:` and the message since
CLM-0014 (the formatting assumed the position was in the message, as it
is for parse errors); it now prints `file:line:col: message`.

**Verification.** 132 tests pass, among them `strings_are_character_lists`,
`atom_chars_is_a_delayed_constraint` (20 queries),
`type_relations_are_constraints` (25 queries),
`built_in_names_cannot_be_defined`, a parse test for the literal and its
escapes, and a JSON test for goal-form residuals. `examples/strings.claimr`
has golden answers checked by hand: `has_prefix('doc:h3fa21c9d', "doc:")`
is true, `id_length` gives 13, `scheme` gives `<d, o, c>`. Formatting,
clippy with warnings denied and the docs linter are clean; every earlier
golden file is byte-identical. The linter's legacy count went from ten to
eleven: `builtins.md` has no frontmatter, like the two other reference
pages.

**Notes against the record**, in the plan: `number(X)` makes an unbound
variable numeric as the record says, with D4's known consequence that
`number(X), { X = a }` answers `a = X` rather than failing as Prolog III
would — a question for the numeric store, not this task; `atom_chars`
also fails on a numeric variable as its atom, which the record implies;
"goal form" residuals travel as strings in the JSON `constraints` list,
shape unchanged.

**Pre-write checks.** The five docs in the task's `affects` are current:
the EBNF view and the README carry the string literal and point to the
built-ins page, `evaluator.md` the symbol table in the store, the two
delayed kinds, the dispatch and the printing, `json-output.md` the note
on goal-form strings, the reference index the new page.
`grammar-authority` held in order (grammar, actions, example, EBNF view,
README view). `answer-soundness` holds: every built-in decides, fails or
waits visibly as a printed residual; none flounders silently or
approximates. Cross-repo impact is the `meta-impact` note.

**All three 2026-11-09 records are now implemented**: ground arithmetic
folding (CLM-0013), lists in three stages (CLM-0014 to CLM-0016), strings
and type relations (CLM-0017). Open from the way: an incubation note for
the anonymous variable `_`, the deferred tableau truncation, an iterative
lowering for deep literals, and the two contract names for aivolution-meta.

## Links

- Task: `docs/implementation/2026-11-09-clm-0017-strings-and-type-relations.md`
- Design record: `docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md`
- Reference: `docs/reference/builtins.md`
- Lists stages: `docs/journal/2026-11/2026-11-09-execution-clm-0014-list-notation.md`,
  `docs/journal/2026-11/2026-11-09-execution-clm-0015-list-length.md`,
  `docs/journal/2026-11/2026-11-09-execution-clm-0016-list-concatenation.md`
- Acceptance entry: `docs/journal/2026-11/2026-11-09-design-fold-arithmetic-lists-strings.md`
- Commits `4ec7e6a` (plan, ticket claim), `838bdb1` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/45
- Views updated: `docs/reference/grammar.md`, `docs/reference/json-output.md`,
  `docs/reference/README.md`, `README.md`,
  `docs/architecture/components/evaluator.md`
