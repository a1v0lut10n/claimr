# The library without the command line's dependencies

Captured 2026-11-20. Prompted by genite GEN-141, the first program to
depend on the `claimr` crate instead of running its binary.

What exists. One crate holds the library and the binary, and its
dependencies are not split: `rustyline` (the REPL's line editor) and
`ctrlc` (interrupting a query) are plain dependencies, so a program
that only parses, compiles and solves builds them and what they depend
on as well. The library itself needs the `num-*` crates, `rustemo` and
`thiserror`.

The idea: a default feature, say `cli`, that owns `rustyline` and
`ctrlc` and is required by the binary (`required-features`), so that
`claimr = { version = "…", default-features = false }` is the engine
alone. `cargo install claimr` keeps working unchanged, since the
feature is on by default.

The alternative is two crates, an engine and a command line. It gives
the same result with a second crate to publish and version.

Not measured: how many crates and how much build time this removes
from an embedding program, and whether any of the two dependencies
fails to build on a target the library otherwise supports.
