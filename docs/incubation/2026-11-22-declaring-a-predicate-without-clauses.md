# Declaring a predicate that may have no clauses

Captured 2026-11-22. Deferred by the design record
`2026-11-22-undefined-predicates-are-reported.md`: once every call to a
predicate no clause defines is reported, a program that *means* to call
a predicate it does not define — one supplied by a later part of the
load, by the embedding program, or by a session still being typed —
has no way to say so, and lives with the warning.

Prolog's answer is `:- dynamic p/1.`: a declaration that the predicate
exists, may have no clauses now, and may gain or lose them later. Claimr
already has one declaration form, `:- table p/2.` (CLM-0027), and the
record decided that a `table` declaration is not a definition, so the
same directive syntax could carry a second word — `:- dynamic p/1.`, or a
name of claimr's own, since claimr has no assert/retract and "dynamic"
would promise more than it means; `:- defined p/1.` or `:- external p/1.`
say what is meant: calls to this predicate are not a mistake.

What it would settle: the declared predicate leaves the undefined list
(`Program::undefined()`), the batch and REPL warnings fall silent for it,
and `--explain` still names it at a `no-clause` leaf when a query fails
through it, since the failure is real even when it is intended.

Open: whether a declaration without clauses should also suppress the
first-argument index's "no candidate" shortcut (it should not — nothing
changes at run time); whether genite's rule files want it at all, or
prefer every call defined; and whether the embedding program should be
able to supply the declaration programmatically rather than in the text.

Adjacent work to connect with: `docs/design/2026-11-22-undefined-predicates-are-reported.md`,
`docs/design/2026-11-10-tabling-for-recursive-rules.md` (the directive
syntax), `src/eval/compile.rs` (`Clause::Directive`).
