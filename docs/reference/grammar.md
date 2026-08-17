# Claimr grammar

Formal grammar of the Claimr language (Prolog III inspired). This document is
**authoritative**: `src/lib.rs` implements it and `README.md` quotes an excerpt.
Change the grammar here first, then the parser, then add an example under
`examples/` (the test suite parses every `.claimr` file there).

Notation: `{ x }` = zero or more, `[ x ]` = optional, `|` = alternative.
Whitespace between tokens is insignificant. There is currently no comment syntax.

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

expr           ::= identifier
                 | number
                 | atom
                 | variable

identifier     ::= letter { letter | digit | "_" }
variable       ::= uppercase_letter { letter | digit | "_" }
number         ::= digit { digit } [ "." digit { digit } ]
```
