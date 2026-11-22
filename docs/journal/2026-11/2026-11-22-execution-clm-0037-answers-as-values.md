---
date: 2026-11-22
type: journal
components: [evaluator]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-22-clm-0037-answers-as-values.md
design:
  - docs/design/2026-11-22-answers-as-values.md
meta-impact:
  note: "The `claimr` library now exposes `Term` (`Atom`, `Number`, `Var`, `Compound`, `List`), `Binding`, `Answer::bindings` and `Answer::binding(&str)`. The text fields, the batch output and every `--json` document are unchanged, so aicogito's adapter sees no difference; genite's rule runner (GEN-141) can drop its ` = ` splitting and atom unquoting for `answer.binding(\"Rule\")` matched on `Term::Atom`. Ships in 0.8.0."
---

# CLM-0037 executed: answers as values

- **When:** 2026-11-22 12:53:11

## Context

The record accepted this morning (PR #97): an answer keeps its text and
also carries its bindings as values, so that a program linking the crate
reads the term a query variable is bound to instead of running the
printer backwards on its output. One task, in the answer printer.

## Details

**What was built.** A public `Term` — `Atom(String)` with the name as
content, `Number(Number)`, `Var(String)` by display name, `Compound`,
`List(items, tail)` — and `Binding { name, value }`. The printer's
`render` became `term(addr).to_string()`: an iterative builder walks the
heap with the same naming and cycle rules as before (a determined
numeric variable is a `Number`, the empty-list constant is `List([],
None)`, a `'<>'/2` spine is cut at a cyclic cell as the text cuts it, a
cyclic node reached inside a value is `Var(name)` and queued for its own
equation), and `Term`'s `Display` writes the solved-form syntax, so every
equation, disequation and constraint string is now a term displayed.
`render_answer` pushes a `Binding` beside each equation: the term for a
bound variable, `Number` for a determined one, `(later, Var(earlier))`
for an alias whose text keeps `earlier = later`, and `(_N, term)` for
each named cyclic node after the query variables. `Answer::binding`
looks one up; an unbound or constrained variable returns `None`.
`Display` and `Drop` are iterative, as `ProofNode`'s are. Disequations
and constraints stay text, as the record scopes.

**Verified.** 232 tests pass, six new in `bindings`: a quoted atom, a
number and a nested compound with the text equal to each binding
rendered; a free inner variable with no binding of its own, a merely
constrained variable with none, a determined numeric variable as a
number; aliases `X = Y` and `X = Y, X = Z` binding `Y` and `Z` to
`Var("X")`; the empty, proper and improper list and a string literal as
a list of one-character atoms; `X = f(X)`, `X = <1> . X`, and an inner
cyclic node bound as `_1` after `X`; and the invariant over every
example program — each equation is its binding rendered (or the alias
orientation), bindings and equations the same length, each query
variable at most once. The existing 100 000-deep-term test on a 1 MiB
stack now exercises `Term`'s `Display` and `Drop`. Every golden file is
byte-identical; fmt, clippy and lint clean.

**Findings.** The text did not move: rebuilding the printer over values
left all 226 prior tests and goldens identical on the first run. One
`expect` stays as a structural invariant (the builder leaves exactly one
value for the root), in the style of `machine.rs`; the improper-list tail
needed none. `Drop` on `Term` means a consumer destructures by
reference, which the README snippet shows. The three `Answer` literals in
`json_out.rs`'s tests were the only struct literals in the crate.

**Pre-write checks.** The task's two `affects` docs were updated in the
PR: `evaluator.md` (public surface and the `answer.rs` paragraph) and the
README (the library snippet reads a binding). `answer-soundness` is
untouched: the bindings are the same solved form the text shows, from
the same store; no decision the machine takes changes. Cross-repo impact:
the library additions, noted above; the JSON contract is unchanged.

## Links

- Task: `docs/implementation/2026-11-22-clm-0037-answers-as-values.md`
- Design: `docs/design/2026-11-22-answers-as-values.md`
- PR #98: commits 6eeecc3 (plan), bec5c9b (implementation)
- Consumer: genite GEN-141, `rust/crates/genite-mind-rules/src/lib.rs`
- Changelog: `CHANGELOG.md`, `[Unreleased]`
