# Deviations from the M1 plan

- Plan: [2026-09-01-m1-erlang-neovim.md](superpowers/plans/2026-09-01-m1-erlang-neovim.md)
- Spec: [2026-09-01-whence-design.md](superpowers/specs/2026-09-01-whence-design.md)

Each entry: which task, what the plan said, what was done instead, why. Entries marked *planned* were ruled before the task ran and are confirmed or amended when it lands.

## Task 1 — workspace skeleton

- Plan: `.gitignore` lists `target/`, `nvim/bin/`, `*.log`. Done: also `.superpowers/` (the SDD workspace is scratch, not source).
- Plan: `tempfile` added to dev-dependencies in Task 3. Done: added in Task 1 with the rest of the manifest, so Task 3 touches no manifest.
- Plan: `make test-nvim` assumes plenary is on the packpath. Done: `PLENARY_DIR ?= ~/.local/share/nvim/lazy/plenary.nvim` is exported to `nvim`; `nvim/tests/minimal_init.lua` (Task 10) must read `PLENARY_DIR` and prepend it to the runtimepath.

## Task 3 — host seam

- *Planned.* Plan: `RpcHost::new(writer, inbox: Receiver<Message>, ...)` (test passes `rx` by value). Ruling: `inbox: &mut Receiver<Message>` so the server loop (Task 9) keeps the receiver after a trace; test passes `&mut rx`.

## Task 7 — syntax

- *Planned.* Plan: `Doc` stores captured `tree_sitter::Node`s alongside the tree it owns (self-referential; not expressible in safe Rust). Ruling: captures stored as byte ranges + kind id and re-resolved against the tree on demand.

## Task 6 — Erlang queries

- Plan: `@call.callee` on `maps:get(...)` captures `maps:get`. Done: `tree-sitter-erlang` parses a remote call as `remote(module, fun: call(expr: get, args))`, so `@call.callee` is the bare `get` and `@callee.module`/`@callee.name` are captured from the enclosing `remote`. The engine composes `maps:get` for display (Task 7 `callee_text`); the definition lookup position is the bare name, which is what `erlang_ls` expects.
- Plan: the query test asserts `@call.callee == "maps:get"`. Done: asserts `"get"` plus `@call.args == "(limit, Opts, 10)"`.
- Plan fixture had no anonymous fun. Done: added `_F = fun(X) -> X end,` so `@opaque` is exercised.
- Review fix: vocabulary gained `@through` / `@through.inner` ("classify this node by its inner child"), captured on `remote(fun: call)`, so Task 7 can reach the `@call` inside a remote call without Erlang-specific code. `@return.value` is also captured for `begin … end` and parenthesised bodies. `try … catch` without `of` keeps its body tails uncaptured (a `try_expr` container with no tails → `unresolved`), because the grammar puts body, catch and after expressions as siblings and the tail anchor cannot single out the body's last expression; revisit when a fixture needs it. `catch_expr` and `maybe_expr` added to `@opaque`.
