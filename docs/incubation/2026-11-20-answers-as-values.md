# Answers as values, for a program that embeds claimr

Captured 2026-11-20. Prompted by genite linking claimr 0.7.0 as a
library for the first time (genite GEN-141: integrity rules over the
claims a mind holds). Until then every consumer ran the binary and read
`--json`.

What exists. `Answer` holds its bindings as rendered text:
`equations: Vec<String>` with entries such as `A = 'cl-1472…'`, and the
same for `disequations` and `constraints`. A `ProofNode` holds its
`goal` and its `clause` as text too. That is the right shape for
printing, and `--json` is built from it.

What an embedding program needs is the value a query variable was bound
to. A rule `contradiction(Rule, A, B)` answers with two identifiers,
and the caller wants those two identifiers. Today it splits each
equation at ` = ` and undoes the quoting of an atom by hand, which
means it re-implements a piece of the printer in reverse and breaks if
the printer changes.

The idea: an answer also carries its bindings as terms. For each query
variable, its name and a value in a small public term type: atom,
number (the exact `Number`), string, list, compound, and unbound or
constrained. The text fields stay as they are.

Not thought through: what a cyclic term or a variable that is only
constrained looks like in that type, and whether proof nodes want the
same treatment (the facts a derivation rests on, as terms). The first
need is only ground bindings of query variables.
