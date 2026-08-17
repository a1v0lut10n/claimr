---
date: 2026-08-17
type: design
status: accepted
components: [grammar, parser, evaluator, solver]
aspects: [grammar-authority, exact-arithmetic]
tags: [grammar, arithmetic, terms, prolog-iii]
---

# Arithmetic operators are term constructors, admissible anywhere a term goes

## Context

The evaluator design record (`2026-08-17-evaluator.md`, D6) introduces
arithmetic "in constraint expressions": `+ - * /`, unary minus and
parentheses as operands of `{ … }` relations, with a separate `Arith` AST
type kept apart from terms. The alternative is Prolog III's: operations are
part of the term language — a tree like `x+y` denotes a number when its
operands are numbers, is undefined otherwise, and may appear as an argument
of any term (`Sequence(<x, y>), {z = x*2}` and `f(x+1)` are both ordinary
Prolog III). Choosing between them now matters because it fixes the AST
shape stage 1 (CLM-0004) builds and the store's view of terms.

The constraint-only variant keeps the AST simpler and makes D4's
attribute-term rule ("any non-numeric, non-variable term in a numeric
position is an attribute term") purely syntactic. The Prolog III variant is
more expressive (`p(X+1)`, `f(2*Y)`), matches the reference design, and
avoids a second, artificial expression grammar for the same operators.

## Decision

- **Arithmetic is part of the term grammar.** `Expr` (a term) admits
  `Expr + Expr`, `Expr - Expr`, `Expr * Expr`, `Expr / Expr`, unary `- Expr`
  and `( Expr )`, with the usual precedence (`* /` over `+ -`, unary minus
  tightest) and left associativity, **anywhere a term is allowed** — atom
  arguments, constraint operands, and (later) query answers. There is one
  expression grammar and one AST type; no separate `Arith`.
- **AST**: `Expr` gains `Neg(Box<Expr>)` and
  `Binary { op: ArithOp, left: Box<Expr>, right: Box<Expr> }` with
  `ArithOp = Add | Sub | Mul | Div`. `Constraint { left: Expr, op: RelOp,
  right: Expr }` keeps its shape. No constant folding in the parser
  (`1/3` stays a `Div` node); folding is exact and happens when the
  evaluator compiles a clause.
- **Meaning, refining D4 of the evaluator record**: an arithmetic term
  denotes a number. When the evaluator meets `t1 op t2` or `-t` in any
  position, it introduces a fresh numeric variable `N` and the linear
  equation `N = t1 op t2` (each operand itself a number, a numeric-typed
  variable, an attribute term, or another arithmetic term), and uses `N`
  in the term's place. Operands are numeric-typed by that equation, exactly
  as Prolog III's `+` "constrain[s] x and y to denote numbers"; a
  non-numeric operand makes the store unsatisfiable (failure, not an
  error). Multiplication and division require one operand to be a
  constant at solve time (linearity is a solver check, per D5/D6); `/` is
  exact over Q. The attribute-term rule of D4 applies to
  **non-arithmetic** compound and constant terms in numeric positions;
  arithmetic terms are never attribute terms.

## Consequences

- Stage 1 (CLM-0004) implements the grammar and AST as above; the
  evaluator record's D6 is read with this refinement, and its D4
  attribute-term rule excludes arithmetic terms.
- The grammar's `Args` production needs no change beyond `Expr` itself
  growing; `p(X+1)` parses. Ambiguities are resolved by rustemo priorities;
  zero LR conflicts remains the requirement.
- Unification of arithmetic terms is not structural: `f(X+1) = f(2)` holds
  iff `X+1 = 2` in the store — the compile step's fresh-variable lowering
  makes this automatic (the term is `f(N)` with `N = X+1`), which is why
  lowering happens at compile time rather than in the unifier.
- Printing must render arithmetic terms with minimal parentheses.
