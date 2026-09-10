// SPDX-License-Identifier: Apache-2.0

//! CLM-0011: quoted atoms — identity is content, quoting is spelling
//! (docs/design/2026-09-10-quoted-atoms.md).

use claimr::{Clause, Expr, display_name, name_is_plain, parse_program};

#[test]
fn quoted_spellings_carry_content_and_round_trip() {
    // The AST carries CONTENT: outer quotes gone, escapes resolved.
    let clauses = parse_program(
        "x('situation:frame-in-spec').\nx('it\\'s a test').\nx('a\\\\b').\nx('').\nx(abc).\n",
    )
    .unwrap();
    let names: Vec<String> = clauses
        .iter()
        .map(|c| match c {
            Clause::Fact(atom) => match &atom.args[0] {
                Expr::Ident(name) => name.clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        names,
        vec!["situation:frame-in-spec", "it's a test", "a\\b", "", "abc"]
    );
    // Print-back quotes exactly when needed and escapes what it must —
    // every printed form re-parses to the same content.
    for (content, printed) in [
        ("abc", "abc"),
        ("situation:frame-in-spec", "'situation:frame-in-spec'"),
        ("it's a test", "'it\\'s a test'"),
        ("a\\b", "'a\\\\b'"),
        ("", "''"),
        ("Abc", "'Abc'"),
    ] {
        assert_eq!(display_name(content), printed, "{content}");
    }
    assert!(name_is_plain("a_b1") && !name_is_plain("A") && !name_is_plain("a-b"));
    // A quoted and a plain spelling of the same content are ONE atom:
    // the parsed clauses for x('abc') and x(abc) are equal.
    let a = parse_program("x('abc').").unwrap();
    let b = parse_program("x(abc).").unwrap();
    assert_eq!(a, b);
}
