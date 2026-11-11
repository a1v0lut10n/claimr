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
            r#"{{"error":{{"file":"{path_str}","line":2,"column":1,"kind":"syntax","class":"syntax","message":"#
        )),
        "{stderr}"
    );
    // Unreadable file: exit 2, no line/column.
    let (_, stderr, code) = run(&["--json", "/no/such/file.claimr"]);
    assert_eq!(code, Some(2));
    assert!(
        stderr.contains(
            r#"{"error":{"file":"/no/such/file.claimr","kind":"read","class":"io","message":"cannot read"#
        ),
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
        stdout.starts_with(r#"{"query":"?- \\+ p(X), p(X).","outcome":"error","error":{"kind":"non-ground-condition","class":"program","construct":"\\+ p(X)","variable":"X","clause":"#),
        "{stdout}"
    );
    assert!(
        stdout.contains(r#","message":"condition `\\+ p(X)` reached with `X` unbound"#),
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

/// CLM-0025: a pending sum is a goal-form constraint string; a
/// comprehension explains as a `collected` node.
#[test]
fn sums_and_comprehensions_in_the_json_vocabulary() {
    let path = write_temp(
        "aggregation",
        "p(1). p(2).\n?- { L = <1> . T }, sum(L, S).\n?- { N = |<X | p(X)>| }.\n",
    );
    let (stdout, _, code) = run(&["--json", "--explain", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0));
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines[0].contains(r#""constraints":["sum(<1> . T, S)"]"#),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(r#"{"goal":"<X | p(X)> = <1, 2>","by":"collected","subgoals":[]}"#),
        "{}",
        lines[1]
    );
}

/// CLM-0027: a tabled call explains as a `tabled` node whose subgoals are
/// the recorded derivation; a tabling error is a query error document.
#[test]
fn tabled_calls_in_the_json_vocabulary() {
    let path = write_temp(
        "tabling",
        ":- table r/2.\ne(a, b). e(b, a).\nr(X, Y) :- e(X, Y).\nr(X, Z) :- e(X, Y), r(Y, Z).\n?- r(a, a).\n",
    );
    let (stdout, _, code) = run(&["--json", "--explain", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0));
    assert!(
        stdout.contains(r#"{"goal":"r(a, a)","by":"tabled","subgoals":[{"goal":"r(a, a)","by":"rule","line":4,"column":1,"clause":"r(X, Z) :- e(X, Y), r(Y, Z).","subgoals":["#),
        "{stdout}"
    );
    let path = write_temp(
        "tabling-error",
        ":- table n/1.\nn(X) :- { X > 0 }.\n?- n(X).\n",
    );
    let (stdout, _, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(1));
    assert!(
        stdout.starts_with(r#"{"query":"?- n(X).","outcome":"error","error":{"kind":"non-ground-tabled-answer","class":"program","predicate":"n/1","instance":"n(_1)","message":"a solution of the tabled predicate `n/1` is `n(_1)`"#),
        "{stdout}"
    );
}

/// CLM-0029: the step limit is the existing query error document — no new
/// output shape; the message opens with a fixed phrase.
#[test]
fn a_step_limit_is_a_query_error_document() {
    let path = write_temp(
        "steps",
        "par(a, b). par(b, a).\nanc(X, Z) :- anc(X, Y), par(Y, Z).\nanc(X, Y) :- par(X, Y).\n?- par(a, X).\n?- anc(a, X).\n",
    );
    let (stdout, _, code) = run(&["--json", "--steps", "1000", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(1));
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "{stdout}");
    assert_eq!(
        lines[0],
        r#"{"query":"?- par(a, X).","outcome":"solutions","answers":[{"equations":["X = b"],"disequations":[],"constraints":[]}]}"#
    );
    assert!(
        lines[1].starts_with(r#"{"query":"?- anc(a, X).","outcome":"error","error":{"kind":"step-limit","class":"resource","steps":1000,"goal":"`anc(a, _1)`","message":"step limit of 1000 reached at `anc(a, _1)`;"#),
        "{}",
        lines[1]
    );
}

/// CLM-0031: under `--explain`, an outcome-none document carries its
/// `failure` tree; without the flag the document is unchanged.
#[test]
fn a_failed_query_explains_itself_in_json() {
    let src = "k(c17, 5).\n?- k(c17, K), { K > 7 }.\n?- k(c99, K).\n";
    let path = write_temp("failure", src);
    let (stdout, _, code) = run(&["--json", "--explain", path.to_str().unwrap()]);
    assert_eq!(code, Some(0));
    assert_eq!(
        stdout,
        concat!(
            "{\"query\":\"?- k(c17, K), { K > 7 }.\",\"outcome\":\"none\",\"answers\":[],",
            "\"failure\":{\"branches\":[[",
            "{\"goal\":\"k(c17, 5)\",\"by\":\"fact\",\"line\":1,\"column\":1,\"clause\":\"k(c17, 5).\",\"subgoals\":[]},",
            "{\"goal\":\"{ 5 > 7 }\",\"by\":\"failed\",\"reason\":\"unsatisfiable\",\"conflict\":[",
            "{\"goal\":\"k(c17, 5)\",\"by\":\"fact\",\"line\":1,\"column\":1,\"clause\":\"k(c17, 5).\",\"subgoals\":[]}",
            "],\"subgoals\":[]}]],\"omitted\":0}}\n",
            "{\"query\":\"?- k(c99, K).\",\"outcome\":\"none\",\"answers\":[],",
            "\"failure\":{\"branches\":[[",
            "{\"goal\":\"k(c99, K)\",\"by\":\"failed\",\"reason\":\"no-clause\",\"clauses\":1,\"candidates\":0,\"subgoals\":[]}",
            "]],\"omitted\":0}}\n",
        )
    );
    let (stdout, _, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(0));
    assert_eq!(
        stdout,
        "{\"query\":\"?- k(c17, K), { K > 7 }.\",\"outcome\":\"none\",\"answers\":[]}\n\
         {\"query\":\"?- k(c99, K).\",\"outcome\":\"none\",\"answers\":[]}\n"
    );
}

/// CLM-0033: an error object carries `kind`, `class` and the error's data
/// beside its unchanged `message`, in both documents; the message is the
/// text human mode prints.
#[test]
fn error_objects_carry_kind_class_and_data() {
    // Unsatisfiable facts: a file-level failure with the cited clauses.
    let src = "{ age(bob) >= 18 }.\n{ price(x) > 0 }.\n{ age(bob) < 10 }.\np(1).\n";
    let path = write_temp("kind-facts", src);
    let (stdout, stderr, code) = run(&["--json", path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(1));
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.contains(
            r#""kind":"unsatisfiable-facts","class":"program","conflict":[{"line":1,"column":1,"clause":"{ age(bob) >= 18 }."},{"line":3,"column":1,"clause":"{ age(bob) < 10 }."}],"message":"the program's constraint facts are unsatisfiable (no models); a minimal conflicting set:\n"#
        ),
        "{stderr}"
    );
    // A non-linear residue: unsupported, with the constraint as data; the
    // message is byte for byte the human-mode diagnostic.
    let src = "?- { Y = X * Z }.\n";
    let path = write_temp("kind-nonlinear", src);
    let (stdout, _, code) = run(&["--json", path.to_str().unwrap()]);
    let (_, human, _) = run(&[path.to_str().unwrap()]);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(1));
    let line = stdout.lines().next().unwrap_or_default();
    assert!(
        line.starts_with(r#"{"query":"?- { Y = X * Z }.","outcome":"error","error":{"kind":"non-linear","class":"unsupported","constraint":"X * Z","message":""#),
        "{line}"
    );
    let message = line
        .split(r#""message":""#)
        .nth(1)
        .and_then(|rest| rest.strip_suffix("\"}}"))
        .expect("a message field closes the document");
    let human_message = human
        .trim_end()
        .split(": in `?- { Y = X * Z }.`: ")
        .nth(1)
        .expect("the human diagnostic names the query");
    assert_eq!(message, human_message);
}
