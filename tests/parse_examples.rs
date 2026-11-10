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
fn length_bars_parse_as_a_numeric_term() {
    // CLM-0015: `|L|` is an atomic term; the bars nest.
    let clauses =
        parse_program("p(|L|, |<a> . T| + 1, ||L||) :- { |L| >= 2 * |T| }.\n").expect("parses");
    let Clause::ConstraintRule { head, .. } = &clauses[0] else {
        panic!("a constraint rule")
    };
    assert_eq!(head.args[0], Expr::Len(Box::new(Expr::Var("L".into()))));
    assert!(matches!(&head.args[1], Expr::Binary { left, .. } if matches!(**left, Expr::Len(_))));
    assert!(matches!(&head.args[2], Expr::Len(inner) if matches!(**inner, Expr::Len(_))));
    assert_eq!(
        clauses[0].to_string(),
        "p(|L|, |<a> . T| + 1, ||L||) :- { |L| >= 2 * |T| }."
    );
}

#[test]
fn concatenation_is_right_associative_and_loosest() {
    // CLM-0016: `X . Y . W` is `X . (Y . W)`; `.` binds looser than `+`;
    // a literal tuple on the left is still a tailed list.
    let clauses = parse_program("p(X . Y . W, X . Y + 1, (X . Y) . W, <a> . X . Y, -X . Y).\n")
        .expect("parses");
    let Clause::Fact(p) = &clauses[0] else {
        panic!("a fact")
    };
    let var = |v: &str| Expr::Var(v.to_string());
    let cat = |l: Expr, r: Expr| Expr::Concat(Box::new(l), Box::new(r));
    assert_eq!(p.args[0], cat(var("X"), cat(var("Y"), var("W"))));
    assert!(matches!(&p.args[1], Expr::Concat(_, r) if matches!(**r, Expr::Binary { .. })));
    assert_eq!(p.args[2], cat(cat(var("X"), var("Y")), var("W")));
    assert_eq!(
        p.args[3],
        Expr::List {
            items: vec![Expr::Ident("a".into())],
            tail: Some(Box::new(cat(var("X"), var("Y"))))
        }
    );
    assert!(matches!(&p.args[4], Expr::Concat(l, _) if matches!(**l, Expr::Neg(_))));
    assert_eq!(
        clauses[0].to_string(),
        "p(X . Y . W, X . Y + 1, (X . Y) . W, <a> . X . Y, -X . Y)."
    );
}

#[test]
fn string_literals_are_character_lists() {
    // CLM-0017: "abc" parses to <a, b, c>; "" to <>; escapes as quoted atoms.
    let clauses = parse_program(r#"p("abc", "", "a\"b\\c", "x" . T)."#).expect("parses");
    let Clause::Fact(p) = &clauses[0] else {
        panic!("a fact")
    };
    let chars = |s: &str| -> Vec<Expr> { s.chars().map(|c| Expr::Ident(c.to_string())).collect() };
    assert_eq!(
        p.args[0],
        Expr::List {
            items: chars("abc"),
            tail: None
        }
    );
    assert_eq!(
        p.args[1],
        Expr::List {
            items: vec![],
            tail: None
        }
    );
    assert_eq!(
        p.args[2],
        Expr::List {
            items: chars("a\"b\\c"),
            tail: None
        }
    );
    assert_eq!(
        p.args[3],
        Expr::List {
            items: chars("x"),
            tail: Some(Box::new(Expr::Var("T".into())))
        }
    );
    assert_eq!(
        clauses[0].to_string(),
        r#"p(<a, b, c>, <>, <a, '"', b, '\\', c>, <x> . T)."#
    );
}

#[test]
fn the_anonymous_variable_parses_and_prints_as_written() {
    // CLM-0020: `_` is a variable named `_`; `_foo` is not a name.
    let clauses = parse_program("p(_, X, <_> . _).\n").expect("parses");
    let Clause::Fact(p) = &clauses[0] else {
        panic!("a fact")
    };
    assert_eq!(p.args[0], Expr::Var("_".into()));
    assert!(matches!(&p.args[2], Expr::List { items, tail: Some(t) }
        if items[0] == Expr::Var("_".into()) && **t == Expr::Var("_".into())));
    assert_eq!(clauses[0].to_string(), "p(_, X, <_> . _).");
    assert!(parse_program("p(_foo).\n").is_err());
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
        ("mortal(X) :- human(X.\n", 2, 1), // ')' expected; `X .` may start a concatenation (CLM-0016), so the error is at the next token
        ("{ age(X) 18 }.\n", 1, 10),       // relop expected, got number
        ("?- .\n", 1, 4),                  // empty query body
        ("Human(socrates).\n", 1, 1),      // uppercase-initial atom name
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

#[test]
fn control_constructs_parse_with_iso_precedence_and_round_trip() {
    // CLM-0022: `,` binds tightest, then `->`, then `;` (right-associative);
    // a `;` under `C -> T` is an if-then-else; parentheses group and leave
    // no trace; `\+` takes one goal. Display re-parenthesises exactly where
    // the grammar needs it.
    use claimr::{Clause, Goal, parse_clause};
    let Clause::Rule { body, .. } = parse_clause("u(X) :- a(X) ; b(X) -> c(X) ; d(X).").unwrap()
    else {
        panic!("rule");
    };
    // `a ; ((b -> c) ; d)`
    assert!(matches!(&body[..], [Goal::Or(a, rest)]
        if matches!(&a[..], [Goal::Atom(_)])
        && matches!(&rest[..], [Goal::IfThenElse { els: Some(e), .. }]
            if matches!(&e[..], [Goal::Atom(_)]))));
    let Clause::Rule { body, .. } = parse_clause("v(X) :- (a(X), b(X)), c(X).").unwrap() else {
        panic!("rule");
    };
    assert_eq!(body.len(), 3, "a grouped conjunction splices");
    let Clause::Rule { body, .. } = parse_clause("w(X) :- \\+ (a(X), b(X)), \\+ c(X).").unwrap()
    else {
        panic!("rule");
    };
    assert!(matches!(&body[..], [Goal::Not(g), Goal::Not(h)] if g.len() == 2 && h.len() == 1));
    assert!(matches!(
        parse_clause("x(X) :- (a(X) -> b(X)).").unwrap(),
        Clause::Rule { body, .. } if matches!(&body[..], [Goal::IfThenElse { els: None, .. }])
    ));
    // A constraint block anywhere inside makes a ConstraintRule.
    assert!(matches!(
        parse_clause("y(X) :- a(X) ; { X > 1 }.").unwrap(),
        Clause::ConstraintRule { .. }
    ));
    for src in [
        "p(X) :- (q(X) ; r(X)), s(X).",
        "t(X) :- \\+ (a(X), b(X)), (c(X) -> d(X)).",
        "u() :- a() ; b() -> c() ; d().",
        "v() :- (a() ; b()) ; c().",
        "w() :- \\+ \\+ a(), \\+ { X = 1 }.",
        "x() :- (a() -> b()) -> c() ; d().",
        "y() :- a() -> (b() ; c()) ; d().",
        "?- colour(X), ({ X = red } -> { W = yes } ; { W = no }).",
        "?- a() ; b().",
    ] {
        let clause = parse_clause(src).unwrap();
        assert_eq!(clause.to_string(), src);
        assert_eq!(parse_clause(&clause.to_string()).unwrap(), clause);
    }
    // Redundant parentheses vanish.
    assert_eq!(
        parse_clause("?- (colour(X), size(Y)) ; { X = none }.")
            .unwrap()
            .to_string(),
        "?- colour(X), size(Y) ; { X = none }."
    );
}

#[test]
fn comprehensions_parse_and_round_trip() {
    // CLM-0025: `<T | Body>` is an expression; the body may hold constraint
    // blocks and constructs; it nests in a template; length bars around it
    // nest as before.
    use claimr::{Clause, Expr, Goal, parse_clause};
    let Clause::ConstraintRule { body, .. } =
        parse_clause("c(N) :- { N = |<X | p(X), { X > 1 }>| }.").unwrap()
    else {
        panic!("rule");
    };
    let Goal::Constraint(ce) = &body[0] else {
        panic!("constraint");
    };
    let Expr::Len(inner) = &ce.terms[0].right else {
        panic!("length");
    };
    assert!(matches!(&**inner, Expr::Comprehension { body, .. } if body.len() == 2));
    for src in [
        "c(N) :- { N = |<X | p(X), { X > 1 }>| }.",
        "t(T) :- sum(<A | amount(_, A)>, T).",
        "?- { L = <<X, Y> | p(X), p(Y), { X < Y }> }.",
        "?- { L = <<X, |<Y | p(Y), { Y < X }>|> | p(X)> }.",
        "?- { L = <X | p(X) ; q(X)> }.",
        "?- { L = <X | \\+ p(X), q(X)> }.",
    ] {
        let clause = parse_clause(src).unwrap();
        assert_eq!(clause.to_string(), src);
        assert_eq!(parse_clause(&clause.to_string()).unwrap(), clause);
    }
}

#[test]
fn directives_parse_and_round_trip() {
    // CLM-0027: `:- table name/arity, ….` is a clause; the printer
    // round-trips it, quoting a name exactly when needed.
    use claimr::{Clause, parse_clause, parse_program};
    let clause = parse_clause(":- table ancestor/2, 'part of'/2, z/0.").unwrap();
    let Clause::Directive { name, indicators } = &clause else {
        panic!("directive");
    };
    assert_eq!(name, "table");
    assert_eq!(indicators.len(), 3);
    assert_eq!(indicators[1].name, "part of");
    assert_eq!(clause.to_string(), ":- table ancestor/2, 'part of'/2, z/0.");
    assert_eq!(parse_clause(&clause.to_string()).unwrap(), clause);
    // A rule's `:-` follows an atom; a directive's begins the clause.
    let program = parse_program(":- table a/1.\na(X) :- b(X).\n").unwrap();
    assert_eq!(program.len(), 2);
    assert!(matches!(program[1], Clause::Rule { .. }));
}
