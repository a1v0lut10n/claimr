// SPDX-License-Identifier: Apache-2.0

//! Integration tests: every program under `examples/` must parse in full,
//! and malformed input must fail with a positioned error.

use claimr::{Clause, Expr, Number, parse_program};

fn read_example(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/");
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("cannot read example {name}: {e}"))
}

#[test]
fn socrates_example_parses_completely() {
    let clauses =
        parse_program(&read_example("socrates.claimr")).expect("socrates.claimr should parse");
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
    let clauses = parse_program(&read_example("nested_terms.claimr"))
        .expect("nested_terms.claimr should parse");
    assert_eq!(clauses.len(), 7);
    let Clause::Fact(likes) = &clauses[0] else {
        panic!("first clause is a fact")
    };
    assert!(matches!(likes.args[1], claimr::Expr::Atom(ref a) if a.name == "father"));
}

#[test]
fn numbers_example_parses_exactly() {
    let clauses =
        parse_program(&read_example("numbers.claimr")).expect("numbers.claimr should parse");
    assert_eq!(clauses.len(), 7);
    let Clause::Fact(apple) = &clauses[0] else {
        panic!("first clause is a fact")
    };
    assert_eq!(
        apple.args[1],
        Expr::Number(Number::from_ratio(1, 2).unwrap())
    );
    let Clause::Fact(caviar) = &clauses[2] else {
        panic!("third clause is a fact")
    };
    let Expr::Number(n) = &caviar.args[1] else {
        panic!("price is a number")
    };
    assert_eq!(n.to_string(), "4938271560493827157/4"); // exact, beyond f64 precision
    let Clause::ConstraintFact(c) = &clauses[4] else {
        panic!("fifth clause is a constraint fact")
    };
    assert_eq!(
        c.terms[0].right,
        Expr::Number(Number::from_ratio(37, 2).unwrap())
    );
}

#[test]
fn list_notation_parses_to_list_terms() {
    // CLM-0014: `<>`, `<a, b>`, `<a> . T`; `.` is right-associative and
    // binds looser than arithmetic; `<> . T` is `T`.
    let clauses = parse_program(
        "p(<>, <a, 1>, <X> . T, <a> . <b> . T, <a> . T + 1, (<a> . T) + 1, <> . T).
",
    )
    .expect("parses");
    let Clause::Fact(p) = &clauses[0] else {
        panic!("a fact")
    };
    let list = |items: Vec<Expr>, tail: Option<Expr>| Expr::List {
        items,
        tail: tail.map(Box::new),
    };
    let var = |v: &str| Expr::Var(v.to_string());
    assert_eq!(p.args[0], list(vec![], None));
    assert_eq!(
        p.args[1],
        list(
            vec![
                Expr::Ident("a".into()),
                Expr::Number(Number::from_ratio(1, 1).unwrap())
            ],
            None
        )
    );
    assert_eq!(p.args[2], list(vec![var("X")], Some(var("T"))));
    // `<a> . <b> . T` is the one chain `<a, b> . T`.
    assert_eq!(
        p.args[3],
        list(
            vec![Expr::Ident("a".into()), Expr::Ident("b".into())],
            Some(var("T"))
        )
    );
    assert!(
        matches!(&p.args[4], Expr::List { tail: Some(t), .. } if matches!(**t, Expr::Binary { .. }))
    );
    assert!(matches!(&p.args[5], Expr::Binary { left, .. } if matches!(**left, Expr::List { .. })));
    assert_eq!(p.args[6], var("T"));
    // Display round-trips the notation, parenthesising a list under arithmetic.
    assert_eq!(
        clauses[0].to_string(),
        "p(<>, <a, 1>, <X> . T, <a, b> . T, <a> . T + 1, (<a> . T) + 1, T)."
    );
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
        ("{ age(X) 18 }.\n", 1, 10),                         // relop expected, got number
        ("?- .\n", 1, 4),                                    // empty query body
        ("Human(socrates).\n", 1, 1),                        // uppercase-initial atom name
    ];
    for (src, line, col) in cases {
        let err = parse_program(src).unwrap_err();
        assert_eq!(
            (err.line, err.column),
            (Some(*line), Some(*col)),
            "for {src:?}: {err}"
        );
    }
}
