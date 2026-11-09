---
date: 2026-11-09
type: journal
components: [parser, evaluator]
aspects: []
tasks:
  - docs/implementation/2026-11-09-clm-0019-iterative-lowering.md
---

# CLM-0019 executed: lowering, instantiation, printing and dropping of terms are iterative

- **When:** 2026-11-09 14:34:10

## Context

The SLD machine has been iterative since CLM-0005, but the AST side
recursed once per nesting level: lowering (`Lowerer::term`),
instantiation (`build_with`), the clause `Display`, the derived `Debug`
and the drop glue of `Expr` and `TTerm`. CLM-0016 measured about 6 KB of
stack per level in debug builds and left an iterative lowering as a
follow-up. On the CLI's 8 MiB stack a 10 000-deep term aborted the process
when run or echoed — and so did a 50 000-element list literal, the
realistic case, since a flat `<a, b, …>` lowers to a chain as deep as it
is long.

## Details

**What landed.** `Lowerer::term` and `build_with` walk their trees with an
explicit stack of enter/build frames and a result stack, post-order,
children entered in reverse so they are built left to right; the
instantiation's side effects (`post_neg`, `post_arith`, `post_len`,
`post_concat`) therefore occur in the same order as before. A result
stack out of step is the new `EvalError::InternalInvariant`, reported
rather than panicked and never expected. The clause printer is one
iterative engine over a work stack of expression-with-minimum-precedence,
atom and text pieces, with the parenthesisation rules unchanged; every
golden file and round-trip test is byte-identical. `Expr` and `TTerm`
implement `Drop` by moving their children into a worklist and dropping
them in a loop.

**Found on the way.** A type with `Drop` cannot be destructured by value,
so the concatenation action takes its fields by mutable reference and
`mem::take`. The build script re-emits the actions file through a
pretty-printer that predates `let … else`, and panicked on it; the action
avoids the construct, and the file says why. The derived `Debug` that
`--parse` prints is the one recursion left on the AST side; a test pins
its format, so it stays and is documented.

**Verification.** 135 tests pass. The deep-derivation test runs on a 1 MiB
thread (it needed 64 MiB); the new `deep_literals_cost_no_stack` parses,
lowers, echoes, runs and drops a 100 000-deep term and measures a
200 000-element list literal on the same thread. On the CLI's default
stack in a release build: a 10 000- and a 100 000-deep term run and echo,
a 50 000-element list measures in 0.10 s and a 200 000-element one in
0.37 s, all of which overflowed before; `--parse` of the 100 000-deep term
still overflows, as documented. Formatting, clippy with warnings denied
and the docs linter are clean.

**Pre-write checks.** `evaluator.md`, the one doc in the task's `affects`,
is current (the CLM-0016 recursion remark replaced). No aspect is touched:
what is derived and printed is unchanged, only the stack it costs. No
cross-repo impact.

## Links

- Task: `docs/implementation/2026-11-09-clm-0019-iterative-lowering.md`
- Origin of the follow-up: `docs/journal/2026-11/2026-11-09-execution-clm-0016-list-concatenation.md`
- Commits `aa861f6` (plan, ticket claim), `0c20da8` (implementation)
- PR: https://github.com/a1v0lut10n/claimr/pull/50
- View updated: `docs/architecture/components/evaluator.md`
