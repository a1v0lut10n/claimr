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
atom           ::= name "(" [ args ] ")"
args           ::= expr { "," expr }

name           ::= identifier | quoted_atom     (* CLM-0011: identity is content *)

expr           ::= expr "+" expr          (* left-assoc *)
                 | expr "-" expr          (* left-assoc *)
                 | expr "*" expr          (* left-assoc *)
                 | expr "/" expr          (* left-assoc *)
                 | "-" expr               (* unary minus, binds tightest *)
                 | "(" expr ")"
                 | name
                 | number
                 | atom
                 | variable
                 | tuple                  (* CLM-0014: a list *)
                 | expr "." expr          (* CLM-0016: concatenation; right-assoc, lowest *)
                 | "|" expr "|"           (* CLM-0015: the length of a list *)
                 | string                 (* CLM-0017: a list of characters *)

tuple          ::= "<" [ args ] ">"

identifier     ::= letter { letter | digit | "_" }
quoted_atom    ::= "'" { any character except "'" and "\\" | "\\" any character } "'"
                   (* backslash escapes the next character: \' \\ *)
string         ::= '"' { any character except '"' and "\\" | "\\" any character } '"'
                   (* same escapes: \" \\ ; denotes a list of one-character atoms *)
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

**Lists** (CLM-0014, stage 1 of
[`../design/2026-11-09-lists-as-a-constraint-domain.md`](../design/2026-11-09-lists-as-a-constraint-domain.md))
are Prolog III tuples: `<>` is the empty list, `<a, b, c>` a list, and
`<a, b> . T` the chain `a`, `b` then `T` — the cons pattern when written in a
head (`member(X, <X> . T).`). The concatenation `.` is right-associative and
binds looser than every arithmetic operator (`<a> . T + 1` is `<a> . (T + 1)`,
`-X . Y` is `(-X) . Y`). `<a> . <b> . T` is the one chain `<a, b> . T`, and
`<> . T` is `T`. A list is the chain of binary cells
labelled `'<>'` ending in the constant `'<>'`, so `'<>'(a, '<>')` is the same
term as `<a>`; answers print every such chain in the tuple notation
(`<1, 2, 3>`, `<1, 2> . T`, `<>`), a cyclic chain as an equation
(`X = <1> . X`). `'<>'` is reserved: a program cannot define a predicate of
that name. `<` and `>` keep their relational meaning in constraints and `.`
its terminator meaning after a clause; a term is never followed by either, so
the two roles never meet. A number literal needs digits on both sides of its
point, so `<1> . 2` is a concatenation and `1.5` a number.

**Length** (CLM-0015, stage 2 of the same record): `|L|` is a numeric term,
admissible wherever a term is, denoting the length of the list `L`. Its
value lives in the constraint store like any number, so `{ |L| >= 2 }`,
`{ |Z| = |X| + |Y| }` and `{ |L| = N / 2 }` are ordinary linear constraints
and residual constraints on lengths print as such (`|T| >= 1`). The length
of a complete list folds to the number (`|<a, b>|` is `2`); the length of a
partial list `<a> . T` is `1 + |T|`; two occurrences of `|T|` are one
unknown. A length is implicitly non-negative, and one that becomes
determined must be an integer, or the branch fails; the list of a
determined length becomes a tuple of fresh variables (`{ |L| = 3 }` answers
`L = <_1, _2, _3>`), Prolog III's solved form. `|L|` fails on a cyclic chain
and on anything that is not a list (a number, an atom, another compound),
so `||L||` fails. The bars nest in the grammar; a bar opens a term and the
next unmatched bar closes it.

**Concatenation** (CLM-0016, stage 3 of the same record): `X . Y` with any
left operand denotes the concatenation of two lists. When `X`'s spine is
complete at the moment the term is built, the result is the chain of `X`'s
elements followed by `Y`, as for a literal tuple. Otherwise the term denotes a
list `Z` with the relation `|Z| = |X| + |Y|` in the store at once, and the
cells are unified as soon as `|X|` is known — from `X`'s spine or from the
simplex (`{ Z = X . <c>, Z = <a, b, c> }` gives `X = <a, b>`;
`{ |Z| = 10, <1, 2, 3> . Z = Z . <2, 3, 1> }` is solved, not searched). A
concatenation is never searched: `append/3` remains the predicate for that. A
pending concatenation prints as the residual `Z = X . Y`. A non-list operand
and a cyclic left operand fail. One consequence for diagnostics: since `X .`
can begin a concatenation, a clause missing its `)` before the terminator
(`human(X.`) is reported at the token after the `.`, not at the `.`.

**Strings** (CLM-0017,
[`../design/2026-11-09-strings-as-character-lists-and-type-relations.md`](../design/2026-11-09-strings-as-character-lists-and-type-relations.md))
are not a term kind: `"abc"` denotes the list of one-character atoms
`<a, b, c>`, `""` denotes `<>`, and `"doc:x"` is `<d, o, c, ':', x>`. The
two spellings are one term, so a string prints as the list it is, in answers
and in query echoes alike (Prolog III prints `<'M', 'a', 'x', 9/5, meters,
1'>`, never a string). String concatenation and length are list
concatenation and length. The bridge between an atom and its characters is
the built-in `atom_chars/2`; the type relations `number/1`, `atom/1` and
`list/1` are the other built-ins, see [`builtins.md`](builtins.md). They are
goals, not grammar: `Goal` already parses `atom(X)`.

Semantics of `number`: a literal denotes an **exact rational** — `18.5` is
37/2, `0.10` is 1/10 — and integers are the denominator-1 case. There is no
floating-point type in the language (`exact-arithmetic` aspect; decision in
[`../design/2026-08-17-exact-rational-arithmetic.md`](../design/2026-08-17-exact-rational-arithmetic.md)).
Non-integers print as fractions in lowest terms (`33/32`).
