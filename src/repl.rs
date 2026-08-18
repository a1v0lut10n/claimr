//! The interactive loop (design: `docs/design/2026-08-18-repl-interaction-model.md`).
//!
//! `claimr> ` prompt; input is exactly file syntax — a clause terminated by
//! `.` (multi-line), `?- …` being a query. Queries are answered Prolog-style —
//! first answer, then `;` for the next, Enter/`.`/`q` to stop; other clauses
//! typed at the prompt extend the session; `:` commands load,
//! list, clear and configure; Ctrl-C interrupts a running query. A pipe on
//! stdin drives the same loop without line editing (prompts suppressed, the
//! query echoed), which is how it is tested.

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use claimr::{parse_program_spanned, Clause, EvalError, ParseError, Program};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

/// Where a session clause came from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Origin {
    File(PathBuf),
    Prompt,
}

/// The session: the program being built, and how to read input.
pub struct Repl {
    clauses: Vec<(Clause, Origin)>,
    loaded: Vec<PathBuf>,
    program: Program,
    all_mode: bool,
    limit: Option<usize>,
    interrupt: Arc<AtomicBool>,
    input: Input,
    interactive: bool,
}

enum Input {
    Editor(Box<DefaultEditor>),
    Pipe(io::StdinLock<'static>),
}

/// What one round of reading produced.
enum Read {
    Line(String),
    Interrupted,
    Eof,
}

const HELP: &str = "\
Enter claimr syntax ending in `.`: `?- goals.` is answered, anything else
(fact, rule, `{ … }.`, `{ … } => head.`) is added to the session.
After an answer: `;` then Enter for the next, Enter or `.` to stop.
  :load FILE   append FILE's clauses to the session and answer its queries
  :reload      re-read the loaded files, dropping clauses typed at the prompt
  :list        print the session's clauses
  :clear       empty the session
  :limit N     cap answers per query in :all mode (0 = unlimited)
  :all         toggle between stepping answers and printing them all
  :help        this text
  :quit        leave (also `exit.`, `halt.`, Ctrl-D, or Ctrl-C twice at an empty prompt)
Ctrl-C interrupts a running query.";

impl Repl {
    /// A REPL with an empty session. `limit` seeds `:limit`.
    pub fn new(limit: Option<usize>) -> Result<Self, String> {
        let interactive = io::stdin().is_terminal();
        let input = if interactive {
            let editor = DefaultEditor::new().map_err(|e| format!("cannot start line editor: {e}"))?;
            Input::Editor(Box::new(editor))
        } else {
            Input::Pipe(io::stdin().lock())
        };
        let interrupt = Arc::new(AtomicBool::new(false));
        {
            let flag = interrupt.clone();
            // A second registration (tests spawn processes, not threads) is
            // not an error worth stopping for.
            let _ = ctrlc::set_handler(move || flag.store(true, Ordering::Relaxed));
        }
        Ok(Repl {
            clauses: Vec::new(),
            loaded: Vec::new(),
            program: Program::compile(&[]).expect("empty program compiles"),
            all_mode: false,
            limit,
            interrupt,
            input,
            interactive,
        })
    }

    /// Load a file into the session and answer its queries (as `-i` does).
    pub fn load(&mut self, path: &Path) -> bool {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("claimr: cannot read {}: {e}", path.display());
                return false;
            }
        };
        let clauses = match parse_program_spanned(&source) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("{}:{e}", path.display());
                return false;
            }
        };
        let saved = self.clauses.clone();
        let mut queries = Vec::new();
        for (clause, _) in clauses {
            if matches!(clause, Clause::Query(_)) {
                queries.push(clause);
            } else {
                self.clauses.push((clause, Origin::File(path.to_path_buf())));
            }
        }
        if let Err(e) = self.recompile() {
            eprintln!("{}: {e}", path.display());
            self.clauses = saved;
            self.recompile().expect("previous session compiled");
            return false;
        }
        if !self.loaded.contains(&path.to_path_buf()) {
            self.loaded.push(path.to_path_buf());
        }
        for q in queries {
            println!("{q}");
            self.answer(&q, true);
        }
        true
    }

    fn recompile(&mut self) -> Result<(), EvalError> {
        let clauses: Vec<Clause> = self.clauses.iter().map(|(c, _)| c.clone()).collect();
        self.program = Program::compile(&clauses)?;
        Ok(())
    }

    /// Run the loop until `:quit` or end of input.
    pub fn run(&mut self) {
        if self.interactive {
            println!("claimr {} — :help for commands; :quit, exit. or Ctrl-D to leave.", env!("CARGO_PKG_VERSION"));
        }
        let mut buffer = String::new();
        let mut interrupted_at_prompt = false;
        loop {
            let prompt = if buffer.is_empty() { "claimr> " } else { "    ... " };
            match self.read_line(prompt) {
                Read::Eof => {
                    if self.interactive {
                        println!();
                    }
                    return;
                }
                Read::Interrupted => {
                    self.interrupt.store(false, Ordering::Relaxed);
                    if !buffer.is_empty() {
                        buffer.clear();
                        println!("(input discarded)");
                        interrupted_at_prompt = false;
                    } else if interrupted_at_prompt {
                        // Second Ctrl-C in a row at an empty prompt: leave.
                        return;
                    } else {
                        println!("(Ctrl-C interrupts a running query; to leave: :quit, exit. or Ctrl-D — or Ctrl-C again)");
                        interrupted_at_prompt = true;
                    }
                    continue;
                }
                Read::Line(line) => {
                    interrupted_at_prompt = false;
                    if buffer.is_empty() {
                        let trimmed = line.trim();
                        if trimmed.is_empty() || trimmed.starts_with('%') {
                            continue;
                        }
                        if let Some(cmd) = trimmed.strip_prefix(':') {
                            if !self.command(cmd) {
                                return;
                            }
                            continue;
                        }
                        // The words people reach for to leave, with or without a `.`.
                        if matches!(trimmed.trim_end_matches('.').trim(), "exit" | "quit" | "halt") {
                            return;
                        }
                    }
                    buffer.push_str(&line);
                    buffer.push('\n');
                    match completeness(&buffer) {
                        Completeness::Incomplete => continue,
                        Completeness::Error(e) => {
                            eprintln!("error: {e}");
                            buffer.clear();
                        }
                        Completeness::Complete(clauses) => {
                            buffer.clear();
                            for clause in clauses {
                                self.handle_clause(clause);
                            }
                        }
                    }
                }
            }
        }
    }

    fn read_line(&mut self, prompt: &str) -> Read {
        match &mut self.input {
            Input::Editor(ed) => match ed.readline(prompt) {
                Ok(line) => {
                    let _ = ed.add_history_entry(line.as_str());
                    Read::Line(line)
                }
                Err(ReadlineError::Interrupted) => Read::Interrupted,
                Err(ReadlineError::Eof) => Read::Eof,
                Err(e) => {
                    eprintln!("input error: {e}");
                    Read::Eof
                }
            },
            Input::Pipe(stdin) => {
                let mut line = String::new();
                match stdin.read_line(&mut line) {
                    Ok(0) => Read::Eof,
                    Ok(_) => Read::Line(line.trim_end_matches(['\n', '\r']).to_string()),
                    Err(_) => Read::Eof,
                }
            }
        }
    }

    /// Dispatch a `:` command. Returns false to leave the loop.
    fn command(&mut self, cmd: &str) -> bool {
        let mut parts = cmd.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("");
        let arg = parts.next().map(str::trim).unwrap_or("");
        match name {
            "quit" | "q" | "exit" => return false,
            "help" | "h" | "?" => println!("{HELP}"),
            "load" | "l" => {
                if arg.is_empty() {
                    eprintln!("usage: :load FILE");
                } else {
                    self.load(Path::new(arg));
                }
            }
            "reload" | "r" => {
                let files = std::mem::take(&mut self.loaded);
                self.clauses.clear();
                self.recompile().expect("empty program compiles");
                for f in files {
                    self.load(&f);
                }
            }
            "list" => {
                let mut ordered: Vec<&(Clause, Origin)> = self.clauses.iter().collect();
                ordered.sort_by_key(|(_, o)| matches!(o, Origin::Prompt));
                for (c, _) in ordered {
                    println!("{c}");
                }
            }
            "clear" => {
                self.clauses.clear();
                self.loaded.clear();
                self.recompile().expect("empty program compiles");
            }
            "limit" => match arg.parse::<usize>() {
                Ok(0) => self.limit = None,
                Ok(n) => self.limit = Some(n),
                Err(_) => eprintln!("usage: :limit N  (0 = unlimited)"),
            },
            "all" => {
                self.all_mode = !self.all_mode;
                println!("{}", if self.all_mode { "printing all answers" } else { "stepping answers" });
            }
            _ => eprintln!("unknown command `:{name}` — :help for the list"),
        }
        true
    }

    /// A complete clause typed at the prompt: answer a query, extend the
    /// session with anything else.
    fn handle_clause(&mut self, clause: Clause) {
        if matches!(clause, Clause::Query(_)) {
            if !self.interactive {
                println!("{clause}");
            }
            let all = self.all_mode;
            self.answer(&clause, all);
            return;
        }
        self.clauses.push((clause, Origin::Prompt));
        if let Err(e) = self.recompile() {
            eprintln!("error: {e}");
            self.clauses.pop();
            self.recompile().expect("previous session compiled");
        }
    }

    /// Answer a query against the session; stepping unless `all`.
    fn answer(&mut self, query: &Clause, all: bool) {
        // Compile the session plus this query (programs are small).
        let mut clauses: Vec<Clause> = self.clauses.iter().map(|(c, _)| c.clone()).collect();
        clauses.push(query.clone());
        let program = match Program::compile(&clauses) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("error: {e}");
                return;
            }
        };
        let q = program.queries().last().expect("the query").clone();
        self.interrupt.store(false, Ordering::Relaxed);
        let mut sols = program.solve(&q).with_interrupt(self.interrupt.clone());
        let mut count = 0usize;
        loop {
            match sols.next() {
                Some(a) => {
                    count += 1;
                    if all {
                        println!("{a}");
                        if self.limit.is_some_and(|l| count >= l) {
                            break;
                        }
                        continue;
                    }
                    if !sols.may_continue() {
                        println!("{a}.");
                        break;
                    }
                    print!("{a}");
                    let _ = io::stdout().flush();
                    match self.read_line(" ") {
                        Read::Line(l) if matches!(l.trim(), ";" | "n" | "next") => {
                            println!(" ;");
                        }
                        Read::Interrupted => {
                            println!(".");
                            self.interrupt.store(false, Ordering::Relaxed);
                            break;
                        }
                        _ => {
                            println!(".");
                            break;
                        }
                    }
                }
                None => {
                    if sols.interrupted() {
                        println!("interrupted.");
                        self.interrupt.store(false, Ordering::Relaxed);
                    } else if let Some(e) = sols.error() {
                        eprintln!("in `{}`: {e}", q.text());
                    } else if count == 0 || !all {
                        println!("false.");
                    }
                    break;
                }
            }
        }
    }
}

/// Is the accumulated input a complete clause (or several), incomplete, or
/// wrong? Incompleteness is a syntax error at the very end of the input.
enum Completeness {
    Complete(Vec<Clause>),
    Incomplete,
    Error(ParseError),
}

fn completeness(buffer: &str) -> Completeness {
    match parse_program_spanned(buffer) {
        Ok(cs) => Completeness::Complete(cs.into_iter().map(|(c, _)| c).collect()),
        Err(e) => {
            let at_end = e.offset.is_some_and(|o| o >= buffer.trim_end().len());
            if at_end { Completeness::Incomplete } else { Completeness::Error(e) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completeness_detection() {
        assert!(matches!(completeness("human(socrates)."), Completeness::Complete(_)));
        assert!(matches!(completeness("mortal(X) :- human(X)"), Completeness::Incomplete));
        assert!(matches!(completeness("mortal(X) :-\n"), Completeness::Incomplete));
        assert!(matches!(completeness("?- p(X)"), Completeness::Incomplete));
        assert!(matches!(completeness("?- { X = 1.5 }."), Completeness::Complete(_)));
        assert!(matches!(completeness("?- { X = 1 }. % done"), Completeness::Complete(_)));
        assert!(matches!(completeness("?- p(X) q."), Completeness::Error(_)));
        assert!(matches!(completeness("{ age(x) > 3 }."), Completeness::Complete(_)));
        // `1.` at the end of a query is a number then the terminator.
        assert!(matches!(completeness("?- { X = 1 }."), Completeness::Complete(_)));
        assert!(matches!(completeness("?- p(1"), Completeness::Incomplete));
    }

    #[test]
    fn file_syntax_at_the_prompt() {
        let Completeness::Complete(cs) = completeness("?- p(X), q(X).") else { panic!() };
        assert!(matches!(cs[0], Clause::Query(_)));
        let Completeness::Complete(cs) = completeness("p(a).") else { panic!() };
        assert!(matches!(cs[0], Clause::Fact(_)));
        let Completeness::Complete(cs) = completeness("p(X) :- q(X).") else { panic!() };
        assert!(matches!(cs[0], Clause::Rule { .. }));
    }
}
