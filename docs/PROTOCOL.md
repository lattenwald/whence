# Whence — host protocol

- Status: implemented (M1)
- Engine version: 0.1.0 (`engine/Cargo.toml`; reported by `initialize`)
- Source of truth: [design spec §4](superpowers/specs/2026-09-01-whence-design.md#4-host-protocol) — when this file and the spec disagree, the spec wins and this file is wrong
- Audience: authors of a *host* — the editor plugin that drives the engine (Neovim in M1, VS Code in M3)

The engine is a subprocess. It never spawns language servers, never reads
editor state, and contains no editor-specific code: everything it cannot
compute from source text it asks the host for. The host spawns it, answers
`host/*` requests, and renders the tree.

## Framing

stdio JSON-RPC 2.0 with LSP-style `Content-Length` framing — see the
[LSP base protocol](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#baseProtocol).
Each message is `Content-Length: <bytes>\r\n\r\n<utf-8 json>`. Other headers
are ignored; `Content-Length` is required. This is exactly what
`vim.lsp.rpc` and `vscode-jsonrpc` speak, so a host reuses its editor's RPC
client rather than writing framing code.

Both directions carry requests: the host calls `initialize` / `whence/trace` /
`shutdown`, and the engine calls `host/*` back *while a trace is running*.
The host's RPC client must therefore be able to serve incoming requests, not
only issue them.

## Conventions

- **Positions** are 0-based lines and 0-based **UTF-16** columns, as in LSP,
  so LSP results pass through untouched. (The `whence replay` CLI takes
  1-based `line:col` — that is a debugging affordance, not the protocol.)
- **Paths** are absolute, as plain strings.
- A host returning an **empty list** is a normal answer: the trace records an
  `unresolved` stop and continues. A host returning a **JSON-RPC error**
  aborts the whole trace with an error.

## Host → engine

### `initialize`

```jsonc
// params
{ "root": "/abs/project", "capabilities": { "documentHighlight": false } }
// result
{ "version": "0.1.0", "languages": ["erlang"] }
```

`root` bounds the trace: a definition outside it stops as `external`, and node
ids are hashed from root-relative paths so a tree is identical at any checkout
path. `capabilities` is optional and each flag defaults to false;
`capabilities.documentHighlight` declares whether the host can answer
`host/documentHighlight`, and when false the engine never sends it. The M1
engine has no rebinding pass and never sends it either way; the Neovim host
declares `false` until M2.

### `whence/trace`

```jsonc
// params
{ "file": "/abs/project/src/a.erl", "line": 6, "col": 4,
  "limits": { "depth": 64, "fanout": 8, "nodes": 400, "time_ms": 10000 } }
// result
Tree   // see below
```

`limits` is optional and each field defaults to the value shown. Traces are
**single-flight**: a request that reaches the engine while it is blocked
waiting for a `host/*` reply is answered `busy` immediately; one that arrives
after the trace's last host request simply waits in the queue and is served
normally once the trace returns. Either way a host should not have a second
`whence/trace` outstanding.

### `shutdown`

```jsonc
// params
{}
// result
{}
```

The engine replies and exits the loop.

### `exit`

A notification (no `id`, no result). It is honoured **between** traces; while
a trace is running it is dropped, so it does not abort one. **EOF on stdin
ends the session in either case** — closing the pipe is both a valid teardown
and the only way to abort a running trace.

### Error codes

Codes are from [`engine/src/protocol/mod.rs`](../engine/src/protocol/mod.rs).

| Code | Name | When the engine sends it |
|---|---|---|
| `-32700` | `E_PARSE` | reserved; never sent. Malformed framing or JSON is an I/O failure that ends the session |
| `-32600` | `E_INVALID_REQUEST` | `whence/trace` before `initialize` (`"not initialized"`); any request reaching the engine while it waits on a `host/*` reply (`"busy"`) — `shutdown` included. A message that is neither request, notification nor response is *not* answered with this code: like a parse error, it ends the session |
| `-32601` | `E_METHOD_NOT_FOUND` | unknown method |
| `-32602` | `E_INVALID_PARAMS` | params do not deserialize |
| `-32000` | `E_HOST` | a `host/*` request failed (host error, malformed host result, or a broken pipe), so the trace was aborted |
| `-32001` | `E_NO_LANGUAGE` | no language data for the file's extension |
| `-32002` | `E_NOT_IDENTIFIER` | the cursor is not on an identifier |

`-32000`, `-32001` and `-32002` are `whence/trace` outcomes: an error
response, never a tree.

## Engine → host (only during a trace)

```jsonc
host/text               { "file" }                → { "text": string }
host/definition         { "file", "line", "col" } → Location[]
host/references         { "file", "line", "col",
                          "includeDeclaration": bool } → Location[]
host/documentHighlight  { "file", "line", "col" } → Highlight[]
```

```jsonc
Location  = { "file": string,
              "range": { "start": {"line","col"}, "end": {"line","col"} } }
Highlight = { "range": { "start": {"line","col"}, "end": {"line","col"} },
              "kind": "text" | "read" | "write" }
```

- `host/text` must return **buffer** content when the file is open, including
  unsaved edits, and disk content otherwise. The engine parses exactly this
  text and reports positions into it; serving stale text produces wrong
  edges.
- `host/documentHighlight` is optional and is only sent when
  `initialize` declared the capability. It is unused by Erlang (single
  assignment) and exists for mutation tracking in M2.

## Tree

```jsonc
{
  "root": Node,
  "stats": { "nodes": int, "truncated": int, "host_requests": int, "ms": int }
}

Node = {
  "id": string,          // stable hash of (root-relative path, line, col, kind, frame)
  "kind": "binding" | "branch" | "param" | "call_result" | "field" | "stop",
  "label": string,       // identifier or short expression text
  "loc": { "file": string, "line": int, "col": int },
  "via": "match" | "rebind" | "mutation" | "arg" | "return" | "field_set"
       | "field" | null,
  "snippet": string,     // the source line, trimmed
  "stop": { "reason": "external" | "entry_point" | "literal" | "unresolved" | "limit",
            "detail": string } | null,
  "children": [Node],
  "truncated": int       // children dropped by the fan-out bound
}
```

`via` says how the value *above* is fed by this node; the root carries one
too. Children are ordered same-file first, then by path, then by position;
rebindings newest-first. `stop` is non-null exactly when `kind` is `"stop"`; a
stop node is usually a leaf but may carry children (the references that were
not call sites, so the user can jump to each).
See [spec §5.1](superpowers/specs/2026-09-01-whence-design.md#51-tree) for the
model and [§5.5](superpowers/specs/2026-09-01-whence-design.md#55-honesty-rule)
for the honesty rule the tree obeys: every edge points at a syntax node the
engine actually identified.

### Full example

`Z = Y, Y = X, f(X) -> …` with no callers of `f/1`
([`engine/tests/fixtures/erlang/local_chain/expected.json`](../engine/tests/fixtures/erlang/local_chain/expected.json)):

```json
{
  "root": {
    "children": [
      {
        "children": [
          {
            "children": [
              {
                "children": [],
                "id": "f44bf1016bee2cd0",
                "kind": "stop",
                "label": "X",
                "loc": {
                  "col": 2,
                  "file": "a.erl",
                  "line": 3
                },
                "snippet": "f(X) ->",
                "stop": {
                  "detail": "no call sites of f/1",
                  "reason": "entry_point"
                },
                "truncated": 0,
                "via": null
              }
            ],
            "id": "ec2f1a90d3cb941b",
            "kind": "param",
            "label": "X",
            "loc": {
              "col": 2,
              "file": "a.erl",
              "line": 3
            },
            "snippet": "f(X) ->",
            "stop": null,
            "truncated": 0,
            "via": "arg"
          }
        ],
        "id": "8562c1ef8da36e4c",
        "kind": "binding",
        "label": "Y",
        "loc": {
          "col": 4,
          "file": "a.erl",
          "line": 4
        },
        "snippet": "Y = X,",
        "stop": null,
        "truncated": 0,
        "via": "match"
      }
    ],
    "id": "f71905781ce44925",
    "kind": "binding",
    "label": "Z",
    "loc": {
      "col": 4,
      "file": "a.erl",
      "line": 5
    },
    "snippet": "Z = Y,",
    "stop": null,
    "truncated": 0,
    "via": "match"
  },
  "stats": {
    "host_requests": 5,
    "ms": 0,
    "nodes": 4,
    "truncated": 0
  }
}
```

(`loc.file` is absolute on the wire; the golden is normalised to
root-relative so it does not depend on the checkout path.)

## Replay fixtures (`host.json`)

A fixture directory is a self-contained stand-in for a host: source files plus
`host.json`, which maps each positional request to its recorded answer. It
backs the trace tests, `whence replay <dir> <file:line:col>`, and
`whence replay --serve <dir>` (a full engine that answers its own `host/*`,
used by the Neovim plugin tests). See
[spec §9](superpowers/specs/2026-09-01-whence-design.md#9-testing).

```jsonc
{
  "definition":        { "<rel-path>:<line>:<col>": Location[] },
  "references":        { "<rel-path>:<line>:<col>|decl"
                       | "<rel-path>:<line>:<col>|nodecl": Location[] },
  "documentHighlight": { "<rel-path>:<line>:<col>": Highlight[] }
}
```

- Keys use the path **relative to the fixture directory**, and the same
  0-based line/col as the protocol. `references` keys carry the
  `includeDeclaration` flag as a `|decl` / `|nodecl` suffix.
- `Location.file` may be relative (resolved against the fixture directory) or
  absolute (used as-is, which is how a fixture expresses an `external` stop).
- `host/text` is not recorded: the replay host reads the fixture's files from
  disk, so the sources in the directory *are* the buffer contents.
- An unrecorded request is an error, which fails the test rather than
  silently producing a smaller tree.

Fixtures are produced by `:WhenceRecord {dir}` from a real editor session,
which also copies the touched sources and writes `whence-record.json`
(root, target position, engine version, timestamp) — goldens should reflect
what the language servers actually return, not what a human guessed.

## Writing a host

- **Spawn** `whence serve` with the **working directory set to the project
  root**, and send `initialize` with that same `root` before anything else.
  `whence/trace` before `initialize` is refused with `"not initialized"`.
- **Answer `host/*` requests** — synchronously, or asynchronously as long as
  the response carries the **request id** the engine sent. The engine blocks
  on each one, so an unanswered request hangs the trace; apply your own
  timeout and answer with an error rather than never replying.
- **Buffer text must include unsaved edits.** Read the loaded buffer when
  there is one, the file from disk otherwise.
- **Positions are 0-based, UTF-16.** Convert if your editor reports
  something else — Neovim's per-client `position_encoding` may be `utf-8` or
  `utf-32`.
- **De-duplicate multi-client results.** When several language servers are
  attached to one buffer, merge their answers and drop duplicate locations;
  also flatten LSP `LocationLink[]` into `Location[]`. A single client's
  failure should not sink a request another client answered — report an error
  only when every client failed.
- **A host error must be a code your RPC library can emit.** Neovim's
  `vim.lsp.rpc` asserts that a server-request error code is a member of
  `vim.lsp.protocol.ErrorCodes`, so the Neovim host reports failures as
  `-32603` (`InternalError`); a non-member code is dropped inside a scheduled
  coroutine and the engine waits forever. The engine only uses the message
  text, so any valid code works.
- **Handle the engine dying.** `vim.lsp.rpc` will not fire pending callbacks
  when the process exits; keep your own pending map and fail in-flight traces
  on exit.
- **Teardown**: send `shutdown`, then close stdin (EOF). Do not send
  `shutdown` while the engine is awaiting a host reply — it is answered `busy`
  like any other request. `exit` is honoured between traces and dropped during
  one, so to abort a running trace close stdin rather than sending it.
