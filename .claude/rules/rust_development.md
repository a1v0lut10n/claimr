---
paths:
  - "**/*.rs"
---

<!-- Projected from Aivolution SWE's Rust development mindset (mastermind: aivolution-mastermind). Edit it in the mentarium, not here — a re-projection replaces it. -->

# Rust mindset — the gates

You work through the Rust mindset of Aivolution SWE. These are gates, not
preferences: a change that fails one is not ready for a change proposal.

## Before any change proposal

Run, in the repository root, and fix until all three are clean:

    cargo fmt --all -- --check
    cargo clippy --workspace -- -D warnings
    cargo test --workspace

Where the repository has a Makefile, `make lint` and `make test` are the
same gates — CI reruns them; a proposal that needs CI to find a clippy
warning was not ready. Do not add `#[allow(...)]` to pass a lint; fix
the code, or say in the proposal why the lint is wrong here.

## Always

- Errors: the crate's error type and `?`; no `unwrap`/`expect` outside
  tests; never swallow an error silently — report it to the user or log
  it with `tracing` under the subsystem's `target:`.
- Never `unsafe impl Send`/`Sync` to share a `!Send` resource. Give the
  resource to one named worker thread that owns it and serve requests
  over a channel. No `unsafe` without a `// SAFETY:` invariant.
- Blocking work — file I/O beyond a small config file, subprocesses,
  FFI, network, indexing — never on a UI thread or inside a frame.
- Durable instants are UTC with a `_utc` suffix; durations are `_ms`
  from a monotonic clock; never persist a monotonic reading.
- Tests run against real artefacts (a temp directory, a bare git
  remote and a clone), not mocks of them.
- Add a dependency only when the standard library and the crates already
  in the workspace cannot do it; say why in the proposal.

## Before starting non-trivial work

A plan under `docs/implementation/` with a ticket from `docs/NEXT-TICKET`
(incremented in the first commit on the branch), one reviewable branch,
and the plan updated to what was actually built before the proposal.
