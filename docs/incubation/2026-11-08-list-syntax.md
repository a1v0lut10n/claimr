# Shorthand syntax for lists

Captured 2026-11-08. Lists already work as terms (`nil`, `cons(H, T)`;
`examples/lists.claimr`), but writing them is heavy:
`cons(a, cons(b, cons(c, nil)))` for a three-element list, and the same in every
argument and every answer.

The idea is Prolog's notation as syntax over those terms:

- `[]`, `[a, b, c]` and `[H | T]` parse to `nil` and `cons/2`;
- answers print lists back in the bracket form.

Since it is sugar, no evaluator change is needed. It goes grammar → actions → example →
EBNF view, per the `grammar-authority` rule:

- the grammar (`src/parser/claimr.rustemo`) gains `[`, `]` and `|`, and a list term;
- the actions build `cons`/`nil` terms;
- the answer printer, and the JSON output (`docs/reference/json-output.md`), render them
  as lists. The pinned JSON contract would need a version note.

Data with multi-valued fields (keys, codes, aliases) becomes natural to express:
`aliases(p1, ['alpha', 'beta'])` instead of one fact per value.

Open question: whether `[a | b]`, an improper list, is allowed or refused.
