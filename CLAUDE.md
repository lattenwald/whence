# whence

Variable-provenance tool: Rust engine driven by editor plugins (Neovim first,
VS Code later). The design is in
`docs/superpowers/specs/2026-09-01-whence-design.md`; the motivation in
`docs/INTENT.md`. When code and spec disagree, update the spec in the same
change — it is the record of decisions, not a proposal.

## Non-negotiables from the design

- The engine never spawns language servers and never contains editor-specific
  code. The editor answers `host/*` requests; the engine does everything else.
  Anything that needs the editor goes through that protocol, nothing bypasses it.
- No per-language Rust code. Language support is data under `languages/<lang>/`
  (grammar + queries + `lang.toml`). A construct the query vocabulary cannot
  express is a reason to extend the vocabulary, not to special-case a language.
- Honesty over plausibility: an edge is emitted only when it points at a
  specific syntax node the engine actually identified. When there are several
  candidates, emit all of them or stop with `unresolved` — never pick one.
  A wrong edge is worse than a missing one.

## Working here

- Tickets live in `.tickets/` (`tk` CLI). Pick work from `tk ready`; put
  the ticket ID in the commit message as `[<id>]`.
- Fixtures for trace tests are recorded from real editor sessions
  (`:WhenceRecord`), not written by hand — goldens should reflect what the
  language servers actually return.
- Deliver the requested scope; make routine judgment calls yourself, and voice
  a disagreement in a sentence rather than acting on it. Keep responses short.
- `docs/DEVIATIONS.md` is **not** a changelog, and nothing else may enter it.
  Every entry is one departure from the plan being executed, written as *Plan:
  what it said. Done: what was built instead, and why.* If you cannot quote what
  the plan said, it is not a deviation — do not write it there. Work with no
  plan produces no entries. Never record what a commit already tells (features
  added, files touched, tests written, spec or ticket edits), and never append a
  per-milestone summary of the work. Prune, never accumulate: an entry that has
  stopped being true is deleted, not annotated.

## Commands

- `make test` — engine tests.
- `make test-nvim` — plugin tests; needs `cargo build` first and plenary
  (`PLENARY_DIR=…` if it is not in the lazy.nvim default location).
- `whence replay <fixture-dir> <file:line:col>` — run a trace against a replay
  fixture; the fastest way to debug one.
- `UPDATE_EXPECTED=1 cargo test --test replay` — regenerate the goldens.
  Inspect the diff before committing: a golden is a claim about correctness.
