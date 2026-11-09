---
date: 2026-11-09
type: design
status: accepted
components: [grammar, parser, evaluator, highlight]
aspects: [grammar-authority, answer-soundness]
tags: [negation, disjunction, if-then-else, control, soundness, iso-prolog, prolog-iii]
---

# Negation, disjunction and if-then-else in bodies

## Context

A claimr body is a conjunction: `Goal+` separated by commas in
`src/parser/claimr.rustemo`. Alternatives are separate clauses, and there
is no way to say that a goal has no solution. Two incubation notes of
2026-11-08, written after using claimr to check integrity rules over a
knowledge base, name what that costs:

- **Absence rules** are most of a lint and cannot be written: "a model
  file that has no residual-error claim", "an open claim that no closure
  closes", "a record without a label". A constraint `!=` compares terms;
  it cannot say that a goal fails.
- **Committed choice** cannot be written either: "the curator's value if
  there is one, else the harvested one", "the first matching rule
  decides". Separate clauses duplicate shared goals and still cannot
  express the "else".

**What Prolog III does.** The CACM paper's abstract machine (the evaluator
record) has no control beyond goal order, rule order and backtracking. The
language as shipped has more; the reference manual (*Prolog III, Version
1.3*, PrologIA, December 1991, French) is the source for the following:

- **The cut** `/`, also written `!` in Edinburgh syntax, with the usual
  meaning: it discards the remaining rule choices for the goal being
  executed and for the goals before it in the rule (p. 164, reference
  entry p. 230). The basic-concepts chapter sets it aside in a footnote
  as "a parasite that can appear in a rule tail in place of a term"
  (p. 61): outside the logical reading of a rule.
- **Negation as failure through the cut.** `not(T)` is a predefined rule
  defined in extenso as `not(P) -> P ! fail; not(P) ->;` (p. 305), and
  the environment chapter shows `non(p) -> p / fail; non(p) ->;` and the
  if-then-else idiom `si_alors_sinon(p,a,b) -> p / a;
  si_alors_sinon(p,a,b) -> b;` as the classic uses of the cut (p. 165).
  Marseille syntax has no `\+` and no body operators: `->` is the rule
  neck and `;` the rule terminator.
- **Disjunction only in Edinburgh syntax**, as the predefined `X ; Y`,
  "installs a choice point", defined as `X ; Y :- X. X ; Y :- Y.`
  (p. 419; the term it builds, p. 458). In Marseille syntax alternatives
  are rules. Edinburgh `->` is declared as an operator of priority 700
  (p. 460), not as control.
- **The soundness problem is named.** The Boolean constraints chapter
  negates a circuit predicate with `non` and warns that "the enumeration
  must happen before the negation, otherwise, the variables not yet being
  instantiated, the negation will be systematically verified", then
  rejects that program because forcing the enumeration first "is
  fundamentally opposed to the Prolog III philosophy of installing as
  many constraints as possible before any enumeration" (p. 140). Its two
  answers: test for groundness in the program with the meta-predicate
  `known(T)`, which succeeds when a term's label and arity are known
  (pp. 141, 148–149); or, for Boolean problems, push the negation into
  the Boolean algebra as a constraint, "rigorously" (p. 141).
- **Delay.** `freeze(T, Goal)` postpones a goal until the term `T` is
  known, with `known`, `bound` and `free` as the tests (p. 149); Prolog
  II had the same (*Prolog II, Manuel de référence*, R 2-9).

So Prolog III computes exactly what this record's constructs compute, with
the cut, and leaves the ordering that makes the result right to the
programmer, documenting the wrong answers obtained otherwise. The spellings
here are ISO Prolog's, which do not clash with claimr's `:-` and `.` as
they would with Marseille syntax. What is not Prolog III's is the rule that
makes the constructs sound over a constraint store without the programmer
testing `known` by hand.

**Why the store makes negation hard.** `\+ G` succeeds when `G` has no
solution in the current store and fails when it has one. Success is
stable: a store only grows, so a goal with no solution now has none
later. Failure is not: `r(X) :- \+ { X = a }.` called with `X` unbound
fails because `X = a` has a solution, yet `r(b)` holds. In a conjunction
that only loses answers; under a second negation it produces a wrong one
(`s :- \+ r(X).` succeeds, though `r(b)` is true). If-then-else has the
same flaw in its condition: a condition that binds a variable the rest of
the clause also uses commits to a solution the rest may later exclude,
and the construct's outcome then depends on goal order. That is the
manual's "systematically verified" negation; the `answer-soundness`
aspect forbids it.

The sound reading is simple to state: the decision of a condition must not
depend on anything the rest of the derivation can still change. A
variable the condition shares with its surroundings is such a thing
unless it is already ground; a variable local to the construct is not,
since nothing outside can ever constrain it.

## Decision

1. **Disjunction `A ; B`** in bodies, both alternatives tried in order
   through a choice point, as Prolog III's Edinburgh `;` does. It is not
   rewritten into an auxiliary predicate: the AST keeps the construct and
   the clause printer prints it back, so `:list` and `--parse` show the
   clause as written.

2. **If-then-else `C -> T ; E`**, and **if-then `C -> T`** (else fails).
   The condition runs to its first solution. If it has one, the search
   commits to it: the condition's remaining alternatives are discarded,
   `T` runs in the store the solution left. If it has none, the store is
   restored and `E` runs. A solution of the condition is one the answer
   stage would accept: the store is finalized exactly, as for an answer,
   so a pending disequation or a non-linear residue is decided (or stops
   the query) before the commit, never after it.

3. **Negation `\+ G`**: succeeds when `G` has no solution, binds nothing,
   fails otherwise. It is `(G -> fail ; true)` with the same store
   discipline, and it is implemented as that.

4. **The groundness rule.** A condition (of `->` or `\+`) has *external*
   variables: those that also occur in the clause outside the whole
   construct, in the head, in goals before it or after it. For a query,
   the construct's surroundings are the other goals of the query. When
   the construct is reached, every external variable must be ground: its
   term, dereferenced through the store, contains no unbound variable,
   and every numeric variable in it is determined by the solver. A
   variable that occurs only inside the construct, in the condition
   alone, or shared with `T` or `E`, is the construct's own and may be
   anything. Disjunction has no condition and no rule.

5. **A construct reached otherwise stops the query** with a runtime
   error, `EvalError::NonGroundCondition`, in the way a non-linear residue
   does. The diagnostic names the clause and the construct as written and
   the variable by its source name (`\+ claim(F, residual_error, _)
   reached with F unbound`), since reordering the body is the fix.
   Nothing is ever decided on a non-ground external variable, in either
   direction: a program that is misordered fails loudly on any data. This
   is Prolog III's `known(T)` test, performed by the machine on exactly
   the variables whose state matters, instead of by the programmer.

6. **Spelling and precedence** are ISO's. `,` binds tightest, then `->`,
   then `;`, which is right-associative; `\+` applies to one goal, which
   can be an atom, a constraint block `{ … }`, another `\+`, or a
   parenthesised body. Parentheses group a body as a goal. In EBNF:

   ```ebnf
   body  ::= alt [ ";" body ]
   alt   ::= conj [ "->" conj ]
   conj  ::= goal { "," goal }
   goal  ::= atom | "{" constraint_expr "}" | "\+" goal | "(" body ")"
   ```

   A `;` whose left operand is `C -> T` is an if-then-else, any other `;`
   a disjunction; `a -> b ; c -> d ; e` is `(a -> b) ; ((c -> d) ; e)`.

7. **Not in this record.** The cut: Prolog III has it, and its manual
   shows negation and if-then-else as its classic uses (p. 165); claimr
   takes those two uses as constructs and leaves the cut out, because its
   effect is non-local (it discards choices of goals before it in the
   rule, p. 164) and the groundness rule has no analogue for it. The
   manual's own footnote treats it as outside the logical reading (p. 61),
   and claimr has no non-logical construct so far. `true/0` and `fail/0`
   as built-ins: a program that wants them writes `true().` and
   `fail() :- { 0 = 1 }.`. Constructive negation (a condition on a bounded
   numeric variable returning the complementary constraint, `\+ { X > 5 }`
   under `X > 3` giving `X <= 5`): a solver extension of another order;
   Prolog III's Boolean-algebra answer (p. 141) is of this kind, for its
   Boolean domain. A delayed negation that waits until its external
   variables are ground, in the style of `dif` and the list relations,
   would accept misordered programs; it is Prolog II's and III's
   `freeze(T, Goal)` (p. 149) applied to a negation, and it would be the
   general delayed goal. It is a later record if a program needs it, and
   it would keep this record's rule as its wake condition.

## Consequences

- **Soundness.** With the external variables ground at entry, the
  condition's search sees the same store at every point of the
  derivation, since later goals can only constrain variables the
  condition does not reach: the condition's own variables are fresh and
  unreachable from outside, and its external ones cannot change. Its
  first solution, or its failure, is therefore the same in every
  completion of the derivation. The construct acts as a function of ground
  inputs, and so stays sound under negation and in any nesting. Taking
  `E` or succeeding on `\+ G` asserts "no solution", which is stable in
  any case. Taking `T` reports the condition's first solution and no
  other: that is what committing means, and it is the only incompleteness
  the constructs introduce. `answer-soundness` gains an invariant stating
  the rule, with the test that a non-ground condition is an error and a
  ground one agrees with the clause-by-clause program.

- **Relative to Prolog III.** The constructs compute what Prolog III's
  `not`, `X ; Y` and the `si_alors_sinon` idiom compute (pp. 165, 305,
  419), under new spellings. Two departures, both deliberate: there is no
  cut, and the machine refuses the case the manual warns about (p. 140)
  instead of returning the "systematically verified" negation. The
  departure is in the direction of the manual's own stated philosophy,
  constraints before enumeration, which its cut-based `not` works
  against. `freeze` and `known` are not adopted; the groundness check
  does `known`'s job where it matters, and a delayed goal is a separate
  record.

- **The idiom moves one goal.** Prolog programmers write
  `value(K, X) :- (curated(K, X) -> true() ; harvested(K, X)).`; here `X`
  is external and the clause errors when called with `X` unbound, as it
  must: `value(k, v2)` would succeed while `value(k, X)` returns only the
  first curated value. The sound spelling puts the result into the branch:
  `value(K, V) :- (curated(K, X) -> { V = X } ; harvested(K, V)).`, with
  `X` local to the construct. The reference documents this pair; the
  diagnostic points at it.

- **The absence rules work as written** once their generators come first:
  `missing(F) :- model_file(F), \+ claim(F, residual_error, _).`,
  `unclosed(C) :- claim(C), open(C), \+ closes(_, C).`. The anonymous
  variables are local; `F` and `C` are ground when the negation is
  reached. This is the ordering the manual asks for (p. 140); the
  difference is that the other ordering is an error here, not a wrong
  answer.

- **Machine.** The goal list and choice-point stack already carry what the
  constructs need: a second kind of choice point that resumes with a goal
  list (the else branch, the second alternative) instead of retrying
  clauses, and a commit goal that truncates the choice-point stack to the
  height recorded when the condition started and then finalizes the
  store. The groundness check is a walk of the external variables' terms
  at entry; the compile step records each construct's external variable
  indices and their source names. First-argument indexing, the store, the
  solver and the answer printer are untouched.

- **Grammar path** per `grammar-authority`: three terminals (`;`, `->`,
  `\+`), the body rules above, actions building the AST constructs, an
  example `examples/control.claimr` with answers (the lint idioms, the
  curated-or-harvested idiom, nested forms), the EBNF view, the README,
  and `src/highlight.rs` (`\+` and `->` as operators, `;` as
  punctuation). The REPL's `;` for the next answer is a line of its own
  while an answer is pending, so a `;` inside a query does not meet it.
  `->` is a longer match than `-`, like `=>` and `=` today.

- **Runtime error, not a new output shape.** The error reaches the CLI as
  `EvalError` text and the JSON output as the existing
  `"outcome": "error"` document; no contract changes, no cross-repo
  impact.

- **One task** implements the three constructs together, since negation
  is one line over if-then-else and the example exercises all three. It
  updates `evaluator.md` (the "not yet" list loses negation and
  disjunction; cut becomes deliberately absent), `answer-soundness.md`
  and `docs/reference/grammar.md`.

## Sources

- A. Colmerauer, *An Introduction to Prolog III*, Communications of the
  ACM 33(7), July 1990 (the evaluator record's reference design).
- PrologIA, *Prolog III, Version 1.3*, reference manual, December 1991
  (French), from Colmerauer's archive at alain.colmerauer.free.fr
  (`ArchivesPublications/Prolog3Manuel.pdf`). Page numbers above are the
  manual's own.
- PrologIA, *Prolog II, Manuel de référence*, same archive
  (`ArchivesPublications/Prolog2Manuel.pdf`), section R 2-9 for `not`
  and `freeze`.
