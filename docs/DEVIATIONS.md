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

- Plan: `Doc` stores captured `tree_sitter::Node`s alongside the tree it owns (self-referential; not expressible in safe Rust). Done: captures stored as `(capture index, start byte, end byte, kind id)` and re-resolved against the tree on demand via `descendant_for_byte_range` plus a walk up to the matching range and kind. `has_cap` is a hash-set lookup on that tuple.
- Plan: `returns_of` returns an empty vec plus a warning for non-`tail` languages. Done: it warns once (`log::warn!`) and still returns the tail-branch result — an approximation is more useful than nothing until M2 wires `@return.value` for `return`-statement languages, and no M1 language reaches it.
- Plan: `call_at(value)` where `value` is the `remote` wrapper of `maps:get(..)`. Done: resolved through the `@through`/`@through.inner` captures added in `whe-ngsz`, exposed as `Doc::through`; the engine never names `remote`.
- `destructure` returns `None` on any structural mismatch, per the plan, even after narrowing to a sub-value: a partially matched sub-value would claim precision the pattern does not support.
- Review round 1 added `@construct.cons` to the vocabulary. Ruling: capture it with `(list "|")`. Done: `(list (pipe)) @construct.cons` — `tree-sitter-erlang` wraps the tail in a named `pipe` node, so there is no anonymous `"|"` child of `list` to match. That same wrapper means `[H | T] = [1,2,3]` already fell out as `None` through the kind check; the discriminating case for the added guards is a length mismatch (`[P, Q] = [1,2,3]`), which is what the test asserts.
- `is_literal` is recursive over named children rather than over named *leaves*: a construct is literal when every named child is. Same verdicts on the plan's examples, plus `{[], {}}` (no leaves at all) is literal, and `{ok, 1 + 2}` is not — a computed value is not a place the trace can stop.
- `role_of` returns `Opaque` for any ident under an `@opaque` ancestor, including one in the *body* of an anonymous fun, not only its parameter list; the plan's prose ("an ident inside a fun inside a match RHS is `Use`") describes the ancestor walk not treating the enclosing match as a binding, which it does not.

## Task 6 — Erlang queries

- Plan: `@call.callee` on `maps:get(...)` captures `maps:get`. Done: `tree-sitter-erlang` parses a remote call as `remote(module, fun: call(expr: get, args))`, so `@call.callee` is the bare `get` and `@callee.module`/`@callee.name` are captured from the enclosing `remote`. The engine composes `maps:get` for display (Task 7 `callee_text`); the definition lookup position is the bare name, which is what `erlang_ls` expects.
- Plan: the query test asserts `@call.callee == "maps:get"`. Done: asserts `"get"` plus `@call.args == "(limit, Opts, 10)"`.
- Plan fixture had no anonymous fun. Done: added `_F = fun(X) -> X end,` so `@opaque` is exercised.
- Review fix: vocabulary gained `@through` / `@through.inner` ("classify this node by its inner child"), captured on `remote(fun: call)`, so Task 7 can reach the `@call` inside a remote call without Erlang-specific code. `@return.value` is also captured for `begin … end` and parenthesised bodies. `try … catch` body tails (no `of`) are captured with field-anchored patterns (`exprs: (_) @return.value . catch: …` / `. after: …`); the `of` form captures nothing from the body, which is correct since the body is then a subject, not a tail. `catch_expr` and `maybe_expr` added to `@opaque`.
