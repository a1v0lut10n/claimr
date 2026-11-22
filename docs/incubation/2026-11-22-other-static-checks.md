# Other static checks a compiler could make

Captured 2026-11-22. Deferred by the design record
`2026-11-22-undefined-predicates-are-reported.md`, whose undefined-call
report is the first check the compile step makes on the program text
beyond what it must to run it. Once the pass, the warning line, the JSON
warning document and the REPL's two moments exist, further checks cost
little each — and each is its own decision, since every one has false
positives that need a way to be silenced.

Candidates, roughly in the order Prolog systems found them worth having:

- **Singleton variables.** A named variable occurring once in a clause is
  almost always a typo (`memebr(X, L)` for `member`, or `Y` where `X` was
  meant); the anonymous variable `_` (CLM-0020) is the way to say "once on
  purpose", and a leading underscore (`_Tail`) could be the way to name a
  singleton without a warning, as SWI-Prolog does. The most valuable of
  the candidates for a rule file written by hand.
- **Clauses that can never match.** A head whose first argument the index
  (CLM-0021, CLM-0035) proves no call can reach is unusual; more common
  is a clause shadowed by an earlier identical head with a committed
  construct. Lower value, harder to state exactly.
- **Constraints that are trivially true or false** at compile time
  (`{ 1 > 2 }` in a body, `{ X = X }`), where folding (CLM-0013) already
  knows the value.
- **A predicate defined but never called**, outside queries: dead rules
  in a rule file, or a predicate meant to be queried that no query names.
- **Arity confusion**: `p/2` defined, `p/3` called and undefined — the
  undefined-call report fires, and a note "did you mean `p/2`" would
  point at the likely cause.

Each check would report in the warning format of the undefined-call
record (`kind` per check) and be promotable to an error by the `--strict`
note. None is started here.

Adjacent work to connect with: `docs/design/2026-11-22-undefined-predicates-are-reported.md`,
`docs/design/2026-11-09-anonymous-variable.md`,
`docs/incubation/2026-11-22-a-strict-flag-for-batch-mode.md`,
SWI-Prolog's `library(check)` and its singleton warnings as prior art.
