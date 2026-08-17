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

// --- stage 3: the linear store -----------------------------------------------

fn first(src: &str) -> String {
    answers(src).into_iter().next().unwrap().join(" | ")
}

#[test]
fn linear_equations_determine_variables() {
    assert_eq!(first("?- { X + Y = 10, X - Y = 2 }."), "X = 6, Y = 4");
    assert_eq!(first("?- { 3 * X = 1 }."), "X = 1/3");
    assert_eq!(first("?- { X = 1 / 3 }."), "X = 1/3");
    assert_eq!(first("?- { X = -(2 * Y) }, { Y = 1.5 }."), "X = -3, Y = 3/2");
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
fn inequalities_bounds_and_residuals() {
    assert_eq!(first("?- { X >= 3, X <= 3 }."), "X = 3");
    assert_eq!(first("?- { X > 3, X < 3 }."), "");
    assert_eq!(first("?- { X > 3 }."), "X > 3");
    assert_eq!(first("?- { X > 3 }, { X < 5 }."), "X > 3, X < 5");
    assert_eq!(first("?- { X + Y <= 10 }."), "X + Y <= 10");
    assert_eq!(first("?- { X > Y, Y > Z }."), "X > Y, Y > Z");
    assert_eq!(first("?- { Y = X + 1 }."), "Y = X + 1");
    assert_eq!(first("?- { Y = X + 1 }, { X > 0 }."), "X > 0, Y = X + 1");
    // Two-variable equation with a residual: printed in solved form.
    assert_eq!(first("?- { X + Y = 10, 2*X - Y >= 1/3 }."), "Y = 10 - X, 2*X - Y >= 1/3");
}

#[test]
fn numeric_disequations_are_exact() {
    assert_eq!(first("?- { X - Y = 0 }, { X != Y }."), "");
    assert_eq!(first("same(X, X). ?- { X != 3 }, same(X, 3)."), "");
    assert_eq!(first("same(X, X). ?- { X != 3 }, same(X, 4)."), "X = 4");
    assert_eq!(first("?- { X != 3 }."), "X != 3");
    assert_eq!(first("?- { X > 3 }, { X < 5 }, { X != 4 }."), "X > 3, X < 5, X != 4");
    assert_eq!(first("?- { X + Y = 10 }, { X - Y = 2 }, { X != 6 }."), "");
    // Implied through the store only at answer time: still exact.
    assert_eq!(first("?- { X != Y }, { X >= 1 }, { X <= 1 }, { Y >= 1 }, { Y <= 1 }."), "");
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
    assert_eq!(out[0], vec!["true"]); // entailed by the world
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
    assert_eq!(first("?- { X != Y }, { X = 1 }, { Y = 2 }."), "X = 1, Y = 2");
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
    assert_eq!(Program::compile_spanned(&clauses).unwrap_err(), EvalError::InitialStoreUnsatisfiable);
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
                    if (code.contains("f64") || code.contains("f32")) && !path.ends_with("tests.rs") {
                        hits.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                    }
                }
            }
        }
    }
    let mut hits = Vec::new();
    scan(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut hits);
    assert!(hits.is_empty(), "floating point in src/: {hits:#?}");
}
