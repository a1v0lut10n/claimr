// SPDX-License-Identifier: Apache-2.0

//! Benchmarks (CLM-0034): measuring instruments, not gates. Ignored by
//! default; run them against a release build and read the timings:
//!
//! ```text
//! cargo test --release --test benchmarks -- --ignored --nocapture
//! ```
//!
//! Each test asserts only what the program must answer (the solution
//! counts) and prints how long the binary took. The numbers recorded per
//! version live in `docs/reference/benchmarks.md`; a dependency-free
//! `Instant` around the process is all the precision they need.

use std::process::Command;
use std::time::Instant;

/// The N-queens program of `examples/queens.claimr` with its own queries
/// stripped and one query for `n` appended.
fn queens_program(n: usize) -> String {
    let example = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/queens.claimr"),
    )
    .expect("the queens example exists");
    let mut program: String = example
        .lines()
        .filter(|l| !l.starts_with("?-"))
        .map(|l| format!("{l}\n"))
        .collect();
    program.push_str(&format!("?- queens({n}, Qs).\n"));
    program
}

/// Run the binary on `program` with `args`; the answer lines and the wall time.
fn run(program: &str, args: &[&str]) -> (Vec<String>, std::time::Duration) {
    let dir = std::env::temp_dir().join(format!("claimr-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let file = dir.join("queens.claimr");
    std::fs::write(&file, program).expect("write program");
    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_claimr"))
        .args(args)
        .arg(&file)
        .output()
        .expect("claimr runs");
    let elapsed = started.elapsed();
    std::fs::remove_file(&file).ok();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let answers = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("Qs = "))
        .map(str::to_string)
        .collect();
    (answers, elapsed)
}

#[test]
#[ignore = "a benchmark: run with --release --ignored --nocapture"]
fn queens_first_solution() {
    for n in [8, 10, 12] {
        let (answers, elapsed) = run(&queens_program(n), &["--limit", "1"]);
        assert_eq!(answers.len(), 1, "queens({n}) has a solution");
        println!("queens({n}) first solution: {} ms", elapsed.as_millis());
    }
}

#[test]
#[ignore = "a benchmark: run with --release --ignored --nocapture"]
fn queens_all_solutions_of_eight() {
    let (answers, elapsed) = run(&queens_program(8), &[]);
    assert_eq!(answers.len(), 92, "8 queens has 92 solutions");
    println!("queens(8) all 92 solutions: {} ms", elapsed.as_millis());
}
