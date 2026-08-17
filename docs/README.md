# Claimr documentation

The shared documentation workflow (taxonomy, mutation patterns, filename
conventions, frontmatter schemas) is owned by **aivolution-meta** — see
`docs/README.md` in that repo. This file carries only claimr-specific
additions.

## Layout

```
docs/
├── NEXT-TICKET          # next free CLM ticket number (workspace convention)
├── journal/yyyy-mm/     # immutable, write-once event log
├── tasks/yyyy-mm/       # implementation plans, frozen at status: done
├── design/              # ADR-style decision records, flat and dated
├── architecture/        # current-state description + controlled vocabulary
│   ├── README.md        #   component/aspect names (canonical)
│   ├── components/
│   └── aspects/
├── reference/           # durable topical knowledge — e.g. the language grammar
├── inbox/               # transient triage queue (trends toward empty)
├── incubation/          # idea pipeline, stage 1: raw ideas, no status, no ticket
└── catalyst/            # idea pipeline, stage 2: ideas being actively fed
```

## Repo-specific notes

- [`reference/grammar.md`](reference/grammar.md) is the authoritative grammar
  of the Claimr language; the parser in `src/lib.rs` implements it and the
  README quotes an excerpt. Change the grammar first, then the parser.
- Sample programs live outside `docs/`, in `examples/`, so that the test suite
  can parse them.

## Ticket numbers

`NEXT-TICKET` holds the next free `CLM-NNNN` number — claim protocol in the
workspace-level `CLAUDE.md`.
