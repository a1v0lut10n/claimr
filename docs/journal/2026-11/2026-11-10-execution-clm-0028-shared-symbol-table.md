---
date: 2026-11-10
type: journal
components: [evaluator]
aspects: [answer-soundness]
tasks:
  - docs/implementation/2026-11-10-clm-0028-shared-symbol-table.md
---

# CLM-0028 executed: one symbol table for the program and every store it runs

- **When:** 2026-11-10 23:31:00

## Context

Weighing semi-naive consumption as a design record after CLM-0027, I
measured the chains the plan had called quadratic. The left-recursive
closure is linear; the right-recursive ground chain was quadratic for a
reason unrelated to the strategy: every table evaluation clones the world
store, and since CLM-0017 a store owns a copy of the program's symbol
table, a vector and a hash map of every atom. Reproducing that cost turned
up a correctness bug the same copy causes. A tabled predicate whose answer
is an atom that `atom_chars` builds at run time interns it in the
evaluation's copy; the caller's copy has no such index, or another name at
it:

```
:- table mk/1.
mk(A) :- atom_chars(A, <x, y, z>).
?- mk(A).                           panicked: index out of bounds
?- atom_chars(B, <a, b>), mk(A).    B = ab, A = ab      (should be A = xyz)
```

The wrong answer breaks invariant 8 of `answer-soundness`: what the caller
got depended on what it had interned, not on the program. Both cases came
in with CLM-0027; before it a query ran in one store. The semi-naive
record was set aside and this bugfix ticket claimed instead; it landed as
PR #71.

## Details

**What was built.** `Program.symbols` and `Store.symbols` are one
`Rc<RefCell<Symbols>>`, made once when a program compiles and shared by
the world store and every store cloned from it, the query's and each
evaluation's. A symbol interned at run time in any store is thereby known
to all of them, and a table answer carries indices every consumer resolves
to the same name. The two intern sites in the store borrow mutably for the
call; the answer printer holds a `Ref` for its life, and `render_answer`
and `render_terms` read the store's table instead of taking one, so no
caller passes a table that could differ from the store's. Nothing interns
while a printer is alive. The alternative, a store borrowing the program's
table by lifetime, would thread that lifetime through the machine, the
`Solutions` iterator, the REPL and every test; the program already shares
its tables with the machine through a `RefCell` for the same reason.
Thirty lines changed in four files.

**What was verified.** Two unit tests: the two probes answer `A = xyz`,
and `mk(A), atom_chars(A, L)` round-trips through the shared table; the
world store and the program hold one table (`Rc::ptr_eq`), so the copy
cannot quietly return. 136 unit tests, every integration suite, every
golden file byte-identical, fmt, clippy and the docs linter clean.
Measured in the release build, `?- reach(a0, aN).` under right recursion:
0.06 s, 0.20 s, 0.66 s before for 1 000, 2 000 and 4 000 nodes; 0.02 s,
0.03 s, 0.04 s after, which is what the untabled chain costs.

**Correction to CLM-0027.** The plan's *Quadratic chains* finding, repeated
in `evaluator.md` and in yesterday's execution entry, said a left-recursive
closure runs *n* passes over a growing table. It runs three: the
consumption choice point reads the answer count at retry time, so the
second pass chains through every answer as it is recorded and the third
finds nothing new. Measured, 0.03 s, 0.03 s and 0.04 s for 1 000, 2 000
and 4 000 nodes, on a chain and on a cycle alike. The right-recursive call
with its second argument open is quadratic because its *n* tables hold
*n²/2* answers, the size of its output, which no consumption strategy
changes; call abstraction or subsumptive tabling would, and that is a
different decision. Semi-naive consumption would save a constant factor on
the confirming pass and is not pursued. `evaluator.md` is corrected in
place; the CLM-0027 plan carries an addendum; the CLM-0027 journal entry
stands as written, journal entries being immutable, and this one records
the correction.

**Pre-write checks.** The two docs in the task's `affects` are current:
`evaluator.md` (the store's symbol table is shared; the tabling cost
passage restated from the measurements) and the CLM-0027 plan (the
addendum). `answer-soundness` is restored, not changed: invariant 8 held
again, with the probe as its test; no invariant is restated. No cross-repo
impact: no output shape or vocabulary changes.

## Links

- Task: `docs/implementation/2026-11-10-clm-0028-shared-symbol-table.md`
- PR: https://github.com/a1v0lut10n/claimr/pull/71
- Commits: `5d39796` (plan), `98b06ba` (fix)
- The finding corrected: `docs/implementation/2026-11-10-clm-0027-tabling.md`,
  *Quadratic chains* and its addendum
- The entry it corrects:
  `docs/journal/2026-11/2026-11-10-execution-clm-0027-tabling.md`
- The store's symbol table since CLM-0017:
  `docs/architecture/components/evaluator.md`
