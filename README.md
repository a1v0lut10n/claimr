<p align="center">
  <img src="https://raw.githubusercontent.com/a1v0lut10n/claimr/main/static/img/claimr-logo.svg" alt="Claimr logo" width="480">
</p>

# Claimr — a constraint logic programming language

Claimr is a constraint logic programming language implemented in Rust,
inspired by Prolog III and similar constraint logic programming systems. It
combines logical reasoning with constraint solving, allowing for expressive,
declarative programs.

> The project was originally called *Claim*; it was renamed to **Claimr**
> because `claim` is already taken on crates.io. Source files use the
> `.claimr` extension.

## Features

- **Prolog-like syntax** for facts and rules
- **Constraint solving** integrated into the logic programming paradigm
- **First-class constraints** usable in facts, rules, and queries
- **Exact arithmetic** — numbers are arbitrary-precision rationals, never
  floats; `18.5` means exactly 37/2, and `+ - * /` are term constructors
  usable anywhere a term goes (as in Prolog III)
- **Implication syntax** (`{ … } => head.`) as syntactic sugar
- **Parser generated with [rustemo](https://crates.io/crates/rustemo)**, an
  LR parser generator for Rust — the grammar file is the single source of
  truth for the syntax, and syntax errors carry line/column positions

Current status: the **parser** (`src/parser/`), the **evaluator**
(`src/eval/`: SLD resolution over rational trees, `=`/`!=` on terms, answers
in solved form) and the **linear constraint solver** (`src/solver/`: exact
rational simplex, attribute terms such as `age(X)`, delayed non-linear
products) are implemented. Open: richer answer simplification, a REPL,
non-linear and finite-domain constraints.

## Grammar

The authoritative grammar is [`src/parser/claimr.rustemo`](src/parser/claimr.rustemo);
[`docs/reference/grammar.md`](docs/reference/grammar.md) is an EBNF view of it.
The top-level shape:

```ebnf
program        ::= { clause }
clause         ::= fact | rule | constraint_fact | constraint_rule | implication | query

fact           ::= atom "."
rule           ::= atom ":-" body "."
constraint_fact ::= "{" constraint_expr "}" "."
constraint_rule ::= atom ":-" body_with_constraints "."
query          ::= "?-" (body_with_constraints | "{" constraint_expr "}") "."
implication    ::= "{" constraint_expr "}" "=>" atom "."

constraint_expr ::= constraint_term { "," constraint_term }
constraint_term ::= expr relop expr
relop           ::= "=" | "!=" | "<" | ">" | "<=" | ">="

expr           ::= expr ("+" | "-") expr | expr ("*" | "/") expr | "-" expr
                 | "(" expr ")" | identifier | number | atom | variable
```

Arithmetic operators are term constructors, usable anywhere a term goes
(Prolog III style); `%` starts a comment that runs to the end of the line.

## Examples

See [`examples/socrates.claimr`](examples/socrates.claimr) for a complete
program; the integration tests parse every file under `examples/`.

```claimr
% Facts and rules
human(socrates).
mortal(X) :- human(X).

% Constraints — exact rational arithmetic, usable in terms and constraints
{ age(socrates) > 70 }.
eligible(X) :- { age(X) >= 18 }.
average(X, Y, (X + Y) / 2).
{ X + Y = 10, 2*X - Y >= 1/3 }.

% Implication sugar
{ age(X) >= 18 } => eligible(X).

% Queries
?- mortal(socrates).
?- eligible(alice), { age(alice) >= 18 }.
```

## Installation

### Prerequisites

- Rust 1.85 or newer (edition 2024) — install via [rustup](https://rustup.rs/)

### Building from source

```bash
git clone https://github.com/a1v0lut10n/claimr.git
cd claimr
cargo build --release
cargo test
```

## Usage

```bash
# Run a program: each `?-` query is answered in order
cargo run -- examples/family.claimr

# Or, after `cargo install --path .`
claimr path/to/program.claimr
claimr --limit 5 program.claimr    # cap answers per query (unlimited by default)
claimr --parse program.claimr      # dump the parsed clauses instead of running
claimr                             # the interactive loop
claimr -i program.claimr           # run the program, then continue interactively
```

Interactively, you type claimr syntax exactly as in a file — `?- goals.` is
answered, any other clause (a fact, a rule, a `{ … }.` constraint) is added
to the session — and answers are stepped Prolog-style: `;` for the next one,
Enter to stop.

```text
$ claimr
claimr> human(socrates).
claimr> human(plato).
claimr> mortal(X) :- human(X).
claimr> ?- mortal(W).
W = socrates ;
W = plato.
claimr> eligible(X) :- { age(X) >= 18 }.
claimr> ?- eligible(bob).
age(bob) >= 18.
claimr> :load examples/family.claimr
...
claimr> :quit
```

| Command | |
|---|---|
| `:load FILE` | append the file's clauses to the session and answer its queries |
| `:reload` | re-read the loaded files, dropping clauses typed at the prompt |
| `:list` | print the session's clauses |
| `:clear` | empty the session |
| `:limit N` | cap answers per query in `:all` mode (0 = unlimited) |
| `:all` | toggle between stepping answers and printing them all |
| `:help`, `:quit` | leave with `:quit`, `exit.`, Ctrl-D, or Ctrl-C twice at an empty prompt; Ctrl-C interrupts a running query |

A pipe on stdin drives the same loop: `printf '?- p(X).\n;\n' | claimr`.

Answers are printed in solved form, one per line, `true` when nothing remains
to say and `false` when a query has no answers. Constraints that remain open
are part of the answer:

```text
?- grandparent(tom, Who).
Who = ann
Who = pat
?- { X + Y = 10, X - Y = 2 }.
X = 6, Y = 4
?- average(3, 4, A).
A = 7/2
?- eligible(alice).
age(alice) >= 18
?- { X > 3 }, { X < 5 }, { X != 4 }.
X > 3, X < 5, X != 4
?- p(A).                     % p(X) :- { X = Y + Z, Y > 0, Z > 0 }.
A > 0
?- { X != Y }, same(X, f(Z)), same(Y, f(W)).
X = f(Z), Y = f(W), Z != W
?- omega(X).
X = f(X)
```

Answers are the store *projected onto the query*: internal variables are
eliminated (Gaussian substitution and Fourier–Motzkin), redundant
constraints dropped, and what remains is printed in solved form.

As a library:

```rust
use claimr::{parse_program, Program};

let clauses = parse_program("human(socrates).\nmortal(X) :- human(X).\n?- mortal(W).\n")?;
let program = Program::compile(&clauses)?;
for query in program.queries() {
    for answer in program.solve(query) {
        println!("{answer}"); // W = socrates
    }
}
```

Diagnostics are GCC-style `file:line:column: message` for syntax errors;
load errors (an unsatisfiable set of constraint facts) and runtime errors
(a non-linear constraint still undetermined at answer time — Claimr never
approximates) name the file and the query:

```text
$ claimr broken.claimr
broken.claimr:1:21: Expected one of Neq, Le, Ge, Comma, RParen, RBrace, Eq, Lt, Gt.
$ claimr nonlinear.claimr
nonlinear.claimr: in `?- { Y = X * Z }.`: non-linear constraint `X * Z` is still undetermined; claimr does not approximate (evaluator stage 3 supports linear constraints only)
```

## Project layout

```
claimr/
├── Cargo.toml
├── build.rs             # generates the parser from the grammar (rustemo)
├── src/
│   ├── lib.rs           # public API: parse_program, parse_clause, ParseError
│   ├── ast.rs           # AST types
│   ├── number.rs        # exact rational Number type (no floats)
│   ├── parser/
│   │   ├── claimr.rustemo       # THE grammar (authoritative)
│   │   ├── claimr_actions.rs    # semantic actions: productions -> ast
│   │   └── mod.rs               # includes the generated parser (OUT_DIR)
│   ├── eval/            # evaluator: store (heap, trail, dif, numeric glue), unify, compile, SLD machine, answers
│   ├── solver/          # exact linear solver: delta-rationals, linear expressions, simplex
│   ├── repl.rs          # the interactive loop
│   └── main.rs          # `claimr` CLI: run a program, --parse, or the REPL
├── examples/            # sample .claimr programs (parsed by the tests; *.answers = golden runs)
├── tests/               # integration tests
└── docs/                # documentation workflow (see docs/README.md)
    ├── reference/grammar.md
    ├── journal/  tasks/  design/  architecture/  inbox/
    └── incubation/  catalyst/
```

## Development

```bash
cargo build                                # also regenerates the parser if the grammar changed
cargo test
cargo clippy --all-targets -- -D warnings
```

To change the language: edit `src/parser/claimr.rustemo`, then adjust
`src/parser/claimr_actions.rs` (rustemo appends stubs for new productions and
preserves your edits), add an example under `examples/`, and update the EBNF
view in `docs/reference/grammar.md`.

Documentation, decision records, and the development journal follow the
aivolution documentation workflow — see [`docs/README.md`](docs/README.md).
Branches are named `<type>/CLM-NNNN-short-name`; `docs/NEXT-TICKET` holds
the next free ticket number.

## License

This project is licensed under the [Apache License 2.0](LICENSE).

Copyright 2026 Aivolution GmbH

## Contributing

Contributions are welcome — open a Pull Request:

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/CLM-NNNN-amazing-feature`)
3. Commit your changes
4. Push to the branch and open a Pull Request
