//! Integration tests: every program under `examples/` must parse in full.

use claimr::{Clause, parse_program};

fn read_example(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/");
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("cannot read example {name}: {e}"))
}

#[test]
fn socrates_example_parses_completely() {
    let src = read_example("socrates.claimr");
    let (rest, clauses) = parse_program(&src).expect("socrates.claimr should parse");
    assert!(rest.is_empty(), "unconsumed input: {rest:?}");
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
fn every_example_file_parses() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).expect("examples dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("claimr") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read example");
        parse_program(&src).unwrap_or_else(|e| panic!("{} failed to parse: {e}", path.display()));
        seen += 1;
    }
    assert!(seen > 0, "no .claimr examples found");
}
