# The anonymous variable `_`

Captured 2026-11-09. Prompted by implementing stage 1 of the lists record
(CLM-0014): the record's own examples write `member(X, <X> . _)` and
`has_prefix(Id, Prefix) :- atom_chars(Id, Cs), { Cs = Prefix . _ }.`, and
neither parsed, because the grammar has no anonymous variable. The examples
and the shipped programs use named variables instead (`member(X, <X> . T)`),
and the three list plans and the strings plan each note the gap.

The idea is the Prolog-family `_`: a variable whose name is not a name.

- **One terminal.** `_` on its own (the `Var` regex requires an uppercase
  initial, so `_` and `_foo` are currently lexical errors). Whether
  `_`-prefixed names such as `_Tail` should mean "named but hidden", as in
  several Prologs, is a separate question; the minimal idea is the bare `_`.
- **A fresh variable per occurrence.** Two `_` in one clause are two
  variables; the clause's variable map allocates a new index for each
  occurrence instead of looking a name up.
- **Hidden in answers.** An `_` in a query is not an answer variable: it
  never prints an equation and is not reported among the query variables,
  while anything reachable from it that is also reachable from a named
  variable prints as usual (`_1`, `_2` for internal variables, as today).
- **Printed as `_`** by the clause printer (`:list`, query echoes,
  `--parse`), since the AST would carry it as its own expression node or as
  a reserved name.

This is small, but it touches the grammar (one terminal), the highlight
lexer (coverage test), the AST, the compile step's variable numbering, and
the answer printer's notion of query variables, so it deserves its own
ticket rather than riding along with a feature.

## Adjacent work to connect with

- `docs/design/2026-11-09-lists-as-a-constraint-domain.md` and
  `docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md`,
  whose examples assume `_`.
- `docs/implementation/2026-11-09-clm-0014-list-notation.md`, which
  records the gap under *Deviation from the design record*.
- `docs/design/2026-09-10-quoted-atoms.md`, the last change to the name
  terminals, for how a lexical addition went through the grammar path.
