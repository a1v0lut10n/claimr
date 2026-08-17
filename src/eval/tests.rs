//! Machine-level tests through the public API.

use crate::{parse_program, parse_program_spanned, EvalError, Program};

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
    assert_eq!(Program::compile(&clauses).unwrap_err(), EvalError::InitialStoreUnsatisfiable);
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
    assert_eq!(out[0], vec!["X = f(Z), Y = f(W), f(Z) != f(W)"]);
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
            let src = format!("count(zero). count(s(N)) :- count(N). ?- {}.", calls.join(", "));
            let out = answers(&src);
            assert_eq!(out[0], vec!["true"]);
            // And a long goal list: 20 000 goals in one query.
            let many: Vec<&str> = std::iter::repeat_n("p", 20_000).collect();
            let src = format!("p(). ?- {}.", many.iter().map(|p| format!("{p}()")).collect::<Vec<_>>().join(", "));
            assert_eq!(answers(&src)[0], vec!["true"]);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn stage_2_boundary_is_enforced_with_positions() {
    for (src, needle) in [
        ("p(X) :- { X > 1 }.", "numeric relation `>`"),
        ("{ X + 1 = 3 }.", "arithmetic terms"),
        ("p(1 + 1).", "arithmetic terms"),
        ("?- p(X), { X <= 2 }.", "numeric relation `<=`"),
    ] {
        let clauses = parse_program_spanned(&format!("ok(1).\n{src}\n")).unwrap();
        let err = Program::compile_spanned(&clauses).unwrap_err();
        let EvalError::Unsupported { clause, span, what, .. } = &err else {
            panic!("expected Unsupported for {src}, got {err:?}");
        };
        assert_eq!(*clause, 2, "{src}");
        assert_eq!(span.map(|s| (s.line, s.column)), Some((2, 1)), "{src}");
        assert!(what.contains(needle), "{src}: {what}");
        assert!(err.to_string().starts_with("2:1: clause 2 `"), "{err}");
    }
    // Number literals in = / != are trees and fine.
    assert_eq!(answers("?- { X = 3 }, { X != 4 }.")[0], vec!["X = 3"]);
}
