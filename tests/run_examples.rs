//! Golden runs: every `examples/<name>.claimr` that has an
//! `examples/<name>.answers` sibling is executed through the `claimr` binary
//! and its output compared byte-for-byte. Also covers `--limit` and the CLI's
//! error paths.

use std::path::Path;
use std::process::Command;

fn claimr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_claimr"))
}

#[test]
fn golden_answers_match() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("examples dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("answers") {
            continue;
        }
        let program = path.with_extension("claimr");
        let expected = std::fs::read_to_string(&path).expect("read .answers");
        let out = claimr().arg(&program).output().expect("run claimr");
        assert!(
            out.status.success(),
            "{} failed: {}",
            program.display(),
            String::from_utf8_lossy(&out.stderr)
        );
        let actual = String::from_utf8(out.stdout).expect("utf-8");
        assert_eq!(actual, expected, "answers differ for {}", program.display());
        checked += 1;
    }
    assert!(checked >= 4, "expected golden files for the pure examples, found {checked}");
}

#[test]
fn limit_caps_an_infinite_query() {
    let dir = std::env::temp_dir().join(format!("claimr-limit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("nat.claimr");
    std::fs::write(&file, "nat(zero).\nnat(s(N)) :- nat(N).\n?- nat(X).\n").unwrap();
    let out = claimr().args(["--limit", "3"]).arg(&file).output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "?- nat(X).\nX = zero\nX = s(zero)\nX = s(s(zero))\n"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn errors_are_reported_and_exit_1() {
    let dir = std::env::temp_dir().join(format!("claimr-err-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.claimr");
    // Syntax error: positioned, exit 1.
    std::fs::write(&file, "ok(1)\n").unwrap();
    let out = claimr().arg(&file).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains(":2:1: Expected"));
    // Unsatisfiable initial store: load error, exit 1.
    std::fs::write(&file, "{ X > 3, X < 2 }.\n?- ok(1).\n").unwrap();
    let out = claimr().arg(&file).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("constraint facts are unsatisfiable"));
    // Non-linear residue at answer time: runtime error naming the query, exit 1.
    std::fs::write(&file, "?- { Y = X * Z }.\n").unwrap();
    let out = claimr().arg(&file).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("in `?- { Y = X * Z }.`: non-linear constraint"), "{err}");
    // Usage errors exit 2.
    let out = claimr().output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn parse_flag_dumps_the_ast() {
    let program = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/socrates.claimr");
    let out = claimr().arg("--parse").arg(&program).output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("Fact(Atom { name: \"human\""));
    assert!(String::from_utf8_lossy(&out.stderr).contains("parsed 7 clause(s)"));
}
