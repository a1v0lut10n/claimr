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
    assert_eq!(lines[1], r#"{"query":"?- ok(s2).","outcome":"none","answers":[]}"#);
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
fn diagnostics_are_structured_and_exit_codes_hold() {
    // Parse error: structured on stderr, exit 1, stdout silent.
    let path = write_temp("broken", "broken(\n");
    let (stdout, stderr, code) = run(&["--json", path.to_str().unwrap()]);
    let path_str = path.to_str().unwrap().to_string();
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(1));
    assert!(stdout.is_empty(), "{stdout}");
    assert!(
        stderr.starts_with(&format!(r#"{{"error":{{"file":"{path_str}","line":2,"column":1,"#)),
        "{stderr}"
    );
    // Unreadable file: exit 2, no line/column.
    let (_, stderr, code) = run(&["--json", "/no/such/file.claimr"]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains(r#"{"error":{"file":"/no/such/file.claimr","message":"cannot read"#), "{stderr}");
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
