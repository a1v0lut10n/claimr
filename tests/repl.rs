// SPDX-License-Identifier: Apache-2.0

//! The interactive loop, driven through a pipe (no TTY): stepping, session
//! extension, commands, loading, errors, `-i` mode.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn run(args: &[&str], input: &str) -> (String, String, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_claimr"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn claimr");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn stepping_answers() {
    let (out, _, code) = run(
        &[],
        "human(socrates).\nhuman(plato).\nmortal(X) :- human(X).\n\
         ?- mortal(W).\n;\n\
         ?- mortal(W).\n\n\
         ?- mortal(W).\n;\n;\n\
         ?- mortal(zeus).\n\
         ?- human(socrates).\n",
    );
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "?- mortal(W).\nW = socrates ;\nW = plato.\n\
         ?- mortal(W).\nW = socrates.\n\
         ?- mortal(W).\nW = socrates ;\nW = plato.\n\
         ?- mortal(zeus).\nfalse.\n\
         ?- human(socrates).\ntrue.\n"
    );
}

#[test]
fn lists_echo_in_their_notation() {
    // CLM-0014: clauses and answers print lists as written.
    let (out, _, code) = run(&[], "first(X, <X> . T).\n:list\n?- first(F, <1, 2> . R).\n");
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "first(X, <X> . T).\n?- first(F, <1, 2> . R).\nF = 1.\n"
    );
}

#[test]
fn the_anonymous_variable_echoes_and_is_not_answered() {
    // CLM-0020.
    let (out, _, code) = run(
        &[],
        "first(X, <X> . _).\n:list\n?- first(F, <1, 2>).\n?- first(_, <1, 2>).\n",
    );
    assert_eq!(code, 0);
    assert_eq!(
        out,
        "first(X, <X> . _).\n?- first(F, <1, 2>).\nF = 1.\n?- first(_, <1, 2>).\ntrue.\n"
    );
}

#[test]
fn stepping_past_the_last_alternative_prints_false() {
    // Two clauses, second fails: after `;` the search is exhausted.
    let (out, _, _) = run(&[], "p(a).\np(X) :- q(X).\n?- p(Y).\n;\n");
    assert_eq!(out, "?- p(Y).\nY = a ;\nfalse.\n");
}

#[test]
fn all_mode_and_limit() {
    let (out, _, _) = run(
        &[],
        "nat(zero).\nnat(s(N)) :- nat(N).\n:all\n:limit 3\n?- nat(X).\n:limit 0\n:all\n?- nat(X).\n\n",
    );
    assert_eq!(
        out,
        "printing all answers\n?- nat(X).\nX = zero\nX = s(zero)\nX = s(s(zero))\n\
         stepping answers\n?- nat(X).\nX = zero.\n"
    );
    // --limit seeds :limit.
    let (out, _, _) = run(
        &["--limit", "2"],
        "nat(zero).\nnat(s(N)) :- nat(N).\n:all\n?- nat(X).\n",
    );
    assert_eq!(
        out,
        "printing all answers\n?- nat(X).\nX = zero\nX = s(zero)\n"
    );
}

#[test]
fn clauses_at_the_prompt_extend_the_session() {
    let (out, err, _) = run(
        &[],
        "eligible(X) :- { age(X) >= 18 }.\n\
         { age(bob) > 70 }.\n\
         ?- eligible(bob).\n\
         { age(bob) < 10 }.\n\
         ?- eligible(bob).\n\
         :list\n",
    );
    assert_eq!(
        out,
        "?- eligible(bob).\nage(bob) > 70.\n\
         ?- eligible(bob).\nage(bob) > 70.\n\
         eligible(X) :- { age(X) >= 18 }.\n{ age(bob) > 70 }.\n"
    );
    assert!(err.contains("constraint facts are unsatisfiable"), "{err}");
    // CLM-0030: the two facts that conflict are cited (the session compiles
    // without positions, so by text alone), the rule is not.
    assert!(
        err.contains("\n  { age(bob) > 70 }.  fact\n  { age(bob) < 10 }.  fact"),
        "{err}"
    );
    assert!(!err.contains("eligible"), "{err}");
}

#[test]
fn load_reload_clear() {
    let family = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/family.claimr");
    let load = format!(":load {}\n", family.display());
    let (out, _, _) = run(
        &[],
        &format!(
            "{load}parent(zeus, athena).\n?- parent(zeus, C).\n\n:reload\n?- parent(zeus, C).\n:clear\n?- parent(tom, C).\n"
        ),
    );
    // Loading answers the file's queries (all answers, as batch does)...
    assert!(
        out.starts_with("?- parent(tom, X).\nX = bob\nX = liz\n"),
        "{out}"
    );
    // ...then the prompt-typed fact is visible, dropped by :reload (which
    // re-answers the file's queries), and :clear empties everything.
    assert!(out.contains("?- parent(zeus, C).\nC = athena.\n"), "{out}");
    assert_eq!(
        out.matches("?- parent(tom, X).\nX = bob\nX = liz\n")
            .count(),
        2,
        "{out}"
    );
    assert!(
        out.ends_with("?- parent(zeus, C).\nfalse.\n?- parent(tom, C).\nfalse.\n"),
        "{out}"
    );
}

#[test]
fn errors_at_the_prompt() {
    let (out, err, code) = run(
        &[],
        "?- p(X) q.\n\
         ?- { Y = X * Z }.\n\
         :bogus\n\
         :quit\n\
         ?- never().\n",
    );
    assert_eq!(code, 0);
    assert!(err.contains("error: 1:9: Expected"), "{err}");
    assert!(
        err.contains("in `?- { Y = X * Z }.`: non-linear constraint `X * Z`"),
        "{err}"
    );
    assert!(err.contains("unknown command `:bogus`"), "{err}");
    assert!(!out.contains("never"), "quit stops the loop: {out}");
}

#[test]
fn multiline_input_and_comments() {
    let (out, _, _) = run(
        &[],
        "% a comment line\n\
         mortal(X) :-\n    human(X).\n\
         human(socrates).\n\n\
         ?- mortal(W). % trailing comment\n",
    );
    assert_eq!(out, "?- mortal(W).\nW = socrates.\n");
}

#[test]
fn interactive_flag_runs_then_continues() {
    let family = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/family.claimr");
    let (out, _, code) = run(&["-i", family.to_str().unwrap()], "?- sibling(pat, S).\n");
    assert_eq!(code, 0);
    assert!(
        out.starts_with("?- parent(tom, X).\nX = bob\nX = liz\n"),
        "{out}"
    );
    assert!(out.ends_with("?- sibling(pat, S).\nS = ann.\n"), "{out}");
    // Batch mode is unchanged.
    let (batch, _, _) = run(&[family.to_str().unwrap()], "");
    let expected = std::fs::read_to_string(family.with_extension("answers")).unwrap();
    assert_eq!(batch, expected);
}

#[test]
fn exit_words_leave_the_loop() {
    for word in ["exit.", "quit.", "halt.", "exit", "quit"] {
        let (out, _, code) = run(&[], &format!("p(a).\n{word}\n?- p(X).\n"));
        assert_eq!(code, 0, "{word}");
        assert!(
            !out.contains("X = a"),
            "{word} should leave before the query: {out}"
        );
    }
}

#[test]
fn eof_and_help_exit_cleanly() {
    let (out, _, code) = run(&[], ":help\n");
    assert_eq!(code, 0);
    assert!(out.contains(":load FILE"));
    let (_, err, code) = run(&["--help"], "");
    assert_eq!(code, 2);
    assert!(err.contains("usage: claimr"));
}

#[test]
fn control_constructs_list_as_written_and_step() {
    // CLM-0022: the clause printer round-trips `;`, `->` and `\+`; a
    // disjunction's alternatives step with `;`; a runtime error from the
    // groundness rule is reported and the session continues.
    let (out, err, code) = run(
        &[],
        "q(1). q(2).\nv(V) :- (q(X) -> { V = X } ; { V = none }).\nm(F) :- \\+ q(F).\n:list\n?- q(X) ; { X = 3 }.\n;\n;\n?- v(V).\n?- m(F).\n?- m(3).\n",
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "q(1).\nq(2).\nv(V) :- q(X) -> { V = X } ; { V = none }.\nm(F) :- \\+ q(F).\n?- q(X) ; { X = 3 }.\nX = 1 ;\nX = 2 ;\nX = 3.\n?- v(V).\nV = 1.\n?- m(F).\n?- m(3).\ntrue.\n"
    );
    assert!(
        err.contains("condition `\\+ q(F)` reached with `F` unbound, in `m(F) :- \\+ q(F).`"),
        "{err}"
    );
}

#[test]
fn explain_toggles_the_proof_tree_under_answers() {
    // CLM-0023: `:explain` records proof trees; the session compiles without
    // positions, so a citation is the kind and, for a rule, its text. While
    // stepping, the tree follows the answer and the marker takes its own line.
    let (out, err, code) = run(
        &[],
        "q(1). q(2).\nr(X) :- q(X).\n:explain\n?- r(X).\n;\n:explain\n?- r(1).\n",
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "explaining answers\n?- r(X).\nX = 1\n  r(1)  rule  r(X) :- q(X).\n    q(1)  fact\n ;\nX = 2.\n  r(2)  rule  r(X) :- q(X).\n    q(2)  fact\nnot explaining answers\n?- r(1).\ntrue.\n"
    );
}

#[test]
fn a_directive_at_the_prompt_tables_a_predicate() {
    // CLM-0027: a directive extends the session like any clause, `:list`
    // prints it as written, and the tabled closure terminates on a cycle.
    let (out, err, code) = run(
        &[],
        ":- table anc/2.\npar(a, b). par(b, a).\nanc(X, Z) :- anc(X, Y), par(Y, Z).\nanc(X, Y) :- par(X, Y).\n:list\n?- anc(a, X).\n;\n;\n",
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        ":- table anc/2.\npar(a, b).\npar(b, a).\nanc(X, Z) :- anc(X, Y), par(Y, Z).\nanc(X, Y) :- par(X, Y).\n?- anc(a, X).\nX = b ;\nX = a.\n"
    );
}

/// CLM-0029: `--steps` seeds `:steps`; a query over budget is a runtime
/// error that keeps the session; the budget spans the answers stepped with
/// `;`; `:steps 0` lifts it.
#[test]
fn step_budget_at_the_prompt() {
    let (out, err, code) = run(
        &["--steps", "200"],
        "par(a, b).\npar(b, a).\n\
         anc(X, Z) :- anc(X, Y), par(Y, Z).\nanc(X, Y) :- par(X, Y).\n\
         ?- anc(a, X).\n\
         ?- par(a, X).\n\
         :steps 300\n\
         p(a).\np(X) :- loop(X).\nloop(X) :- loop(X).\n\
         ?- p(X).\n;\n\
         :steps 0\n\
         ?- par(b, X).\n",
    );
    assert_eq!(code, 0, "{err}");
    assert!(
        err.contains("in `?- anc(a, X).`: step limit of 200 reached at `anc(a, _1)`"),
        "{err}"
    );
    assert!(
        err.contains("in `?- p(X).`: step limit of 300 reached at `loop(X)`"),
        "{err}"
    );
    assert!(out.contains("X = b.\n"), "{out}");
    assert!(out.contains("X = a ;\n"), "{out}");
    assert!(out.ends_with("X = a.\n"), "{out}");
    // Without a budget the same session answers as before.
    let (out, err, _) = run(&[], "par(a, b).\n:steps x\n?- par(a, X).\n");
    assert!(err.contains("usage: :steps N"), "{err}");
    assert_eq!(out, "?- par(a, X).\nX = b.\n");
}

#[test]
fn explain_shows_why_a_query_failed() {
    // CLM-0031: under `:explain` a query with no answer prints its failure
    // tree after `false.`; the session compiles without positions, so a
    // citation is the kind alone.
    let (out, err, code) = run(
        &[],
        "q(1).\n:explain\n?- q(3).\n?- q(1), { 1 > 2 }.\n:explain\n?- q(3).\n",
    );
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "explaining answers\n\
         ?- q(3).\nfalse.\n  q(3)  failed: no clause (the clause cannot match the first argument)\n\
         ?- q(1), { 1 > 2 }.\nfalse.\n  q(1)  fact\n  { 1 > 2 }  failed: unsatisfiable with\n\
         not explaining answers\n?- q(3).\nfalse.\n"
    );
}
