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

Current status: the **parser** is implemented (`src/parser/`); evaluation and
constraint solving are not yet.

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
# Parse a Claimr program and print its clauses
cargo run -- examples/socrates.claimr

# Or, after `cargo install --path .`
claimr path/to/program.claimr
```

As a library:

```rust
use claimr::{parse_program, Clause};

let clauses = parse_program("human(socrates).\n?- human(socrates).\n")?;
assert!(matches!(clauses[0], Clause::Fact(_)));
```

Syntax errors come back as `claimr::ParseError` with `line`/`column`:

```text
$ claimr broken.claimr
broken.claimr:1:21: Expected one of Neq, Le, Ge, Comma, RParen, RBrace, Eq, Lt, Gt.
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
│   └── main.rs          # `claimr` CLI: parse a file, print clauses
├── examples/            # sample .claimr programs (also parsed by the tests)
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

[MIT License](LICENSE)

## Contributing

Contributions are welcome — open a Pull Request:

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/CLM-NNNN-amazing-feature`)
3. Commit your changes
4. Push to the branch and open a Pull Request
