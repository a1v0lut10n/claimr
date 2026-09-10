---
date: 2026-09-10
type: design
status: accepted
components: [parser, grammar, eval, highlight]
aspects: [grammar-authority]
tags: [CLM-0011, atoms, grammar]
---

# Quoted atoms

## Context

CLM-0011 (proposed from aicogito's Stage D): symbolic data whose
natural spelling carries punctuation — record ids like
`situation:frame-in-design-specification` — cannot be an atom today,
forcing consumers to mangle (aicogito's id↔atom manifest) and making
concrete external ids inexpressible in authored programs. The Prolog
family's answer is the quoted atom. Three semantic choices need
fixing before the grammar moves: the escape convention, atom
identity, and print-back.

## Decision

1. **Syntax**: `'…'` — a single-quoted terminal
   (`/'(?:[^'\\]|\\.)*'/`). **Backslash escapes**: `\` escapes the
   next character, so `\'` is a quote and `\\` a backslash (any other
   `\c` is `c`). Chosen over ISO's doubled `''`: the backslash form
   is one clean regex alternation for the LR lexer, matches the
   escaping convention the CLI's JSON output already speaks, and avoids the
   doubled-quote boundary ambiguity in a longest-match tokenizer.
2. **Identity is content.** `'abc'` and `abc` are the same atom;
   quoting is spelling, not meaning. The AST keeps names as plain
   strings (unescaped content), so unification, the store, and the
   solver need no change at all.
3. **Print-back quotes exactly when needed.** A name matching plain
   identifier spelling (`[a-z][A-Za-z0-9_]*`) prints bare; anything
   else prints quoted with `'` and `\` escaped. One helper
   (`ast::display_name`) serves the AST printer and the solved-form
   answer printer, so `X = 'situation:frame-in-spec'` round-trips as
   valid source.
4. **Where it may appear**: everywhere a plain identifier names an
   atom — a compound's functor and a standalone constant (a `Name`
   nonterminal over `Ident | QuotedIdent`). Variables stay
   uppercase-plain; there is no quoted variable.

## Consequences

Easier: external identifiers become first-class (aicogito's manifest
can become a pass-through, and constraints files can name concrete
record ids); the highlight API gains one token branch; the JSON
output needs nothing (its strings were always escaped).

Harder: the empty atom `''` now exists (content ""); it unifies only
with itself and always prints quoted — admitted rather than
special-cased. `docs/reference/grammar.md` and the README's EBNF
must move with the grammar, per the grammar-authority aspect.
