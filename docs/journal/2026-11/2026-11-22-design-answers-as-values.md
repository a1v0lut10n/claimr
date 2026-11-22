---
date: 2026-11-22
type: journal
components: [evaluator]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-22-answers-as-values.md
meta-impact:
  note: "Accepted: the `claimr` library's `Answer` gains `bindings: Vec<Binding { name, value: Term }>` and `Answer::binding(&str) -> Option<&Term>`, with a new public `Term` (`Atom`, `Number`, `Var`, `Compound`, `List`) whose `Display` is the solved-form renderer. The text fields are unchanged and `--json` is untouched, so aicogito's adapter sees no change; genite's rule runner (GEN-141) replaces its string splitting and atom unquoting with `binding(\"Rule\")`. The field addition makes the next release 0.8.0."
---

# Design record accepted: answers as values

- **When:** 2026-11-22 11:45:05

## Context

genite's GEN-141 is the first program to link claimr as a library, and
an `Answer` is text: `equations`, `disequations`, `constraints` as the
solved-form printer writes them. To read the three identifiers a rule
`contradiction(Rule, A, B)` binds, genite splits each equation at ` = `
and undoes the printer's quoting of an atom by hand — the printer run
backwards outside the crate, which breaks on a changed space, cannot
tell `'3'` from `3`, and would need a term parser the day a rule answers
with a compound. The incubation note of 2026-11-20 named it; of the
three notes from that embedding, the undefined-call report went first as
a correctness hazard, and this one second as the library's missing half.
The record landed as PR #96, graduating the note by rename, and is
accepted here.

## Details

**Decision.** An answer keeps its text and also carries its bindings as
values. A public `claimr::Term` holds exactly what an answer's heap can
hold — `Atom(String)`, `Number(Number)`, `Var(String)`,
`Compound(String, Vec<Term>)`, `List(Vec<Term>, Option<Box<Term>>)` — and
nothing a program is merely written with (no arithmetic, comprehension,
concatenation or length). Atom and functor names are content, as
`ast::Expr::Ident`, `ast::Atom::name` and the symbol table hold them
(quoted-atoms record, decision 2: identity is content), and `Term`'s
`Display` is the solved-form renderer with `display_name` quoting, so a
test pins every non-alias equation to `name = value`. `Answer::bindings`
has one entry per query variable that has an equation, in order, then
one per named cyclic node `_N`, as the text writes them;
`Answer::binding(&str)` looks one up. The shapes follow the solved form:
a determined numeric variable is a `Number`; an unbound or merely
constrained variable has no binding, its constraints stay in the text
fields and it appears as `Var(name)` inside other values; an alias
`A = B` binds the later variable to `Var("A")`, so each query variable
has at most one binding; a cyclic term is its equations, `X = f(X)` as a
compound containing `Var("X")`, an inner cyclic node as `Var("_1")` with
its own binding appended. No string variant: a string is a list of
one-character atoms in the value as in print.

**Rejected.** Exposing the heap (`Store`, `Addr`, `Cell`) behind a
`Solutions` lifetime — ties every consumer to the machine's
representation. Reusing `ast::Expr` — arms that cannot occur in a value.
A public term parser over the printed text — recovers what the printer
threw away. A JSON-like generic value — loses the exact rational and the
atom/string/variable distinction. Cycles by reference — a graph a
consumer cannot pattern-match, and different from the text.

**Deferred, each with its reason in the record:** disequations and
numeric constraints as values (no consumer; the numeric part needs a
linear form), proof goals as terms (genite reads explanations as
lines), a `bindings` field in the JSON answer object (a term encoding is
a contract of its own), an `as_text()` on `Term` (the printer never
guesses a string reading).

**Pre-write checks.** A design record lists no `affects` and closes no
task. `answer-soundness` is untouched: the bindings are the same solved
form the text shows, from the same store after the same `finalize`;
nothing is decided differently and nothing reported the text does not.
Cross-repo impact: the library additions, noted above; the JSON contract
is unchanged.

## Links

- Design record: `docs/design/2026-11-22-answers-as-values.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/96
- Graduated note: `docs/incubation/2026-11-20-answers-as-values.md` (renamed into the record)
- Related: `docs/design/2026-09-10-quoted-atoms.md`, `docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md`, `docs/design/2026-08-17-evaluator.md` (D7)
- Consumer: genite GEN-141, `rust/crates/genite-mind-rules/src/lib.rs`
- Implementing task: to follow as CLM-0037
