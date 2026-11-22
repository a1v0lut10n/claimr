---
date: 2026-11-22
type: task
status: in-progress
affects:
  - docs/architecture/components/evaluator.md
  - README.md
components: [evaluator]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-22-answers-as-values.md
tags: [CLM-0037, answers, bindings, term, library, solved-form]
---

# CLM-0037: Answers as values

## Premise

The accepted record `2026-11-22-answers-as-values.md`: an `Answer` keeps
its text fields and also carries its bindings as values — a public
`Term` (`Atom`, `Number`, `Var`, `Compound`, `List`) whose `Display` is
the solved-form renderer, `Answer::bindings: Vec<Binding>` with one
entry per query variable that has an equation and one per named cyclic
node, and `Answer::binding(&str) -> Option<&Term>`. The shapes follow
the solved form: a determined numeric variable is a `Number`, an unbound
or constrained variable has no binding, an alias binds the later
variable to `Var(earlier)`, a cyclic term is its equations.

## Shape of the implementation

**One walk, two outputs.** `Printer::render(addr) -> String` becomes a
wrapper over a new `Printer::term(addr) -> Term`, built iteratively
(a work stack of visits and finishes over a value stack) with the same
naming and cycle rules the text renderer has today: a numeric variable
with a determined value is a `Number`; an unbound variable is `Var` under
its fixed name (`name_for`); the empty-list constant is `List([], None)`;
a `'<>'/2` chain is `List(items, tail)` with the spine cut at a cyclic
cell as `push_list` cuts it; a cyclic compound reached inside a value is
`Var(name)`, freshly named and queued for its own equation when first
met. The text of every equation, disequation and constraint is then
`Term`'s `Display`, so the two cannot disagree. `Display` and `Drop` on
`Term` are iterative, like `ProofNode`'s; the existing
`deep_literals_cost_no_stack` test (a 100 000-deep term rendered and
dropped on a 1 MiB stack) covers them.

**`render_answer`.** Pass 3 pushes a `Binding { name, value }` beside
each equation: for a bound variable, the term; for a determined numeric
variable, `Number`; for an alias, the text keeps `earlier = later` and
the binding is `(later, Var(earlier))`. Pass 5 and the explanation pass
push a binding `(_N, term)` beside each cyclic node's equation. Nothing
else changes: disequations and constraints stay text.

**Public surface.** `Term`, `Binding` in `answer.rs`, re-exported from
`eval` and the crate root; `Answer.bindings` and `Answer::binding`.
`Term` derives `Debug`, `Clone`, `PartialEq`, `Eq`; it implements `Drop`,
so a consumer destructures it by reference.

## Action items

1. `answer.rs`: `Term` (+ iterative `Display`, `Drop`), `Binding`,
   `Printer::term`, `render` as its wrapper, `Answer.bindings`,
   `Answer::binding`; exports in `eval/mod.rs` and `lib.rs`.
2. Tests (`src/eval/tests.rs`, module `bindings`): the shapes — a tree
   with a quoted atom, a number and a compound; an unbound inner variable
   (`X = f(Y)`, `Y` without a binding); a determined numeric variable; an
   alias; the empty list, a proper list, an improper list, a string
   literal as a list of one-character atoms; `X = f(X)`; `X = <1> . X`; an
   inner cyclic node named `_1` with its own binding; `binding()` on an
   absent and a present name. The invariant over every example program:
   each non-alias equation equals `name = value` rendered, bindings in
   equation order, each query variable at most once.
3. Docs: `evaluator.md` (public surface, the renderer paragraph),
   README (the library snippet reads a binding), `CHANGELOG.md`
   (`[Unreleased]`, Added), this plan.

## Findings

(filled in as the work proceeds)

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test`; every golden file byte-identical (the text is unchanged).
- [ ] The invariant test over `examples/*.claimr`.
- [ ] `aivolution lint .` clean; the two `affects` docs current.
