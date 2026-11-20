# A call to a predicate no clause defines

Captured 2026-11-20. Prompted by genite GEN-141, where the first rule
written against claimr as a library used `dif(V, W)` where claimr
writes `{ V != W }`. The rule found nothing and nothing said why.

What exists. A call to a predicate with no clause fails, as in Prolog
with `unknown` set to `fail`. A failed query asked with `--explain`
does say it: `nosuch(X)  failed: no clause (undefined predicate)`.
Checked on 0.7.0:

```claimr
p(a).
s(X) :- p(X).
s(X) :- p(X), nosuch(X).
?- nosuch(X).
?- s(X).
```

The first query prints `false`, and with `--explain` names the
undefined predicate. The second prints `X = a` and never mentions
`nosuch/1`: the clause that calls it fails quietly beside one that
succeeds. Without `--explain`, and through the library unless the
caller asks for `Solutions::failure()`, neither says anything.

For a rule file that checks integrity this is the worst failure there
is: a misspelt name makes a rule that reports no violation, which
reads as a clean result.

The idea: the compile step knows every predicate that has a clause and
every built-in, so it can tell which calls reach neither. Report them:
as a warning on the command line, and as a list the library returns
with the compiled program (`Program::undefined()` or the like), so that
the embedding program decides whether it is an error.

Not thought through: predicates meant to be supplied later (the REPL
adds clauses to a session; a program may be loaded in parts), calls
built at run time if claimr ever has them, and whether a declaration
(`:- dynamic p/1.` in Prolog's words) is wanted for a predicate that is
allowed to have no clause.
