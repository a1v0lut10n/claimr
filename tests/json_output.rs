// SPDX-License-Identifier: Apache-2.0

//! CLM-0010: the machine-readable output contract, pinned end to end
//! through the real binary. Consumers (aicogito's reasoning adapter
//! among them) dispatch on these exact shapes; a change here is a
//! consumer break and must be deliberate.

use std::process::Command;

fn run(args: &[&str]) -> (String, String, Option<i32>) {
    let out = Command::new(env!("CARGO_BIN_EXE_claimr"))
        .args(args)
        .output()
        .expect("claimr runs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code(),
    )
}

fn write_temp(name: &str, content: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("clm0010-{name}-{}.claimr", std::process::id()));
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn one_document_per_query_with_the_documented_shapes() {
    let path = write_temp(
        "shapes",
        "slot(s1, expression, supported).\n\
         slot(s1, sense, assumed).\n\
         ok(S) :- slot(S, expression, supported).\n\
         { age(alice) = 30 }.\n\
         ?- ok(s1).\n\
         ?- ok(s2).\n\
         ?- slot(s1, R, B).\n\
         ?- { age(alice) > 25 }.\n",
    );
    let (stdout, stderr, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0), "stderr: {stderr}");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 4, "{stdout}");
    // Proved without bindings: one answer, three empty lists.
    assert_eq!(
        lines[0],
        r#"{"query":"?- ok(s1).","outcome":"solutions","answers":[{"equations":[],"disequations":[],"constraints":[]}]}"#
    );
    // No solution.
    assert_eq!(
        lines[1],
        r#"{"query":"?- ok(s2).","outcome":"none","answers":[]}"#
    );
    // Bindings, one equation per variable, one object per answer.
    assert_eq!(
        lines[2],
        r#"{"query":"?- slot(s1, R, B).","outcome":"solutions","answers":[{"equations":["R = expression","B = supported"],"disequations":[],"constraints":[]},{"equations":["R = sense","B = assumed"],"disequations":[],"constraints":[]}]}"#
    );
    // Constraint answers in solved form.
    assert_eq!(
        lines[3],
        r#"{"query":"?- { age(alice) > 25 }.","outcome":"solutions","answers":[{"equations":[],"disequations":[],"constraints":["age(alice) = 30"]}]}"#
    );
}

#[test]
fn length_residuals_are_constraint_strings() {
    // CLM-0015: a residual on a list length is one more constraint string;
    // the document shape is unchanged.
    let path = write_temp("lengths", "?- { |L| > 2, L = <a> . T }.\n");
    let (stdout, stderr, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert_eq!(
        stdout.trim_end(),
        r#"{"query":"?- { |L| > 2, L = <a> . T }.","outcome":"solutions","answers":[{"equations":["L = <a> . T"],"disequations":[],"constraints":["|T| > 1"]}]}"#
    );
}

#[test]
fn pending_concatenations_are_constraint_strings() {
    // CLM-0016: a pending concatenation is one more constraint string.
    let path = write_temp("concat", "?- { Z = X . Y, X = <a> . T }.\n");
    let (stdout, stderr, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert_eq!(
        stdout.trim_end(),
        r#"{"query":"?- { Z = X . Y, X = <a> . T }.","outcome":"solutions","answers":[{"equations":["X = <a> . T"],"disequations":[],"constraints":["|T| = -|Y| + |Z| - 1","Z = <a> . T . Y"]}]}"#
    );
}

#[test]
fn goal_form_residuals_are_constraint_strings() {
    // CLM-0017: pending type relations and atom_chars print in goal form.
    let path = write_temp("goals", "?- atom(X), atom_chars(A, <a> . T).\n");
    let (stdout, stderr, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0), "stderr: {stderr}");
    assert_eq!(
        stdout.trim_end(),
        r#"{"query":"?- atom(X), atom_chars(A, <a> . T).","outcome":"solutions","answers":[{"equations":[],"disequations":[],"constraints":["atom(X)","atom_chars(A, <a> . T)"]}]}"#
    );
}

#[test]
fn diagnostics_are_structured_and_exit_codes_hold() {
    // Parse error: structured on stderr, exit 1, stdout silent.
    let path = write_temp("broken", "broken(\n");
    let (stdout, stderr, code) = run(&["--json", path.to_str().unwrap()]);
    let path_str = path.to_str().unwrap().to_string();
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(1));
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.starts_with(&format!(
            r#"{{"error":{{"file":"{path_str}","line":2,"column":1,"#
        )),
        "{stderr}"
    );
    // Unreadable file: exit 2, no line/column.
    let (_, stderr, code) = run(&["--json", "/no/such/file.claimr"]);
    assert_eq!(code, Some(2));
    assert!(
        stderr.contains(r#"{"error":{"file":"/no/such/file.claimr","message":"cannot read"#),
        "{stderr}"
    );
    // --json guards: REPL and --parse forms refuse with usage.
    let (_, stderr, code) = run(&["--json"]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains("--json needs a file"), "{stderr}");
    let path = write_temp("guard", "a.\n");
    let (_, stderr, code) = run(&["--json", "--parse", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(2));
    assert!(stderr.contains("don't combine"), "{stderr}");
}

/// The human batch format is byte-identical to the pre-CLM-0010
/// output — the JSON mode is an addition, never a change.
#[test]
fn the_human_format_is_untouched() {
    let path = write_temp("human", "ok(a).\n?- ok(a).\n?- ok(b).\n");
    let (stdout, _, code) = run(&[path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "?- ok(a).\ntrue\n?- ok(b).\nfalse\n");
}

/// CLM-0022: the groundness rule's runtime error is the existing query
/// error document — no new output shape.
#[test]
fn a_non_ground_condition_is_a_query_error_document() {
    let path = write_temp("control", "p(a).\n?- \\+ p(X), p(X).\n");
    let (stdout, _, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(1));
    assert!(
        stdout.starts_with(r#"{"query":"?- \\+ p(X), p(X).","outcome":"error","error":{"message":"condition `\\+ p(X)` reached with `X` unbound"#),
        "{stdout}"
    );
}

/// CLM-0023: `--explain` adds an `explanation` field to each answer object;
/// without it the document is unchanged.
#[test]
fn explain_adds_the_explanation_field_only_when_asked() {
    let src = "k(c17, 5).\nviolation(C) :- k(C, K), { K > 3 }.\n?- violation(C).\n";
    let path = write_temp("explain", src);
    let (stdout, _, code) = run(&["--json", "--explain", path.to_str().unwrap()]);
    assert_eq!(code, Some(0));
    assert_eq!(
        stdout,
        "{\"query\":\"?- violation(C).\",\"outcome\":\"solutions\",\"answers\":[{\"equations\":[\"C = c17\"],\"disequations\":[],\"constraints\":[],\"explanation\":[{\"goal\":\"violation(c17)\",\"by\":\"rule\",\"line\":2,\"column\":1,\"clause\":\"violation(C) :- k(C, K), { K > 3 }.\",\"subgoals\":[{\"goal\":\"k(c17, 5)\",\"by\":\"fact\",\"line\":1,\"column\":1,\"clause\":\"k(c17, 5).\",\"subgoals\":[]},{\"goal\":\"{ 5 > 3 }\",\"by\":\"constraint\",\"line\":2,\"column\":1,\"subgoals\":[]}]}]}]}\n"
    );
    let (stdout, _, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0));
    assert!(!stdout.contains("explanation"), "{stdout}");
    // A deep derivation serialises without recursion.
    let mut deep = String::new();
    for i in 0..5000 {
        deep.push_str(&format!("next(a{i}, a{}).\n", i + 1));
    }
    deep.push_str("reach(X, X).\nreach(X, Z) :- next(X, Y), reach(Y, Z).\n?- reach(a0, a5000).\n");
    let path = write_temp("explain-deep", &deep);
    let (stdout, _, code) = run(&["--json", "--explain", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0));
    assert!(
        stdout.contains("\"goal\":\"reach(a5000, a5000)\",\"by\":\"fact\""),
        "deepest leaf present"
    );
    assert_eq!(stdout.matches("\"by\":\"rule\"").count(), 5000);
}
