// SPDX-License-Identifier: Apache-2.0

//! `claimr` command-line entry point.
//!
//! - `claimr <file.claimr>` loads the program (parse, compile, initial store)
//!   and runs its `?-` queries in order, printing each query and its answers.
//! - `claimr` with no file starts the interactive loop; `claimr -i <file>`
//!   runs the file, then continues interactively with its program.
//! - `--parse` prints the parsed clauses instead; `--limit N` caps the answers
//!   printed per query (unlimited by default; seeds the REPL's `:limit`);
//!   `--steps N` (CLM-0029) is a budget of resolution steps per query, reached
//!   as a runtime error (unlimited by default; seeds the REPL's `:steps`).
//! - `--json` (CLM-0010, batch only) emits one JSON document per query on
//!   stdout and structured diagnostics on stderr — the machine-readable
//!   contract in `docs/reference/json-output.md`; the human format above is
//!   untouched.
//! - `--explain` (CLM-0023) prints each answer's proof tree under it (and
//!   adds the `explanation` field to the JSON answer objects).
//!
//! Diagnostics are GCC-style `file:line:column: message`.

mod json_out;
mod repl;

use std::{env, fs, path::Path, process::ExitCode};

use claimr::{Program, parse_program_spanned};

const USAGE: &str = "\
usage: claimr [--parse] [--limit N] [--steps N] [--explain] <file.claimr>   run a program
       claimr --json [--limit N] [--steps N] [--explain] <file.claimr>      run a program, JSON per query
       claimr [--limit N] [--steps N]                                        interactive loop
       claimr -i <file.claimr>                                   run, then interactive
--explain prints each answer's proof tree (the clauses and constraints that derived it).
--steps N stops a query that takes more than N resolution steps with an error (unlimited by default).";

struct Options {
    parse_only: bool,
    interactive: bool,
    json: bool,
    explain: bool,
    limit: Option<usize>,
    /// CLM-0029: a budget of resolution steps per query.
    steps: Option<u64>,
    path: Option<String>,
}

fn parse_args() -> Result<Options, String> {
    let mut parse_only = false;
    let mut interactive = false;
    let mut json = false;
    let mut explain = false;
    let mut limit = None;
    let mut steps = None;
    let mut path = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--parse" => parse_only = true,
            "--json" => json = true,
            "--explain" => explain = true,
            "-i" | "--interactive" => interactive = true,
            "--limit" => {
                let n = args.next().ok_or("--limit needs a number")?;
                limit = Some(
                    n.parse::<usize>()
                        .map_err(|_| format!("bad --limit value {n:?}"))?,
                );
            }
            "--steps" => {
                let n = args.next().ok_or("--steps needs a number")?;
                steps = Some(
                    n.parse::<u64>()
                        .map_err(|_| format!("bad --steps value {n:?}"))?,
                );
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
    // CLM-0010: --json is the batch contract — it has no REPL or
    // parse-dump form.
    if json && (path.is_none() || interactive) {
        return Err(format!("--json needs a file (batch mode)\n{USAGE}"));
    }
    if json && parse_only {
        return Err(format!("--json and --parse don't combine\n{USAGE}"));
    }
    Ok(Options {
        parse_only,
        interactive,
        json,
        explain,
        limit,
        steps,
        path,
    })
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
        let mut repl = match repl::Repl::new(opts.limit, opts.steps) {
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
            if opts.json {
                eprintln!(
                    "{}",
                    json_out::error_document(path, None, None, &format!("cannot read: {e}"))
                );
            } else {
                eprintln!("claimr: cannot read {path}: {e}");
            }
            return ExitCode::from(2);
        }
    };

    let clauses = match parse_program_spanned(&source) {
        Ok(c) => c,
        Err(e) => {
            if opts.json {
                eprintln!(
                    "{}",
                    json_out::error_document(path, e.line, e.column, &e.message)
                );
            } else {
                eprintln!("{path}:{e}");
            }
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
            if opts.json {
                let span = e.span();
                eprintln!(
                    "{}",
                    json_out::error_document(
                        path,
                        span.map(|s| s.line),
                        span.map(|s| s.column),
                        &e.to_string(),
                    )
                );
            } else {
                match e.span() {
                    Some(s) => eprintln!("{path}:{}:{}: {e}", s.line, s.column),
                    None => eprintln!("{path}: {e}"),
                }
            }
            return ExitCode::FAILURE;
        }
    };

    if opts.json {
        // CLM-0010: one document per query; a runtime failure is that
        // query's document, then the run stops (as in human mode).
        for query in program.queries() {
            let mut answers = Vec::new();
            let mut solutions = program.solve(query);
            if let Some(n) = opts.steps {
                solutions = solutions.with_step_limit(n);
            }
            if opts.explain {
                solutions = solutions.with_explanations();
            }
            for answer in solutions.by_ref() {
                answers.push(answer);
                if opts.limit.is_some_and(|l| answers.len() >= l) {
                    break;
                }
            }
            if let Some(e) = solutions.error() {
                println!(
                    "{}",
                    json_out::query_error_document(query.text(), &e.to_string())
                );
                return ExitCode::FAILURE;
            }
            println!("{}", json_out::query_document(query.text(), &answers));
        }
        return ExitCode::SUCCESS;
    }

    for query in program.queries() {
        println!("{}", query.text());
        let mut count = 0usize;
        let mut solutions = program.solve(query);
        if let Some(n) = opts.steps {
            solutions = solutions.with_step_limit(n);
        }
        if opts.explain {
            solutions = solutions.with_explanations();
        }
        for answer in solutions.by_ref() {
            println!("{answer}");
            for line in answer.explanation_lines() {
                println!("{line}");
            }
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
