---
date: 2026-08-18
type: design
status: accepted
components: [cli, evaluator]
aspects: [answer-soundness]
tags: [repl, cli, ux]
---

# The REPL: Prolog-style stepping, GHCi-style commands, clauses at the prompt

## Context

The evaluator record (D3, D7) runs a file's `?-` queries at load time and
prints all answers eagerly, leaving interactive use to "a REPL as a
follow-up decision". With all four evaluator stages done, an interactive
loop is what makes claimr usable for exploration: ask a query, look at one
answer, ask for the next, add a fact, ask again. The choices to make are the
user-facing contract — how answers are stepped, how programs are loaded and
extended, how the loop is controlled — because they are hard to change once
people have habits, and because they must respect `answer-soundness`
(stepping must never show an answer the batch mode would not).

The two natural models: Prolog's (a query at `?- `, one answer, `;` for
the next, `.` to stop; programs are consulted from files; nothing else at
the prompt), and a language-shell model (GHCi, the Rust/Python REPLs:
definitions typed at the prompt become part of the session, `:commands`
control the environment). Claimr programs are just clause lists with the
world (constraint facts) and queries interleaved, so a session *is* a
program being edited incrementally — the shell model fits it as well as
Prolog's stepping fits answers.

## Decision

- **Entry points.** `claimr` with no file starts the REPL; `claimr file`
  runs the file as today (batch); `claimr -i file` (or `--interactive`)
  runs the file — its clauses loaded, its queries answered as in batch —
  and then drops into the REPL with the file's program as the session.
- **Prompt and input.** The prompt is `claimr> ` (continuation `    ... `).
  Input is **exactly claimr file syntax**: one clause terminated by `.`,
  possibly spanning lines; `?- …` is a query, anything else is a clause.
  The `?-` is *not* optional: a bare `p(a).` is a valid fact *and* a valid
  query, and a prompt that reads bare goals as queries (Prolog's) cannot
  also accept definitions — Prolog's toplevel takes no definitions at all.
  Consistency with files wins over three keystrokes. Comments and blank
  lines are ignored.
- **Answers are stepped Prolog-style.** The first answer is printed and, if
  the search could continue, the loop waits: `;` (or `n`) followed by Enter
  asks for the next answer; a bare Enter, `.` or `q` stops the query — the
  same keys as Prolog (read as a line, not a single keypress). When the search is exhausted after an answer,
  nothing more is printed; when a query has no (further) answer, `false.` is
  printed. Answers themselves are exactly the batch answers (`Solutions`
  is shared; nothing is recomputed differently), followed by ` ;` when
  waiting and `.` when final — Prolog's typography.
- **Clauses at the prompt extend the session.** A fact, rule,
  implication or constraint fact typed at the prompt is appended to the
  session program (the store's world is rebuilt: constraint facts must
  remain jointly satisfiable, else the clause is rejected with the
  diagnostic and not added). Later queries see it. This is the shell
  model: the session is the program.
- **Commands** are lines starting with `:` (GHCi-style; not claimr syntax,
  so no grammar change): `:load file` (append the file's clauses to the
  session and answer its queries, like `-i`), `:reload` (re-read every
  loaded file, dropping prompt-typed clauses), `:list` (print the session's
  clauses in canonical syntax, prompt-typed ones last), `:clear` (empty
  session), `:limit N` (cap answers printed per query in `:all` mode),
  `:all` toggles stepping vs. printing all answers, `:help`, `:quit` (also
  Ctrl-D at the prompt).
- **Interruption and leaving.** Ctrl-C during a query aborts it back to
  the prompt (`interrupted.`); Ctrl-C at the prompt discards a partial
  input line, and at an *empty* prompt prints how to leave — a second
  Ctrl-C in a row then leaves (Ctrl-C is not an exit by itself, and ⌘C on
  macOS is copy). Leaving: `:quit`, Ctrl-D, or the words people reach for
  — `exit`, `quit`, `halt`, with or without a `.`. The machine polls a
  shared flag between resolution steps.
- **Diagnostics** keep the batch conventions: syntax errors show
  `line:col:` relative to the typed input; runtime errors print as in batch
  (`in \`?- …\`: …`); loading a file reports `file:line:col:`.
- **Non-interactive stdin** (a pipe) drives the same loop without line
  editing: it is how the REPL is tested, and it lets `echo '?- p(X).' |
  claimr` work as a filter — with the stepping keys read from stdin too.
- **Implementation.** `src/repl.rs` in the binary crate; line editing with
  `rustyline` (pure Rust, history in memory for the session), Ctrl-C via
  `ctrlc` setting an `AtomicBool` the evaluator exposes as an optional
  interrupt hook on `Solutions`. No new grammar; the parser's
  `parse_program_spanned` parses prompt input.

Alternatives considered: pure Prolog model (no clauses at the prompt,
`consult/1`-style loading) — rejected because claimr has no directive
syntax and its programs are naturally incremental; printing all answers by
default in the REPL — rejected (infinite answer sets, and stepping is the
expected interactive experience); a `!`-prefixed or `\`-prefixed command
syntax — `:` chosen for familiarity and because `:` cannot start a claimr
clause.

## Consequences

- The evaluator gains an interrupt hook (`Solutions::with_interrupt(Arc<
  AtomicBool>)`) polled per step — a small, contained change to
  `machine.rs`; batch mode is unaffected.
- The CLI grows two crates (`rustyline`, `ctrlc`) and a `repl.rs`; the
  binary's usage line changes; `README.md` and `CLAUDE.md` describe the
  REPL; the `cli` vocabulary row is updated.
- Prompt-typed clauses make the session a program that is *not* in any
  file: `:list` is the way to see it; persisting a session to a file is a
  possible later command (`:save`), not part of this decision.
- Tests drive the loop through a pipe, so every stepping/loading/command
  behaviour is covered by integration tests without a TTY.
