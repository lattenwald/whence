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
initialize      { root: string, capabilities: { documentHighlight: bool, implementation: bool } }
                → { version: string, languages: [string] }
whence/trace    { file, line, col, limits?: { depth?, fanout?, nodes?, time_ms?, split? } }
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
                          Lists a variable's occurrences for the write step
                          (M2 §3.1); host/references is the fallback.
host/implementation     { file, line, col } → [Location]
                          Optional; host declares support in initialize.
                          Implementations of an abstract method (M2 §3.3).
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
  "id": string,          // stable: hash(parent path, root-relative file, line, col, kind)
                         // path-dependent, so a place reached twice is two ids
  "kind": "binding" | "branch" | "param" | "call_result" | "field" | "stop",
  "label": string,       // identifier or short expression text
  "loc": { "file", "line", "col" },
  "via": "match" | "rebind" | "mutation" | "arg" | "return" | "field_set"
       | "field" | "element" | null,
  "snippet": string,     // the source line, trimmed
  "stop": { "reason": "external" | "entry_point" | "literal" | "unresolved" | "limit",
            "detail": string } | null,
  "children": [Node],
  "truncated": number    // children dropped by the fan-out bound
}
```

Children are ordered: same-file sites first, then by path, then by position.
Rebindings appear newest-first (closest to the use at the top).

`via` describes the edge to the parent, not the node's own kind: a `param`
reached through a binding or branch pattern is `via: match`; `via: arg` is for
a parameter reached from a call site or through the frame stack. A `stop` node
normally has no children, but one that stands for several rejected places
(§5.2, references that are not call sites) carries one child per place so the
user can jump to them.

### 5.2 Step function

Depth-first worklist over `(expression, frame_stack)`. Each step asks "what
feeds this expression?" and is answered by the generic engine using the
language's captures (§6) for structure and the host for resolution:

| Expression | Children |
|---|---|
| Variable use | `host/definition` on the variable → its binding site. Definitions are de-duplicated by (file, range); more than one distinct definition (one per `case` branch, say) → a `branch` node at the use whose children are every definition's node, `via: match`, since choosing one would be a guess and listing all is not (§5.5). Unless the language is `single_assignment`, the variable's occurrences between binding and use are classified by syntax into `binding` nodes `via: rebind`/`mutation` and `unresolved: may be written by …` stops, newest first, ahead of the definition node(s): [M2 design §3.1](2026-09-03-m2-rust-go-design.md#31-variable-use). |
| Binding site (pattern `X = E`, `let x = E`, `x := E`, `a, b = f()`) | The RHS `E`; if the pattern destructures, the RHS sub-expression matching the sub-pattern that binds our identifier, when the RHS is a matching constructor/tuple; a positional pattern against a non-constructor RHS (a call) traces the RHS with a pending *index* projection; otherwise the whole RHS with `via: match`. A loop pattern (`@binding.element`) traces the iterable `via: element`. A declaration without a value is a stop (M2 §3.2). |
| Any value that is a branching expression (`case`/`if`/`try`/`begin`/parens), most often a binding's RHS | A `branch` node `via: match`, labelled with the first line of the expression, whose children are that expression's tails (nested branches expanded in turn), each `via: match`. Fan-out bound applies. |
| Function parameter, frame stack **non-empty** | The call-site argument bound in the top frame (one child, no host call), narrowed by the parameter's pattern the same way a binding site is when both sit in one file. |
| Function parameter, frame stack **empty** | `host/references` on the enclosing function → for each call site (a reference lying inside a `@call.callee`; the server has already tied it to this function, so no name or arity is compared), the argument at the parameter's index → `param` node, `via: arg`. A call site with no argument at that index is reported like a stray, with `detail: call has no argument <i>`. No references at all (or only the declaration) → `stop: entry_point`. References that are *not* call sites (an `-export` entry, a `fun f/1` value, a file with no grammar) and **no** call sites at all → `stop: unresolved: <N> reference(s) to <name>/<arity> are not call sites`, carrying one `stop: unresolved: reference is not a call site` child per in-root reference so the user can jump to it. When call sites exist alongside such references, M1 shows only the call sites (a missing edge, never a wrong one); listing the strays as extra siblings is tracked as a follow-up. Fan-out bound applies. |
| Call `f(A, B)` | `host/definition` on the callee, de-duplicated by (file, range). All definitions outside `root` → `stop: external` with `detail` = callee text; some inside and some outside → `stop: unresolved: <N> definitions, some outside root`. All inside root → every definition is a callee whose clauses are collected, and for each return expression of those clauses (tail expressions, `return` statements, every clause body) a child of one `call_result` node `via: return`; tracing continues inside the callee with a pushed frame `{param_i → arg_i}`. Callee not found → `stop: unresolved`. A definition that is an abstract method (`@function.abstract`) is expanded through `host/implementation` into its in-root implementations (M2 §3.3). Methods carry their receiver in the frame beside the arguments (M2 §3.4). |
| Field access `R#r.f`, `s.f`, `s.F` | A `field` node labelled `f of C` `via: field` whose one child is the trace of the container `C` (`via: field`) with `f` *pending*: every step of that trace (bindings, branches, callee returns, frame arguments, parameters) runs unchanged, and the pending field is applied where the trace reaches a `@construct`. A construction that sets `f` continues with that value, `via: field_set`; an update construct that does not set `f` continues with its `@construct.base`, `via: field`; a literal, an opaque, or a construction without `f` → `stop: unresolved: no field f …`. Nested accesses stack their pending fields. Nothing is matched by name: the container is resolved through the host like any variable, so a shadowing binding in a nested fun, a container built in a callee or another file, and one bound by destructuring all resolve or stop honestly (§5.5). |
| Literal / constructor with no variable parts | `stop: literal`. |
| Anything else (receive, closure capture, macro expansion, dynamic call, `apply`, reflection, method on unresolved receiver) | `stop: unresolved`, `detail` names the construct. |

### 5.3 Context sensitivity

Descending into a callee pushes a frame mapping the callee's parameters to
this call's arguments; returning to a parameter inside that callee resolves
through the frame. Reaching a parameter with an empty stack is the genuine
"who calls this?" case and goes to `host/references`. This keeps
call-result descent from expanding into every caller of every callee.

### 5.4 Bounds and termination

- Defaults: `depth = 64`, `fanout = 8`, `nodes = 400`, `split = true`;
  overridable per request. Hitting a bound emits `stop: limit` (never a silent
  cut); dropped siblings are counted in the parent's `truncated`.
- `split = false` keeps the tree a single path: wherever it would fork (several
  definitions, call sites, return expressions, branch tails) the parent gets
  one `stop: unresolved: <N> candidates: <what>` child instead, the candidates
  counted in `truncated`. One switch, one code path, so no fork can escape it.
- Cycle cut: an expression already on the current expansion path with the same
  frame stack → `stop: unresolved: recursion`. Revisiting an expression on a
  different path is allowed (diamonds are common: multi-clause callees, case
  branches sharing a subject).
- A call whose callee and argument expressions are already on the frame stack is
  not entered again → `stop: unresolved: recursive call to <name>/<arity>`. Descending into the same
  callee with *different* argument expressions is a genuine deeper call and
  proceeds, bounded by `depth`.
- Node ids and the frame hash are built from root-relative paths, so the same
  workspace yields the same tree wherever it is checked out.
- Wall-clock budget: 10 s default, then `stop: limit: time` on open branches
  and the partial tree is returned. `time_ms = 0` trips on the first expansion,
  so the root itself is the `stop: limit: time`.

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

Vocabulary (v1 below; M2 additions in the [M2 design §6](2026-09-03-m2-rust-go-design.md#6-vocabulary-parent-§6)):

```
@binding  @binding.pattern  @binding.value
@call     @call.callee      @call.args
@function @function.name    @function.params  @function.body
@return   @return.value     @return.container
@literal
@field    @field.container  @field.name
@construct  @construct.field.name  @construct.field.value  @construct.base
```

Generic derivations (no per-language code): arguments are the named children
of `@call.args`; argument index is position among them; parameters are the
`@param` nodes owned by `@function.params` — `@param` is required, a language
without it declares no parameters — and parameter index is position among
them; callee identifier position is the end of `@call.callee`.

Returns are data, not a flag: `@return` marks every place a value leaves a
function (a body tail, a `return` operand), and `@return.container` /
`@return.value` let the engine descend from there to branch tails. A tail
language marks its body tails, a statement language its `return` operands,
a language with both marks both; the engine only ever asks "which `@return`
nodes does this function own" (a nested `@opaque` or `@function` owns its
own).

`lang.toml` quirks the engine understands: `single_assignment = true|false`
(skips the write step of M2 §3.1). A construct that cannot be expressed by
queries plus this flag is a reason to extend the vocabulary, not to add
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

- Requires Neovim ≥ 0.11 (`vim.fs.relpath`, 3-argument `vim.str_byteindex`);
  the plugin refuses to load on older versions with a `vim.notify` error.
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
logic. Detailed design: [VS Code extension design](2026-09-03-vscode-extension-design.md).

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
  reflect actual language-server behavior (`elp` for the Erlang fixtures;
  `rust-analyzer` / `gopls` later).
- **Protocol tests**: framing, error paths, host returning empty/error.
- **Neovim plugin**: plenary smoke tests using the replay engine
  (`whence replay --serve <dir>`), checking panel content and jump targets.
- **Bounds**: synthetic fixtures for fan-out, depth, recursion, and time
  budget each produce the expected `stop: limit` shape.

## 10. Distribution

- GitHub Releases from a CI matrix: linux x86_64/aarch64, macOS
  arm64/x86_64, windows x86_64; `SHA256SUMS` published alongside.
- Neovim plugin: works with any plugin manager; first-use download as in §7.
- VS Code: platform-specific VSIX per target with the engine binary inside,
  attached to the same GitHub Release; marketplace publishing later.

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
  slotted in behind the same three host requests if the server disappoints.
- **Vocabulary churn.** Rust and Go will stress captures designed against
  Erlang. Expected; M2 budgets for it.
- **Multi-clause and pattern-heavy Erlang** produces wide return fan-out.
  The fan-out bound and clause ordering (same-file first, textual order) keep
  it readable; if not, per-node clause grouping is a rendering change only.
- **Binary size** grows ~1–2 MB per bundled grammar. Acceptable.
