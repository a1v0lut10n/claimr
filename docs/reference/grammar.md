# Claimr grammar

EBNF view of the Claimr language (Prolog III inspired). **The authoritative
grammar is [`src/parser/claimr.rustemo`](../../src/parser/claimr.rustemo)** —
the parser is generated from that file at build time, so it cannot drift from
it; this document is kept in step by hand for readers who prefer EBNF. Change
the grammar there first, then the actions (`src/parser/claimr_actions.rs`),
then add an example under `examples/` (the test suite parses every `.claimr`
file there), then update this view.

Notation: `{ x }` = zero or more, `[ x ]` = optional, `|` = alternative.
Whitespace between tokens is insignificant, as are line comments: `%` to the
end of the line, anywhere whitespace may appear.

Two notational differences between this EBNF and the rustemo grammar, neither
changing the language: `rule` and `constraint_rule` share one syntactic shape
there (the clause is classified by whether its body contains a `{ … }` goal),
and a query's bare `{ constraint_expr }` form is a body with a single
constraint goal.

## Top-Level Structure

```ebnf
program        ::= { clause }
clause         ::= fact | rule | constraint_fact | constraint_rule | implication | query
```

## Facts and Rules

```ebnf
fact           ::= atom "."
rule           ::= atom ":-" body "."
```

## Constraint Facts and Rules

```ebnf
constraint_fact     ::= "{" constraint_expr "}" "."
constraint_rule     ::= atom ":-" body_with_constraints "."
```

## Queries

```ebnf
query          ::= "?-" (body_with_constraints | "{" constraint_expr "}") "."
```

## Implication Syntax (optional sugar)

```ebnf
implication    ::= "{" constraint_expr "}" "=>" atom "."
```

## Bodies

```ebnf
body                 ::= goal { "," goal }
body_with_constraints ::= extended_goal { "," extended_goal }

goal                 ::= atom
extended_goal        ::= goal | "{" constraint_expr "}"
```

## Constraints

```ebnf
constraint_expr ::= constraint_term { "," constraint_term }
constraint_term ::= expr relop expr
relop           ::= "=" | "!=" | "<" | ">" | "<=" | ">="
```

## Atoms and Terms

```ebnf
atom           ::= identifier "(" [ args ] ")"
args           ::= expr { "," expr }

expr           ::= expr "+" expr          (* left-assoc, lowest *)
                 | expr "-" expr          (* left-assoc, lowest *)
                 | expr "*" expr          (* left-assoc *)
                 | expr "/" expr          (* left-assoc *)
                 | "-" expr               (* unary minus, binds tightest *)
                 | "(" expr ")"
                 | identifier
                 | number
                 | atom
                 | variable

identifier     ::= letter { letter | digit | "_" }
variable       ::= uppercase_letter { letter | digit | "_" }
number         ::= digit { digit } [ "." digit { digit } ]
comment        ::= "%" { any character except newline }
```

Arithmetic operators are **term constructors** (Prolog III): they are admitted
wherever a term is — atom arguments (`p(X + 1)`) as well as constraint
operands (`{ 2*X - Y >= 1/3 }`). Precedence is unary minus, then `*` `/`, then
`+` `-`; binary operators associate to the left; parentheses group and leave no
trace in the AST. Negative numbers are unary minus applied to a literal. The
parser does not fold constants (`1/3` is a division node); an arithmetic term
denotes a number at evaluation time and its operands must then be numbers —
see [`../design/2026-08-17-arithmetic-in-terms.md`](../design/2026-08-17-arithmetic-in-terms.md).
Linearity is a solver check, not a grammar rule.

Semantics of `number`: a literal denotes an **exact rational** — `18.5` is
37/2, `0.10` is 1/10 — and integers are the denominator-1 case. There is no
floating-point type in the language (`exact-arithmetic` aspect; decision in
[`../design/2026-08-17-exact-rational-arithmetic.md`](../design/2026-08-17-exact-rational-arithmetic.md)).
Non-integers print as fractions in lowest terms (`33/32`).
