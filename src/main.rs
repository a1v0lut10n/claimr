//! `claimr` command-line entry point.
//!
//! `claimr <file.claimr>` loads the program (parse, compile, initial store)
//! and runs its `?-` queries in order, printing each query and its answers.
//! `--parse` prints the parsed clauses instead; `--limit N` caps the answers
//! printed per query (unlimited by default). Diagnostics are GCC-style
//! `file:line:column: message`.

use std::{env, fs, process::ExitCode};

use claimr::{parse_program_spanned, Program};

const USAGE: &str = "usage: claimr [--parse] [--limit N] <file.claimr>";

struct Options {
    parse_only: bool,
    limit: Option<usize>,
    path: String,
}

fn parse_args() -> Result<Options, String> {
    let mut parse_only = false;
    let mut limit = None;
    let mut path = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--parse" => parse_only = true,
            "--limit" => {
                let n = args.next().ok_or("--limit needs a number")?;
                limit = Some(n.parse::<usize>().map_err(|_| format!("bad --limit value {n:?}"))?);
            }
            "-h" | "--help" => return Err(USAGE.to_string()),
            s if s.starts_with('-') => return Err(format!("unknown option {s}\n{USAGE}")),
            _ => {
                if path.replace(arg).is_some() {
                    return Err(USAGE.to_string());
                }
            }
        }
    }
    let path = path.ok_or(USAGE)?;
    Ok(Options { parse_only, limit, path })
}

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    let path = &opts.path;

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("claimr: cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };

    let clauses = match parse_program_spanned(&source) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{path}:{e}");
            return ExitCode::FAILURE;
        }
    };

    if opts.parse_only {
        for (clause, _) in &clauses {
            println!("{clause:?}");
        }
        eprintln!("parsed {} clause(s) from {path}", clauses.len());
        return ExitCode::SUCCESS;
    }

    let program = match Program::compile_spanned(&clauses) {
        Ok(p) => p,
        Err(e) => {
            match e.span() {
                Some(_) => eprintln!("{path}:{e}"),
                None => eprintln!("{path}: {e}"),
            }
            return ExitCode::FAILURE;
        }
    };

    for query in program.queries() {
        println!("{}", query.text());
        let mut count = 0usize;
        for answer in program.solve(query) {
            println!("{answer}");
            count += 1;
            if opts.limit.is_some_and(|l| count >= l) {
                break;
            }
        }
        if count == 0 {
            println!("false");
        }
    }
    ExitCode::SUCCESS
}
