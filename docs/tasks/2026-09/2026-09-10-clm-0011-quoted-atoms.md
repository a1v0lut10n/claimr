---
date: 2026-09-10
type: task
status: proposed
affects:
  - docs/reference/grammar.md
components: [parser, grammar, eval]
aspects: [grammar-authority]
design: []
tags: [CLM-0011, grammar, atoms, aicogito]
---

# CLM-0011: Quoted atoms

## Objective

Single-quoted atoms in the grammar — `'situation:frame-in-spec'`,
ISO-Prolog style — so symbolic data whose natural spelling carries
punctuation can be an atom without mangling. Atoms compare by
content; an atom whose spelling isn't a plain identifier prints back
quoted.

## Context

A change proposal from aicogito (Stage D): its records carry ids
like `situation:frame-in-design-specification`, and its reasoning
adapter must currently mangle them onto the wire
(`r_situation_frame_in_design_specification`) through an id↔atom
manifest, restoring them in every answer — collision-suffixed,
longest-first, tested, and entirely dissolvable by this feature.
The mangling also creates a real expressiveness gap: an authored
constraints file cannot name a concrete record at all (the author
cannot know the mangling), so frame constraints are restricted to
variables. Quoted atoms make external identifiers first-class
citizens of the language — fitting for a language whose workspace
job is reasoning over records. Prolog III lineage makes the quoted
form the natural spelling.

## Deliverables

- Grammar: a quoted-atom terminal (`'…'` with an escape for the
  quote itself, e.g. doubled `''` or `\'` — decide in the design),
  producing an ordinary atom in the AST. Per the grammar-authority
  aspect: grammar → actions → an example under `examples/` → the
  EBNF view in `docs/reference/grammar.md`.
- Unification and answer printing: quoted and plain spellings of the
  same content are the same atom; solved-form output quotes exactly
  when the spelling isn't a plain identifier.
- The highlight token API (CLM-0009) learns the new token.

## Verification

- [ ] `?- X = 'situation:frame-in-spec'.` parses and answers
      `X = 'situation:frame-in-spec'`.
- [ ] `'abc' = abc` unifies (same atom, one spelling plain).
- [ ] aicogito-reason's manifest can become a pass-through (tracked
      aicogito-side).
- [ ] `cargo test` green, incl. a golden example.
