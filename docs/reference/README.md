# Reference

Durable, distilled, topical knowledge; mutate-in-place.

- [`builtins.md`](builtins.md) — the four built-in predicates
  (`atom_chars/2`, `number/1`, `atom/1`, `list/1`), what each decides and
  when it waits (CLM-0017).
- [`grammar.md`](grammar.md) — EBNF view of the Claimr grammar. The
  authoritative grammar is `src/parser/claimr.rustemo`, from which the parser
  is generated (see the `grammar-authority` aspect in
  [`../architecture/README.md`](../architecture/README.md)).
