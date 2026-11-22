# A `--strict` flag: warnings fail the run in batch mode

Captured 2026-11-22. Deferred by the design record
`2026-11-22-undefined-predicates-are-reported.md`, which makes an
undefined call a warning on stderr with the exit code unchanged, and
leaves the decision whether it is an error to the embedding program
through `Program::undefined()`.

The gap: a program that runs the *binary* rather than linking the crate —
aicogito's reasoning adapter, a shell script, a CI job that checks a
rule file — gets the warning as text on stderr and exit 0, and has to
parse stderr to refuse the run. A `--strict` flag (or `-W error`, in the
compilers' spelling) would turn every warning into a load error: the
file-level error document on stderr under `--json`, with a kind of its
own (`undefined-predicate`, class `program`), no query run, exit 1.

Shape, if taken: a flag beside `--limit` and `--steps`; the REPL has no
need of it (it warns at the moment a query depends on the call). The
error object would carry the call sites as data, as the unsatisfiable
facts carry their `conflict` (CLM-0033), so a consumer reads positions
rather than prose.

Open: whether "strict" should cover future warnings too (singleton
variables, unreachable clauses — the other-static-checks note) by one
switch, or whether each warning class gets its own promotion as
compilers do (`-Werror=…`); and whether a rule file wants to declare its
own strictness in the text, as a directive, rather than on the command
line that happens to run it.

Adjacent work to connect with: `docs/design/2026-11-22-undefined-predicates-are-reported.md`,
`docs/design/2026-11-11-json-error-kind-and-class.md` (where a promoted
warning would land as an error kind), aicogito `crates/aicogito-reason`
(the binary consumer), `docs/incubation/2026-11-22-other-static-checks.md`.
