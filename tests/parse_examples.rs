//! Integration tests: every program under `examples/` must parse in full,
//! and malformed input must fail with a positioned error.

use claimr::{Clause, parse_program};

fn read_example(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/");
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("cannot read example {name}: {e}"))
}

#[test]
fn socrates_example_parses_completely() {
    let clauses = parse_program(&read_example("socrates.claimr")).expect("socrates.claimr should parse");
    assert_eq!(clauses.len(), 7);
    assert!(matches!(clauses[0], Clause::Fact(_)));
    assert!(matches!(clauses[1], Clause::Rule { .. }));
    assert!(matches!(clauses[2], Clause::ConstraintFact(_)));
    assert!(matches!(clauses[3], Clause::ConstraintRule { .. }));
    assert!(matches!(clauses[4], Clause::Implication { .. }));
    assert!(matches!(clauses[5], Clause::Query(_)));
    assert!(matches!(clauses[6], Clause::Query(_)));
}

#[test]
fn nested_terms_example_parses_completely() {
    let clauses = parse_program(&read_example("nested_terms.claimr")).expect("nested_terms.claimr should parse");
    assert_eq!(clauses.len(), 7);
    let Clause::Fact(likes) = &clauses[0] else { panic!("first clause is a fact") };
    assert!(matches!(likes.args[1], claimr::Expr::Atom(ref a) if a.name == "father"));
}

#[test]
fn every_example_file_parses() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).expect("examples dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("claimr") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read example");
        parse_program(&src).unwrap_or_else(|e| panic!("{}:{e}", path.display()));
        seen += 1;
    }
    assert!(seen > 0, "no .claimr examples found");
}

#[test]
fn errors_are_positioned() {
    let cases: &[(&str, usize, usize)] = &[
        ("human(socrates)\nmortal(X) :- human(X).\n", 2, 1), // missing '.' — next token starts line 2
        ("mortal(X) :- human(X.\n", 1, 21),                  // ')' expected, got '.'
        ("{ age(X) 18 }.\n", 1, 10),                          // relop expected, got number
        ("?- .\n", 1, 4),                                     // empty query body
        ("Human(socrates).\n", 1, 1),                         // uppercase-initial atom name
    ];
    for (src, line, col) in cases {
        let err = parse_program(src).unwrap_err();
        assert_eq!((err.line, err.column), (Some(*line), Some(*col)), "for {src:?}: {err}");
    }
}
