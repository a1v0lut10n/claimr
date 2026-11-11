---
date: 2026-11-11
type: task
status: done
affects:
  - docs/architecture/components/evaluator.md
  - docs/architecture/aspects/answer-soundness.md
  - docs/reference/json-output.md
components: [evaluator, cli]
aspects: [answer-soundness]
design:
  - docs/design/2026-11-11-explanations-of-failure.md
tags: [CLM-0030, explanations, failure, minimal-conflict, replay, quickxplain]
---

# CLM-0030: Step records, the replay minimiser and the load-time conflict set

## Premise

The accepted record `2026-11-11-explanations-of-failure.md` explains a
failed step by a *minimal unsatisfiable subset* of the derivation's earlier
steps — clause heads, posted constraints, built-ins, table answers,
collected lists and the program's constraint facts — found by QuickXplain
with the store as the exact oracle: the steps are posted into a fresh
store over the program's symbol table and finalized. This first task lands
the machinery and its exactness tests, and the one place it is already
visible: a program whose constraint facts are unsatisfiable fails to load
with a minimal conflicting set of facts named by position and text. No
query output changes; the failure tree is CLM-0031.

## Shape of the implementation

**A step is a closed, replayable record** (`src/eval/explain.rs`,
`Step`): `Head { key, idx, head, call }` (the clause's head template and
the call's, both closed), `Constraint(op, a, b)` (an `=`, `!=` or
relation, closed), `Builtin(b, call)`, `Tabled { call, answer }` (the
answer a ground template), `Collected { into, list }` (the list a ground
template), `Fact(idx)` (the program's `initial[idx]`) and `Answer` (the
finalize). *Closed* means a `TTerm` whose variables are the derivation's
heap addresses of the frame variables they stood for (`close(tpl, frame)`
rewrites `Var(i)` to `Var(frame[i])`), so steps taken under different
frames agree on which variable is which, and a replay allocates one fresh
variable per distinct id. A fact keeps its own namespace, as in
`post_initial`.

**The arena keeps what closing needs.** `ProofKind::Clause` gains the
clause's frame and the call's source (template and frame); `Constraint`
and `Builtin` gain the goal template and frame; `Tabled` the call's
source; `Collected` already has the target address (the list is read from
the heap with `Store::snapshot` at closing time, ground by CLM-0025). The
call's source travels as a `CallRef<'p>` — address, template, frame —
through `try_clauses`, `Retry::Clauses`, the tabling paths and
`Goal::Resolve` (whose template is the variant key and whose frame is the
abstract variables). Nothing is allocated on the hot path beyond an `Rc`
clone of a frame; closing happens only when steps are asked for.
`Solutions::derivation_steps()` (crate-visible) closes the current
derivation: the program's facts in order, then the arena's records in
order — the arena holds exactly the live derivation's successful steps,
since backtracking truncates it.

**The oracle** (`explain::satisfiable(program, steps) ->
Result<bool, EvalError>`) builds `Store::with_symbols(program.symbols)`,
replays the steps in order — head: build both templates with `build_with`
over the shared variable map, `post_eq`; constraint: `post_eq_goal` /
`post_dif_goal` / `post_rel`; built-in: the machine's dispatch, moved to a
free function; tabled and collected: build and `post_eq`; fact: the
program's fact posted on a fresh local frame — and ends with `finalize`.
A failed post or finalize is `false`; a store error is `Err`. Replaying
every step of a derivation is the derivation's own constraint system, so
an answer's steps are satisfiable.

**The minimiser** (`explain::minimal_conflict(program, candidates,
failing) -> Result<Vec<usize>, EvalError>`): QuickXplain (Junker 2004)
with background `{failing}`, candidates in the given order, returning the
indices of a minimal subset `C` with `C ∪ {failing}` unsatisfiable and
every proper subset satisfiable; the failing step alone unsatisfiable
gives the empty set. Recursion depth is logarithmic in the candidate
count.

**Load time.** `Program.initial` becomes `Vec<Fact { goals, nvars, span,
text }>`; `post_initial` reports the index of the first fact that fails;
`compile_iter` then minimises over the facts before it and returns
`EvalError::InitialStoreUnsatisfiable { conflict }`, a list of
`(Option<Span>, String)`. Display: the message as today, then one line
per fact, `  { age(bob) >= 18 }.  fact 3:1` (`fact` alone without a
position). The CLI's human path prints it as it prints any load error;
`--json` carries it in the stderr document's `message`, shape unchanged.
A minimiser error falls back to the bare message.

## Action items

1. `explain.rs`: `Step`, `close`, `satisfiable`, `minimal_conflict`.
2. Machine: `CallRef`, the enriched arena records, `derivation_steps()`;
   the built-in dispatch as a free function shared with the oracle.
3. Compile: `Fact` with position and text; `post_initial` → failing
   index; the load-time conflict; the error variant and message.
4. Tests — unit: an answer's steps replay satisfiable, across a rule, a
   tabled call, a comprehension and a built-in; a conflict set is
   unsatisfiable with the failing step and satisfiable with any member
   removed, and is the expected two steps for `eligible(bob)` against
   `{ age(bob) < 18 }`; a world fact is cited when it is the cause; a
   head clash is explained by the failing step alone; the set reported
   among several minimal ones is deterministic; the load error lists
   exactly the conflicting facts with positions and leaves the unrelated
   one out. Integration: the batch and REPL load-error messages carry the
   cited facts.
5. Docs: `evaluator.md` (the step records, `explain.rs`, the load-time
   conflict), `answer-soundness.md` (invariant 9, the conflict-set part;
   CLM-0031 adds the failure tree), `json-output.md` (the load-error
   message); this plan.

## Findings

- **`#[expect(dead_code)]` for the slice boundary.** Outside tests nothing
  asks for a derivation's steps until CLM-0031, so rustc reports the arena's
  new fields, `CallRef`, `Tpl`, `close` and the `Step` variants as dead. One
  marker on `derivation_steps`, `#[cfg_attr(not(test), expect(dead_code,
  reason = …))]`, covers them all: an item with an allowed or expected
  dead-code lint counts as a live root, so everything it reaches is live.
  Markers on the reached items were "unfulfilled" and had to go. `expect`
  rather than `allow` so that CLM-0031, which uses the method, is forced to
  remove the marker.
- **`abstract` is a reserved word.** The abstract call's `CallRef` is
  `abstract_ref`.
- **Replay re-reads `=` and `!=`.** `post_eq_goal` / `post_dif_goal` decide
  numeric or tree by what the operands denote when posted (CLM-0026); a
  subset replay may reach a goal in a different state than the derivation
  did, so the subset's system is claimr's operational reading of those
  steps in that order. The property tested is exactly the record's: the
  reported set replays unsatisfiable, each proper subset satisfiable.
- **The head step is part of a conflict.** `eligible(bob)` against
  `{ age(bob) < 18 }` cites the rule's head *and* its body constraint:
  without the head, `age(X) >= 18` is about another unknown. The record's
  "three constraints" for the integrity checker will usually be a head and
  two constraints, which is what a reader needs to see.

## Verification

- [x] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test` (146 unit tests, six new in `conflict_sets`; the batch and
      REPL load-error assertions extended); existing golden files
      byte-identical.
- [x] A query's output is unchanged with and without `--explain` (the
      golden runs, `explain.claimr` included, and the REPL suite pass
      unchanged).
- [x] `aivolution lint .` clean; the three `affects` docs current.
