---
date: 2026-11-10
type: design
status: proposed
components: [grammar, parser, evaluator, cli]
aspects: [grammar-authority, answer-soundness]
tags: [tabling, recursion, termination, transitive-closure, memoisation, directives, linear-tabling, prolog-iii]
---

# Tabling for recursive rules: declared predicates memoise their ground answers

## Context

The incubation note of 2026-11-08 came from rules over a knowledge graph,
where transitive predicates are the norm: part-of chains, derived-from
lineage, broader-than in a concept hierarchy. Under SLD resolution the
natural definition of a transitive closure,

```
ancestor(X, Z) :- ancestor(X, Y), parent(Y, Z).
ancestor(X, Y) :- parent(X, Y).
```

never terminates: the first goal of the first clause is an instance of the
call itself. The right-recursive version terminates on acyclic data only,
and a real graph holds cycles, through data errors if nothing else; on a
cycle it produces the same answers again without end. Neither the
groundness rule of CLM-0022 nor the collection of CLM-0025 helps: a
comprehension over `ancestor` on cyclic data never finishes collecting.

**What exists.** The machine (`machine.rs`) is an iterative SLD engine:
a call takes the candidate clauses of its predicate in order
(first-argument indexing, CLM-0021), unifies the head against the call in
the shared store, pushes a choice point for the remaining clauses and
prepends the body. The store is one trail-undone object per query; a
`Program` carries the clauses, the index, the built-ins and the initial
world store. Three constructs already run a *sub-search* inside a
derivation and decide something from its outcome — if-then-else and
negation (CLM-0022) commit on a first solution, a comprehension (CLM-0025)
collects every solution — under the rule that the sub-search's external
variables are ground, that every solution is finalized exactly, and that
a collected instance is a closed term (`Store::snapshot`, a `TTerm` with
numbers for determined variables). Explanations (CLM-0023) record, for
every goal, the clause or constraint that resolved it; invariant 6 of
`answer-soundness` says a proof cites only the answer's own derivation.

**What Prolog III does.** The manual (Version 1.3, December 1991) names
the problem and offers rewriting as the answer. Its first example of
control, pp. 162–163, is transitivity: solving `plus_grand(Jo, x)` with
`plus_grand(x, y) -> plus_grand(x, z) plus_grand(z, y);` "retrouve une
instance de ce même but à effacer. Le programme se met alors à boucler et
se termine par un débordement de pile", and "la manière correcte d'écrire
ce programme consiste à enlever la récursivité à gauche", with a
right-recursive rewrite over data that happens to be acyclic. There is no
memoisation in Prolog III, no depth bound, and no termination guarantee
beyond what the programmer arranges; `freeze` (p. 149) delays a goal
until a variable is known, which reorders a search but does not make a
cyclic one finite.

**What tabling is.** OLDT resolution (Tamaki & Sato, 1986) and SLG
resolution (Chen & Warren, 1996), as in XSB and SWI-Prolog's `:- table`,
memoise the answers of declared predicates per *variant* of the call: a
call that is a renaming of one already under evaluation consumes the
answers found so far instead of recursing, every answer is recorded once,
and a table is *complete* when no evaluation can add to it. On a finite
set of answers this terminates whatever the data's shape and whatever the
recursion's direction, and the transitive closure of a cyclic graph is a
finite table. SLG suspends and resumes consumers inside one search;
*linear tabling* (B-Prolog; Zhou et al., TPLP 2008) gets the same answers
for definite programs by re-running the leading looping call until an
iteration adds nothing, which costs recomputation and needs no
suspension machinery. Tabling over *constraint* answers — storing stores,
checking entailment between them (Cui & Warren, 2000) — is a research
topic, not a feature of any of these systems' defaults.

## Decision

1. **A directive declares a tabled predicate.** The grammar gains a clause
   form `Directive: ':-' Ident Indicator+[Comma] '.'` with
   `Indicator: Name '/' Number`, written

   ```
   :- table ancestor/2.
   ```

   The directive name is checked in the compile step (`table` is the only
   one; an unknown directive, a non-integer arity or a tabled built-in is
   a positioned compile error). A tabled predicate's clauses are ordinary
   clauses; only the way its calls are resolved changes. SLD stays the
   default for every undeclared predicate, as in Prolog III and in every
   tabled Prolog: tabling changes answer multiplicity (below) and costs
   memory, so the program says where it wants it.

2. **A table belongs to the program, not to the query, and holds ground
   answers.** The table of a call is keyed by the call's *variant*: its
   arguments dereferenced, with every unbound variable — constrained or
   not — replaced by a canonical fresh one. The table is computed by a
   **sub-machine over the world store alone** (a store seeded from the
   program's initial store, not the caller's), so its content depends on
   the program and nothing else; it is therefore valid across
   backtracking, across the queries of a file, and across REPL steps, and
   is discarded whenever the program's clauses change (`:load`, `:reload`,
   `:clear`, a clause at the prompt). Each answer is the tuple of the
   call's arguments as a closed term, taken by `Store::snapshot` after
   `finalize`, exactly as a comprehension takes an instance; a solution
   whose arguments are not ground stops the query with
   `EvalError::NonGroundTabledAnswer { predicate, instance }`. A table is
   a *set* in insertion order: an answer derived twice is recorded once.

3. **A call consumes its table by unification in the caller's store.**
   When a tabled call is reached, its table is completed if it is not
   already, and the call then unifies with each recorded answer in turn
   against the caller's current store, with a choice point per remaining
   answer, as if the table were a list of facts. A constraint the caller
   holds on an argument's variable (`{ X > 3 }, ancestor(X, Y)`) is
   applied at that unification — the table is for the abstracted call,
   the filtering is exact because the answers are ground — so no call is
   refused for its store.

4. **Completion by iteration (linear tabling).** The sub-machine keeps a
   stack of the tables under evaluation. A tabled call whose table is
   complete consumes it. One whose table does not exist starts an
   evaluation: its clauses are run to exhaustion, every solution added to
   the table. One whose table is *under evaluation* — a recursive variant
   call — consumes the answers recorded so far and marks every table above
   it on the stack as depending on it. When an evaluation ends and its
   table depends on no table below it, the table and those that depend on
   it are complete; otherwise the leading table of the dependency re-runs
   its evaluation, and repeats until a run adds no answer to any table in
   the group, then marks them all complete. Every iteration either adds an
   answer or terminates, so a finite answer set is reached in finitely
   many iterations on any data, cyclic included; a predicate with
   infinitely many ground answers (`nat(s(X)) :- nat(X)`) does not
   terminate under tabling either, and interruption (Ctrl-C) reaches the
   sub-machine as it reaches the main one.

5. **A sub-search is decided on complete tables only.** If a condition
   (`->`, `\+`) or a comprehension body consumes a table that is under
   evaluation — a negation or an aggregate over a predicate inside its own
   recursive group — the query stops with
   `EvalError::IncompleteTable { predicate, construct }`: the construct
   would otherwise be decided on a partial answer set. Negation through
   recursion (well-founded semantics, SLG's delaying) is deferred; the
   stratified case — the negated predicate's table completes before the
   construct runs — works, which covers `\+ ancestor(a, b)` and
   `|<Y | ancestor(X, Y)>|` wherever `ancestor`'s group does not contain
   the caller.

6. **Explanations.** A table records, with each answer, the proof tree of
   the derivation that first produced it (closed `ProofNode`s from the
   sub-machine's arena). A tabled call in an answer's explanation is a
   node `by: "tabled"` whose subgoals are that recorded derivation; the
   derivations an iteration repeats or abandons are not cited. Invariant 6
   holds: the cited derivation is the one the answer came from.

7. **Not in this record.** Answer subsumption and mode-directed tabling
   (`min`, `max`, lattices); non-ground or constrained answers; tabled
   negation inside a recursive group; incremental tabling under clause
   changes (tables are discarded wholesale); a step or depth limit for
   untabled recursion — the incubation note's smaller alternative stays
   available as its own record if hangs in untabled code prove to be a
   problem in practice.

## Consequences

- **Transitive rules over real graphs terminate.** Both recursions of
  `ancestor` give the same finite table on any `parent` relation, cycles
  included, and give it once: `ancestor(a, Z)` on a graph with two paths
  to `c` answers `Z = c` once, where SLD answers it twice. Answer
  *multiplicity* is the visible semantic change, confined to declared
  predicates. Counting with a comprehension over a tabled predicate counts
  distinct answers, which is what "the number of ancestors" means.

- **Soundness.** Every table answer is a solution of the program over the
  world store, finalized exactly and ground; consuming it by unification
  in the caller's store is SLD over a fact, so invariants 1 and 2 of
  `answer-soundness` hold as they do for facts. The table depends on the
  program only, so no backtracking state leaks into it — the reason the
  sub-machine runs on the world store and not the caller's: a tabled body
  that mentions a world unknown (`old(bob) :- { age(bob) > 70 }`) must not
  be memoised as empty because one caller had posted `{ age(bob) < 60 }`.
  Invariant 5 extends to tables: a sub-search decided only on complete
  information. Invariant 6 holds through the recorded derivations.
  `answer-soundness` gains the stated property that a table's content is a
  function of the program.

- **Grammar.** The first directive of the language; `Directive` is a
  general form so later declarations reuse it. `grammar-authority`:
  grammar, actions, an example under `examples/` with a cyclic graph, the
  EBNF view and the README. `:-` now begins a clause as well as separating
  head from body; no LR conflict is expected since a rule's `:-` follows
  an atom and a directive's begins the clause, to be checked with
  `rcomp --force --dot` as for every grammar change.

- **Evaluator.** A `Tables` registry on the `Program` side (behind a
  `RefCell`, or owned by the `Solutions` iterator's program handle, since
  `Program::solve` takes `&self`); a sub-machine constructor over a cloned
  world store; `Retry::Answers` for consumption; the evaluation stack and
  dependency marks; `Resolution::Tabled` (`"tabled"` in `--json`, a new
  value of `by` — document shape unchanged); two errors. Cost: one store
  clone per leading evaluation, recomputation across iterations, and
  tables held for the program's lifetime. Programs that do not declare a
  table pay nothing on the call path beyond a lookup of the predicate's
  tabled flag, which can sit in the index.

- **CLI.** Tables are cleared on any change to the program in the REPL;
  `:list` prints directives with the clauses. `--explain` shows tabled
  calls with their recorded derivations.

- **Harder.** Debugging a looping *untabled* recursion is as it was:
  Ctrl-C or a stack limit, which the manual's own example ends in. A
  predicate that cannot be tabled because its answers carry constraints
  (`reach(X, Y) :- { D(X, Y) > 0 }`-shapes) is reported, not silently
  approximated, and stays untabled until the deferred work exists.

- **One task** implements the directive, the sub-machine and linear
  completion, consumption, the two errors, the explanation node, an
  example with a cyclic `parent` graph and both recursions, tests for the
  variant key with constrained callers, the store-independence case above,
  negation and comprehension over complete and incomplete tables,
  duplicate elimination, REPL invalidation and interruption inside a
  sub-machine; `grammar.md`, `README.md`, `evaluator.md`,
  `answer-soundness.md`, `json-output.md` and the architecture README's
  `cli` row follow.

## Sources

- Incubation note `2026-11-08-tabling-for-recursive-rules.md` (graduated
  by this record).
- Prolog III manual, Version 1.3 (PrologIA, 1991): control and the
  transitivity example, pp. 162–163; `freeze`, p. 149.
- H. Tamaki, T. Sato, *OLD resolution with tabulation*, ICLP 1986.
- W. Chen, D. S. Warren, *Tabled evaluation with delaying for general
  logic programs*, JACM 43(1), 1996 (SLG).
- N.-F. Zhou, T. Sato, Y.-D. Shen, *Linear tabling strategies and
  optimizations*, TPLP 8(1), 2008.
- B. Cui, D. S. Warren, *A system for tabled constraint logic
  programming*, CL 2000.
- `docs/design/2026-11-09-negation-disjunction-and-if-then-else.md`
  (the groundness rule and the commit), `2026-11-09-aggregation-list-
  comprehensions-and-sum.md` (collection and `snapshot`),
  `2026-11-09-explanations-of-answers.md` (invariant 6).
