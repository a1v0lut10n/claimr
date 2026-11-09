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
    // 100 000 resolution steps: ten walks down a 10 000-deep Peano number.
    // The machine is iterative; what does recurse is the AST side (parsing
    // actions, lowering, Display of a 10 000-deep literal), so run on a
    // thread with a roomy stack — the machine would be indifferent either way.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
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
        })
        .unwrap()
        .join()
        .unwrap();
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
    // Stated consequence of D4: attribute terms make numeric typing succeed.
    assert_eq!(out[6], vec!["X = foo, foo > 3"]);
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
