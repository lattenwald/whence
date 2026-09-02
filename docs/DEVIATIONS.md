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

## Task 8 — trace core

- Plan: `Expr::Value(PathBuf, Pos)`. Done: `Expr::Value(PathBuf, Span)` where `Span` is `(start byte, end byte, kind id)` — a position does not name a node (`pick` and `pick(3)` start at the same place), and Task 7's `Doc` re-resolves nodes from exactly that triple. Frame arguments are `(PathBuf, Span)` for the same reason; the visited set stays keyed on `(file, Pos, frame hash)` as planned.
- Plan/spec: "zero call sites → `stop: entry_point`". Done: the `param` node is emitted with the `entry_point` stop as its only child — the spec table's column is *Children*, and the parameter's own location is worth showing. Same shape as the plan's expected tree for `local_chain`.
- `via` is set when a node is built, from its kind: `binding` → `match`, `param` → `arg`, `call_result` → `return`, `field` → `field_set`, stops → `null`. It reads as "how the value above is fed by this node", so the root carries one too.
- Matching a reference to a call site compares `callee_text` to the function name exactly or after a `mod:` prefix, not a plain `ends_with`: `flag` ends with `g`.
- Resolving a parameter through the frame pops that frame while expanding the argument. The argument is the *caller's* expression, so its own parameters must resolve against the caller's frame (spec §5.3); without the pop a two-deep call chain resolves the outer parameter in the inner callee's frame.
- `node_count` increments once per `expand` call, before the budget checks (bound is `> limits.nodes`), so `limit: nodes` trips on the way down. Counting at node *construction* never trips: children are built before their parent.
- Enumerating a file's function clauses and locating a clause's `@function.name` are done by walking the tree and testing `has_cap`; `Doc` exposes neither. Both are vocabulary-only and belong in `syntax.rs` if a second caller appears.
- A reference outside the language registry (a hit in a file with no grammar) is skipped rather than failing the trace.
- `stats.host_requests` counts `host/text` alongside `definition`/`references`: it is the count of requests the editor answered.
- Review round 1 (three rulings, spec §5.4 updated in the same commit):
  node ids, `func_id` and the frame hash are built from root-relative paths
  (`Node::stop` takes the root-relative id path as its first argument; the absolute-path variant was removed in round 2),
  so a trace is identical at any checkout path;
  the cycle set holds only the *current expansion path* (`(file, Span, frame hash)`,
  inserted on entry and removed on return) so diamonds — two clauses returning the same
  parameter, two case branches sharing a subject — are expanded rather than falsely called
  recursion;
  a call whose `func_id` *and* argument spans already sit on the frame stack is not entered
  again (`unresolved: recursive call to <name>/<arity>`), which is what actually terminates
  `loop(S) -> loop(S).` and accumulator recursion — the frame hash grows with every push, so
  the path cut alone never fired for them.
- Review round 2 (ruling, spec §5.1 and the §5.2 field row updated in the same commit): a
  field access whose container has no visible construction no longer stops. It emits a
  `field` node labelled `f of C` `via: field` whose one child is the trace of the container
  itself, also `via: field` (new `Via::Field`, serde `"field"`). In Erlang a record almost
  always arrives as a parameter or a call result, so the old `stop: unresolved: field f of C`
  ended nearly every real field trace at the first hop. The engine still never picks the
  field out of whatever the container resolves to — the container's trace simply continues
  and ends where it ends.
- Not implemented, deliberately: the `documentHighlight` rebinding pass of §5.2. Erlang is single-assignment, so it would emit nothing; `Via::Rebind`/`Mutation` wait for M2.

## Task 10 — Neovim engine and host

- Plan: host errors (timeout, failure) are reported as JSON-RPC error `-32000`. Done: `-32603` (InternalError). `vim.lsp.rpc`'s `server_request` path asserts the error code is a member of `vim.lsp.protocol.ErrorCodes`; with `-32000` the assertion fires inside the scheduled coroutine and no response is ever sent, so the engine blocks forever on that request. The engine only uses the message text, so `HostError::Rpc` is unaffected.
- Tests: LSP-backed paths of `host.lua` (encoding conversion, LocationLink flattening, multi-client merge) are not covered headlessly (no live server); `host/text`, the no-client case and dispatch are. Covered by the live dogfood in the M1 exit check instead.
