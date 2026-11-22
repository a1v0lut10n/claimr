---
date: 2026-11-22
type: journal
components: [evaluator, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-22-undefined-predicates-are-reported.md
meta-impact:
  note: "Accepted: `claimr --json` will emit, before any query runs, one stderr document per call to a predicate no clause defines — `{\"warning\": {\"file\", \"line\"?, \"column\"?, \"kind\": \"undefined-predicate\", \"predicate\": \"nosuch/1\", \"clause\": \"…\", \"message\"}}`, the error document's shape with `warning` for `error` and no `class` — with stdout and the exit code unchanged. A consumer that reads stderr only on a non-zero exit (aicogito's reasoning adapter today) sees no change; one that wants the warnings reads them. The library gains `Program::undefined()`, which genite (GEN-141) can treat as an error. The suggestion stands: add claimr-json-output and claimr-token-api to the contract vocabulary."
---

# Design record accepted: undefined predicates are reported

- **When:** 2026-11-22 01:12:07

## Context

The first program to link claimr as a library, genite's GEN-141, wrote
`dif(V, W)` where claimr writes `{ V != W }`; the rule found nothing and
nothing said why. The incubation note of 2026-11-20 named the hazard,
and checking it showed the worst form: a clause calling an undefined
predicate fails quietly beside one that succeeds, and `\+ nosuch(X)`
succeeds vacuously, so an integrity check passes because the predicate
that would find a counter-example is misspelt. Of the three notes from
that embedding, this one was taken first because it is a correctness
hazard where the other two are maintenance and packaging. The record
landed as PR #93, graduating the note by rename, and is accepted here.

## Details

**Decision.** The semantics do not move: a call to a predicate with no
clause fails, as the evaluator record decided and invariant 3 of
`answer-soundness` states. What changes is the silence. The compile
step, which already sees every call with its clause and position,
reports every call site whose predicate has no clause and is not a
built-in — a `:- table` declaration is not a definition — in bodies,
conditions, negations, comprehension bodies and queries alike.
`Program::undefined()` returns them (`UndefinedCall { name, arity, span,
clause }`) for the embedding program to judge. Batch mode warns once per
load on stderr, `file:line:col: warning: nosuch/1 is called here and no
clause defines it`, one line per call site, exit code and stdout
unchanged; under `--json` each warning is a stderr document shaped like
the error document, without `class`. The REPL does not warn on a clause
typed at the prompt, since the next line may define the predicate; it
warns on `:load`, and before answering a query for the undefined
predicates the query can reach through the call graph. `--explain` keeps
naming the predicate at its `no-clause` leaf when a query fails through
it.

**Why a warning and a list, not an error.** ISO Prolog's default is an
existence error and Mercury refuses to compile; claimr keeps the
logic-program reading — a clause set is a program and a goal nothing
defines has no derivation — because a program loaded in parts, or a
session still being typed, legitimately calls what it has not yet
defined. The static report says what the author needs to hear once, at
load, and the embedding program decides what it means: genite will treat
a non-empty list as an error. Rejected: an existence error at run time
(changes semantics and breaks partial loads); silence until `--explain`
is asked (the status quo that caused the accident).

**Deferred, and pencilled into incubation in the same PR:** a
declaration for a predicate allowed to have no clauses (Prolog's
`dynamic`, under a word that promises no assert/retract); a `--strict`
flag promoting warnings to load errors for programs that run the binary;
other static checks, singleton variables first.

**Pre-write checks.** A design record lists no `affects` and closes no
task. `answer-soundness` invariant 3 is unchanged in meaning; the
implementing task adds a sentence that the undefined calls are reported
statically while the call still fails. Cross-repo impact: the stderr
warning document and the library accessor, noted above.

## Links

- Design record: `docs/design/2026-11-22-undefined-predicates-are-reported.md`
- Record PR: https://github.com/a1v0lut10n/claimr/pull/93
- Graduated note: `docs/incubation/2026-11-20-a-call-to-an-undefined-predicate.md` (renamed into the record)
- Deferrals: `docs/incubation/2026-11-22-declaring-a-predicate-without-clauses.md`, `…-a-strict-flag-for-batch-mode.md`, `…-other-static-checks.md`
- Consumer: genite GEN-141, `rust/crates/genite-mind-rules`
- Implementing task: to follow as CLM-0036
