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
- **Implication syntax** (`{ … } => head.`) as syntactic sugar
- **Parser implemented with [nom](https://crates.io/crates/nom)**, a parser
  combinator library for Rust

Current status: the **parser** is implemented (`src/lib.rs`); evaluation and
constraint solving are not yet.

## Grammar

The full grammar lives in [`docs/reference/grammar.md`](docs/reference/grammar.md).
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
```

## Examples

See [`examples/socrates.claimr`](examples/socrates.claimr) for a complete
program; the integration tests parse every file under `examples/`.

```claimr
human(socrates).
mortal(X) :- human(X).

{ age(socrates) > 70 }.
eligible(X) :- { age(X) >= 18 }.

{ age(X) >= 18 } => eligible(X).

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

let (_, clauses) = parse_program("human(socrates).\n?- human(socrates).\n")?;
assert!(matches!(clauses[0], Clause::Fact(_)));
```

## Project layout

```
claimr/
├── Cargo.toml
├── src/
│   ├── lib.rs           # AST types + nom parser (library crate `claimr`)
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
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

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
