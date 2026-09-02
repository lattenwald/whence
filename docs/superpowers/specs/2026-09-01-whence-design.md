# Whence — design

- Status: draft, awaiting review
- Created: 2026-09-01
- Supersedes: nothing; refines [INTENT.md](../../INTENT.md)

## 1. Summary

`whence` answers "where does this value come from?" for a variable under the
cursor, as a bounded tree of the assignments, rebindings, mutations, call-site
arguments and callee returns that feed it, ending at explicit *stop* nodes
(external code, entry point, literal, or "couldn't follow"). It targets Erlang,
Rust and Go, is used from Neovim first and VS Code second, and is built as a
language-agnostic Rust engine driven by the editor.

## 2. Decisions made during design

| Question | Decision | Why |
|---|---|---|
| Who answers definition/references? | The editor's already-running language servers, via the host plugin. The engine never spawns servers. | Both target editors (Neovim, VS Code) can answer these; avoids duplicate servers, cold indexing and per-project LSP config. |
| Standalone CLI? | Dropped as a product. `whence replay <fixture>` remains for tests and debugging only. | All users are in Neovim or VS Code. |
| Engine language | Rust, single static binary. | tree-sitter bindings, distribution as one file. |
| Interaction model | One-shot bounded tree per invocation (depth generous, fan-out bounded). Lazy expansion later. | Matches INTENT; node IDs kept stable so expansion can be added. |
| Boundary rules in v1 | literal, external (outside root), entry point (no callers), unresolved. Curated "input source" lists later. | Mechanical rules first; the curated list is data added later. |
| Call results | Descend into the callee's return expressions, context-sensitively. | Answers the actual question; fan-out bound prunes growth. |
| Per-language code | None. Per-language *data*: tree-sitter grammar + query files mapping node kinds to a fixed capture vocabulary. | Grammars name nodes differently; LSP exposes no syntax; a mapping is the irreducible minimum. |
| Grammar/query delivery | Erlang, Rust, Go compiled into the binary. Runtime `.so` loading + download registry kept as the path for other languages, not in M1–M3. | Zero network, zero C compiler, no version drift for the common case. |
| Binary delivery | Prebuilt GitHub Releases; Neovim plugin fetches on first use; VS Code platform-specific VSIX. | One plugin install per editor, no configuration. |
| Milestones | M1 Erlang + Neovim; M2 Rust + Go; M3 VS Code. | Erlang is the simplest provenance model and the motivating case. |

## 3. Architecture

```
whence/
  engine/      Rust crate → binary `whence`. Protocol, trace algorithm,
               query loader, bundled grammars/queries. Editor-agnostic.
  nvim/        Lua plugin: spawns engine, answers host requests with
               vim.lsp, renders panel, jumps.                      (M1)
  vscode/      TS extension: same role via vscode.execute*Provider. (M3)
  languages/   Per-language query data, embedded into the engine at
               build time (see §6).
  docs/        INTENT.md, PROTOCOL.md, this spec.
```

- The **engine** is a subprocess spawned by the editor, one per editor
  session, communicating over stdio JSON-RPC 2.0 with LSP-style
  `Content-Length` framing (so hosts reuse `vim.lsp.rpc` /
  `vscode-jsonrpc`). It holds no trace state between requests in v1.
- The **host** does three things: spawn the engine; answer its requests
  (buffer text, definition, references, document highlights); render the tree
  and jump. No analysis lives in the host.
- Everything language-specific is data loaded through one interface (§6).

## 4. Host protocol

Documented for the VS Code author in `docs/PROTOCOL.md`; this is the
normative summary. Positions are 0-based, UTF-16 columns, as in LSP, so hosts
pass LSP results through untouched. Paths are absolute.

### Host → engine

```
initialize      { root: string, capabilities: { documentHighlight: bool } }
                → { version: string, languages: [string] }
whence/trace    { file, line, col, limits?: { depth?, fanout?, nodes? } }
                → Tree | error
shutdown        {} → {}
exit            (notification) ends the loop; so does EOF on stdin
```

`whence/trace` errors (JSON-RPC error, not a tree): no language for file,
cursor not on an identifier, host request failed. `whence/trace` before
`initialize` is an invalid request. Traces are single-flight: a request that
arrives while one is running is answered "busy".

### Engine → host (only during a trace)

```
host/text               { file } → { text }
                          Buffer content if the file is open (unsaved edits
                          included), else disk content.
host/definition         { file, line, col } → [Location]
host/references         { file, line, col, includeDeclaration: bool } → [Location]
host/documentHighlight  { file, line, col } → [{ range, kind: "read"|"write"|"text" }]
                          Optional; host declares support in initialize.
                          Used for mutation tracking (Rust/Go).
```

`Location = { file, range: { start: {line, col}, end: {line, col} } }`.

A host returning an empty list is a normal answer (produces an `unresolved`
stop); a host returning an error aborts the trace with an error.

## 5. Trace algorithm and tree model

### 5.1 Tree

```jsonc
{
  "root": Node,
  "stats": { "nodes": 132, "truncated": 4, "host_requests": 41, "ms": 380 }
}

Node = {
  "id": string,          // stable: hash(file, line, col, kind, frame)
  "kind": "binding" | "param" | "call_result" | "field" | "stop",
  "label": string,       // identifier or short expression text
  "loc": { "file", "line", "col" },
  "via": "match" | "rebind" | "mutation" | "arg" | "return" | "field_set"
       | "field" | null,
  "snippet": string,     // the source line, trimmed
  "stop": { "reason": "external" | "entry_point" | "literal" | "unresolved" | "limit",
            "detail": string } | null,
  "children": [Node],
  "truncated": number    // children dropped by the fan-out bound
}
```

Children are ordered: same-file sites first, then by path, then by position.
Rebindings appear newest-first (closest to the use at the top).

### 5.2 Step function

Depth-first worklist over `(expression, frame_stack)`. Each step asks "what
feeds this expression?" and is answered by the generic engine using the
language's captures (§6) for structure and the host for resolution:

| Expression | Children |
|---|---|
| Variable use | `host/definition` on the variable → its binding site. If `documentHighlight` is available and returns `write` occurrences between the binding and the use, each write becomes a `binding` node with `via: rebind`/`mutation`, newest first, then the original binding. |
| Binding site (pattern `X = E`, `let x = E`, `x := E`, `a, b = f()`) | The RHS `E`; if the pattern destructures, the RHS sub-expression matching the sub-pattern that binds our identifier, when the RHS is a matching constructor/tuple; otherwise the whole RHS with `via: match`. |
| Function parameter, frame stack **non-empty** | The call-site argument bound in the top frame (one child, no host call). |
| Function parameter, frame stack **empty** | `host/references` on the enclosing function → for each call site, the argument at the parameter's index → `param` node, `via: arg`. Zero call sites → `stop: entry_point`. Fan-out bound applies. |
| Call `f(A, B)` | `host/definition` on the callee. Definition outside `root` → `stop: external` with `detail` = callee text. Inside root → for each return expression of the callee (tail expressions, `return` statements, every clause body), a `call_result` node `via: return`; tracing continues inside the callee with a pushed frame `{param_i → arg_i}`. Callee not found → `stop: unresolved`. |
| Field access `R#r.f`, `s.f`, `s.F` | Nearest visible construction or update of the container on the current path (`R = #r{f = V}`, `s.f = V`, struct literal with field `f`) → `field` node `via: field_set`, child = that field's value. No construction in sight → `field` node labelled `f of C` `via: field`, whose one child is the trace of the container `C` itself (`via: field`), which ends wherever tracing `C` ends. The engine never picks the field out of what `C` resolves to; a construction the container's own trace reaches shows up as an ordinary node on that path. |
| Literal / constructor with no variable parts | `stop: literal`. |
| Anything else (receive, closure capture, macro expansion, dynamic call, `apply`, reflection, method on unresolved receiver) | `stop: unresolved`, `detail` names the construct. |

### 5.3 Context sensitivity

Descending into a callee pushes a frame mapping the callee's parameters to
this call's arguments; returning to a parameter inside that callee resolves
through the frame. Reaching a parameter with an empty stack is the genuine
"who calls this?" case and goes to `host/references`. This keeps
call-result descent from expanding into every caller of every callee.

### 5.4 Bounds and termination

- Defaults: `depth = 64`, `fanout = 8`, `nodes = 400`; overridable per
  request. Hitting a bound emits `stop: limit` (never a silent cut); dropped
  siblings are counted in the parent's `truncated`.
- Cycle cut: an expression already on the current expansion path with the same
  frame stack → `stop: unresolved: recursion`. Revisiting an expression on a
  different path is allowed (diamonds are common: multi-clause callees, case
  branches sharing a subject).
- A call whose callee and argument expressions are already on the frame stack is
  not entered again → `stop: unresolved: recursion`. Descending into the same
  callee with *different* argument expressions is a genuine deeper call and
  proceeds, bounded by `depth`.
- Node ids and the frame hash are built from root-relative paths, so the same
  workspace yields the same tree wherever it is checked out.
- Wall-clock budget: 10 s default, then `stop: limit: time` on open branches
  and the partial tree is returned.

### 5.5 Honesty rule

An edge is emitted only when it points at a specific syntax node that the
engine identified through a capture or a host answer. Where a step would
require choosing among candidates (which clause returned, whether a `&mut`
callee wrote the value), all candidates are emitted as siblings, or the trace
stops. The engine never picks one silently.

## 6. Language data

The engine knows a fixed vocabulary of tree-sitter captures. A language is a
directory under `languages/<lang>/`, embedded into the binary at build time:

```
languages/erlang/
  grammar/        git submodule or vendored src/parser.c [+ scanner.c]
  whence.scm      queries producing the vocabulary below
  lang.toml       filetypes/extensions, declared quirks
```

Vocabulary (v1; expected to grow in M2):

```
@binding  @binding.pattern  @binding.value
@call     @call.callee      @call.args
@function @function.name    @function.params  @function.body
@return.value
@literal
@field    @field.container  @field.name
@construct  @construct.field.name  @construct.field.value
```

Generic derivations (no per-language code): arguments are the named children
of `@call.args`; argument index is position among them; parameters likewise
from `@function.params`; callee identifier position is the end of
`@call.callee`.

`lang.toml` quirks the engine understands: `returns = "tail" | "return" |
"both"`, `multi_assign = true|false`, `mutable_ref_markers = ["&mut"]`,
`single_assignment = true|false`. A construct that cannot be expressed by
queries plus these flags is a reason to extend the vocabulary, not to add
language-specific code.

nvim-treesitter-textobjects already ships `@assignment.lhs/rhs`,
`@call.inner/outer`, `@parameter.inner`, `@return.inner` for all three
languages. M1 includes a spike to see whether those can seed `whence.scm`;
either way the files are owned in this repo and pinned with the grammar.

Runtime loading of additional languages (parser `.so` from a search path,
download-and-compile registry) is out of scope for M1–M3 but nothing in the
loader interface prevents it: the embedded set is one implementation of
`LanguageSource`.

## 7. Neovim plugin (M1)

- Commands `:Whence` and `:whence`, plus `<Plug>(whence)`. Cursor must be on
  an identifier; otherwise `vim.notify` error.
- Engine spawned lazily on first use and reused for the session. Binary
  lookup: `vim.g.whence_bin` → `$PATH` → `stdpath("data")/whence/bin/whence`.
  If not found, the plugin offers to download the matching release
  (`:WhenceInstall`), verifying a published checksum; users with `cargo` can
  instead set `build = "cargo build --release"` in their plugin spec.
- Host answers: `host/text` from the loaded buffer or `vim.fn.readfile`;
  definition/references/documentHighlight via `vim.lsp.buf_request_all` on
  the buffer for that file (opened hidden and unlisted if needed), merging
  results from all attached clients.
- Panel: right vertical split, `filetype=whence`, one node per line indented
  by depth; stop nodes and `… N more` lines highlighted distinctly;
  `foldexpr` from indentation. Keys: `<CR>` jump, `p` preview in the source
  window, `R` re-run from the node under cursor, `q` close. Panel is
  read-only and reused across invocations.
- `:WhenceRecord` runs a trace and writes a replay fixture (§9) for the
  current cursor into a chosen directory.

## 8. VS Code extension (M3)

Same host role: spawn the platform binary bundled in the VSIX, answer
`host/*` via `vscode.commands.executeCommand("vscode.executeDefinitionProvider" |
"vscode.executeReferenceProvider" | "vscode.executeDocumentHighlights", …)`,
render a `TreeView` whose items reveal the location on click. No analysis
logic. Detailed UX deferred to M3.

## 9. Testing

- **Query tests** per language: fixture source files with expected capture
  sets, run against the embedded grammar. No host.
- **Trace tests** with a **replay host**: a fixture is a directory of source
  files plus `host.json` mapping each `host/*` request (method + params) to
  its recorded answer; the test runs `whence/trace` and diffs against a
  golden tree. Unmatched requests fail the test. `whence replay <dir>
  file:line:col [--json] [--fanout N] [--depth N]` runs the same from a
  terminal for debugging (`line:col` are 1-based there, unlike the protocol).
- Fixtures are seeded from real sessions with `:WhenceRecord`, so goldens
  reflect actual `erlang_ls` / `rust-analyzer` / `gopls` behavior.
- **Protocol tests**: framing, error paths, host returning empty/error.
- **Neovim plugin**: plenary smoke tests using the replay engine
  (`whence replay --serve <dir>`), checking panel content and jump targets.
- **Bounds**: synthetic fixtures for fan-out, depth, recursion, and time
  budget each produce the expected `stop: limit` shape.

## 10. Distribution

- GitHub Releases from a CI matrix: linux x86_64/aarch64, macOS
  arm64/x86_64, windows x86_64; `SHA256SUMS` published alongside.
- Neovim plugin: works with any plugin manager; first-use download as in §7.
- VS Code: platform-specific VSIX per target, published to the marketplace
  or distributed internally.

## 11. Milestones

1. **M1** — Engine skeleton (protocol, trace core, replay host), Erlang
   language data, Neovim plugin with panel and jump, release CI. Exit: on a
   real Erlang project, `:Whence` on a request-handler variable produces a
   tree that ends in `external`/`entry_point` stops, and every edge shown is
   correct on inspection.
2. **M2** — Rust and Go language data; mutation via `documentHighlight`;
   multi-value assignment; `&mut`/pointer candidates as siblings; vocabulary
   revisions.
3. **M3** — VS Code extension using the same binary and protocol.
4. **Later** — lazy expansion (`whence/expand { id }`), curated input-source
   labels, runtime language loading, persistent cache.

## 12. Task tracking

Work is tracked with the `tk` CLI in `.tickets/` (committed with the repo).

- One `epic` ticket per milestone (M1, M2, M3, Later), tagged `m1`…`m3`.
- The implementation plan for a milestone is turned into `task` tickets with
  `--parent <epic>`; ordering constraints become `tk dep` edges, so
  `tk ready` lists what can be started and `tk blocked` what is waiting.
- Spec changes discovered mid-implementation get a `tk add-note` on the
  affected ticket and an edit to this document in the same change.
- Bugs found during dogfooding are `bug` tickets linked to the milestone
  epic; they do not block the epic unless `tk dep` says so.
- Ticket IDs are referenced in commit messages as `[<id>]`.

## 13. Risks and open points

- **LSP quality is the ceiling.** Missing references or definitions from a
  server become `unresolved` stops. Mitigation: the honesty rule and the
  replay fixtures make gaps visible; an index-based Erlang resolver could be
  slotted in behind the same three host requests if `erlang_ls` disappoints.
- **Vocabulary churn.** Rust and Go will stress captures designed against
  Erlang. Expected; M2 budgets for it.
- **Multi-clause and pattern-heavy Erlang** produces wide return fan-out.
  The fan-out bound and clause ordering (same-file first, textual order) keep
  it readable; if not, per-node clause grouping is a rendering change only.
- **Binary size** grows ~1–2 MB per bundled grammar. Acceptable.
