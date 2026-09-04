# Deviations

Where the work departed from the plan it was executing, milestone by milestone. Each entry: which task, what the plan said, what was done instead, why. Entries marked *planned* were ruled before the task ran and are confirmed or amended when it lands.

## M1 — Erlang and Neovim

- Plan: [2026-09-01-m1-erlang-neovim.md](superpowers/plans/2026-09-01-m1-erlang-neovim.md)
- Spec: [2026-09-01-whence-design.md](superpowers/specs/2026-09-01-whence-design.md)

### Task 1 — workspace skeleton

- Plan: `.gitignore` lists `target/`, `nvim/bin/`, `*.log`. Done: also `.superpowers/` (the SDD workspace is scratch, not source).
- Plan: `tempfile` added to dev-dependencies in Task 3. Done: added in Task 1 with the rest of the manifest, so Task 3 touches no manifest.
- Plan: `make test-nvim` assumes plenary is on the packpath. Done: `PLENARY_DIR ?= ~/.local/share/nvim/lazy/plenary.nvim` is exported to `nvim`; `nvim/tests/minimal_init.lua` (Task 10) must read `PLENARY_DIR` and prepend it to the runtimepath.

### Task 3 — host seam

- *Planned.* Plan: `RpcHost::new(writer, inbox: Receiver<Message>, ...)` (test passes `rx` by value). Ruling: `inbox: &mut Receiver<Message>` so the server loop (Task 9) keeps the receiver after a trace; test passes `&mut rx`.

### Task 7 — syntax

- Plan: `Doc` stores captured `tree_sitter::Node`s alongside the tree it owns (self-referential; not expressible in safe Rust). Done: captures stored as `(capture index, start byte, end byte, kind id)` and re-resolved against the tree on demand via `descendant_for_byte_range` plus a walk up to the matching range and kind. `has_cap` is a hash-set lookup on that tuple.
- Plan: `returns_of` returns an empty vec plus a warning for non-`tail` languages. Done: it warns once (`log::warn!`) and still returns the tail-branch result — an approximation is more useful than nothing until M2 wires `@return.value` for `return`-statement languages, and no M1 language reaches it. *Superseded after M1 (2026-09-02):* the `returns` quirk is gone; return roots are the `@return` capture (spec §6), so a statement language needs a query line, not engine code.
- Plan: `call_at(value)` where `value` is the `remote` wrapper of `maps:get(..)`. Done: resolved through the `@through`/`@through.inner` captures added in `whe-ngsz`, exposed as `Doc::through`; the engine never names `remote`.
- `destructure` returns `None` on any structural mismatch, per the plan, even after narrowing to a sub-value: a partially matched sub-value would claim precision the pattern does not support.
- Review round 1 added `@construct.cons` to the vocabulary. Ruling: capture it with `(list "|")`. Done: `(list (pipe)) @construct.cons` — `tree-sitter-erlang` wraps the tail in a named `pipe` node, so there is no anonymous `"|"` child of `list` to match. That same wrapper means `[H | T] = [1,2,3]` already fell out as `None` through the kind check; the discriminating case for the added guards is a length mismatch (`[P, Q] = [1,2,3]`), which is what the test asserts.
- `is_literal` is recursive over named children rather than over named *leaves*: a construct is literal when every named child is. Same verdicts on the plan's examples, plus `{[], {}}` (no leaves at all) is literal, and `{ok, 1 + 2}` is not — a computed value is not a place the trace can stop.
- `role_of` returns `Opaque` for any ident under an `@opaque` ancestor, including one in the *body* of an anonymous fun, not only its parameter list; the plan's prose ("an ident inside a fun inside a match RHS is `Use`") describes the ancestor walk not treating the enclosing match as a binding, which it does not.

### Task 6 — Erlang queries

- Plan: `@call.callee` on `maps:get(...)` captures `maps:get`. Done: `tree-sitter-erlang` parses a remote call as `remote(module, fun: call(expr: get, args))`, so `@call.callee` is the bare `get` and `@callee.module`/`@callee.name` are captured from the enclosing `remote`. The engine composes `maps:get` for display (Task 7 `callee_text`); the definition lookup position is the bare name, which is what `erlang_ls` expects.
- Plan: the query test asserts `@call.callee == "maps:get"`. Done: asserts `"get"` plus `@call.args == "(limit, Opts, 10)"`.
- Plan fixture had no anonymous fun. Done: added `_F = fun(X) -> X end,` so `@opaque` is exercised.
- Review fix: vocabulary gained `@through` / `@through.inner` ("classify this node by its inner child"), captured on `remote(fun: call)`, so Task 7 can reach the `@call` inside a remote call without Erlang-specific code. `@return.value` is also captured for `begin … end` and parenthesised bodies. `try … catch` body tails (no `of`) are captured with field-anchored patterns (`exprs: (_) @return.value . catch: …` / `. after: …`); the `of` form captures nothing from the body, which is correct since the body is then a subject, not a tail. `catch_expr` and `maybe_expr` added to `@opaque`.

### Task 8 — trace core

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

### Task 10 — Neovim engine and host

- Plan: host errors (timeout, failure) are reported as JSON-RPC error `-32000`. Done: `-32603` (InternalError). `vim.lsp.rpc`'s `server_request` path asserts the error code is a member of `vim.lsp.protocol.ErrorCodes`; with `-32000` the assertion fires inside the scheduled coroutine and no response is ever sent, so the engine blocks forever on that request. The engine only uses the message text, so `HostError::Rpc` is unaffected.
- Tests: LSP-backed paths of `host.lua` (encoding conversion, LocationLink flattening, multi-client merge) are not covered headlessly (no live server); `host/text`, the no-client case and dispatch are. Covered by the live dogfood in the M1 exit check instead. *Round 1 amendment:* the pure halves of those paths are now covered — `M._from_utf16`/`M._to_utf16` (ASCII, BMP, astral × utf-8/utf-32, both directions) and `M._locations_from` (`Location`, `Location[]`, `LocationLink[]`, cross-client de-dup) are exported for that purpose. What remains uncovered is only the `buf_request_sync` call itself.
- Review round 1 (ruling): when several clients answer one host request, a client's error is dropped with a `vim.notify(..., WARN)` naming it as long as another client answered; only an all-client failure is reported to the engine (which then aborts the trace, spec §4). A single server that dislikes a position must not sink a trace another server can answer.
- Review round 1: `vim.lsp.rpc` never fires pending request callbacks when the engine process dies, so a trace in flight hung with no feedback. `engine.lua` keeps its own pending-callback map keyed by request id and, on `on_exit`, notifies and fails every pending trace with `{ code = -32000, message = "engine exited" }` — this error never reaches `vim.lsp.rpc`, so the engine's own `E_HOST` code applies here. `M.start` therefore returns a small wrapper (`request`/`notify`/`is_closing`/`terminate`) rather than the raw rpc client.
- Review round 1: `bufnr_for` unlists only buffers it created (`vim.fn.bufexists` checked before `bufadd`); tracing through a file the user already has open must not drop it from `:ls`.

### Task 12 — Neovim recorder and installer

- Plan: the recorder wraps `host.handle`, and `_replay` (Task 10) "routes answers through `host.handle`". It did not — `init.lua` passed the replay handler to `engine.start` as `opts.handle`, bypassing `host.handle` entirely, so a recording made under `_replay` would have captured nothing. Done: `setup({_replay=dir})` installs the replay handler *as* `host.handle`, and `engine.lua` resolves `require("whence.host").handle` per request instead of once at `start`, so a recorder installed while an engine is already running is seen. `host.lua` itself is untouched.
- Plan: `:WhenceRecord` prints the replay command. Done: it also writes `dir/whence-record.json` (`root`, `file`, `line`, `col`, `engine_version`, `recorded_at`, `conflicts`) so the recording can be replayed later without the printed hint. `whence trace`/`trace_at` gained an optional `on_done` callback (fired on every error path and, on success, *before* `panel.show`, so a render error cannot strand the wrapper) and `whence.root(file)` is exported for the command.
- Review round 1: the recorded position is the cursor, passed explicitly as `record.begin(dir, root, target)`. Inferring it from the first positional host request yields the identifier's *start*, because the engine normalises the cursor before asking — a fact about the engine, not about what the user recorded. The inference remains only as a fallback when no target is supplied (`record.begin(dir, root)`).
- Review round 1: the archive checksum is computed in process — `vim.fn.sha256(vim.fn.readfile(path, "B"))` — not by shelling out. `sha256sum` does not exist on Windows (which is in the target table) and is spelled `shasum -a 256` on macOS. Verified byte-exact against `sha256sum` on the 52 MB engine binary.
- Review round 1: a repeated request answered differently is not silently overwritten. The first answer is kept, the key is appended to `conflicts` in `whence-record.json`, and `finish()` warns: the fixture then cannot reproduce the session it was recorded from, and a golden built from it would be fiction.
- `record.begin` refuses a non-empty directory: `host.json` is rewritten wholesale but source files copied by an earlier recording would survive into the new fixture.

### Whole-branch review — honesty pass

Rulings from the M1 branch review; each landed with the spec section it changes.

- Spec §5.2 field row: `field_source` kept only the *last* earlier construction of the
  container, so `case K of 1 -> R = #r{a = one}; 2 -> R = #r{a = two} end, R#r.a` showed
  `two` alone — a wrong edge, not a missing one. Done: all matching constructions become
  children of the one `field` node (`via: field_set`, source order, fan-out bound);
  exactly one is the old shape, zero keeps the container-trace fallback (`via: field`).
  Fixture `honesty_field_branches/`.
- Spec §5.2 parameter row: `entry_point` was reported whenever no *call site* was found,
  including when the server did return references (a `fun cb/1` passed to `lists:map`, an
  `-export` entry, a reference in a file with no grammar). "Nobody calls this" and "the
  callers are not shaped like calls" are different answers. Done: references are counted;
  with none (or only the declaration, detected as a `@function.name` capture at the
  reference) the stop stays `entry_point`, otherwise it is
  `unresolved: <N> reference(s) to <name>/<arity> are not call sites` carrying one
  jumpable `unresolved: reference is not a call site` child per in-root reference,
  fan-out bounded. Fixture `honesty_callback_ref/`. This is the one place a `stop` node
  has children (spec §5.1).
- Spec §5.2 variable-use and call rows: `pick_definition` picked the same-file definition,
  and a call was `external` if *any* definition was outside the root. Done: definitions are
  de-duplicated by (file, range); for a variable, more than one distinct definition is
  `unresolved: <N> definitions`; for a callee, all-outside is `external`, mixed is
  `unresolved: <N> definitions, some outside root`, and all-inside collects the clauses of
  every definition into one `call_result`. Fixture `honesty_multi_def/`.
- Spec §5.1 and §5.2 (new row): a binding whose value is a `@return.container`
  (`case`/`if`/`try`/`begin`/parens) stopped with `unresolved: case_expr`, which is where
  most real Erlang provenance ends. Done: `NodeKind::Branch` (serde `"branch"`), label =
  the container's first line clipped to 40 chars, `via: match` from the binding, one child
  per tail — also `via: match`, since each tail is what the value matches to. `syntax.rs`
  exposes the existing `expand_return` walk as `Doc::tails_of`. `render.rs` and
  `nvim/lua/whence/panel.lua` needed no change: both treat kinds generically and only test
  for `"stop"` (covered by a panel test). Fixture `honesty_case_rhs/`.
- A parameter bound through a destructuring pattern (`read_body(#req{body = B}) -> B`)
  received the whole call argument. Done: `destructure(param_pattern, ident, arg)` narrows
  it, mirroring `BoundBy`, on both the frame and the references path. Limitation:
  `Doc::destructure` resolves captures in one document, so an argument from another file
  keeps its whole span. Fixture `honesty_param_destructure/`.
- Spec §5.1: `via` describes the edge from the parent. A `param` reached through a binding
  or branch pattern was left at the constructor's `via: arg`; it is now `via: match`.
  `via: arg` remains for the frame and call-site rows. Asserted in `local_chain`.
- The wall-clock check was `now() > deadline`, so `time_ms = 0` depended on the clock
  ticking. Now `>=`: a zero budget stops at the first expansion, which is what the new
  replay test asserts.
- `RpcHost` treats a JSON `null` result for the `Location[]`/`Highlight[]` methods as an
  empty list. LSP allows `null` there and a VS Code host passes it through; deserialising
  it as an error would abort the whole trace over a normal "nothing found".
- Dead surface removed: `syntax::arg_index` (and its assertion, replaced by one on the
  argument text), `vocab::BRANCH` (the `@branch` capture stays in `whence.scm` — language
  data may describe more than the engine reads), and `engine.lua`'s `opts.handle`
  injection, which nothing has passed since Task 12 routed the recorder through
  `host.handle`. `pos::to_point`/`from_point` and `RpcHost::new` are used only by tests
  and are `#[cfg(test)]`-gated rather than deleted.
- Spec §5.2 variable-use row: the `documentHighlight` rebinding pass is marked as M2, not
  as M1 behaviour, and `nvim/lua/whence/engine.lua` declares
  `capabilities.documentHighlight = false` — the engine never sends the request in M1, so
  declaring support was misleading.

## M2 — Rust and Go

- Plan: [2026-09-03-m2-rust-go.md](superpowers/plans/2026-09-03-m2-rust-go.md)

### Erlang `@function.group` node

Plan: Erlang marks `(fun_decl) @function.group`, and `Doc::clauses_of(group)` returns every
clause sharing that group. Done: tree-sitter-erlang 0.20 gives each clause its own `fun_decl`,
so that capture separated the clauses it was meant to join and broke frame matching; Erlang
marks `(source_file) @function.group` and `clauses_of(group, name, arity)` keeps the name and
arity filter that the group alone cannot supply. A language whose functions are one node still
groups each on itself, so name and arity never merge distinct functions there.

### A constant Erlang map is now a literal

Plan: the Erlang goldens stay byte-identical through Task 2. Done: `live_callers` records
`#{limit => 5}` as `stop: literal` instead of `unresolved: constructed value map_expr`. The
keyed-construct rule of spec §6 — skip `@construct.field.name`, check each entry's
`@construct.field.value` — cannot tell an Erlang `map_field` from a Rust `field_initializer`,
so making a struct of constants a literal makes a map of constants one too, which is the more
honest answer. Node ids are unchanged.

### The receiver shift applies to call sites found through references

Plan: in the references loop of `param_like`, `Slot::Arg(i)` takes `call.args.get(i)`; the
occurrence step's mutable-parameter rule likewise reads `param_is_mutable(i)`. Done: both apply
the same shift `call_result` does — a call with no `@call.receiver` to a declaration with a
receiver and one argument too many passes the receiver first — so an argument is not matched to
the slot one place to its left (spec §3.4), and `T::m(&mut x)` is a write to `x`. The rule lives
in one helper every call site uses.

### A write is classified by its place chain, not by containment

Plan: an occurrence is a write when it is "inside the `@assign.target` of an `@assign` node".
Done: the target is taken through `@through`, narrowed to the element containing the occurrence
when it is a positional construct, and the occurrence must lie on that place's chain of
containers — `@field.container` or the new `@place.base` capture. Containment alone marks the `i`
of `arr[i] = 3` and the key of `m[k] = v` as mutated, and they are only read; a wrong edge is
worse than a missing one. Spec §3.1 rule 1 and §6 carry the rule and the capture.

### Two rules the Rust query exposed

Plan: Task 6 touches no engine source but `engine/src/lang/mod.rs`. Done: two rules in
`engine/src/syntax.rs` that only Rust's shapes can reach. `role_of` numbers a `Role::Param` by its
position in `FnDecl::params` rather than among the named children of `@function.params` — Rust puts
`self_parameter` inside `(parameters)`, so the receiver, which `FnDecl` excludes, shifted every
later parameter onto the wrong argument. `expand_return` pushes a leaf through `@through`, as §2
requires of every classified value — a `return e` block tail is `@return` twice, once as the tail
and once as the operand, and without it the tree grew a second leaf for the same value. Neither
changes anything for Erlang.

### Implementations are expanded before the outside-root verdict

Plan: `call_result` expands an abstract callee inside the loop that builds targets, "then run the
existing outside-root / not-a-function / `FuncId` logic over `here`" — a loop that only runs after
the `external` / "some outside root" checks over the definitions. Done: the expansion is a pass
over the definitions *before* those checks, and they see the implementations. Spec §3.3 requires an
implementation outside the root to count toward `external`; inside the target loop it could not,
because the verdict was already taken on the abstract declaration alone.

## M3 — VS Code extension

- Plan: [2026-09-03-m3-vscode.md](superpowers/plans/2026-09-03-m3-vscode.md)
- Spec: [2026-09-03-vscode-extension-design.md](superpowers/specs/2026-09-03-vscode-extension-design.md)

### Task 1 — scaffold and toolchain

- Plan: `lint` is `eslint src test scripts`. Done: `eslint .` — eslint 10 fails with "No files matching the pattern" until `scripts/` exists (Task 6).
- `.gitignore` also ignores `vscode/.vscode-test/`, the VS Code build the test runner downloads.
- Plan: TypeScript 5. Done: `npm install` resolved TypeScript 6 and eslint 10; both compile and lint the sources clean.

### Task 2 — engine client and replay host

- `vscode/eslint.config.mjs` gained an `ignores` entry for `.vscode-test/`, `out/` and `dist/`. `eslint .` does not read `.gitignore`, so once the test runner had downloaded VS Code it tried to lint the bundled extensions' own configs and died on their missing dev dependencies.

### Task 3 — host answers from VS Code providers

- `vscode/test/host.test.ts`: the mixed `Location` / `LocationLink` array returned by the stub definition provider is cast `as unknown as vscode.LocationLink[]`. `ProviderResult<Definition | LocationLink[]>` admits no mixed array, so the plan's literal did not typecheck; the cast keeps the test exercising both shapes in one provider result.

### Task 4 — tree view, decorations, commands

- `vscode/test/tree.test.ts`, "re-runs from a node": the plan re-runs from the first child of the root, whose own location is `a.erl:4:4`. The `local_chain` fixture records host answers only for the trace at `6:4`, so that re-run failed with `unrecorded` and left the tree unchanged. Done: the item is the root node with its location set to `6:4`, which still exercises `whence.rerunFromNode` replacing the result through the command path.

### Review of Tasks 2 and 3

- Plan: the engine settles on the child's `exit`. Done: on `close` — a spawn that never starts (missing binary, EACCES) emits `error` and `close` but no `exit`, so `exited` never settled and `onExit` never fired.
- Plan: `replayHost` starts `loadFixture` and awaits it per request. Done: a no-op `catch` is attached to it as well; a missing or invalid `host.json` rejected before the first `await` and surfaced as an unhandled rejection.

### Task 5 — recorder

- Plan: the replayed and live roots are compared with a plain `assert.deepEqual`. Node ids do hash root-relative paths, but `loc.file` is absolute, so every node differed by its root (the fixture directory vs. the temporary recording directory). Done: both trees are compared with their own root prefix stripped.

### Review of Tasks 4 and 5

- Plan: `engineFor` inserts the engine into the per-root map before the handshake and only removes it on exit, so one failed `initialize` on a still-live process made every later trace in that root fail with `not initialized`. Done: a failed handshake removes and kills it.
- Plan: `whence.preview` and `whence.open` are registered without an error path. Done: they route rejections through `report` like every other command; a click on a tree item whose file had moved did nothing at all.
- Plan: `record.begin` checks the "already recording" flag, then awaits `mkdir`/`readdir`. Two overlapping recordings both passed the check, nested their host wrappers, and `finish` wrote the wrong one and left a wrapper installed for the rest of the session. Done: `begin` claims the flag before its first `await`, and the test pins the guard that actually fires.
- Plan: the preview/open test asserts `activeTextEditor` after each command, which the headless test host resolves to the most recently changed input either way, so it could not fail. Done: it asserts the tab's `isPreview`. The engine-death test is renamed to what it does — stopping the engines and tracing again; killing the process is not reachable through `WhenceApi`.

### Cleanup pass over the plan's code

- Plan: every `Item` carries `root`. Done: `Item` is `{ node }` and `getTreeItem` reads the root from the one result, so `getParent` no longer invents a `?? ""` fallback that could never be right.
- Plan: `record.ts` declares its own method→section table and `{ file, line, col }` literals. Done: `SECTION`/`Sections` come from `hostReplay.ts` and `Loc` from `types.ts`.
- Plan: `host.text` reads the open document else `workspace.fs.readFile` + `TextDecoder`. Done: `openTextDocument`, which is that logic, and which the file's other answers already use.
- Plan: `Decorations.apply` recomputes every node's word range in every visible editor on each tree selection. Done: `set` buckets nodes by file once, and `select` repaints only the strong layer of the two editors involved.
- Plan: `traceAt` awaits `tree.show` before setting decorations. Done: decorations first; nothing in them depends on the reveal.
- Plan: the CI job spells out `cargo build`, `npm ci && npm run lint`, `xvfb-run -a npm test`. Done: `make vscode-deps` and `make test-vscode`, like the Neovim job.
- Plan: `Engine.trace` clears its single-flight slot in a `.finally` chained on the inner request. Done: an `async` `finally`, so a caller awaiting one trace before starting the next cannot be told `busy` by microtask ordering.
- Plan: liveness is `child.exitCode !== null`, which stays `null` when the spawn never started. Done: the engine records its own `close` code, and a `stopping` flag set by `kill`/`dispose` keeps a deliberate exit (SIGKILL closes with `null`) from being reported as a crash.
- Plan: `onExit` deletes the root's entry unconditionally. Done: only when the dying engine is still the mapped one, or a retry during its `close` orphans the live process past `stopEngines`.
- Plan: `recordAt` calls `record.begin` first. Done: it refuses while a trace runs — wrapping the host first captured the other trace's answers and left a fixture in the directory the user picked, which `begin` then refuses to reuse.
- Plan: `copySource` overwrites the copied file. Done: a second, different `host/text` answer for one file is a conflict; the tree was built from the first text. `nvim/lua/whence/record.lua` has the same gap (`whe-zcjm`).
- Plan: the `vsix` job lists its targets as a bare sequence and `targets.test.ts` scans for `- target:`. That regex saw only the build job, so a platform added there and in `TARGETS` would ship with no VSIX. Done: both matrices use `- target:` and the test requires each triple in both.
- Plan: `request` rethrows anything that is not a `ResponseError`. A process that dies mid-request fails the stdin write before `close` arrives, so the caller saw Node's "Cannot call write after a stream was destroyed" — green locally, red on the slower CI runner. Done: a write that fails while the engine is closed, closing, or no longer writable becomes `EngineError(E_HOST, "engine exited …")`.
