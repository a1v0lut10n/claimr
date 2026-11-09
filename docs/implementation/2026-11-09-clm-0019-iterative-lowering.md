---
date: 2026-11-09
type: task
status: in-progress
affects:
  - docs/architecture/components/evaluator.md
components: [parser, evaluator]
aspects: []
design: []
tags: [CLM-0019, deep-terms, lists, stack, robustness]
---

# CLM-0019: Iterative lowering, instantiation, printing and dropping of terms

## Premise

The SLD machine is iterative (CLM-0005), but the AST side recurses once
per nesting level: `Lowerer::term` (compile), `build_with`
(instantiation), the clause `Display` (query echoes, `:list`), the derived
`Debug` (`--parse`), and the drop glue of `Expr` and `TTerm`. On the CLI's
8 MiB main stack a 10 000-deep term aborts the process with a stack
overflow when run or echoed, and so does a **50 000-element list
literal**, since a flat `<a, b, …>` lowers to a chain that deep — the
realistic case, for data loaded as lists. CLM-0016 found lowering costs
about 6 KB of stack per level in debug builds and left an iterative
lowering as a follow-up.

## Scope

In: explicit-stack versions of lowering, instantiation and the `Display`
of `Expr`/`Atom`; iterative `Drop` for `Expr` and `TTerm`; the actions
adapted (a type with `Drop` cannot be destructured by value); tests that
run deep terms and long lists on a small thread; `evaluator.md`.

Out: the derived `Debug` of the AST, which `--parse` prints and a test
pins by format; its recursion is noted. Parsing is already iterative (LR).

## Action items

1. **Lowering** (`src/eval/compile.rs`): `Lowerer::term` walks the `Expr`
   with an explicit stack of enter/build frames and a result stack,
   post-order, so children are built before their parent in source order;
   `List` builds its chain from the item results. `atom` and `constraint`
   call it; no recursion remains.
2. **Instantiation** (`build_with`): the same frames over `TTerm` with an
   address result stack; side effects (`post_neg`, `post_arith`,
   `post_len`, `post_concat`) occur in the same post-order as before.
3. **Printing** (`src/ast.rs`): one iterative engine renders `Expr` and
   `Atom` from a work stack of expression-with-minimum-precedence, atom,
   and text items; parenthesisation is unchanged (same precedence rules,
   same output). `Expr::fmt` and `Atom::fmt` call it.
4. **Dropping**: `impl Drop for Expr` and `for TTerm` move the children
   into a worklist and drop them in a loop, so a chain of any depth drops
   in constant stack. The actions' `expr_concat_expr` destructures by
   mutable reference and `mem::take` instead of by value.
5. **Tests**: the deep-derivation test loses its 64 MiB thread and runs
   on a 1 MiB one, with a 10 000-deep term parsed, compiled, echoed and
   run; a new test lowers and runs a 200 000-element list literal and
   prints a 100 000-deep term, on the same small thread. Golden examples
   unchanged.
6. **Docs**: `evaluator.md` (the recursion remark from CLM-0016 replaced);
   this plan.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean; every golden file byte-identical.
- [ ] The CLI runs a 100 000-deep term and a 200 000-element list literal
      on its default stack.
- [ ] `aivolution lint .` clean; `evaluator.md` current.
