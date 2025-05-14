# Claim - A Constraint Logic Programming Language

Claim is a constraint logic programming language implemented in Rust, inspired by Prolog III and similar constraint logic programming systems. It combines logical reasoning with constraint solving capabilities, allowing for expressive and declarative programming.

## Features

- **Full Prolog-like syntax** for facts and rules
- **Constraint solving** integrated into the logic programming paradigm
- **First-class constraints** that can be used in facts, rules, and queries
- **Support for implication syntax** as syntactic sugar
- **Parser implemented with nom**, a powerful parser combinator library for Rust

## Grammar

Claim's syntax is defined by a formal grammar that supports the following constructs:

### Top-Level Structure

```ebnf
program        ::= { clause }
clause         ::= fact | rule | constraint_fact | constraint_rule | implication | query
```

### Facts and Rules

```ebnf
fact           ::= atom "."
rule           ::= atom ":-" body "."
```

### Constraint Facts and Rules

```ebnf
constraint_fact     ::= "{" constraint_expr "}" "."
constraint_rule     ::= atom ":-" body_with_constraints "."
```

### Queries and Implications

```ebnf
query          ::= "?-" (body_with_constraints | "{" constraint_expr "}") "."
implication    ::= "{" constraint_expr "}" "=>" atom "."
```

### Constraints

```ebnf
constraint_expr ::= constraint_term { "," constraint_term }
constraint_term ::= expr relop expr
relop           ::= "=" | "!=" | "<" | ">" | "<=" | ">="
```

## Examples

### Simple Facts and Rules

```claim
human(socrates).
mortal(X) :- human(X).
```

### Constraints example

```claim
{ age(socrates) > 70 }.
eligible(X) :- { age(X) >= 18 }.
```

### Implications

```claim
{ age(X) >= 18 } => eligible(X).
```

### Queries

```claim
?- mortal(socrates).
?- eligible(alice), { age(alice) >= 18 }.
```

## Installation

### Prerequisites

- Rust (1.48 or newer)
- Cargo (comes with Rust)

### Building from Source

Clone the repository and build with Cargo:

```bash
# Clone the repository
git clone <repository-url>
cd claim

# Build the project
cargo build --release

# Run tests
cargo test
```

## Usage

```bash
# Run the Claim interpreter
cargo run

# Execute a Claim program from a file
cargo run -- path/to/program.claim
```

## Development

Claim is built using:

- **nom**: For parsing the language grammar
- **Rust's type system**: For representing the language AST
- **Immutable data structures**: For efficient constraint solving

### Project Structure

- `src/lib.rs`: Core language parser and data structures
- `src/claim_grammar.txt`: Formal grammar definition
- `src/main.rs`: CLI entry point

## License

[MIT License](LICENSE)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
