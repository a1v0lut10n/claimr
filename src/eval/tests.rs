// SPDX-License-Identifier: Apache-2.0

//! Machine-level tests through the public API.

use crate::{EvalError, Program, parse_program, parse_program_spanned};

fn answers(src: &str) -> Vec<Vec<String>> {
    let clauses = parse_program(src).expect("parses");
    let program = Program::compile(&clauses).expect("compiles");
    program
        .queries()
        .iter()
        .map(|q| program.solve(q).map(|a| a.to_string()).collect())
        .collect()
}

#[test]
fn facts_rules_and_backtracking() {
    let out = answers(
        "parent(tom, bob). parent(tom, liz). parent(bob, ann).
         grandparent(X, Z) :- parent(X, Y), parent(Y, Z).
         ?- parent(tom, X).
         ?- grandparent(tom, W).
         ?- parent(ann, X).",
    );
    assert_eq!(out[0], vec!["X = bob", "X = liz"]);
    assert_eq!(out[1], vec!["W = ann"]);
    assert!(out[2].is_empty());
}

#[test]
fn recursion_over_lists() {
    let out = answers(
        "append(nil, L, L).
         append(cons(H, T), L, cons(H, R)) :- append(T, L, R).
         ?- append(X, Y, cons(a, cons(b, nil))).",
    );
    assert_eq!(
        out[0],
        vec![
            "X = nil, Y = cons(a, cons(b, nil))",
            "X = cons(a, nil), Y = cons(b, nil)",
            "X = cons(a, cons(b, nil)), Y = nil",
        ]
    );
}

#[test]
fn lists_print_in_their_notation() {
    // CLM-0014: `<>` chains print as `<a, b>`, `<a, b> . T`, `<>`; a cycle
    // prints as an equation; a hand-spelled `'<>'` cell is the same term.
    let out = answers(
        "first(X, <X> . T).
         ?- first(X, <1, 2, 3>).
         ?- { L = <1, 2> . T }.
         ?- { X = <a> . <b> . T }.
         ?- { X = <1> . X }.
         ?- { X = <> }.
         ?- { X = '<>'(a, '<>') }.
         ?- { '<>'(a, '<>') = <a> }.
         ?- { X = <1> . 2 }.
         ?- { X = '<>'(a, b, c) }.
         ?- { X = <a> . T + 1 }.",
    );
    assert_eq!(out[0], vec!["X = 1"]);
    assert_eq!(out[1], vec!["L = <1, 2> . T"]);
    assert_eq!(out[2], vec!["X = <a, b> . T"]);
    assert_eq!(out[3], vec!["X = <1> . X"]);
    assert_eq!(out[4], vec!["X = <>"]);
    assert_eq!(out[5], vec!["X = <a>"]);
    assert_eq!(out[6], vec!["true"]);
    assert_eq!(out[7], vec!["X = <1> . 2"]);
    assert_eq!(out[8], vec!["X = '<>'(a, b, c)"]);
    // `.` binds looser than `+`: the tail is the arithmetic term.
    assert_eq!(out[9], vec!["X = <a> . _1, _1 = T + 1"]);
}

#[test]
fn list_lengths_are_numeric_terms() {
    // CLM-0015: |L| lives in the store; ground lengths fold; a determined
    // length makes the list a tuple of fresh variables; non-integers,
    // negatives, cycles and non-lists fail.
    let out = answers(
        "?- { N = |<a, b, c>| }.
         ?- { N = |<>| }.
         ?- { |L| > 2 }.
         ?- { N = |L| }.
         ?- { L = <1> . T, |L| = N }.
         ?- { |L| = 3 }.
         ?- { |L| = N / 2, N = 6 }.
         ?- { |L| = 1 / 2 }.
         ?- { |L| = -1 }.
         ?- { X = <1> . X, N = |X| }.
         ?- { N = |foo| }.
         ?- { N = |f(a)| }.
         ?- { N = |5| }.
         ?- { N = ||L|| }.
         ?- { |Z| = |X| + |Y|, Z = <a, b, c>, X = <a> }.
         ?- { |L| + |M| = 3, |L| = |M| }.
         ?- { L = <a> . T, |T| = 2 }.
         ?- { |L| >= 2, L = <a> }.
         ?- { |L| = |L| + 0, N = |L|, N > 1 }.",
    );
    assert_eq!(out[0], vec!["N = 3"]);
    assert_eq!(out[1], vec!["N = 0"]);
    assert_eq!(out[2], vec!["|L| > 2"]);
    assert_eq!(out[3], vec!["N = |L|"]);
    assert_eq!(out[4], vec!["L = <1> . T, N = |T| + 1"]);
    assert_eq!(out[5], vec!["L = <_1, _2, _3>"]);
    assert_eq!(out[6], vec!["L = <_1, _2, _3>, N = 6"]);
    for (i, failing) in out.iter().enumerate().take(14).skip(7) {
        assert!(failing.is_empty(), "query {i} should fail: {failing:?}");
    }
    assert_eq!(out[14], vec!["Z = <a, b, c>, X = <a>, Y = <_1, _2>"]);
    // 3/2 is exact-determined only at answer time, and is not an integer.
    assert!(out[15].is_empty());
    assert_eq!(out[16], vec!["L = <a, _1, _2>, T = <_1, _2>"]);
    assert!(out[17].is_empty());
    assert_eq!(out[18], vec!["N = |L|, N > 1"]);
}

#[test]
fn list_lengths_follow_backtracking() {
    let out = answers(
        "p(<a>). p(<b, c>). p(X).
         at_least(L, K) :- { |L| >= K }.
         ?- p(L), { |L| = 2 }.
         ?- p(L), { |L| = N }, { N > 1 }.
         ?- at_least(L, 2), { L = <a> . T }.
         ?- at_least(<a>, 2).",
    );
    assert_eq!(out[0], vec!["L = <b, c>", "L = <_1, _2>"]);
    assert_eq!(out[1], vec!["L = <b, c>, N = 2", "N = |L|, N > 1"]);
    assert_eq!(out[2], vec!["L = <a> . T, |T| >= 1"]);
    assert!(out[3].is_empty());
}

#[test]
fn list_concatenation_is_a_constraint() {
    // CLM-0016: X . Y with a known left operand is the chain; with an open
    // one it is pending until |X| is known, from the spine or the simplex.
    let out = answers(
        "last(L, X) :- { L = F . <X> }.
         has_prefix(L, P) :- { L = P . S }.
         ?- { Z = <a, b> . <c> }.
         ?- { Z = X . Y }.
         ?- { Z = X . Y, X = <a> }.
         ?- { Z = X . <c>, Z = <a, b, c> }.
         ?- { Z = X . Y, Y = <c>, Z = <a, b, c> }.
         ?- { <a, b, c> = X . Y, |X| = 1 }.
         ?- { Z = X . Y, |Z| = 2, |X| = 1 }.
         ?- last(<1, 2, 3>, X).
         ?- has_prefix(<a, b, c>, <a, b>).
         ?- has_prefix(<a, b, c>, <b>).
         ?- has_prefix(<a, b, c>, P), { |P| = 2 }.
         ?- { |Z| = 10, <1, 2, 3> . Z = Z . <2, 3, 1> }.
         ?- { Z = X . Y . W }.
         ?- { Z = (X . Y) . W }.
         ?- { Z = foo . <a> }.
         ?- { Z = X . foo, X = <a> }.
         ?- { X = <1> . X, Z = X . <a> }.
         ?- { Z = -X . Y }.",
    );
    assert_eq!(out[0], vec!["Z = <a, b, c>"]);
    assert_eq!(out[1], vec!["Z = X . Y"]);
    assert_eq!(out[2], vec!["Z = <a> . Y, X = <a>"]);
    assert_eq!(out[3], vec!["Z = <a, b, c>, X = <a, b>"]);
    assert_eq!(out[4], vec!["Z = <a, b, c>, X = <a, b>, Y = <c>"]);
    assert_eq!(out[5], vec!["X = <a>, Y = <b, c>"]);
    assert_eq!(out[6], vec!["Z = <_1, _2>, X = <_1>, Y = <_2>"]);
    assert_eq!(out[7], vec!["X = 3"]);
    assert_eq!(out[8], vec!["true"]);
    assert!(out[9].is_empty());
    assert_eq!(out[10], vec!["P = <a, b>"]);
    assert_eq!(out[11], vec!["Z = <1, 2, 3, 1, 2, 3, 1, 2, 3, 1>"]);
    assert_eq!(
        out[12],
        vec!["|Z| = |Y| + |W| + |X|, _1 = Y . W, Z = X . _1"]
    );
    assert_eq!(
        out[13],
        vec!["|Z| = |X| + |Y| + |W|, _1 = X . Y, Z = _1 . W"]
    );
    for (i, failing) in out.iter().enumerate().skip(14) {
        assert!(failing.is_empty(), "query {i} should fail: {failing:?}");
    }
}

#[test]
fn list_concatenation_bounds_the_search() {
    // The two-clause reverse runs in both directions: in the reversed one
    // the length relation prunes the search, so it terminates.
    let out = answers(
        "reverse(<>, <>).
         reverse(<E> . X, Y . <E>) :- reverse(X, Y).
         palindrome(<>).
         palindrome(<E>).
         palindrome(<E> . X . <E>) :- palindrome(X).
         ?- reverse(<1, 2, 3>, R).
         ?- reverse(X, <1, 2, 3>).
         ?- reverse(X, <>).
         ?- palindrome(<a, b, a>).
         ?- palindrome(<a, b, c>).
         ?- { |P| = 3 }, palindrome(P).",
    );
    assert_eq!(out[0], vec!["R = <3, 2, 1>"]);
    assert_eq!(out[1], vec!["X = <3, 2, 1>"]);
    assert_eq!(out[2], vec!["X = <>"]);
    assert_eq!(out[3], vec!["true"]);
    assert!(out[4].is_empty());
    assert_eq!(out[5], vec!["P = <_1, _2, _1>"]);
}

#[test]
fn a_list_variable_aliased_to_another_shares_its_length() {
    // Regression: binding a list variable that has a length entry to another
    // variable must post |W| = |Y|, in either orientation. Found when the
    // reversed `reverse` failed to terminate.
    let out = answers(
        "?- { Z = Y . <E>, |Z| = 1, W = Y2 . <E2>, W = Y }.
         ?- { Z = Y . <E>, |Z| = 1, W = Y2 . <E2>, Y = W }.
         ?- { |L| = N, L = M, |M| = 2 }.",
    );
    assert!(out[0].is_empty());
    assert!(out[1].is_empty());
    assert_eq!(out[2], vec!["L = <_1, _2>, N = 2, M = <_1, _2>"]);
}

#[test]
fn strings_are_character_lists() {
    // CLM-0017: "abc" is <a, b, c>; escapes as in quoted atoms; prints as
    // the list.
    let out = answers(
        r#"?- { X = "abc" }.
           ?- { X = "" }.
           ?- { X = "a\"b\\c" }.
           ?- { "ab" = <a, b> }.
           ?- { X = "ab" . "cd" }.
           ?- { N = |"abc"| }."#,
    );
    assert_eq!(out[0], vec!["X = <a, b, c>"]);
    assert_eq!(out[1], vec!["X = <>"]);
    assert_eq!(out[2], vec!["X = <a, '\"', b, '\\\\', c>"]);
    assert_eq!(out[3], vec!["true"]);
    assert_eq!(out[4], vec!["X = <a, b, c, d>"]);
    assert_eq!(out[5], vec!["N = 3"]);
}

#[test]
fn atom_chars_is_a_delayed_constraint() {
    let out = answers(
        r#"append(<>, L, L).
           append(<H> . T, L, <H> . R) :- append(T, L, R).
           has_prefix(Id, Prefix) :- atom_chars(Id, Cs), { Cs = Prefix . S }.
           id_length(Id, N) :- atom_chars(Id, Cs), { N = |Cs| }.
           scheme(Id, Scheme) :- atom_chars(Id, Cs), append(Scheme, <':'> . R, Cs).
           ?- atom_chars(hello, L).
           ?- atom_chars(A, "abc").
           ?- atom_chars(A, <>).
           ?- atom_chars('', L).
           ?- atom_chars(A, L).
           ?- atom_chars(A, <a> . T).
           ?- atom_chars(A, <a, X>), { X = b }.
           ?- atom_chars(A, L), { A = ab }.
           ?- atom_chars(A, L), { L = <x, y> }.
           ?- has_prefix('doc:h3fa21c9d', "doc:").
           ?- has_prefix('doc:h3fa21c9d', "cl-").
           ?- id_length('doc:h3fa21c9d', N).
           ?- scheme('doc:h3fa21c9d', S).
           ?- atom_chars(A, <a, 1>).
           ?- atom_chars(A, <ab>).
           ?- atom_chars(3, L).
           ?- atom_chars(f(a), L).
           ?- atom_chars(<>, L).
           ?- atom_chars(A, foo).
           ?- atom_chars(A, L), { A > 1 }."#,
    );
    assert_eq!(out[0], vec!["L = <h, e, l, l, o>"]);
    assert_eq!(out[1], vec!["A = abc"]);
    assert_eq!(out[2], vec!["A = ''"]);
    assert_eq!(out[3], vec!["L = <>"]);
    assert_eq!(out[4], vec!["atom_chars(A, L)"]);
    assert_eq!(out[5], vec!["atom_chars(A, <a> . T)"]);
    assert_eq!(out[6], vec!["A = ab, X = b"]);
    assert_eq!(out[7], vec!["A = ab, L = <a, b>"]);
    assert_eq!(out[8], vec!["A = xy, L = <x, y>"]);
    assert_eq!(out[9], vec!["true"]);
    assert!(out[10].is_empty());
    assert_eq!(out[11], vec!["N = 13"]);
    assert_eq!(out[12], vec!["S = <d, o, c>"]);
    for (i, failing) in out.iter().enumerate().skip(13) {
        assert!(failing.is_empty(), "query {i} should fail: {failing:?}");
    }
}

#[test]
fn type_relations_are_constraints() {
    let out = answers(
        "?- number(3).
         ?- number(a).
         ?- number(<a>).
         ?- number(X), { X = 3 }.
         ?- number(X).
         ?- atom(a).
         ?- atom('doc:x').
         ?- atom(3).
         ?- atom(<>).
         ?- atom(<a>).
         ?- atom(X).
         ?- atom(X), { X = a }.
         ?- atom(X), { X = Y }, { Y = b }.
         ?- list(<>).
         ?- list(<a>).
         ?- list(a).
         ?- list(X), { X = <a> . T }.
         ?- atom(X), { X = <a> }.
         ?- atom(X), { X > 3 }.
         ?- { X > 3 }, atom(X).
         ?- list(X), { X = a }.
         ?- number(X), atom(X).
         ?- atom(X), number(X).
         ?- atom(X), list(X).
         ?- atom(X), { X = Y }, { Y = <> }.",
    );
    assert_eq!(out[0], vec!["true"]);
    assert!(out[1].is_empty());
    assert!(out[2].is_empty());
    assert_eq!(out[3], vec!["X = 3"]);
    assert_eq!(out[4], vec!["true"]);
    assert_eq!(out[5], vec!["true"]);
    assert_eq!(out[6], vec!["true"]);
    assert!(out[7].is_empty());
    assert!(out[8].is_empty());
    assert!(out[9].is_empty());
    assert_eq!(out[10], vec!["atom(X)"]);
    assert_eq!(out[11], vec!["X = a"]);
    assert_eq!(out[12], vec!["X = b, Y = b"]);
    assert_eq!(out[13], vec!["true"]);
    assert_eq!(out[14], vec!["true"]);
    assert!(out[15].is_empty());
    assert_eq!(out[16], vec!["X = <a> . T"]);
    for (i, failing) in out.iter().enumerate().skip(17) {
        assert!(failing.is_empty(), "query {i} should fail: {failing:?}");
    }
}

#[test]
fn built_in_names_cannot_be_defined() {
    let clauses = parse_program_spanned("ok(a).\natom(X) :- ok(X).\n").expect("parses");
    let err = Program::compile_spanned(&clauses).expect_err("reserved");
    assert!(matches!(
        err,
        EvalError::ReservedBuiltin { ref name, arity: 1, .. } if name == "atom"
    ));
    let span = err.span().expect("positioned");
    assert_eq!((span.line, span.column), (2, 1));
    assert_eq!(
        err.to_string(),
        "`atom/1` is a built-in predicate and cannot be defined"
    );
    // Another arity is an ordinary predicate.
    assert_eq!(
        answers("list(X, Y) :- ok(X). ok(a). ?- list(a, b).")[0],
        vec!["true"]
    );
}

#[test]
fn the_anonymous_variable_is_fresh_and_unnamed() {
    // CLM-0020: `_` is a fresh variable at each occurrence and never an
    // answer variable.
    let out = answers(
        "p(a, b).
         q(X, Y) :- p(X, _), p(_, Y).
         first(X, <X> . _).
         ?- p(_, _).
         ?- p(X, _).
         ?- p(_, X), p(X, _).
         ?- q(X, Y).
         ?- first(X, <1, 2>).
         ?- { X = f(_) }.
         ?- { X = f(_, _) }.
         ?- { _ > 3 }.
         ?- { X = _ }.
         ?- { X = <_, _> }, { |X| = 2 }.
         ?- p(_, _), { Y = 1 }.",
    );
    assert_eq!(out[0], vec!["true"]);
    assert_eq!(out[1], vec!["X = a"]);
    assert!(out[2].is_empty()); // X = b, then p(b, _) fails
    assert_eq!(out[3], vec!["X = a, Y = b"]);
    assert_eq!(out[4], vec!["X = 1"]);
    assert_eq!(out[5], vec!["X = f(_1)"]);
    assert_eq!(out[6], vec!["X = f(_1, _2)"]);
    // A constraint on an anonymous variable alone is satisfiable and
    // mentions nothing named: it projects away.
    assert_eq!(out[7], vec!["true"]);
    assert_eq!(out[8], vec!["true"]);
    assert_eq!(out[9], vec!["X = <_1, _2>"]);
    assert_eq!(out[10], vec!["Y = 1"]);
}

#[test]
fn first_argument_indexing_preserves_clause_order_and_answers() {
    // CLM-0021: a call with a bound first argument visits only the clauses
    // that can match, merged with variable-first clauses, in program order.
    let out = answers(
        "p(a, 1). p(X, 2). p(b, 3). p(a, 4). p(f(Y), 5). p(1/2, 6). p(f(Y, Z), 7).
         ?- p(a, N).
         ?- p(b, N).
         ?- p(c, N).
         ?- p(f(q), N).
         ?- p(f(q, r), N).
         ?- p(0.5, N).
         ?- p(2, N).
         ?- p(W, N).
         ?- { V = 1/2 }, p(V, N).
         ?- p(V, N), { V = a }.",
    );
    assert_eq!(out[0], vec!["N = 1", "N = 2", "N = 4"]);
    assert_eq!(out[1], vec!["N = 2", "N = 3"]);
    assert_eq!(out[2], vec!["N = 2"]);
    assert_eq!(out[3], vec!["N = 2", "N = 5"]);
    assert_eq!(out[4], vec!["N = 2", "N = 7"]);
    // Numbers key exactly: 0.5 is 1/2.
    assert_eq!(out[5], vec!["N = 2", "N = 6"]);
    assert_eq!(out[6], vec!["N = 2"]);
    // An unbound first argument sees every clause, in order.
    assert_eq!(out[7].len(), 7);
    assert_eq!(out[7][0], "W = a, N = 1");
    assert_eq!(out[7][6], "W = f(_1, _2), N = 7");
    // A determined numeric variable is bound to its number (CLM-0026), so
    // the index sees the number: the variable-first clause and the number
    // clause match, exactly as for `p(0.5, N)`.
    assert_eq!(out[8], vec!["V = 1/2, N = 2", "V = 1/2, N = 6"]);
    // `{ V = a }` with `V` bound to `1/2` is a numeric equation (rule 2) in
    // which the constant `a` names an attribute term, as `{ 1/2 = a }` does;
    // whether a bare constant should be admitted there is deferred by the
    // record (item 5).
    assert_eq!(
        out[9],
        vec![
            "V = a, N = 1",
            "V = a, N = 2",
            "V = a, N = 4",
            "V = 1/2, N = 6, a = 1/2"
        ]
    );
}

/// CLM-0026 (docs/design/2026-11-09-numbers-numeric-variables-and-attribute-
/// terms.md): a determined numeric variable is bound to its number, a block
/// `=`/`!=` is numeric when either side denotes a number, and unification
/// never creates an attribute term — the record's probe, in both goal orders.
#[test]
fn numeric_equality_follows_what_the_operands_denote() {
    let prog = "{ age(alice) = 30 }.
                person(alice, 30).
                q(f(a)).
                same(X, X).";
    let out = answers(&format!(
        "{prog}
         ?- person(alice, A), {{ A = age(alice) }}.
         ?- {{ A = 30 }}, {{ A = age(alice) }}.
         ?- {{ A = age(alice) }}, person(alice, A).
         ?- person(alice, A), {{ A != age(alice) }}.
         ?- {{ f(a) = 2 }}.
         ?- q(2).
         ?- {{ M = 2 }}, q(M).
         ?- q(M), {{ M = 2 }}.
         ?- {{ M > 1 }}, q(M).
         ?- q(M), {{ M > 1 }}.
         ?- {{ X = <2> }}, {{ X = <f(a)> }}.
         ?- {{ X = 31 }}, {{ X = age(bob) }}.
         ?- {{ X = age(bob) }}, {{ X = 31 }}.
         ?- {{ X > 1 }}, {{ X < 3 }}, {{ 2 * X = 5 }}, same(X, 5/2).
         ?- {{ X > 1 }}, {{ X < 3 }}, {{ 2 * X = 5 }}, same(X, 2).
         ?- {{ X = Y }}, {{ Y = 3 }}, same(X, 3).
         ?- {{ X = Y }}, {{ Y = 3 }}, {{ X != Z }}, same(Z, 3).
         ?- {{ A = 30 }}, {{ A != 30 }}.
         ?- {{ X != 3 }}, {{ X > 2 }}, {{ X < 4 }}."
    ));
    // The integrity-checker shape: a stored number meets its attribute term
    // (rule 2 — a variable bound to a number denotes it), in either route to
    // the number.
    assert_eq!(out[0], vec!["A = 30, age(alice) = 30"]);
    assert_eq!(out[1], vec!["A = 30, age(alice) = 30"]);
    // With the block first, `A` is unbound and meets a tree: tree
    // unification (D4), and the head's number then fails against the tree.
    assert!(out[2].is_empty());
    assert!(out[3].is_empty()); // 30 != age(alice) with age(alice) = 30
    // A compound in numeric position names an attribute term (unchanged).
    assert_eq!(out[4], vec!["f(a) = 2"]);
    // Rule 1: `{ M = 2 }` makes `M` the number 2, so `q(M)` fails as `q(2)`
    // does; a compound already denoted by the operand is in numeric position.
    assert!(out[5].is_empty());
    assert!(out[6].is_empty());
    assert_eq!(out[7], vec!["M = f(a), f(a) = 2"]);
    // Rule 3: an undetermined numeric variable meeting a compound fails.
    assert!(out[8].is_empty());
    assert_eq!(out[9], vec!["M = f(a), f(a) > 1"]);
    // Rule 4: inside structures, numbers are leaves.
    assert!(out[10].is_empty());
    assert_eq!(out[11], vec!["X = 31, age(bob) = 31"]);
    assert_eq!(out[12], vec!["X = age(bob), age(bob) = 31"]);
    // Determination by the store binds the variable to its number: head
    // unification with that number succeeds, with another fails.
    assert_eq!(out[13], vec!["X = 5/2"]);
    assert!(out[14].is_empty());
    assert_eq!(out[15], vec!["X = 3, Y = 3"]);
    assert!(out[16].is_empty());
    assert!(out[17].is_empty());
    // An undetermined variable stays a variable of the solver.
    assert_eq!(out[18], vec!["X > 2, X < 4, X != 3"]);
}

#[test]
fn deterministic_lookups_leave_no_choice_point() {
    let clauses = parse_program(
        "k(c1, 1). k(c2, 2). k(c3, 3).
         ?- k(c2, K).
         ?- k(C, 2).",
    )
    .expect("parses");
    let program = Program::compile(&clauses).expect("compiles");
    // One candidate: the answer is found with nothing left to try.
    let mut s = program.solve(&program.queries()[0]);
    assert_eq!(s.next().map(|a| a.to_string()), Some("K = 2".to_string()));
    assert!(!s.may_continue());
    // Unbound first argument: all three clauses are candidates.
    let mut s = program.solve(&program.queries()[1]);
    assert_eq!(s.next().map(|a| a.to_string()), Some("C = c2".to_string()));
    assert!(s.may_continue());
}

#[test]
fn the_list_constructor_cannot_be_defined_as_a_predicate() {
    let clauses = parse_program_spanned("ok(a).\n'<>'(a, b).\n").expect("parses");
    let err = Program::compile_spanned(&clauses).expect_err("reserved");
    assert!(matches!(err, EvalError::ReservedName { ref name, .. } if name == "<>"));
    let span = err.span().expect("positioned");
    assert_eq!((span.line, span.column), (2, 1));
    assert!(err.to_string().starts_with("`'<>'` is reserved"));
    // Calling it is only an undefined predicate (no clauses): it fails.
    assert!(answers("?- '<>'(a, b).")[0].is_empty());
}

#[test]
fn constraint_goals_in_bodies_and_queries() {
    let out = answers(
        "same(X, X).
         p(X) :- { X = f(Y) }, same(Y, a).
         ?- p(Z).
         ?- same(A, B).
         ?- same(A, B), { A != B }.
         ?- { X = 0.50 }, same(X, 0.5).",
    );
    assert_eq!(out[0], vec!["Z = f(a)"]);
    assert_eq!(out[1], vec!["A = B"]);
    assert!(out[2].is_empty());
    assert_eq!(out[3], vec!["X = 1/2"]);
}

#[test]
fn implication_is_sugar_for_a_constraint_rule() {
    let out = answers(
        "{ X = ok } => fine(X).
         ?- fine(ok).
         ?- fine(bad).
         ?- fine(Y).",
    );
    assert_eq!(out[0], vec!["true"]);
    assert!(out[1].is_empty());
    assert_eq!(out[2], vec!["Y = ok"]);
}

#[test]
fn constraint_facts_form_the_initial_store() {
    // Satisfiable facts load; queries see nothing observable from them yet
    // (they only mention their own fresh variables), but load must succeed.
    let out = answers("{ X != a }. { f(A) = f(b) }. p(1). ?- p(X).");
    assert_eq!(out[0], vec!["X = 1"]);
    // Internal-only disequations are projected away; reachable ones stay.
    let out = answers("p(X) :- { Y != a }. q(X) :- { X != a }. ?- p(Z). ?- q(Z).");
    assert_eq!(out[0], vec!["true"]);
    assert_eq!(out[1], vec!["Z != a"]);
    // Unsatisfiable facts: no models.
    let clauses = parse_program("{ a != a }. p(1).").unwrap();
    assert_eq!(
        Program::compile(&clauses).unwrap_err(),
        EvalError::InitialStoreUnsatisfiable
    );
    let clauses = parse_program("{ X = a }. { X != a }. p(1).").unwrap();
    // Different clauses have different X: satisfiable.
    assert!(Program::compile(&clauses).is_ok());
}

#[test]
fn undefined_predicate_fails() {
    let out = answers("p(1). ?- q(X). ?- p(X), q(X).");
    assert!(out[0].is_empty());
    assert!(out[1].is_empty());
}

#[test]
fn cyclic_answers_print_as_equations() {
    let out = answers(
        "omega(X) :- { X = f(X) }.
         ?- omega(X).
         ?- omega(X), { X = f(f(X)) }.
         ?- { X = g(Y, Y), Y = h(X) }.
         ?- { X = f(X, a) }, { X = f(X, b) }.",
    );
    assert_eq!(out[0], vec!["X = f(X)"]);
    assert_eq!(out[1], vec!["X = f(X)"]);
    assert_eq!(out[2], vec!["X = g(Y, Y), Y = h(X)"]);
    assert!(out[3].is_empty());
}

#[test]
fn dif_answers_show_pending_disequations() {
    let out = answers(
        "same(X, X).
         ?- { X != Y }, same(X, f(Z)), same(Y, f(W)).
         ?- { X != Y }, same(X, f(Z)), same(Y, f(W)), same(Z, W).
         ?- { X != a }, same(X, b).",
    );
    assert_eq!(out[0], vec!["X = f(Z), Y = f(W), Z != W"]);
    assert!(out[1].is_empty());
    assert_eq!(out[2], vec!["X = b"]);
}

#[test]
fn solutions_are_lazy_and_resumable() {
    let clauses = parse_program("nat(zero). nat(s(N)) :- nat(N). ?- nat(X).").unwrap();
    let program = Program::compile(&clauses).unwrap();
    let mut sols = program.solve(&program.queries()[0]);
    assert_eq!(sols.next().unwrap().to_string(), "X = zero");
    assert_eq!(sols.next().unwrap().to_string(), "X = s(zero)");
    assert_eq!(sols.next().unwrap().to_string(), "X = s(s(zero))");
    // Infinite: we can simply stop asking.
}

#[test]
fn deep_derivations_do_not_touch_the_rust_stack() {
    // 100 000 resolution steps: ten walks down a 10 000-deep Peano number,
    // then a long goal list. Since CLM-0019 the AST side (lowering,
    // instantiation, printing, dropping) is iterative too, so this runs on a
    // 1 MiB thread; only the parser's own frames remain, and they are flat.
    on_small_stack(|| {
        let mut term = "zero".to_string();
        for _ in 0..10_000 {
            term = format!("s({term})");
        }
        let calls: Vec<String> = (0..10).map(|_| format!("count({term})")).collect();
        let src = format!(
            "count(zero). count(s(N)) :- count(N). ?- {}.",
            calls.join(", ")
        );
        let out = answers(&src);
        assert_eq!(out[0], vec!["true"]);
        // And a long goal list: 20 000 goals in one query.
        let many: Vec<&str> = std::iter::repeat_n("p", 20_000).collect();
        let src = format!(
            "p(). ?- {}.",
            many.iter()
                .map(|p| format!("{p}()"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert_eq!(answers(&src)[0], vec!["true"]);
    });
}

fn on_small_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("no panic on the small stack");
}

#[test]
fn deep_literals_cost_no_stack() {
    // CLM-0019: a 100 000-deep term is parsed, lowered, echoed (the query
    // text is its Display), run and dropped; a 200 000-element list literal
    // lowers to a chain that deep and is measured and dropped — all on a
    // 1 MiB thread.
    on_small_stack(|| {
        let mut term = "zero".to_string();
        for _ in 0..100_000 {
            term = format!("s({term})");
        }
        let src = format!("depth(zero, 0).\n?- {{ X = {term} }}.");
        let clauses = parse_program(&src).expect("parses");
        let program = Program::compile(&clauses).expect("compiles");
        let q = &program.queries()[0];
        assert!(q.text().starts_with("?- { X = s(s(s("));
        // `s(` and `)` per level around `zero`.
        assert_eq!(q.text().len(), 100_000 * 3 + "?- { X = zero }.".len());
        let out: Vec<String> = program.solve(q).map(|a| a.to_string()).collect();
        assert_eq!(out.len(), 1);
        assert!(out[0].starts_with("X = s(s(s("));
        drop(program);
        drop(clauses);

        let items: Vec<String> = (0..200_000).map(|i| i.to_string()).collect();
        let src = format!("?- {{ N = |<{}>| }}.", items.join(", "));
        let out = answers(&src);
        assert_eq!(out[0], vec!["N = 200000"]);
    });
}

// --- stage 3: the linear store -----------------------------------------------

fn first(src: &str) -> String {
    answers(src).into_iter().next().unwrap().join(" | ")
}

#[test]
fn linear_equations_determine_variables() {
    assert_eq!(first("?- { X + Y = 10, X - Y = 2 }."), "X = 6, Y = 4");
    assert_eq!(first("?- { 3 * X = 1 }."), "X = 1/3");
    assert_eq!(first("?- { X = 1 / 3 }."), "X = 1/3");
    assert_eq!(
        first("?- { X = -(2 * Y) }, { Y = 1.5 }."),
        "X = -3, Y = 3/2"
    );
    assert_eq!(first("?- { X = 0.1 + 0.2 }, { X = 0.3 }."), "X = 3/10");
    assert_eq!(first("?- { X = 1 / 0 }."), "");
}

#[test]
fn arithmetic_in_heads_and_arguments() {
    let prog = "sum(X, Y, X + Y).
                average(X, Y, (X + Y) / 2).
                temperature(fahrenheit(F), celsius((F - 32) * 5 / 9)).
                discounted(P, D) :- { D = P - P / 10, D > 0 }.
                same(X, X).";
    let out = answers(&format!(
        "{prog}
         ?- sum(1, 2, S).
         ?- average(3, 4, A).
         ?- temperature(fahrenheit(212), C).
         ?- temperature(F, celsius(100)).
         ?- discounted(100, D).
         ?- discounted(P, 90).
         ?- same(f(X + 1), f(2)).
         ?- sum(X, Y, 10), same(X, 4)."
    ));
    assert_eq!(out[0], vec!["S = 3"]);
    assert_eq!(out[1], vec!["A = 7/2"]);
    assert_eq!(out[2], vec!["C = celsius(100)"]);
    assert_eq!(out[3], vec!["F = fahrenheit(212)"]);
    assert_eq!(out[4], vec!["D = 90"]);
    assert_eq!(out[5], vec!["P = 100"]);
    assert_eq!(out[6], vec!["X = 1"]);
    assert_eq!(out[7], vec!["X = 4, Y = 6"]);
}

#[test]
fn ground_arithmetic_folds_to_numbers() {
    // CLM-0013: no solver row is made for arithmetic over numbers, and the
    // answers are what the solver would have printed.
    assert_eq!(first("?- { X = 2 * 3 + 4 / 2 }."), "X = 8");
    assert_eq!(first("?- { X = -(1 / 3) * 3 }."), "X = -1");
    assert_eq!(first("?- { 10 * 3 > 10 * 2 }."), "true");
    assert_eq!(first("?- { 10 * 2 > 10 * 3 }."), "");
    assert_eq!(first("?- { X = 1 / 0 }."), "");
    assert_eq!(first("?- { X = 7 / 2 + 1 / 2 }."), "X = 4");
}

#[test]
fn body_goals_are_instantiated_when_reached() {
    // CLM-0013: a body goal's terms are built when the goal runs, so
    // arithmetic over values bound by earlier goals folds, and variables that
    // first appear in a later goal are shared across the clause as before.
    let out = answers(
        "kn(a, 181, 199). kn(b, 250, 120). kn(c, 7, 10).
         violation(C) :- kn(C, K, N), { 10 * K > 7 * N }.
         chain(X) :- step(Y), link(Y, X), { X > Y }.
         step(1). step(5).
         link(1, 3). link(5, 2). link(5, 9).
         ?- violation(C).
         ?- chain(X).",
    );
    assert_eq!(out[0], vec!["C = a", "C = b"]);
    assert_eq!(out[1], vec!["X = 3", "X = 9"]);
}

#[test]
fn inequalities_bounds_and_residuals() {
    assert_eq!(first("?- { X >= 3, X <= 3 }."), "X = 3");
    assert_eq!(first("?- { X > 3, X < 3 }."), "");
    assert_eq!(first("?- { X > 3 }."), "X > 3");
    assert_eq!(first("?- { X > 3 }, { X < 5 }."), "X > 3, X < 5");
    assert_eq!(first("?- { X + Y <= 10 }."), "X + Y <= 10");
    assert_eq!(first("?- { X > Y, Y > Z }."), "X > Y, Y > Z");
    assert_eq!(first("?- { Y = X + 1 }."), "Y = X + 1");
    assert_eq!(first("?- { Y = X + 1 }, { X > 0 }."), "Y = X + 1, X > 0");
    // Two-variable equation with a residual: printed in solved form.
    assert_eq!(
        first("?- { X + Y = 10, 2*X - Y >= 1/3 }."),
        "Y = 10 - X, 2*X - Y >= 1/3"
    );
}

#[test]
fn numeric_disequations_are_exact() {
    assert_eq!(first("?- { X - Y = 0 }, { X != Y }."), "");
    assert_eq!(first("same(X, X). ?- { X != 3 }, same(X, 3)."), "");
    assert_eq!(first("same(X, X). ?- { X != 3 }, same(X, 4)."), "X = 4");
    assert_eq!(first("?- { X != 3 }."), "X != 3");
    assert_eq!(
        first("?- { X > 3 }, { X < 5 }, { X != 4 }."),
        "X > 3, X < 5, X != 4"
    );
    assert_eq!(first("?- { X + Y = 10 }, { X - Y = 2 }, { X != 6 }."), "");
    // Implied through the store only at answer time: still exact.
    assert_eq!(
        first("?- { X != Y }, { X >= 1 }, { X <= 1 }, { Y >= 1 }, { Y <= 1 }."),
        ""
    );
}

#[test]
fn attribute_terms_and_congruence() {
    let prog = "eligible(X) :- { age(X) >= 18 }.
                { age(socrates) > 70 }.
                same(X, X).";
    let out = answers(&format!(
        "{prog}
         ?- eligible(socrates).
         ?- eligible(alice).
         ?- eligible(X), same(X, socrates).
         ?- eligible(X).
         ?- {{ age(bob) = 3 }}, eligible(bob).
         ?- {{ foo > 3 }}.
         ?- {{ X > 3 }}, same(X, foo).
         ?- same(X, foo), {{ X > 3 }}.
         ?- {{ X = 3 }}, {{ X = f(a) }}.
         ?- {{ X = f(a) }}, {{ X = 3 }}.
         ?- same(X, 3), same(X, f(a)).
         ?- {{ age(X) + 1 >= 19 }}, same(X, bob)."
    ));
    assert_eq!(out[0], vec!["age(socrates) > 70"]); // the world's constraint on the touched unknown
    assert_eq!(out[1], vec!["age(alice) >= 18"]);
    assert_eq!(out[2], vec!["X = socrates, age(socrates) > 70"]);
    assert_eq!(out[3], vec!["age(X) >= 18"]);
    assert!(out[4].is_empty()); // 3 >= 18 fails
    assert_eq!(out[5], vec!["foo > 3"]);
    // CLM-0026, rule 3: a numeric variable meeting a constant in unification
    // fails (D4 as written) — unification never creates an attribute term.
    // A constant already denoted by the operand of a block relation is in
    // numeric position and names one.
    assert!(out[6].is_empty());
    assert_eq!(out[7], vec!["X = foo, foo > 3"]);
    assert_eq!(out[8], vec!["X = 3, f(a) = 3"]);
    assert_eq!(out[9], vec!["X = f(a), f(a) = 3"]);
    assert!(out[10].is_empty()); // tree unification of 3 with f(a) fails
    assert_eq!(out[11], vec!["X = bob, age(bob) >= 18"]);
}

#[test]
fn delayed_products() {
    let prog = "same(X, X).";
    let out = answers(&format!(
        "{prog}
         ?- {{ Y = X * Z }}, same(X, 2), same(Z, 3).
         ?- {{ Y = X * Z }}, same(Z, 3), {{ X = 2 }}.
         ?- {{ Y = X / Z }}, same(Z, 4), same(X, 1).
         ?- {{ Y = X / Z }}, same(Z, 0)."
    ));
    assert_eq!(out[0], vec!["Y = 6, X = 2, Z = 3"]);
    assert_eq!(out[1], vec!["Y = 6, X = 2, Z = 3"]);
    assert_eq!(out[2], vec!["Y = 1/4, X = 1, Z = 4"]);
    assert!(out[3].is_empty()); // division by a determined zero fails
    // Still non-linear at answer time: an error, never an approximate answer.
    let clauses = parse_program("?- { Y = X * Z }.").unwrap();
    let program = Program::compile(&clauses).unwrap();
    let mut sols = program.solve(&program.queries()[0]);
    assert!(sols.next().is_none());
    assert!(matches!(sols.error(), Some(EvalError::NonLinear { .. })));
}

#[test]
fn determined_variables_wake_difs_and_congruence() {
    // dif on numeric variables decided when they become determined.
    assert_eq!(first("?- { X != Y }, { X = 1 }, { Y = 1 }."), "");
    assert_eq!(
        first("?- { X != Y }, { X = 1 }, { Y = 2 }."),
        "X = 1, Y = 2"
    );
    // tree dif over structures containing numeric variables.
    assert_eq!(first("?- { f(X) != f(3) }, { X >= 3, X <= 3 }."), "");
    assert_eq!(first("?- { f(X) != f(3) }, { X >= 4 }."), "X >= 4");
    // congruence via a numeric variable inside an attribute term.
    assert_eq!(first("?- { age(X) > 1 }, { X = 3 }, { age(3) < 1 }."), "");
}

#[test]
fn numeric_examples_from_the_readme_run() {
    let out = answers(
        "human(socrates).
         mortal(X) :- human(X).
         { age(socrates) > 70 }.
         eligible(X) :- { age(X) >= 18 }.
         { age(X) >= 18 } => eligible(X).
         ?- mortal(socrates).
         ?- eligible(alice), { age(alice) >= 18 }.",
    );
    assert_eq!(out[0], vec!["true"]);
    // Two clauses for eligible/1 (rule and desugared implication): two answers.
    assert_eq!(out[1], vec!["age(alice) >= 18", "age(alice) >= 18"]);
}

#[test]
fn compile_spanned_still_works_and_initial_store_checks_numerics() {
    let clauses = parse_program_spanned("{ X > 3, X < 2 }. p(1).").unwrap();
    assert_eq!(
        Program::compile_spanned(&clauses).unwrap_err(),
        EvalError::InitialStoreUnsatisfiable
    );
    let clauses = parse_program_spanned("{ age(a) > 3 }. p(1). ?- p(X).").unwrap();
    assert!(Program::compile_spanned(&clauses).is_ok());
}

#[test]
fn no_floating_point_in_the_crate() {
    // The exact-arithmetic aspect: no f32/f64 anywhere in src/ (comments and
    // this test excepted).
    fn scan(dir: &std::path::Path, hits: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path, hits);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (i, line) in text.lines().enumerate() {
                    let code = line.split("//").next().unwrap_or("");
                    if (code.contains("f64") || code.contains("f32")) && !path.ends_with("tests.rs")
                    {
                        hits.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                    }
                }
            }
        }
    }
    let mut hits = Vec::new();
    scan(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut hits,
    );
    assert!(hits.is_empty(), "floating point in src/: {hits:#?}");
}

// --- stage 4: projection and simplification ----------------------------------

#[test]
fn internal_variables_are_eliminated() {
    let prog = "p(X) :- { X = Y + Z, Y > 0, Z > 0 }.
                q(X) :- { X = Y + 1, Y != 2 }.
                r(X, Y) :- { X > Y, Y > Z, X > Z }.
                s(X) :- { X > 3, X > 2 }.
                t(X) :- { X + Y = 10, X + Y = 10 }.
                u(X) :- { X = 2 * Y, Y >= 1, Y <= 3 }.
                v(X) :- { X = Y + Z, Y > 0, Z >= 0 }.
                w(X) :- { X = 3 * Y + 1, Y >= 1 }.
                b(X) :- { X = Y + Z, Y >= 0, Y <= 1, Z >= 0, Z <= 1 }.";
    let out = answers(&format!(
        "{prog}
         ?- p(A). ?- q(A). ?- r(A, B). ?- s(A). ?- t(A). ?- u(A). ?- v(A). ?- w(A). ?- b(A)."
    ));
    assert_eq!(out[0], vec!["A > 0"]);
    assert_eq!(out[1], vec!["A != 3"]); // the stage-3 projection bug
    assert_eq!(out[2], vec!["A > B"]);
    assert_eq!(out[3], vec!["A > 3"]);
    assert_eq!(out[4], vec!["true"]);
    assert_eq!(out[5], vec!["A >= 2, A <= 6"]);
    assert_eq!(out[6], vec!["A > 0"]); // strictness survives FM
    assert_eq!(out[7], vec!["A >= 4"]); // non-unit coefficient
    assert_eq!(out[8], vec!["A >= 0, A <= 2"]);
}

#[test]
fn redundant_constraints_are_dropped() {
    assert_eq!(first("?- { X > 3, X > 2 }."), "X > 3");
    assert_eq!(first("?- { X > Y, Y > Z }."), "X > Y, Y > Z");
    assert_eq!(first("?- { X + Y <= 10, X + Y <= 20 }."), "X + Y <= 10");
    assert_eq!(first("?- { 2 * X <= 6 }."), "X <= 3");
    assert_eq!(first("?- { X > Y, Y > Z, X > Z }."), "X > Y, Y > Z");
    assert_eq!(first("?- { X = Y + 1, X = Y + 1 }."), "X = Y + 1");
}

#[test]
fn tree_disequations_print_in_reduced_form() {
    let out = answers(
        "same(X, X).
         ?- { X != Y }, same(X, f(Z)), same(Y, f(W)).
         ?- { f(X, b) != f(a, Y) }, same(X, a).
         ?- { X != Y }, same(X, f(Z, V)), same(Y, f(W, U)).
         ?- { X != Y }, same(X, f(Z, a)), same(Y, f(W, b)).",
    );
    assert_eq!(out[0], vec!["X = f(Z), Y = f(W), Z != W"]);
    assert_eq!(out[1], vec!["X = a, Y != b"]);
    // Two pairs: a genuine disjunction, printed as the full terms.
    assert_eq!(out[2], vec!["X = f(Z, V), Y = f(W, U), f(Z, V) != f(W, U)"]);
    // Decidably different: dropped.
    assert_eq!(out[3], vec!["X = f(Z, a), Y = f(W, b)"]);
}

#[test]
fn projection_budget_fallback_stays_correct() {
    // An internal W with 20 lower and 20 upper bounds on public variables
    // would produce 400 combinations: over the budget, so W is named instead.
    let args: Vec<String> = (0..20)
        .map(|i| format!("A{i}"))
        .chain((0..20).map(|i| format!("B{i}")))
        .collect();
    let mut body = String::from("{ ");
    for i in 0..20 {
        body.push_str(&format!("W >= A{i}, W <= B{i}, "));
    }
    body.push_str("A0 >= 1 }");
    let src = format!(
        "p({}) :- {body}. ?- p({}).",
        args.join(", "),
        args.join(", ")
    );
    let out = answers(&src);
    let ans = &out[0][0];
    assert!(ans.contains("A0 >= 1"), "{ans}");
    assert!(
        ans.contains("_1 >= A0") || ans.contains("A0 <= _1"),
        "survivor named: {ans}"
    );
}

#[test]
fn answers_are_deterministic() {
    let src = "p(X, Y) :- { X = A + B, Y = A - B, A > 0, B > 0, A != B }.
               ?- p(U, V).";
    let first_run = answers(src);
    for _ in 0..5 {
        assert_eq!(answers(src), first_run);
    }
}

#[test]
fn interrupt_flag_stops_a_runaway_query() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    // `p() :- p().` never terminates; the flag stops it between steps.
    let clauses = parse_program("p() :- p(). ?- p().").unwrap();
    let program = Program::compile(&clauses).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let mut sols = program
        .solve(&program.queries()[0])
        .with_interrupt(flag.clone());
    // Set the flag from another thread shortly after starting.
    let setter = {
        let flag = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            flag.store(true, Ordering::Relaxed);
        })
    };
    assert!(sols.next().is_none());
    assert!(sols.interrupted());
    assert!(sols.error().is_none());
    setter.join().unwrap();
    // A finished query is not "may_continue".
    let clauses = parse_program("q(a). q(b). ?- q(X).").unwrap();
    let program = Program::compile(&clauses).unwrap();
    let mut sols = program.solve(&program.queries()[0]);
    assert_eq!(sols.next().unwrap().to_string(), "X = a");
    assert!(sols.may_continue());
    assert_eq!(sols.next().unwrap().to_string(), "X = b");
    assert!(!sols.may_continue());
}

/// Run a program whose single query is expected to stop with a runtime
/// error; returns the error.
fn query_error(src: &str) -> EvalError {
    let clauses = parse_program(src).expect("parses");
    let program = Program::compile(&clauses).expect("compiles");
    let q = &program.queries()[0];
    let mut sols = program.solve(q);
    let answers: Vec<String> = sols.by_ref().map(|a| a.to_string()).collect();
    sols.error()
        .cloned()
        .unwrap_or_else(|| panic!("expected an error, got answers {answers:?}"))
}

#[test]
fn disjunction_tries_both_alternatives_in_order() {
    // CLM-0022: `;` is a choice point; a conjunction on either side; nested
    // and query-level forms; a parenthesised conjunction splices.
    let out = answers(
        "colour(red). colour(blue). size(big). size(small).
         feature(X) :- colour(X) ; size(X).
         pair(X, Y) :- (colour(X), size(Y)) ; { X = none, Y = none }.
         ?- feature(X).
         ?- pair(X, Y).
         ?- colour(X) ; size(X) ; { X = other }.
         ?- (colour(X) ; size(X)), { X != blue }.",
    );
    assert_eq!(out[0], vec!["X = red", "X = blue", "X = big", "X = small"]);
    assert_eq!(
        out[1],
        vec![
            "X = red, Y = big",
            "X = red, Y = small",
            "X = blue, Y = big",
            "X = blue, Y = small",
            "X = none, Y = none",
        ]
    );
    assert_eq!(
        out[2],
        vec!["X = red", "X = blue", "X = big", "X = small", "X = other"]
    );
    assert_eq!(out[3], vec!["X = red", "X = big", "X = small"]);
}

#[test]
fn if_then_else_commits_to_the_first_solution() {
    // CLM-0022: the condition's other solutions and the else branch are
    // discarded on commit; the else branch runs on the restored store; an
    // if-then without else fails when the condition does.
    let out = answers(
        "curated(k1, cv1). curated(k1, cv2). harvested(k1, hv1). harvested(k2, hv2).
         value(K, V) :- (curated(K, X) -> { V = X } ; harvested(K, V)).
         sign(X, S) :- { X < 0 } -> { S = neg } ; { X = 0 } -> { S = zero } ; { S = pos }.
         ?- value(k1, V).
         ?- value(k2, V).
         ?- value(k3, V).
         ?- sign(-1, S).
         ?- sign(0, S).
         ?- sign(2, S).
         ?- curated(k1, X) -> { Y = X }.
         ?- harvested(k9, X) -> { Y = X }.
         ?- curated(k1, X), ({ X = cv1 } -> { N = 1 } ; { N = 2 }).",
    );
    assert_eq!(out[0], vec!["V = cv1"]);
    assert_eq!(out[1], vec!["V = hv2"]);
    assert!(out[2].is_empty());
    assert_eq!(out[3], vec!["S = neg"]);
    assert_eq!(out[4], vec!["S = zero"]);
    assert_eq!(out[5], vec!["S = pos"]);
    assert_eq!(out[6], vec!["X = cv1, Y = cv1"]);
    assert!(out[7].is_empty());
    assert_eq!(out[8], vec!["X = cv1, N = 1", "X = cv2, N = 2"]);
}

#[test]
fn negation_succeeds_exactly_when_the_goal_has_no_solution() {
    // CLM-0022: `\+ G` binds nothing; its own variables are local; it
    // nests; it applies to constraint blocks and parenthesised bodies.
    let out = answers(
        "model_file(m1). model_file(m2). model_file(m3).
         claim(m1, residual_error, low). claim(m3, residual_error, high).
         missing(F) :- model_file(F), \\+ claim(F, residual_error, _).
         colour(red). colour(blue). size(big).
         member(X, <X> . _).
         member(X, <_> . T) :- member(X, T).
         ?- missing(F).
         ?- \\+ colour(green).
         ?- \\+ colour(red).
         ?- \\+ \\+ colour(red).
         ?- member(X, <1, 2, 3>), \\+ { X = 2 }.
         ?- \\+ (colour(X), size(X)).
         ?- \\+ colour(X).
         ?- colour(X), \\+ { X = red }.",
    );
    assert_eq!(out[0], vec!["F = m2"]);
    assert_eq!(out[1], vec!["true"]);
    assert!(out[2].is_empty());
    assert_eq!(out[3], vec!["true"]);
    assert_eq!(out[4], vec!["X = 1", "X = 3"]);
    assert_eq!(out[5], vec!["true"]);
    // `X` occurs only inside the construct: it is local, and the negation
    // asks whether no colour exists.
    assert!(out[6].is_empty());
    assert_eq!(out[7], vec!["X = blue"]);
}

#[test]
fn a_condition_is_finalized_before_the_commit() {
    // CLM-0022: a solution of the condition is one the answer stage would
    // accept. `X` is determined through the tableau, not by its bounds, so
    // only the exact (finalize) check sees `X != 1` violated; the else
    // branch is taken, and the negation holds.
    let out = answers(
        "?- ({ X + Y = 2, X - Y = 0, X != 1 } -> { Z = yes } ; { Z = no }).
         ?- { X + Y = 2, X - Y = 0 }, ({ X != 1 } -> { Z = yes } ; { Z = no }).
         ?- \\+ { X + Y = 2, X - Y = 0, X != 1 }.",
    );
    assert_eq!(out[0], vec!["Z = no"]);
    assert_eq!(out[1], vec!["X = 1, Y = 1, Z = no"]);
    assert_eq!(out[2], vec!["true"]);
    // A non-linear residue at the commit stops the query, as at an answer.
    let err = query_error("?- ({ X * Y = 2 } -> { Z = yes } ; { Z = no }).");
    assert!(matches!(err, EvalError::NonLinear { .. }), "{err}");
}

#[test]
fn a_non_ground_external_variable_is_an_error_not_a_decision() {
    // CLM-0022, the groundness rule: a variable the condition shares with
    // the clause outside the construct must be ground when the construct is
    // reached. The error names the construct, the variable and the clause.
    let err = query_error(
        "p(a). p(b).
         r(X) :- \\+ { X = a }.
         s() :- \\+ r(X).
         ?- s().",
    );
    assert_eq!(
        err,
        EvalError::NonGroundCondition {
            construct: "\\+ { X = a }".into(),
            variable: "X".into(),
            clause: "r(X) :- \\+ { X = a }.".into(),
        }
    );
    // The Prolog idiom with the result in the condition: `X` is in the head.
    let err = query_error(
        "curated(k, v). true().
         value(K, X) :- (curated(K, X) -> true() ; harvested(K, X)).
         ?- value(k, X).",
    );
    assert!(
        matches!(&err, EvalError::NonGroundCondition { variable, .. } if variable == "X"),
        "{err}"
    );
    // In a query the surroundings are the other goals.
    let err = query_error("p(a).\n?- \\+ p(X), p(X).");
    assert!(
        matches!(&err, EvalError::NonGroundCondition { construct, clause, .. }
            if construct == "\\+ p(X)" && clause == "?- \\+ p(X), p(X)."),
        "{err}"
    );
    // A numeric variable is ground only when the solver determines it.
    let err = query_error("?- { X > 1 }, ({ X > 5 } -> { Z = yes } ; { Z = no }).");
    assert!(
        matches!(&err, EvalError::NonGroundCondition { variable, .. } if variable == "X"),
        "{err}"
    );
    let out = answers(
        "?- { X + Y = 2, X - Y = 0 }, ({ X > 5 } -> { Z = yes } ; { Z = no }).
         ?- { X = f(Y), Y = 3 }, (\\+ { X = f(3) } -> { Z = yes } ; { Z = no }).",
    );
    assert_eq!(out[0], vec!["X = 1, Y = 1, Z = no"]);
    assert_eq!(out[1], vec!["X = f(3), Y = 3, Z = no"]);
    // A variable bound to a term with an unbound variable is not ground.
    let err = query_error("?- { X = f(Y) }, (\\+ { X = f(3) } -> { Z = yes } ; { Z = no }).");
    assert!(
        matches!(&err, EvalError::NonGroundCondition { variable, .. } if variable == "X"),
        "{err}"
    );
}

#[test]
fn the_groundness_rule_exempts_the_constructs_own_variables() {
    // CLM-0022: variables shared only with the then or else branch, or with
    // the other alternative of a disjunction, are the construct's own.
    let out = answers(
        "q(1). q(2). r(a).
         then_shared(V) :- (q(X) -> { V = X } ; { V = none }).
         else_shared(V) :- (r(X) -> { V = X } ; q(X), { V = X }).
         else_only(V) :- (r(z) -> { V = found } ; q(X), { V = X }).
         not_local() :- \\+ (q(X), { X > 5 }).
         ?- then_shared(V).
         ?- else_shared(V).
         ?- else_only(V).
         ?- not_local().
         ?- q(X) ; r(X).",
    );
    assert_eq!(out[0], vec!["V = 1"]);
    assert_eq!(out[1], vec!["V = a"]);
    assert_eq!(out[2], vec!["V = 1", "V = 2"]);
    assert_eq!(out[3], vec!["true"]);
    assert_eq!(out[4], vec!["X = 1", "X = 2", "X = a"]);
}

#[test]
fn control_constructs_and_may_continue() {
    // CLM-0022: an untried else branch or alternative is an alternative the
    // REPL may step to; a commit leaves none behind.
    let src = "q(1). q(2).
         ?- q(X) ; { X = 3 }.
         ?- q(X) -> { Y = X } ; { Y = 0 }.";
    let clauses = parse_program(src).expect("parses");
    let program = Program::compile(&clauses).expect("compiles");
    let mut sols = program.solve(&program.queries()[0]);
    assert_eq!(sols.next().unwrap().to_string(), "X = 1");
    assert!(sols.may_continue());
    assert_eq!(sols.next().unwrap().to_string(), "X = 2");
    assert!(sols.may_continue());
    assert_eq!(sols.next().unwrap().to_string(), "X = 3");
    assert!(!sols.may_continue());
    let mut sols = program.solve(&program.queries()[1]);
    assert_eq!(sols.next().unwrap().to_string(), "X = 1, Y = 1");
    assert!(!sols.may_continue());
}

/// Answers with explanations (CLM-0023), compiled with positions.
fn explained(src: &str) -> Vec<Vec<crate::Answer>> {
    let clauses = parse_program_spanned(src).expect("parses");
    let program = Program::compile_spanned(&clauses).expect("compiles");
    program
        .queries()
        .iter()
        .map(|q| program.solve(q).with_explanations().collect())
        .collect()
}

const INTEGRITY: &str = "k(c17, 5). k(c18, 2).
n(c17, 3). n(c18, 4).
violation(C) :- k(C, K), n(C, N), { K > N }.
?- violation(C).";

#[test]
fn explanations_record_the_derivation_with_citations() {
    // CLM-0023: the proof tree cites the rule and the facts by position and
    // text, and the constraint as the instance posted; off by default.
    use crate::Resolution::*;
    let out = explained(INTEGRITY);
    assert_eq!(out[0].len(), 1);
    let a = &out[0][0];
    assert_eq!(a.to_string(), "C = c17");
    let roots = a.explanation.as_ref().expect("explained");
    assert_eq!(roots.len(), 1);
    let root = &roots[0];
    assert_eq!(root.goal, "violation(c17)");
    assert_eq!(root.by, Rule);
    assert_eq!((root.line, root.column), (Some(3), Some(1)));
    assert_eq!(
        root.clause.as_deref(),
        Some("violation(C) :- k(C, K), n(C, N), { K > N }.")
    );
    let subs: Vec<(&str, crate::Resolution, Option<usize>, Option<usize>)> = root
        .subgoals
        .iter()
        .map(|n| (n.goal.as_str(), n.by, n.line, n.column))
        .collect();
    assert_eq!(
        subs,
        vec![
            ("k(c17, 5)", Fact, Some(1), Some(1)),
            ("n(c17, 3)", Fact, Some(2), Some(1)),
            ("{ 5 > 3 }", Constraint, Some(3), Some(1)),
        ]
    );
    assert_eq!(root.subgoals[0].clause.as_deref(), Some("k(c17, 5)."));
    assert_eq!(root.subgoals[2].clause, None);
    assert_eq!(
        a.explanation_lines(),
        vec![
            "  violation(c17)  rule 3:1  violation(C) :- k(C, K), n(C, N), { K > N }.",
            "    k(c17, 5)  fact 1:1",
            "    n(c17, 3)  fact 2:1",
            "    { 5 > 3 }  constraint 3:1",
        ]
    );
    // Off by default: the answer is the same and carries no explanation.
    let plain = answers(INTEGRITY);
    assert_eq!(plain[0], vec!["C = c17"]);
    let clauses = parse_program(INTEGRITY).unwrap();
    let program = Program::compile(&clauses).unwrap();
    let first = program.solve(&program.queries()[0]).next().unwrap();
    assert!(first.explanation.is_none());
    assert!(first.explanation_lines().is_empty());
}

#[test]
fn explanations_cover_the_control_constructs_and_builtins() {
    // CLM-0023: a committed condition's proofs go under it and the then
    // branch follows; a failed condition and a negation are `no-solution`
    // leaves as written; disjunction is transparent; a built-in and a
    // query-level constraint are leaves without a citation.
    use crate::Resolution::*;
    let out = explained(
        "curated(k1, cv1). harvested(k2, hv2).
value(K, V) :- (curated(K, X) -> { V = X } ; harvested(K, V)).
model_file(m1). claim(m1, x, y). model_file(m2).
missing(F) :- model_file(F), \\+ claim(F, x, _).
colour(red). colour(blue).
feature(X) :- colour(X) ; { X = none }.
?- value(k1, V).
?- value(k2, V).
?- missing(F).
?- feature(X).
?- atom_chars(A, \"ab\"), { L = |\"abc\"| }.",
    );
    let kinds = |a: &crate::Answer| -> Vec<(String, crate::Resolution, usize)> {
        let root = &a.explanation.as_ref().unwrap()[0];
        root.subgoals
            .iter()
            .map(|n| (n.goal.clone(), n.by, n.subgoals.len()))
            .collect()
    };
    assert_eq!(
        kinds(&out[0][0]),
        vec![
            (
                "curated(K, X) -> { V = X } ; harvested(K, V)".to_string(),
                Committed,
                1
            ),
            ("{ cv1 = cv1 }".to_string(), Constraint, 0),
        ]
    );
    assert_eq!(
        out[0][0].explanation.as_ref().unwrap()[0].subgoals[0].subgoals[0].goal,
        "curated(k1, cv1)"
    );
    assert_eq!(
        kinds(&out[1][0]),
        vec![
            (
                "curated(K, X) -> { V = X } ; harvested(K, V)".to_string(),
                NoSolution,
                0
            ),
            ("harvested(k2, hv2)".to_string(), Fact, 0),
        ]
    );
    assert_eq!(
        kinds(&out[2][0]),
        vec![
            ("model_file(m2)".to_string(), Fact, 0),
            ("\\+ claim(F, x, _)".to_string(), NoSolution, 0),
        ]
    );
    assert_eq!(out[3].len(), 3);
    assert_eq!(
        kinds(&out[3][0]),
        vec![("colour(red)".to_string(), Fact, 0)]
    );
    assert_eq!(
        kinds(&out[3][1]),
        vec![("colour(blue)".to_string(), Fact, 0)]
    );
    assert_eq!(
        kinds(&out[3][2]),
        vec![("{ none = none }".to_string(), Constraint, 0)]
    );
    let roots = out[4][0].explanation.as_ref().unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!(
        (roots[0].goal.as_str(), roots[0].by),
        ("atom_chars(ab, <a, b>)", Builtin)
    );
    assert_eq!(
        (roots[1].goal.as_str(), roots[1].by),
        ("{ 3 = 3 }", Constraint)
    );
    assert_eq!((roots[1].line, roots[1].clause.as_deref()), (None, None));
}

#[test]
fn explanation_names_agree_with_the_answer() {
    // CLM-0023: the explanation is rendered by the answer's own printer.
    let out = explained("p(f(Y)) :- q(Y).\nany(_).\nq(Y) :- any(Y).\n?- p(X).");
    let a = &out[0][0];
    assert_eq!(a.to_string(), "X = f(_1)");
    assert_eq!(
        a.explanation_lines(),
        vec![
            "  p(f(_1))  rule 1:1  p(f(Y)) :- q(Y).",
            "    q(_1)  rule 3:1  q(Y) :- any(Y).",
            "      any(_1)  fact 2:1",
        ]
    );
}

#[test]
fn explanations_discard_abandoned_derivations() {
    // CLM-0023: a clause tried and backtracked over leaves no record.
    let out = explained("a(1). a(2).\nb(2).\nr(X) :- a(X), b(X).\n?- r(X).");
    assert_eq!(out[0].len(), 1);
    assert_eq!(
        out[0][0].explanation_lines(),
        vec![
            "  r(2)  rule 3:1  r(X) :- a(X), b(X).",
            "    a(2)  fact 1:7",
            "    b(2)  fact 2:1",
        ]
    );
}

#[test]
fn cited_clauses_reproduce_the_answer() {
    // CLM-0023, answer-soundness: an explanation cites only steps on the
    // answer's own derivation, so the cited clauses alone, as a program,
    // give the same answer.
    let out = explained(INTEGRITY);
    let roots = out[0][0].explanation.as_ref().unwrap();
    let mut cited: Vec<String> = Vec::new();
    let mut work: Vec<&crate::ProofNode> = roots.iter().collect();
    while let Some(n) = work.pop() {
        if let Some(c) = &n.clause {
            if !cited.contains(c) {
                cited.push(c.clone());
            }
        }
        work.extend(n.subgoals.iter());
    }
    assert_eq!(cited.len(), 3);
    let program = format!("{}\n?- violation(C).", cited.join("\n"));
    assert_eq!(answers(&program)[0], vec!["C = c17"]);
}

#[test]
fn a_deep_derivation_explains_on_a_small_stack() {
    // CLM-0023: the arena is built into a tree, rendered and dropped without
    // recursion, so a 20 000-deep derivation explains on a 1 MiB thread. The
    // chain keeps every goal instance small (an explanation prints each
    // goal in full, so a recursion over a deep term is quadratic in text by
    // nature, as is the indentation of the human form in depth): the human
    // lines are exercised at a depth of 500, the tree at 20 000.
    fn chain(n: usize) -> String {
        let mut src = String::new();
        for i in 0..n {
            src.push_str(&format!("next(a{i}, a{}).\n", i + 1));
        }
        src.push_str("reach(X, X).\nreach(X, Z) :- next(X, Y), reach(Y, Z).\n");
        src.push_str(&format!("?- reach(a0, a{n})."));
        src
    }
    on_small_stack(|| {
        let out = explained(&chain(20_000));
        let a = &out[0][0];
        assert_eq!(a.to_string(), "true");
        let roots = a.explanation.as_ref().unwrap();
        let (mut depth, mut node) = (1, &roots[0]);
        while let Some(last) = node.subgoals.last() {
            depth += 1;
            node = last;
        }
        assert_eq!(depth, 20_001);
        assert_eq!(
            (node.goal.as_str(), node.by),
            ("reach(a20000, a20000)", crate::Resolution::Fact)
        );
        drop(out);
        let out = explained(&chain(500));
        let lines = out[0][0].explanation_lines();
        // Per level a `reach` node and a `next` leaf; the last level a fact.
        assert_eq!(lines.len(), 2 * 500 + 1);
        assert_eq!(
            lines[0],
            "  reach(a0, a500)  rule 502:1  reach(X, Z) :- next(X, Y), reach(Y, Z)."
        );
        assert_eq!(lines[1], "    next(a0, a1)  fact 1:1");
        assert_eq!(lines[1000].trim_start(), "reach(a500, a500)  fact 501:1");
    });
}

#[test]
fn a_long_numeric_derivation_is_linear() {
    // CLM-0024: each step of the countdown leaves a determined numeric
    // variable and a settled disequation behind; the determination pass
    // after every post must not walk them again. Before the change 2 000
    // steps took 55 s in this profile; the budget below is a generous
    // multiple of the linear cost.
    let start = std::time::Instant::now();
    let out = answers(
        "down(0).
         down(N) :- { N > 0, M = N - 1, M != -1 }, down(M).
         ?- down(2000).",
    );
    assert_eq!(out[0], vec!["true"]);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(20),
        "took {:?}",
        start.elapsed()
    );
}

#[test]
fn comprehensions_collect_all_solutions_in_order() {
    // CLM-0025: `<T | Body>` is the list of the instances of T for each
    // solution of Body, in solution order; `<>` when there is none; nested
    // in a template it is collected per solution; its length is a count.
    let out = answers(
        "p(1). p(2). p(3).
         member_of(m1, c1). member_of(m2, c1). member_of(m3, c2).
         cohort(c1). cohort(c2).
         small(C) :- cohort(C), { |<M | member_of(M, C)>| < 2 }.
         ?- { L = <X | p(X)> }.
         ?- { L = <<X, Y> | p(X), p(Y), { X < Y }> }.
         ?- { L = <X | p(X), { X > 7 }> }.
         ?- { L = <<X, |<Y | p(Y), { Y < X }>|> | p(X)> }.
         ?- { N = |<X | p(X)>| }, { N > 2 }.
         ?- small(C).
         ?- { L = <X + 1 | p(X)> }.
         ?- \\+ { |<X | p(X)>| > 5 }.
         ?- cohort(C), ({ |<M | member_of(M, C)>| > 1 } -> { S = big } ; { S = small }).",
    );
    assert_eq!(out[0], vec!["L = <1, 2, 3>"]);
    assert_eq!(out[1], vec!["L = <<1, 2>, <1, 3>, <2, 3>>"]);
    assert_eq!(out[2], vec!["L = <>"]);
    assert_eq!(out[3], vec!["L = <<1, 0>, <2, 1>, <3, 2>>"]);
    assert_eq!(out[4], vec!["N = 3"]);
    assert_eq!(out[5], vec!["C = c2"]);
    assert_eq!(out[6], vec!["L = <2, 3, 4>"]);
    assert_eq!(out[7], vec!["true"]);
    assert_eq!(out[8], vec!["C = c1, S = big", "C = c2, S = small"]);
}

#[test]
fn comprehensions_apply_the_groundness_rules() {
    // CLM-0025: an external variable (shared with the clause outside the
    // comprehension, the rest of its goal included) must be ground; every
    // instance must be ground (no unbound or undetermined variable, no
    // cycle); each solution is finalized exactly before its instance is
    // taken.
    let err = query_error("p(1).\n?- { N = |<X | p(X, N)>| }.");
    assert!(
        matches!(&err, EvalError::NonGroundCondition { construct, variable, .. }
            if construct == "<X | p(X, N)>" && variable == "N"),
        "{err}"
    );
    let err = query_error("q(X) :- { X > 1 }.\n?- { L = <X | q(X)> }.");
    assert_eq!(
        err,
        EvalError::NonGroundSolution {
            construct: "<X | q(X)>".into(),
            instance: "X".into(),
        }
    );
    let err = query_error("r(f(Y)).\n?- { L = <T | r(T)> }.");
    assert!(
        matches!(&err, EvalError::NonGroundSolution { instance, .. } if instance == "f(_1)"),
        "{err}"
    );
    // A cyclic instance has no finite copy.
    let err = query_error("?- { L = <X | { X = f(X) }> }.");
    assert!(matches!(&err, EvalError::NonGroundSolution { .. }), "{err}");
    // A determined numeric variable is ground; a solution a disequation
    // rejects is not a solution (the exact check at each one).
    let out = answers(
        "?- { L = <X | { X + Y = 2, X - Y = 0 }> }.
         ?- { L = <X | { X + Y = 2, X - Y = 0, X != 1 }> }.",
    );
    assert_eq!(out[0], vec!["L = <1>"]);
    assert_eq!(out[1], vec!["L = <>"]);
    // A comprehension in a head or a constraint fact is a compile error; the
    // constraint of an implication `{c} => head` is a body and admits one.
    for src in ["bad(<X | p(X)>).", "{ |<X | p(X)>| > 1 }."] {
        let clauses = parse_program_spanned(src).expect("parses");
        let err = Program::compile_spanned(&clauses).unwrap_err();
        assert!(
            matches!(err, EvalError::ComprehensionInHead { span: Some(_) }),
            "{src}: {err}"
        );
    }
    let out = answers("p(1). p(2).\n{ |<X | p(X)>| > 1 } => two().\n?- two().");
    assert_eq!(out[0], vec!["true"]);
}

#[test]
fn sum_is_a_linear_relation_over_a_list() {
    // CLM-0025: exact, solvable in either direction, delayed on an open
    // spine, printed in goal form while pending; a non-list fails.
    let out = answers(
        "amount(a, 1/3). amount(b, 1/3). amount(c, 1/3).
         total(T) :- sum(<A | amount(_, A)>, T).
         average(V) :- { L = <A | amount(_, A)> }, sum(L, T), { V = T / |L| }.
         ?- total(T).
         ?- average(V).
         ?- sum(<1, X, 3>, 10).
         ?- sum(<>, S).
         ?- sum(L, S), { L = <1, 2> }.
         ?- { L = <1> . T }, sum(L, S).
         ?- sum(<1> . T, S), { T = <2, 3> }.
         ?- sum(<X, Y>, 5), { X - Y = 1 }.
         ?- sum(a, S).
         ?- sum(<age(alice), 1>, S), { age(alice) = 29 }.",
    );
    assert_eq!(out[0], vec!["T = 1"]);
    assert_eq!(out[1], vec!["V = 1/3"]);
    assert_eq!(out[2], vec!["X = 6"]);
    assert_eq!(out[3], vec!["S = 0"]);
    assert_eq!(out[4], vec!["L = <1, 2>, S = 3"]);
    assert_eq!(out[5], vec!["L = <1> . T, sum(<1> . T, S)"]);
    assert_eq!(out[6], vec!["T = <2, 3>, S = 6"]);
    assert_eq!(out[7], vec!["X = 3, Y = 2"]);
    assert!(out[8].is_empty());
    assert_eq!(out[9], vec!["S = 30, age(alice) = 29"]);
    // sum/2 is reserved.
    let clauses = parse_program("sum(X, Y).").unwrap();
    assert!(matches!(
        Program::compile(&clauses).unwrap_err(),
        EvalError::ReservedBuiltin { .. }
    ));
}

#[test]
fn a_comprehension_explains_as_one_collected_leaf() {
    // CLM-0025 with CLM-0023: the solutions' proofs are discarded; the leaf
    // shows the comprehension and its list.
    let out = explained("p(1). p(2).\ncount(N) :- { N = |<X | p(X)>| }.\n?- count(N).");
    assert_eq!(
        out[0][0].explanation_lines(),
        vec![
            "  count(2)  rule 2:1  count(N) :- { N = |<X | p(X)>| }.",
            "    <X | p(X)> = <1, 2>  collected",
            "    { 2 = 2 }  constraint 2:1",
        ]
    );
    assert_eq!(
        out[0][0].explanation.as_ref().unwrap()[0].subgoals[0].by,
        crate::Resolution::Collected
    );
}

#[test]
fn a_large_collection_builds_on_a_small_stack() {
    // CLM-0025: 20 000 solutions are copied and the list is rebuilt
    // iteratively.
    on_small_stack(|| {
        let mut src = String::new();
        for i in 0..20_000 {
            src.push_str(&format!("v(a{i}).\n"));
        }
        src.push_str("?- { N = |<X | v(X)>| }.\n?- { L = <X | v(X)> }, { |L| = 20000 }.");
        let out = answers(&src);
        assert_eq!(out[0], vec!["N = 20000"]);
        assert_eq!(out[1].len(), 1);
    });
}

// --- CLM-0027: tabling -----------------------------------------------------

const CYCLE: &str = "par(a, b). par(b, c). par(c, a). par(c, d). par(b, d).";

#[test]
fn tabled_transitive_closure_terminates_on_a_cycle_in_either_direction() {
    // docs/design/2026-11-10-tabling-for-recursive-rules.md: the left- and
    // right-recursive definitions give the same finite table on cyclic
    // data, each answer once, in insertion order.
    let out = answers(&format!(
        ":- table anc/2, reach/2.
         {CYCLE}
         anc(X, Z) :- anc(X, Y), par(Y, Z).
         anc(X, Y) :- par(X, Y).
         reach(X, Y) :- par(X, Y).
         reach(X, Z) :- par(X, Y), reach(Y, Z).
         ?- anc(a, X).
         ?- reach(a, X).
         ?- anc(X, d).
         ?- anc(a, a).
         ?- anc(d, X).
         ?- anc(X, Y)."
    ));
    assert_eq!(out[0], vec!["X = b", "X = c", "X = d", "X = a"]);
    assert_eq!(out[1], vec!["X = b", "X = c", "X = d", "X = a"]);
    assert_eq!(out[2], vec!["X = a", "X = b", "X = c"]);
    assert_eq!(out[3], vec!["true"]);
    assert!(out[4].is_empty());
    // Three sources reach all four nodes; nothing twice.
    assert_eq!(out[5].len(), 12);
    let mut sorted = out[5].clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 12);
}

#[test]
fn tables_are_sets_and_belong_to_the_program() {
    // Two paths to one answer give it once; the second query consumes the
    // table the first computed (the registry lives on the program).
    let clauses = parse_program(
        ":- table r/2.
         e(a, b). e(a, c). e(b, d). e(c, d).
         r(X, Y) :- e(X, Y).
         r(X, Z) :- e(X, Y), r(Y, Z).
         ?- r(a, d).
         ?- r(a, X).",
    )
    .unwrap();
    let program = Program::compile(&clauses).unwrap();
    let first: Vec<String> = program
        .solve(&program.queries()[0])
        .map(|a| a.to_string())
        .collect();
    assert_eq!(first, vec!["true"]);
    let second: Vec<String> = program
        .solve(&program.queries()[1])
        .map(|a| a.to_string())
        .collect();
    assert_eq!(second, vec!["X = b", "X = c", "X = d"]);
    // Answers recorded over both queries: `r(a, d)`, `r(b, d)`, `r(c, d)`
    // (the first; `r(d, d)` is empty), then `r(b, _)`, `r(c, _)` and the
    // three of `r(a, _)` — each once.
    assert_eq!(program.tables.borrow().total, 8);
}

#[test]
fn a_tabled_call_is_filtered_by_the_callers_store() {
    // The table is for the abstracted call; the caller's constraints on the
    // arguments apply when the ground answers are unified, in either goal
    // order, so no call is refused for its store.
    let out = answers(
        ":- table num/1.
         num(1). num(2). num(3).
         same(X, X).
         ?- { X > 1 }, num(X).
         ?- num(X), { X > 1 }.
         ?- { X != 2 }, num(X).
         ?- same(X, 3), num(X).
         ?- num(4).",
    );
    assert_eq!(out[0], vec!["X = 2", "X = 3"]);
    assert_eq!(out[1], vec!["X = 2", "X = 3"]);
    assert_eq!(out[2], vec!["X = 1", "X = 3"]);
    assert_eq!(out[3], vec!["X = 3"]);
    assert!(out[4].is_empty());
}

#[test]
fn a_table_is_computed_over_the_world_not_the_callers_store() {
    // The record's counterexample: a tabled body over a world unknown must
    // not be memoised under a caller's constraint. With the world fixing the
    // unknown the solution is a fact (entailed, whatever the caller posted,
    // which here just fails); without it the solution is conditional and
    // the table refuses it.
    let out = answers(
        ":- table old/1, older/1.
         { age(bob) = 75 }. { age(cy) > 70 }.
         old(bob) :- { age(bob) > 70 }.
         older(cy) :- { age(cy) > 60 }.
         ?- ({ age(bob) < 60 }, old(bob) ; old(bob)).
         ?- old(bob).
         ?- older(cy).",
    );
    assert_eq!(out[0], vec!["true"]);
    assert_eq!(out[1], vec!["true"]);
    assert_eq!(out[2], vec!["true"]);
    let err = query_error(
        ":- table old/1.
         old(bob) :- { age(bob) > 70 }.
         ?- old(bob).",
    );
    assert!(
        matches!(&err, EvalError::ConstrainedTabledAnswer { predicate, residual, .. }
            if predicate == "old/1" && residual == "age(bob) > 70"),
        "{err:?}"
    );
    // A disequation decided inside the predicate is fine; one left pending
    // on a world unknown is a residual.
    let out = answers(
        ":- table dd/1.
         num(1). num(2). num(3).
         dd(X) :- num(X), { X != 2 }.
         ?- dd(X).",
    );
    assert_eq!(out[0], vec!["X = 1", "X = 3"]);
}

#[test]
fn tabled_answers_must_be_finite_ground_terms() {
    for src in [
        ":- table n/1. n(X) :- { X > 0 }. ?- n(X).",
        ":- table t/1. t(X). ?- t(X).",
        ":- table c/1. c(X) :- { X = f(X) }. ?- c(X).",
    ] {
        let err = query_error(src);
        assert!(
            matches!(&err, EvalError::NonGroundTabledAnswer { .. }),
            "{src}: {err:?}"
        );
    }
}

#[test]
fn sub_searches_are_decided_on_complete_tables_only() {
    // A negation or a comprehension over a completed table is fine; inside
    // the predicate's own recursive group the table is incomplete when the
    // construct would be decided, and the query stops (invariant 5).
    let out = answers(&format!(
        ":- table anc/2.
         {CYCLE}
         anc(X, Z) :- anc(X, Y), par(Y, Z).
         anc(X, Y) :- par(X, Y).
         ?- \\+ anc(d, a).
         ?- \\+ anc(a, d).
         ?- {{ N = |<X | anc(a, X)>| }}.
         ?- (anc(a, d) -> {{ R = yes }} ; {{ R = no }})."
    ));
    assert_eq!(out[0], vec!["true"]);
    assert!(out[1].is_empty());
    assert_eq!(out[2], vec!["N = 4"]);
    assert_eq!(out[3], vec!["R = yes"]);
    let err = query_error(&format!(
        ":- table s/1.
         {CYCLE}
         s(X) :- par(X, Y), \\+ s(Y).
         ?- s(a)."
    ));
    assert!(
        matches!(&err, EvalError::IncompleteTable { predicate, construct }
            if predicate == "s/1" && construct == "\\+ s(Y)"),
        "{err:?}"
    );
    let err = query_error(&format!(
        ":- table cnt/1.
         {CYCLE}
         cnt(X) :- par(X, Y), {{ N = |<Z | cnt(Z)>| }}, {{ N > 0 }}.
         ?- cnt(a)."
    ));
    assert!(
        matches!(&err, EvalError::IncompleteTable { construct, .. } if construct == "<Z | cnt(Z)>"),
        "{err:?}"
    );
}

#[test]
fn mutual_recursion_completes_as_one_group() {
    let out = answers(
        ":- table p/1, q/1, even/1, odd/1.
         p(X) :- q(X).
         p(1).
         q(X) :- p(X).
         q(2).
         even(zero).
         even(s(X)) :- odd(X).
         odd(s(X)) :- even(X).
         ?- p(X).
         ?- q(X).
         ?- even(s(s(zero))).
         ?- odd(s(s(zero))).
         ?- odd(s(zero)).",
    );
    assert_eq!(out[0], vec!["X = 2", "X = 1"]);
    assert_eq!(out[1], vec!["X = 2", "X = 1"]);
    assert_eq!(out[2], vec!["true"]);
    assert!(out[3].is_empty());
    assert_eq!(out[4], vec!["true"]);
}

#[test]
fn directives_are_checked_at_compile_time() {
    for (src, needle) in [
        (":- tabel foo/1.", "unknown directive `tabel`"),
        (":- table sum/2.", "built-in"),
        (":- table '<>'/2.", "built-in"),
        (":- table foo/1.5.", "non-negative integer"),
    ] {
        let clauses = parse_program_spanned(src).expect("parses");
        let err = Program::compile_spanned(&clauses).unwrap_err();
        assert!(
            matches!(&err, EvalError::BadDirective { message, span: Some(_) } if message.contains(needle)),
            "{src}: {err:?}"
        );
    }
    // A table for a predicate with no clauses is empty; a nullary one works.
    let out = answers(":- table und/1, z/0. z(). ?- und(X). ?- z().");
    assert!(out[0].is_empty());
    assert_eq!(out[1], vec!["true"]);
}

#[test]
fn a_tabled_call_explains_with_its_recorded_derivation() {
    // The `tabled` node carries the derivation that first produced the
    // answer; its cited clauses reproduce the answer (invariant 6).
    use crate::Resolution::*;
    let out = explained(
        ":- table anc/2.
         par(a, b). par(b, c). par(c, a).
         anc(X, Z) :- anc(X, Y), par(Y, Z).
         anc(X, Y) :- par(X, Y).
         ?- anc(a, c).",
    );
    let roots = out[0][0].explanation.as_ref().unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].by, Tabled);
    assert_eq!(roots[0].goal, "anc(a, c)");
    let rule = &roots[0].subgoals[0];
    assert_eq!(rule.by, Rule);
    assert_eq!(rule.line, Some(3));
    assert_eq!(rule.subgoals[0].by, Tabled);
    assert_eq!(rule.subgoals[0].goal, "anc(a, b)");
    assert_eq!(rule.subgoals[1].by, Fact);
    assert_eq!(rule.subgoals[1].goal, "par(b, c)");
    let mut cited: Vec<String> = Vec::new();
    let mut work: Vec<&crate::ProofNode> = roots.iter().collect();
    while let Some(n) = work.pop() {
        if let Some(c) = &n.clause {
            if !cited.contains(c) {
                cited.push(c.clone());
            }
        }
        work.extend(n.subgoals.iter());
    }
    let program = format!(":- table anc/2.\n{}\n?- anc(a, c).", cited.join("\n"));
    assert_eq!(answers(&program)[0], vec!["true"]);
}

#[test]
fn a_long_tabled_chain_runs_on_a_small_stack() {
    // Evaluations nest in the machine's own stack, not Rust's: a chain of
    // 400 distinct variant calls (right recursion) and a 400-pass leader
    // (left recursion) both run on a 1 MiB thread.
    fn chain(n: usize) -> String {
        let mut src = String::from(":- table reach/2, anc/2.\n");
        for i in 0..n {
            src.push_str(&format!("next(a{i}, a{}).\n", i + 1));
        }
        src.push_str("reach(X, Y) :- next(X, Y).\nreach(X, Z) :- next(X, Y), reach(Y, Z).\n");
        src.push_str("anc(X, Z) :- anc(X, Y), next(Y, Z).\nanc(X, Y) :- next(X, Y).\n");
        src.push_str(&format!(
            "?- reach(a0, a{n}).\n?- {{ N = |<Y | reach(a0, Y)>| }}.\n?- anc(a0, a{n}).\n?- {{ N = |<Y | anc(a0, Y)>| }}."
        ));
        src
    }
    on_small_stack(|| {
        let out = answers(&chain(400));
        assert_eq!(out[0], vec!["true"]);
        assert_eq!(out[1], vec!["N = 400"]);
        assert_eq!(out[2], vec!["true"]);
        assert_eq!(out[3], vec!["N = 400"]);
    });
}

#[test]
fn an_interrupt_inside_an_evaluation_cleans_the_tables() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    // Infinitely many ground answers: tabling does not terminate either; the
    // flag stops it, and the half-built table is forgotten so the next query
    // starts clean.
    let clauses = parse_program(
        ":- table nat/1.
         nat(zero).
         nat(s(X)) :- nat(X).
         ?- nat(X), { 1 = 2 }.
         ?- nat(s(zero)).",
    )
    .unwrap();
    let program = Program::compile(&clauses).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let mut sols = program
        .solve(&program.queries()[0])
        .with_interrupt(flag.clone());
    let setter = {
        let flag = flag.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            flag.store(true, Ordering::Relaxed);
        })
    };
    assert!(sols.next().is_none());
    assert!(sols.interrupted());
    setter.join().unwrap();
    assert!(program.tables.borrow().is_empty());
    // The next query recomputes from scratch — with an answer bound the
    // recursion is finite.
    let out: Vec<String> = program
        .solve(&program.queries()[1])
        .map(|a| a.to_string())
        .collect();
    assert_eq!(out, vec!["true"]);
}

/// CLM-0028: the program and every store it runs share one symbol table,
/// so an atom interned at run time inside a table evaluation (by
/// `atom_chars`) means the same name in the caller's store. Before, the
/// evaluation's store was a clone of the world's table: the first query
/// panicked on an unknown symbol index and the second answered `A = ab`.
#[test]
fn a_symbol_interned_in_a_table_evaluation_is_known_to_the_caller() {
    let out = answers(
        ":- table mk/1.
         mk(A) :- atom_chars(A, <x, y, z>).
         ?- mk(A).
         ?- atom_chars(B, <a, b>), mk(A).
         ?- mk(A), atom_chars(A, L).",
    );
    assert_eq!(out[0], vec!["A = xyz"]);
    assert_eq!(out[1], vec!["B = ab, A = xyz"]);
    assert_eq!(out[2], vec!["A = xyz, L = <x, y, z>"]);
}

/// CLM-0028: the world store holds the program's own table, not a copy —
/// the clone per evaluation was quadratic over nested evaluations.
#[test]
fn the_world_store_shares_the_programs_symbol_table() {
    let clauses = parse_program("p(a). ?- p(X).").unwrap();
    let program = Program::compile(&clauses).unwrap();
    assert!(std::rc::Rc::ptr_eq(
        &program.symbols,
        &program.world.symbols
    ));
    let out: Vec<String> = program
        .solve(&program.queries()[0])
        .map(|a| a.to_string())
        .collect();
    assert_eq!(out, vec!["X = a"]);
}
