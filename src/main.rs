// SPDX-License-Identifier: Apache-2.0

//! `claimr` command-line entry point.
//!
//! - `claimr <file.claimr>` loads the program (parse, compile, initial store)
//!   and runs its `?-` queries in order, printing each query and its answers.
//! - `claimr` with no file starts the interactive loop; `claimr -i <file>`
//!   runs the file, then continues interactively with its program.
//! - `--parse` prints the parsed clauses instead; `--limit N` caps the answers
//!   printed per query (unlimited by default; seeds the REPL's `:limit`).
//!
//! Diagnostics are GCC-style `file:line:column: message`.

mod repl;

use std::{env, fs, path::Path, process::ExitCode};

use claimr::{parse_program_spanned, Program};

const USAGE: &str = "\
usage: claimr [--parse] [--limit N] <file.claimr>   run a program
       claimr [--limit N]                            interactive loop
       claimr -i <file.claimr>                       run, then interactive";

struct Options {
    parse_only: bool,
    interactive: bool,
    limit: Option<usize>,
    path: Option<String>,
}

fn parse_args() -> Result<Options, String> {
    let mut parse_only = false;
    let mut interactive = false;
    let mut limit = None;
    let mut path = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--parse" => parse_only = true,
            "-i" | "--interactive" => interactive = true,
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
    if interactive && path.is_none() {
        return Err(format!("-i needs a file\n{USAGE}"));
    }
    if parse_only && path.is_none() {
        return Err(format!("--parse needs a file\n{USAGE}"));
    }
    Ok(Options { parse_only, interactive, limit, path })
}

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    // Interactive: no file, or -i file.
    if opts.path.is_none() || opts.interactive {
        let mut repl = match repl::Repl::new(opts.limit) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("claimr: {e}");
                return ExitCode::from(2);
            }
        };
        if let Some(path) = &opts.path {
            if !repl.load(Path::new(path)) {
                return ExitCode::FAILURE;
            }
        }
        repl.run();
        return ExitCode::SUCCESS;
    }

    let path = opts.path.as_deref().expect("batch mode has a path");
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
        let mut solutions = program.solve(query);
        for answer in solutions.by_ref() {
            println!("{answer}");
            count += 1;
            if opts.limit.is_some_and(|l| count >= l) {
                break;
            }
        }
        if let Some(e) = solutions.error() {
            eprintln!("{path}: in `{}`: {e}", query.text());
            return ExitCode::FAILURE;
        }
        if count == 0 {
            println!("false");
        }
    }
    ExitCode::SUCCESS
}
