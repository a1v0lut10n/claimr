---
date: 2026-11-10
type: task
status: in-progress
affects:
  - docs/architecture/components/evaluator.md
  - docs/implementation/2026-11-10-clm-0027-tabling.md
components: [evaluator]
aspects: [answer-soundness]
tags: [CLM-0028, bugfix, tabling, symbols, store]
---

# CLM-0028: One symbol table for the program and every store it runs

## Premise

Since CLM-0017 a `Store` owns a **clone** of the program's symbol table
(`Store::with_symbols`), so that `atom_chars` can intern an atom it builds
at run time. That was harmless while a query ran in one store. CLM-0027
runs every table evaluation in a fresh clone of the world store, and the
clone breaks the invariant the design relied on, that a `Symbol` means
the same name in every store a query touches:

```
:- table mk/1.
mk(A) :- atom_chars(A, <x, y, z>).

?- mk(A).                           panics: index out of bounds (symbol 10 of 10)
?- atom_chars(B, <a, b>), mk(A).    B = ab, A = ab       (should be A = xyz)
```

The evaluation's store interns `xyz` as the next free symbol and records
`mk(xyz)` in the table as that index; the caller's store has either no
such symbol (the panic) or, having interned `ab` first, a different one
(the wrong answer). The second case violates invariant 1 of
`answer-soundness`: the answer is not a consequence of the program.

The same clone is also the quadratic cost measured while weighing semi-naive
consumption: the table is a vector and a hash map of every atom in the
program, copied once per evaluation. A right-recursive chain of n nodes
queried with a ground second argument nests n evaluations, so n copies of
n atoms. Release build, `?- reach(a0, aN).`:

| n     | tabled  | untabled |
|-------|---------|----------|
| 1 000 | 0.06 s  |          |
| 2 000 | 0.20 s  |          |
| 4 000 | 0.66 s  | 0.04 s   |

## Shape of the fix

**One table, shared.** `Program.symbols` and `Store.symbols` become
`Rc<RefCell<Symbols>>`, one allocation per compiled program: the compile
step interns into a plain `Symbols` as today and wraps it once; the world
store is built over the shared handle; `Store::clone` (the query store,
every evaluation store) clones the handle. A symbol interned at run time
in any store is thereby known to all of them, and a table answer carries
indices every consumer resolves to the same name. The REPL recompiles the
session per query, so each program still has its own table, as before.

**API.** `Store::with_symbols(Rc<RefCell<Symbols>>)`; `Store::default()`
makes its own table (tests, `Program::default`). The two intern sites in
the store (`atom_chars` both ways) borrow mutably for the call. The
renderer's `Printer` holds a `Ref<'_, Symbols>` for its life instead of a
`&Symbols`; `render_answer` and `render_terms` lose their `symbols`
parameter and read the store's. Nothing interns while a printer is alive
(printing is read-only), so the borrow cannot conflict. `Program` and
`Store` keep `Clone` and `Debug`; neither is `Send`, and nothing requires
it (the REPL's interrupt is an `AtomicBool` the machine polls).

**Why `Rc<RefCell<_>>` and not a lifetime.** A `Store<'p>` borrowing the
program's table would thread a lifetime through `Solutions`, the machine,
the REPL and every test; the program already shares its tables with the
machine through a `RefCell` for the same reason (`Program::solve` takes
`&self`).

## Action items

1. `Symbols` shared: `Program`, `Store`, `with_symbols`, the intern sites,
   the printer and the two render functions, the machine's call sites.
2. Tests: the two probes above as unit tests (the wrong answer is the
   important one: it must print `A = xyz`); a structural test that the
   world store and a query's store share one table (`Rc::ptr_eq`), so the
   cost cannot quietly return.
3. Docs: `evaluator.md` (the symbol-table sentence under the store; the
   cost sentence under tabling, which also carries the wrong claim that a
   left-recursive closure runs *n* passes — see *Findings*); an addendum to
   the CLM-0027 plan correcting its *Quadratic chains* finding.

## Findings

- **The CLM-0027 cost finding was wrong.** It said a left-recursive closure
  over a chain runs *n* passes and re-consumes every answer on each. It
  runs three: the consumption choice point reads the answer count at retry
  time, so the second pass chains through every answer as it is recorded,
  and the third finds nothing new. Measured (release, `|<X | reach(a0, X)>|`
  over a chain): 0.03 s, 0.03 s, 0.04 s for 1 000, 2 000, 4 000 nodes. The
  right-recursive open call is quadratic because its *n* tables hold *n²/2*
  answers, which is the size of its output (0.39 s, 1.51 s, 6.00 s); the
  right-recursive ground call was quadratic because of this clone. Semi-naive
  consumption would save a constant factor on the confirming pass and is
  not pursued; the journal entry of CLM-0027 stands as written and this
  task's entry records the correction.

## Verification

- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      `cargo test`; existing golden files byte-identical.
- [ ] The two probes: no panic, `A = xyz`.
- [ ] `?- reach(a0, a4000).` tabled within a small factor of untabled.
- [ ] `aivolution lint .` clean; the two `affects` docs current.
