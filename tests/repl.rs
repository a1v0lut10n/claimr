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
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
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
    let (out, _, _) = run(&["--limit", "2"], "nat(zero).\nnat(s(N)) :- nat(N).\n:all\n?- nat(X).\n");
    assert_eq!(out, "printing all answers\n?- nat(X).\nX = zero\nX = s(zero)\n");
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
    assert!(out.starts_with("?- parent(tom, X).\nX = bob\nX = liz\n"), "{out}");
    // ...then the prompt-typed fact is visible, dropped by :reload (which
    // re-answers the file's queries), and :clear empties everything.
    assert!(out.contains("?- parent(zeus, C).\nC = athena.\n"), "{out}");
    assert_eq!(out.matches("?- parent(tom, X).\nX = bob\nX = liz\n").count(), 2, "{out}");
    assert!(out.ends_with("?- parent(zeus, C).\nfalse.\n?- parent(tom, C).\nfalse.\n"), "{out}");
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
    assert!(err.contains("in `?- { Y = X * Z }.`: non-linear constraint `X * Z`"), "{err}");
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
    assert!(out.starts_with("?- parent(tom, X).\nX = bob\nX = liz\n"), "{out}");
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
        assert!(!out.contains("X = a"), "{word} should leave before the query: {out}");
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
