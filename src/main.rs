//! `claimr` command-line entry point.
//!
//! Usage: `claimr <file.claimr>` — parses the file and prints the resulting
//! clauses, or reports where parsing failed. Evaluation is not implemented yet.

use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: claimr <file.claimr>");
        return ExitCode::from(2);
    };

    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("claimr: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };

    match claimr::parse_program(&source) {
        Ok(clauses) => {
            for clause in &clauses {
                println!("{clause:?}");
            }
            eprintln!("parsed {} clause(s) from {path}", clauses.len());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{path}:{e}");
            ExitCode::FAILURE
        }
    }
}
