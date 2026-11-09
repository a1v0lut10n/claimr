---
date: 2026-11-09
type: task
status: done
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
   with an explicit stack of `Walk::Enter`/`Walk::Build` frames and a
   result stack, post-order, children entered in reverse so they are built
   left to right; `List` builds its chain from the item results. A result
   stack out of step is `EvalError::InternalInvariant` (new variant,
   reported rather than panicked; never expected). **Done.**
2. **Instantiation** (`build_with`): the same frames over `TTerm` with an
   address result stack; `post_neg`, `post_arith`, `post_len`,
   `post_concat` run in the same post-order as before. **Done.**
3. **Printing** (`src/ast.rs`): `render` drives a work stack of
   `Piece::Expr(e, min_prec)`, `Piece::Atom`, `Piece::Text`; `Expr::fmt`
   and `Atom::fmt` call it; parenthesisation rules unchanged, every golden
   file and the parse tests' round trips byte-identical. **Done.**
4. **Dropping**: `impl Drop for Expr` and `for TTerm` move children into a
   worklist (`take_children`) and drop them in a loop. `expr_concat_expr`
   destructures by mutable reference and `mem::take`; it also avoids
   `let … else`, which the pretty-printer rustemo re-emits the actions
   file through does not know (the build script panicked on it).
   **Done.**
5. **Tests**: `deep_derivations_do_not_touch_the_rust_stack` runs on a
   1 MiB thread (was 64 MiB); `deep_literals_cost_no_stack` parses,
   lowers, echoes, runs and drops a 100 000-deep term and measures a
   200 000-element list literal on the same 1 MiB thread. 135 tests.
   **Done.**
6. **Docs**: `evaluator.md` (the CLM-0016 recursion remark replaced); this
   plan. **Done.**

## Measured on the CLI (release, default 8 MiB stack)

| Input | before | after |
|---|---|---|
| 10 000-deep term, run / echo | stack overflow | `true` / prints |
| 100 000-deep term, run / echo | stack overflow | `true` / prints |
| 50 000-element list literal, `|L|` | stack overflow | `N = 50000`, 0.10 s |
| 200 000-element list literal, `|L|` | stack overflow | `N = 200000`, 0.37 s |
| `--parse` of a 100 000-deep term | stack overflow | stack overflow (derived `Debug`, documented) |

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` clean (135 tests); every golden file byte-identical.
- [x] The CLI runs a 100 000-deep term and a 200 000-element list literal
      on its default stack.
- [x] `aivolution lint .` clean; `evaluator.md` current.
