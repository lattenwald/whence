# M1 — Erlang + Neovim Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `:Whence` on an Erlang variable in Neovim shows a provenance tree in a panel, with jumpable nodes and honest stop nodes, produced by a language-agnostic Rust engine that gets definitions/references from Neovim's `erlang_ls`.

**Architecture:** A Rust binary `whence` speaks JSON-RPC over stdio to a host. The host (Neovim Lua plugin, or a replay host reading recorded answers in tests) supplies file text and LSP answers. The engine parses files with an embedded tree-sitter grammar, maps grammar nodes to a fixed capture vocabulary via a per-language `whence.scm`, and runs a generic worklist trace bounded by depth/fan-out/node limits.

**Tech Stack:** Rust 1.98 (edition 2024), `tree-sitter 0.27`, `tree-sitter-erlang 0.20`, `serde`/`serde_json 1`, `clap 4`, `anyhow`, `thiserror`; Neovim ≥ 0.10 Lua with `vim.lsp.rpc`; `plenary.nvim` for plugin tests; GitHub Actions for releases.

**Spec:** `docs/superpowers/specs/2026-09-01-whence-design.md`

## Global Constraints

- Engine never spawns language servers and contains no editor-specific code (spec §3). Only `host/text`, `host/definition`, `host/references`, `host/documentHighlight` cross the seam (§4).
- No per-language Rust code; language support is data under `languages/<lang>/` (§6).
- Wire positions are 0-based line, UTF-16 column (§4). Engine converts to byte offsets internally.
- Default limits: `depth = 64`, `fanout = 8`, `nodes = 400`, wall-clock 10 s (§5.4). Bounds produce `stop: limit`, never a silent cut.
- Honesty rule (§5.5): emit an edge only for a specific identified syntax node; several candidates → siblings or `unresolved`.
- Tree JSON shape exactly as §5.1 (`id, kind, label, loc, via, snippet, stop, children, truncated`).
- Ticket IDs in commit messages as `[<id>]` (§12). Commit messages must not contain the word "claude" (repo hook).
- The user stages and commits; the plan's commit steps state *what* to commit, the executor asks the user to stage or, when running autonomously with permission, stages exactly the listed files.

---

## File structure

```
Cargo.toml                       workspace: members = ["engine"]
Makefile                         build / test / nvim-link targets
engine/Cargo.toml
engine/build.rs                  embeds languages/*/whence.scm + lang.toml (include_str via generated file)
engine/src/main.rs               clap: `whence serve`, `whence replay <dir> <file:line:col> [--json]`
engine/src/lib.rs                pub mod declarations
engine/src/protocol/mod.rs       JSON-RPC message types + Content-Length framing
engine/src/protocol/framing.rs   read_message / write_message over Read/Write
engine/src/host.rs               `Host` trait, `Location`, `Highlight`, `HostError`
engine/src/host_rpc.rs           `RpcHost`: engine→host requests over the same stdio connection
engine/src/host_replay.rs        `ReplayHost`: answers from fixture `host.json`
engine/src/pos.rs                Pos (line, utf16 col) ⇄ byte offset conversions
engine/src/tree.rs               Tree, Node, NodeKind, Via, Stop, StopReason, Limits, Stats (serde)
engine/src/lang/mod.rs           `Language` (grammar + compiled queries + quirks), registry lookup by extension
engine/src/lang/vocab.rs         capture-name constants and `Vocab` accessor over QueryMatches
engine/src/lang/embedded.rs      generated table of embedded languages (from build.rs)
engine/src/syntax.rs             `Doc`: parsed file + structural questions (binding_at, call_at, returns_of, …)
engine/src/trace/mod.rs          `trace(host, lang_registry, req) -> Tree`
engine/src/trace/step.rs         one step: Expr → children (the §5.2 table)
engine/src/trace/frame.rs        argument frame stack, visited set, budget
engine/src/server.rs             `serve()` loop: initialize / whence/trace / shutdown
engine/tests/fixtures/erlang/<case>/  *.erl + host.json + expected.json
engine/tests/replay.rs           runs every fixture, diffs against expected.json
languages/erlang/whence.scm
languages/erlang/lang.toml
nvim/plugin/whence.lua           :Whence, :whence, :WhenceRecord, :WhenceInstall
nvim/lua/whence/init.lua         public setup(), trace()
nvim/lua/whence/engine.lua       spawn + JSON-RPC via vim.lsp.rpc, host request dispatch
nvim/lua/whence/host.lua         answers host/* using vim.lsp
nvim/lua/whence/panel.lua        buffer rendering, folds, keymaps, jump
nvim/lua/whence/record.lua       fixture recorder
nvim/lua/whence/install.lua      release download
nvim/tests/panel_spec.lua        plenary tests against `whence replay --serve`
docs/PROTOCOL.md                 host protocol reference
.github/workflows/ci.yml         cargo test, plenary tests
.github/workflows/release.yml    cross-platform binaries + SHA256SUMS
```

---

### Task 1: Workspace skeleton and tickets

**Files:**
- Create: `Cargo.toml`, `engine/Cargo.toml`, `engine/src/main.rs`, `engine/src/lib.rs`, `Makefile`, `.gitignore`, `rust-toolchain.toml`
- Create: `.tickets/` via `tk create`

**Interfaces:**
- Produces: workspace that builds; `make test` runs `cargo test --workspace`.

- [ ] **Step 1: Create tickets**

```bash
tk create "M1: Erlang + Neovim" -t epic --tags m1 -d "See docs/superpowers/plans/2026-09-01-m1-erlang-neovim.md"
# note the printed ID as $M1, then one task per plan task:
for t in "workspace skeleton" "protocol framing and messages" "host trait, rpc and replay hosts" \
         "tree model and limits" "language registry and vocabulary" "erlang queries" \
         "syntax questions on a parsed doc" "trace core" "server loop and cli" \
         "neovim engine and host" "neovim panel" "neovim recorder and install" "docs and release ci"; do
  tk create "$t" --parent $M1 --tags m1
done
# then: tk dep <later> <earlier> following the task order in this plan (each task depends on the previous one,
# except: neovim tasks depend on "server loop and cli"; "docs and release ci" depends on everything).
```

- [ ] **Step 2: Workspace files**

`Cargo.toml`:
```toml
[workspace]
members = ["engine"]
resolver = "3"

[workspace.package]
edition = "2024"
license = "MIT"
repository = "https://github.com/lattenwald/whence"
```

`rust-toolchain.toml`:
```toml
[toolchain]
channel = "1.98"
```

`engine/Cargo.toml`:
```toml
[package]
name = "whence"
version = "0.1.0"
edition.workspace = true
license.workspace = true
description = "Variable provenance: where does this value come from?"

[[bin]]
name = "whence"
path = "src/main.rs"

[dependencies]
anyhow = "1"
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive"] }
tree-sitter = "0.27"
tree-sitter-erlang = "0.20"
toml = "0.9"
sha2 = "0.10"
log = "0.4"
env_logger = "0.11"

[dev-dependencies]
pretty_assertions = "1"
```

`engine/src/lib.rs`:
```rust
pub mod host;
pub mod host_replay;
pub mod host_rpc;
pub mod lang;
pub mod pos;
pub mod protocol;
pub mod server;
pub mod syntax;
pub mod trace;
pub mod tree;
```
(Add each module as its task creates it; until then keep only the ones that exist.)

`engine/src/main.rs` (placeholder until Task 9):
```rust
fn main() {
    println!("whence {}", env!("CARGO_PKG_VERSION"));
}
```

`Makefile` (follow the makefile-conventions skill: `help` target, `.PHONY`):
```make
.PHONY: help build test test-nvim nvim-link fmt
help: ## Show targets
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN{FS=":.*?## "}{printf "  %-12s %s\n",$$1,$$2}'
build: ## Build release engine
	cargo build --release
test: ## Run engine tests
	cargo test --workspace
test-nvim: ## Run Neovim plugin tests (needs plenary.nvim in packpath)
	nvim --headless -u nvim/tests/minimal_init.lua -c "PlenaryBustedDirectory nvim/tests { minimal_init = 'nvim/tests/minimal_init.lua' }"
nvim-link: build ## Symlink release binary into the plugin's bin/
	mkdir -p nvim/bin && ln -sf $(CURDIR)/target/release/whence nvim/bin/whence
fmt: ## Format
	cargo fmt --all
```

`.gitignore`: `target/`, `nvim/bin/`, `*.log`.

- [ ] **Step 3: Verify**

Run: `cargo build && cargo run -q -- ` — expected output `whence 0.1.0`. Run `make help` — lists targets.

- [ ] **Step 4: Commit**

`chore: workspace skeleton and m1 tickets [<id>]` — files: everything above plus `.tickets/`.

---

### Task 2: Protocol framing and messages

**Files:**
- Create: `engine/src/protocol/mod.rs`, `engine/src/protocol/framing.rs`
- Test: unit tests inside both files

**Interfaces:**
- Produces:
  ```rust
  pub enum Message { Request(Request), Response(Response), Notification(Notification) }
  pub struct Request { pub id: Id, pub method: String, pub params: serde_json::Value }
  pub struct Response { pub id: Id, pub result: Option<Value>, pub error: Option<RpcError> }
  pub struct Notification { pub method: String, pub params: Value }
  pub struct RpcError { pub code: i64, pub message: String, pub data: Option<Value> }
  pub enum Id { Num(i64), Str(String) }
  pub fn read_message(r: &mut impl BufRead) -> io::Result<Option<Message>>  // None on EOF
  pub fn write_message(w: &mut impl Write, m: &Message) -> io::Result<()>
  ```
  Error codes: `-32700` parse, `-32600` invalid request, `-32601` method not found, `-32602` invalid params, `-32000` host failure, `-32001` no language, `-32002` not an identifier.

- [ ] **Step 1: Failing tests** (`framing.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Message, Request, Id};
    use std::io::Cursor;

    #[test]
    fn roundtrip_request() {
        let m = Message::Request(Request { id: Id::Num(1), method: "whence/trace".into(),
            params: serde_json::json!({"file":"/a.erl","line":3,"col":4}) });
        let mut buf = Vec::new();
        write_message(&mut buf, &m).unwrap();
        let s = String::from_utf8(buf.clone()).unwrap();
        assert!(s.starts_with("Content-Length: "));
        assert!(s.contains("\r\n\r\n{"));
        let back = read_message(&mut Cursor::new(buf)).unwrap().unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn eof_returns_none() {
        assert!(read_message(&mut Cursor::new(Vec::<u8>::new())).unwrap().is_none());
    }

    #[test]
    fn response_without_result_or_error_is_error() {
        let raw = b"Content-Length: 12\r\n\r\n{\"id\":1,\"x\":1}";
        assert!(read_message(&mut Cursor::new(raw.to_vec())).is_err());
    }
}
```

- [ ] **Step 2: Run** `cargo test protocol` — FAIL: unresolved imports.

- [ ] **Step 3: Implement** `protocol/mod.rs`

```rust
pub mod framing;
pub use framing::{read_message, write_message};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id { Num(i64), Str(String) }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError { pub code: i64, pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub data: Option<Value> }

#[derive(Debug, Clone, PartialEq)]
pub struct Request { pub id: Id, pub method: String, pub params: Value }
#[derive(Debug, Clone, PartialEq)]
pub struct Response { pub id: Id, pub result: Option<Value>, pub error: Option<RpcError> }
#[derive(Debug, Clone, PartialEq)]
pub struct Notification { pub method: String, pub params: Value }

#[derive(Debug, Clone, PartialEq)]
pub enum Message { Request(Request), Response(Response), Notification(Notification) }

pub const E_PARSE: i64 = -32700;
pub const E_INVALID_REQUEST: i64 = -32600;
pub const E_METHOD_NOT_FOUND: i64 = -32601;
pub const E_INVALID_PARAMS: i64 = -32602;
pub const E_HOST: i64 = -32000;
pub const E_NO_LANGUAGE: i64 = -32001;
pub const E_NOT_IDENTIFIER: i64 = -32002;

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self { Self { code, message: message.into(), data: None } }
}

// Wire form: one flat object. Classify by presence of id/method/result/error.
#[derive(Serialize, Deserialize)]
struct Wire {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")] id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")] method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")] error: Option<RpcError>,
}
```
Implement `Message::to_json(&self) -> Value` and `Message::from_json(Value) -> Result<Message, RpcError>` using `Wire` (request = id+method; notification = method without id; response = id with result or error; otherwise `E_INVALID_REQUEST`). Note: `Wire.jsonrpc` for deserializing must be `String`; use two structs or `#[serde(borrow)]` — simplest is `jsonrpc: String` and set `"2.0".into()` when writing.

`protocol/framing.rs`:
```rust
use super::Message;
use std::io::{self, BufRead, Write};

pub fn read_message(r: &mut impl BufRead) -> io::Result<Option<Message>> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 { return Ok(None); }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() { break; }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len = Some(v.trim().parse().map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad Content-Length"))?);
        }
    }
    let len = len.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    let v: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Message::from_json(v).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.message))
}

pub fn write_message(w: &mut impl Write, m: &Message) -> io::Result<()> {
    let body = serde_json::to_vec(&m.to_json())?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(&body)?;
    w.flush()
}
```

- [ ] **Step 4: Run** `cargo test protocol` — PASS (3 tests).

- [ ] **Step 5: Commit** `feat(engine): json-rpc framing and message types [<id>]`

---

### Task 3: Host trait, RPC host, replay host

**Files:**
- Create: `engine/src/host.rs`, `engine/src/host_rpc.rs`, `engine/src/host_replay.rs`, `engine/src/pos.rs`
- Test: unit tests in `host_replay.rs` and `pos.rs`; `host_rpc.rs` tested via an in-memory pipe

**Interfaces:**
- Produces:
  ```rust
  // pos.rs
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
  pub struct Pos { pub line: u32, pub col: u32 }          // wire: 0-based, UTF-16 col
  pub fn byte_offset(text: &str, p: Pos) -> Option<usize>   // None if out of range
  pub fn pos_of(text: &str, byte: usize) -> Pos
  pub fn to_point(text: &str, p: Pos) -> Option<tree_sitter::Point>  // row + byte column
  pub fn from_point(text: &str, pt: tree_sitter::Point) -> Pos

  // host.rs
  #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
  pub struct Range { pub start: Pos, pub end: Pos }
  #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
  pub struct Location { pub file: PathBuf, pub range: Range }
  #[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")] pub enum HighlightKind { Read, Write, Text }
  #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
  pub struct Highlight { pub range: Range, pub kind: HighlightKind }
  #[derive(Debug, thiserror::Error)] pub enum HostError {
      #[error("host request {method} failed: {message}")] Rpc { method: String, message: String },
      #[error("host does not support {0}")] Unsupported(&'static str),
      #[error(transparent)] Io(#[from] std::io::Error) }
  pub trait Host {
      fn text(&mut self, file: &Path) -> Result<String, HostError>;
      fn definition(&mut self, file: &Path, pos: Pos) -> Result<Vec<Location>, HostError>;
      fn references(&mut self, file: &Path, pos: Pos, include_decl: bool) -> Result<Vec<Location>, HostError>;
      fn document_highlight(&mut self, file: &Path, pos: Pos) -> Result<Vec<Highlight>, HostError>;
      fn request_count(&self) -> u32;
  }
  // host_rpc.rs
  pub struct RpcHost<W: Write> { .. }   // new(writer, inbox: Receiver<Message>, supports_highlight: bool)
  // host_replay.rs
  pub struct ReplayHost { .. }          // ReplayHost::load(dir: &Path) -> anyhow::Result<Self>
  ```
- Fixture format `host.json`:
  ```json
  { "definition":        { "/abs/or/rel/file.erl:12:4": [ {"file":"…","range":{"start":{"line":..,"col":..},"end":{..}}} ] },
    "references":        { "file.erl:30:1|decl": [ … ] },
    "documentHighlight": { "file.erl:12:4": [ {"range":{..},"kind":"read"} ] } }
  ```
  Keys are `relative-path:line:col` (relative to the fixture dir); `references` keys end with `|decl` or `|nodecl`. Files referenced by relative paths resolve against the fixture dir; the replay host rewrites them to absolute paths on load. A request with no key present is an error `HostError::Rpc{message:"unrecorded"}` — tests must fail loudly rather than pass on missing data.

- [ ] **Step 1: Failing tests**

`pos.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn ascii() {
        let t = "ab\ncd";
        assert_eq!(byte_offset(t, Pos{line:1,col:1}), Some(4));
        assert_eq!(pos_of(t, 4), Pos{line:1,col:1});
    }
    #[test] fn utf16_surrogate_pair_counts_two() {
        let t = "𝕏 = 1";           // U+1D54F is 4 bytes UTF-8, 2 UTF-16 units
        assert_eq!(byte_offset(t, Pos{line:0,col:2}), Some(4));
        assert_eq!(pos_of(t, 4), Pos{line:0,col:2});
    }
    #[test] fn out_of_range_is_none() {
        assert_eq!(byte_offset("a", Pos{line:3,col:0}), None);
    }
}
```

`host_replay.rs` (uses a temp dir written by the test):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pos::Pos;
    fn fixture() -> tempfile::TempDir {           // add tempfile = "3" to dev-dependencies
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.erl"), "f(X) -> X.\n").unwrap();
        std::fs::write(d.path().join("host.json"), r#"{
          "definition": { "a.erl:0:8": [ {"file":"a.erl","range":{"start":{"line":0,"col":2},"end":{"line":0,"col":3}}} ] },
          "references": {}, "documentHighlight": {} }"#).unwrap();
        d
    }
    #[test] fn answers_recorded_definition_with_absolute_paths() {
        let d = fixture();
        let mut h = ReplayHost::load(d.path()).unwrap();
        let locs = h.definition(&d.path().join("a.erl"), Pos{line:0,col:8}).unwrap();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].file, d.path().join("a.erl"));
        assert_eq!(h.request_count(), 1);
    }
    #[test] fn unrecorded_request_is_error() {
        let d = fixture();
        let mut h = ReplayHost::load(d.path()).unwrap();
        assert!(h.references(&d.path().join("a.erl"), Pos{line:0,col:0}, true).is_err());
    }
    #[test] fn text_reads_from_disk() {
        let d = fixture();
        let mut h = ReplayHost::load(d.path()).unwrap();
        assert_eq!(h.text(&d.path().join("a.erl")).unwrap(), "f(X) -> X.\n");
    }
}
```

`host_rpc.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::*;
    use std::sync::mpsc;
    #[test] fn definition_sends_request_and_reads_matching_response() {
        let (tx, rx) = mpsc::channel();
        let mut out = Vec::new();
        let mut h = RpcHost::new(&mut out, rx, false);
        // pre-load the answer the "host" will send; id 1 is the first id RpcHost allocates
        tx.send(Message::Response(Response { id: Id::Num(1), error: None,
            result: Some(serde_json::json!([{"file":"/x.erl","range":{"start":{"line":1,"col":2},"end":{"line":1,"col":3}}}])) })).unwrap();
        let locs = h.definition(std::path::Path::new("/x.erl"), Pos{line:5,col:6}).unwrap();
        assert_eq!(locs[0].range.start, Pos{line:1,col:2});
        let sent = String::from_utf8(out).unwrap();
        assert!(sent.contains(r#""method":"host/definition""#));
        assert!(sent.contains(r#""line":5"#));
    }
    #[test] fn highlight_unsupported_when_host_lacks_capability() {
        let (_tx, rx) = mpsc::channel::<Message>();
        let mut out = Vec::new();
        let mut h = RpcHost::new(&mut out, rx, false);
        assert!(matches!(h.document_highlight(std::path::Path::new("/x"), Pos{line:0,col:0}), Err(HostError::Unsupported(_))));
    }
}
```

- [ ] **Step 2: Run** `cargo test pos host` — FAIL to compile.

- [ ] **Step 3: Implement**

`pos.rs`: iterate lines with `text.split_inclusive('\n')`; within the target line walk `char_indices()`, accumulating `c.len_utf16()` until reaching `col`; `pos_of` is the inverse. `to_point` = `Point { row: line, column: byte_offset - line_start }`.

`host_rpc.rs` design: the server (Task 9) owns stdin reading on a thread and forwards every incoming `Message` to an `mpsc::Sender`. During a trace, `RpcHost` writes a request with a fresh `Id::Num(n)` to the shared writer and blocks on `rx.recv()` until a `Response` with that id arrives (responses with other ids or notifications are logged and dropped; requests from the host during a trace are answered with `E_INVALID_REQUEST` "busy"). Deserialize `result` into `Vec<Location>` / `Vec<Highlight>` / `{text}`; an `error` becomes `HostError::Rpc`. Params are `{"file": path, "line", "col"}` plus `includeDeclaration` for references. `request_count` increments per request.

`host_replay.rs`: `load` reads `host.json` into three `HashMap<String, Vec<...>>`, rewrites each `Location.file` that is relative to `dir.join(file)`. Lookup key built as `"{rel}:{line}:{col}"` where `rel = file.strip_prefix(dir)`; for references append `|decl`/`|nodecl`. `text` reads from disk.

- [ ] **Step 4: Run** `cargo test pos host` — PASS (8 tests).

- [ ] **Step 5: Commit** `feat(engine): host seam with rpc and replay implementations [<id>]`

---

### Task 4: Tree model and limits

**Files:**
- Create: `engine/src/tree.rs`
- Test: unit test in file

**Interfaces:**
- Produces (serialized exactly as spec §5.1):
  ```rust
  #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
  #[serde(rename_all = "snake_case")] pub enum NodeKind { Binding, Param, CallResult, Field, Stop }
  #[serde(rename_all = "snake_case")] pub enum Via { Match, Rebind, Mutation, Arg, Return, FieldSet }
  #[serde(rename_all = "snake_case")] pub enum StopReason { External, EntryPoint, Literal, Unresolved, Limit }
  pub struct Stop { pub reason: StopReason, pub detail: String }
  pub struct Loc { pub file: PathBuf, pub line: u32, pub col: u32 }
  pub struct Node { pub id: String, pub kind: NodeKind, pub label: String, pub loc: Loc,
                    pub via: Option<Via>, pub snippet: String, pub stop: Option<Stop>,
                    pub children: Vec<Node>, pub truncated: u32 }
  pub struct Stats { pub nodes: u32, pub truncated: u32, pub host_requests: u32, pub ms: u64 }
  pub struct Tree { pub root: Node, pub stats: Stats }
  #[derive(Deserialize, Clone, Copy)] #[serde(default)]
  pub struct Limits { pub depth: u32, pub fanout: u32, pub nodes: u32, pub time_ms: u64 }
  impl Default for Limits { depth: 64, fanout: 8, nodes: 400, time_ms: 10_000 }
  impl Node {
      pub fn stop(loc: Loc, label: &str, snippet: &str, reason: StopReason, detail: impl Into<String>) -> Node
      pub fn count(&self) -> u32   // nodes in subtree incl. self
  }
  pub fn node_id(file: &Path, line: u32, col: u32, kind: &NodeKind, frame_hash: u64) -> String // 16 hex chars of sha256
  ```

- [ ] **Step 1: Failing test**

```rust
#[test] fn serializes_to_spec_shape() {
    let n = Node::stop(Loc{file:"/a.erl".into(),line:1,col:2}, "X", "X = 1", StopReason::Literal, "integer");
    let v = serde_json::to_value(&n).unwrap();
    assert_eq!(v["kind"], "stop");
    assert_eq!(v["stop"]["reason"], "literal");
    assert_eq!(v["via"], serde_json::Value::Null);
    assert_eq!(v["truncated"], 0);
    assert_eq!(v["children"], serde_json::json!([]));
    assert_eq!(v["id"].as_str().unwrap().len(), 16);
}
#[test] fn limits_default_and_partial_override() {
    let l: Limits = serde_json::from_str(r#"{"fanout":3}"#).unwrap();
    assert_eq!((l.depth, l.fanout, l.nodes), (64, 3, 400));
}
```

- [ ] **Step 2: Run** `cargo test tree` — FAIL. **Step 3: Implement** as specified (`node_id` = hex of first 8 bytes of `sha256(format!("{}:{}:{}:{:?}:{}", …))`). **Step 4: Run** — PASS. **Step 5: Commit** `feat(engine): tree model [<id>]`

---

### Task 5: Language registry and capture vocabulary

**Files:**
- Create: `engine/build.rs`, `engine/src/lang/mod.rs`, `engine/src/lang/vocab.rs`, `engine/src/lang/embedded.rs` (generated into `OUT_DIR`, included by `mod.rs`)
- Create: `languages/erlang/lang.toml`, `languages/erlang/whence.scm` (minimal, expanded in Task 6)
- Test: unit tests in `lang/mod.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct Quirks { pub returns: Returns, pub multi_assign: bool, pub single_assignment: bool,
                      pub mutable_ref_markers: Vec<String> }
  #[serde(rename_all = "lowercase")] pub enum Returns { Tail, Return, Both }
  pub struct Language { pub name: &'static str, pub ts: tree_sitter::Language,
                        pub query: tree_sitter::Query, pub quirks: Quirks, pub extensions: Vec<String> }
  pub struct Registry { .. }
  impl Registry {
      pub fn embedded() -> anyhow::Result<Registry>          // compiles all embedded languages' queries
      pub fn for_file(&self, path: &Path) -> Option<&Language>
      pub fn names(&self) -> Vec<&'static str>
  }
  // vocab.rs — capture names, one const per vocabulary entry:
  pub const BINDING: &str = "binding";  BINDING_PATTERN = "binding.pattern";  BINDING_VALUE = "binding.value";
  CALL, CALL_CALLEE, CALL_ARGS, FUNCTION, FUNCTION_NAME, FUNCTION_PARAMS, FUNCTION_BODY,
  RETURN_VALUE, LITERAL, FIELD, FIELD_CONTAINER, FIELD_NAME, CONSTRUCT, CONSTRUCT_FIELD_NAME, CONSTRUCT_FIELD_VALUE,
  IDENT = "ident"        // any identifier that can be traced (Erlang: var)
  pub fn required() -> &'static [&'static str]   // captures every language must define: BINDING*, CALL*, FUNCTION*, RETURN_VALUE, LITERAL, IDENT
  ```
- `lang.toml`:
  ```toml
  name = "erlang"
  extensions = ["erl", "hrl"]
  [quirks]
  returns = "tail"
  multi_assign = false
  single_assignment = true
  mutable_ref_markers = []
  ```
- `build.rs` walks `../languages/*/`, and for each dir writes into `$OUT_DIR/embedded.rs` an entry `("erlang", tree_sitter_erlang::LANGUAGE, include_str!("…/lang.toml"), include_str!("…/whence.scm"))`. The grammar crate is named by convention `tree_sitter_<name>` — adding a language means adding the crate dependency and the directory; still no Rust logic per language. `println!("cargo:rerun-if-changed=../languages")`.

- [ ] **Step 1: Failing tests**

```rust
#[test] fn embedded_registry_has_erlang_and_resolves_extension() {
    let r = Registry::embedded().unwrap();
    assert!(r.names().contains(&"erlang"));
    assert_eq!(r.for_file(Path::new("/p/src/a.erl")).unwrap().name, "erlang");
    assert!(r.for_file(Path::new("/p/README.md")).is_none());
}
#[test] fn every_language_defines_required_captures() {
    let r = Registry::embedded().unwrap();
    for name in r.names() {
        let l = r.for_file(Path::new(&format!("x.{}", r.by_name(name).unwrap().extensions[0]))).unwrap();
        let have: Vec<&str> = l.query.capture_names().to_vec();
        for req in vocab::required() { assert!(have.contains(req), "{name} lacks @{req}"); }
    }
}
```

- [ ] **Step 2: Run** `cargo test lang` — FAIL.

- [ ] **Step 3: Implement** `Registry::embedded()` iterating the generated table: `let ts: tree_sitter::Language = lang_fn.into(); let query = tree_sitter::Query::new(&ts, scm)?` (query compile errors name the language and byte offset). Also add `pub fn by_name(&self, n: &str) -> Option<&Language>`. Minimal `whence.scm` for this task:

```scheme
(var) @ident
(match_expr lhs: (_) @binding.pattern rhs: (_) @binding.value) @binding
(call expr: (_) @call.callee args: (expr_args) @call.args) @call
(function_clause name: (_) @function.name args: (expr_args) @function.params body: (clause_body) @function.body) @function
(clause_body (_) @return.value .)
[(integer) (float) (string) (atom) (char)] @literal
```

- [ ] **Step 4: Run** — PASS. **Step 5: Commit** `feat(engine): embedded language registry and capture vocabulary [<id>]`

---

### Task 6: Erlang queries

**Files:**
- Modify: `languages/erlang/whence.scm`
- Modify: `engine/src/lang/vocab.rs` (add captures introduced here)
- Test: `engine/tests/queries_erlang.rs` with fixture `engine/tests/fixtures/erlang/queries/sample.erl`

**Interfaces:**
- Produces the full v1 vocabulary. Added captures beyond Task 5:
  ```
  @return.container   an expression whose clause tails are returns when it is itself in tail position
                      (Erlang: case_expr, if_expr, try_expr, receive_expr, block_expr, paren_expr)
  @branch             a clause with a pattern matched against a subject (Erlang: cr_clause)
  @branch.pattern     its pattern
  @branch.subject     the expression the patterns are matched against (captured on the case_expr's expr)
  @construct          a constructor expression (tuple, list, record_expr, map_expr, record_update_expr)
  @construct.field.name / @construct.field.value   record/map field pairs
  @field  @field.container  @field.name             record_field_expr / map access
  @callee.module  @callee.name                       parts of a remote callee (erlang: remote)
  @opaque             a construct the engine must not look inside (Erlang: receive_expr, anonymous_fun,
                      macro_call_expr, list_comprehension, binary_comprehension, map_comprehension, try catch clauses)
  ```
  Grammar reference: node types and fields are from `tree-sitter-erlang` `src/node-types.json` (fetched during planning): `match_expr(lhs,rhs)`, `call(expr,args)`, `expr_args(args)`, `remote(module,fun)`, `fun_decl(clause)`, `function_clause(name,args,guard,body)`, `clause_body(exprs)`, `case_expr(expr,clauses)`, `cr_clause(pat,guard,body)`, `if_expr(clauses)`, `if_clause(guard,body)`, `try_expr(exprs,clauses,catch,after)`, `receive_expr(clauses,after)`, `record_expr(name,fields)`, `record_update_expr(expr,name,fields)`, `record_field(name,expr)`, `field_expr(expr)`, `record_field_expr(expr,name,field)`, `map_expr(fields)`, `map_field(key,value)`, `tuple(expr)`, `list(exprs)`, `block_expr(exprs)`, `paren_expr(expr)`, leaves `var atom integer float string char`.

- [ ] **Step 1: Fixture** `sample.erl`

```erlang
-module(sample).
-export([handle/2, pick/1]).

-record(req, {body, peer}).

handle(Req0, Opts) ->
    Body = read_body(Req0),
    Peer = Req0#req.peer,
    R = #req{body = Body, peer = Peer},
    Limit = maps:get(limit, Opts, 10),
    case pick(Limit) of
        {ok, V} -> {V, R};
        error -> {0, R}
    end.

pick(N) when N > 5 -> {ok, N * 2};
pick(_) -> error.

read_body(#req{body = B}) -> B.
```

- [ ] **Step 2: Failing test** `engine/tests/queries_erlang.rs`

```rust
use whence::lang::{Registry, vocab};
use std::collections::BTreeMap;

fn captures(src: &str) -> BTreeMap<String, Vec<String>> {
    let reg = Registry::embedded().unwrap();
    let lang = reg.by_name("erlang").unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.ts).unwrap();
    let tree = parser.parse(src, None).unwrap();
    let mut cur = tree_sitter::QueryCursor::new();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let names = lang.query.capture_names();
    let mut it = cur.matches(&lang.query, tree.root_node(), src.as_bytes());
    while let Some(m) = it.next() {
        for c in m.captures {
            out.entry(names[c.index as usize].to_string()).or_default()
               .push(c.node.utf8_text(src.as_bytes()).unwrap().to_string());
        }
    }
    out
}

#[test] fn bindings_calls_functions() {
    let c = captures(include_str!("fixtures/erlang/queries/sample.erl"));
    assert!(c[vocab::BINDING_PATTERN].contains(&"Body".to_string()));
    assert!(c[vocab::BINDING_VALUE].contains(&"read_body(Req0)".to_string()));
    assert!(c[vocab::CALL_CALLEE].contains(&"maps:get".to_string()));
    assert!(c["callee.module"].contains(&"maps".to_string()));
    assert_eq!(c[vocab::FUNCTION_NAME].iter().filter(|n| *n == "pick").count(), 2, "one @function per clause");
}
#[test] fn tail_returns_and_branches() {
    let c = captures(include_str!("fixtures/erlang/queries/sample.erl"));
    assert!(c["return.container"].iter().any(|s| s.starts_with("case pick(Limit)")));
    assert!(c[vocab::RETURN_VALUE].contains(&"{V, R}".to_string()));
    assert!(c[vocab::RETURN_VALUE].contains(&"error".to_string()));
    assert!(c["branch.subject"].contains(&"pick(Limit)".to_string()));
    assert!(c["branch.pattern"].contains(&"{ok, V}".to_string()));
}
#[test] fn fields_constructs_literals_opaque() {
    let c = captures(include_str!("fixtures/erlang/queries/sample.erl"));
    assert!(c["field.container"].contains(&"Req0".to_string()));
    assert!(c["field.name"].contains(&"peer".to_string()));
    assert!(c["construct.field.name"].contains(&"body".to_string()));
    assert!(c["construct.field.value"].contains(&"Body".to_string()));
    assert!(c[vocab::LITERAL].contains(&"10".to_string()));
    assert!(c[vocab::IDENT].contains(&"Opts".to_string()));
}
```

- [ ] **Step 3: Run** `cargo test --test queries_erlang` — FAIL (missing captures).

- [ ] **Step 4: Write** `languages/erlang/whence.scm`

```scheme
;; identifiers that can be traced
(var) @ident

;; bindings
(match_expr lhs: (_) @binding.pattern rhs: (_) @binding.value) @binding

;; calls
(call expr: (_) @call.callee args: (expr_args) @call.args) @call
(remote module: (remote_module (_) @callee.module) fun: (_) @callee.name)

;; functions: one @function per clause (multi-clause functions yield several)
(function_clause
  name: (_) @function.name
  args: (expr_args) @function.params
  body: (clause_body) @function.body) @function

;; tail positions
(clause_body (_) @return.value .)
[(case_expr) (if_expr) (try_expr) (receive_expr) (block_expr) (paren_expr)] @return.container

;; pattern-matching branches
(case_expr expr: (_) @branch.subject)
(cr_clause pat: (_) @branch.pattern) @branch

;; constructors and field access
[(tuple) (list) (record_expr) (record_update_expr) (map_expr) (map_expr_update)] @construct
(record_field name: (_) @construct.field.name expr: (field_expr (_) @construct.field.value))
(map_field key: (_) @construct.field.name value: (_) @construct.field.value)
(record_field_expr expr: (_) @field.container field: (record_field_name (_) @field.name)) @field

;; literals
[(integer) (float) (string) (atom) (char)] @literal

;; things the engine must stop at
[(receive_expr) (anonymous_fun) (macro_call_expr) (list_comprehension)
 (binary_comprehension) (map_comprehension) (catch_clause) (try_after)] @opaque
```

If a node name or field in this file does not compile against `tree-sitter-erlang 0.20`, fix the query against `node-types.json` from the vendored grammar — do not drop the capture.

- [ ] **Step 5: Add constants** to `vocab.rs`: `RETURN_CONTAINER, BRANCH, BRANCH_PATTERN, BRANCH_SUBJECT, CONSTRUCT, CONSTRUCT_FIELD_NAME, CONSTRUCT_FIELD_VALUE, FIELD, FIELD_CONTAINER, FIELD_NAME, CALLEE_MODULE, CALLEE_NAME, OPAQUE`.

- [ ] **Step 6: Run** — PASS. **Step 7: Commit** `feat(lang): erlang capture queries [<id>]`

---

### Task 7: Structural questions on a parsed document

**Files:**
- Create: `engine/src/syntax.rs`
- Test: `engine/tests/syntax_erlang.rs` (reuses `fixtures/erlang/queries/sample.erl`)

**Interfaces:**
- Produces:
  ```rust
  pub struct Doc<'l> { pub path: PathBuf, pub text: String, pub tree: tree_sitter::Tree, lang: &'l Language,
                       caps: Vec<Cap> /* every (capture_name, node) match, sorted by start byte */ }
  #[derive(Clone, Copy)] pub struct N<'t> (pub tree_sitter::Node<'t>);   // thin wrapper
  pub struct FnDecl<'t> { pub node: N<'t>, pub name: String, pub params: Vec<N<'t>>, pub body: N<'t> }
  pub struct CallSite<'t> { pub node: N<'t>, pub callee: N<'t>, pub args: Vec<N<'t>> }
  pub enum Role<'t> {
      BoundBy { pattern: N<'t>, value: N<'t> },      // ident inside @binding.pattern of a @binding
      Param { func: FnDecl<'t>, index: usize },      // ident is (inside) the index-th param
      BranchPattern { pattern: N<'t>, subject: N<'t> }, // ident inside @branch.pattern; subject from enclosing @branch.subject
      Opaque(N<'t>),                                 // ident inside an @opaque construct
      Use,                                           // plain use (none of the above)
  }
  impl<'l> Doc<'l> {
      pub fn parse(lang: &'l Language, path: PathBuf, text: String) -> Doc<'l>
      pub fn ident_at(&self, p: Pos) -> Option<N>                       // smallest @ident containing p
      pub fn role_of(&self, ident: N) -> Role
      pub fn enclosing_function(&self, n: N) -> Option<FnDecl>
      pub fn call_at(&self, n: N) -> Option<CallSite>                   // n is a @call node or its callee
      pub fn calls_containing(&self, p: Pos) -> Vec<CallSite>           // innermost first; used at call-site locations
      pub fn arg_index(&self, call: &CallSite, n: N) -> Option<usize>   // which argument contains n
      pub fn returns_of(&self, f: &FnDecl) -> Vec<N>                    // tail expressions, through @return.container
      pub fn is_literal(&self, n: N) -> bool                            // n is @literal, or a @construct all of whose
                                                                        //   named leaves are @literal
      pub fn is_opaque(&self, n: N) -> bool
      pub fn field_access(&self, n: N) -> Option<(N /*container*/, String /*field*/)>
      pub fn construct_field(&self, construct: N, field: &str) -> Option<N>   // value for field name
      pub fn destructure(&self, pattern: N, ident: N, value: N) -> Option<N>  // sub-value matching ident's sub-pattern
      pub fn callee_text(&self, call: &CallSite) -> String              // "maps:get" or "pick"
      pub fn callee_name_pos(&self, call: &CallSite) -> Pos             // position to send to host/definition
      pub fn text_of(&self, n: N) -> &str
      pub fn line_of(&self, n: N) -> &str                               // trimmed source line (snippet)
      pub fn pos_of(&self, n: N) -> Pos
      pub fn has_cap(&self, n: N, cap: &str) -> bool
  }
  ```
- `returns_of` rule (generic): start with the last named child of `body` (Erlang `returns = "tail"`; for `"return"` languages collect `@return.value` nodes that are `return_statement` values instead — M2 wires the quirk; M1 implements the `tail` branch only and returns `unimplemented!()`-free empty vec plus a logged warning for other variants). For each candidate: if it `has_cap(RETURN_CONTAINER)`, replace it by every `@return.value` node whose *nearest* `@return.container` ancestor is this node, recursively; otherwise keep it.
- `destructure` rule: if `pattern` and `value` have the same grammar `kind()` and both are `@construct`: tuple/list → match the named child of `value` at the index of the child of `pattern` that contains `ident`, recursing; record/map → find the `@construct.field.name` whose value subtree contains `ident`, then `construct_field(value, that_name)`, recursing. Any mismatch → `None` (caller uses the whole value, `via: match`).

- [ ] **Step 1: Failing tests** `engine/tests/syntax_erlang.rs`

```rust
use whence::{lang::Registry, syntax::{Doc, Role}, pos::Pos};
fn doc() -> (Registry, String) { (Registry::embedded().unwrap(), include_str!("fixtures/erlang/queries/sample.erl").to_string()) }
/// Position of the `nth` (0-based) occurrence of `needle`, offset by `skip` bytes into it.
fn at_skip(text: &str, needle: &str, nth: usize, skip: usize) -> Pos {
    let mut from = 0;
    for _ in 0..nth { from += text[from..].find(needle).unwrap() + needle.len(); }
    let idx = from + text[from..].find(needle).unwrap() + skip;
    whence::pos::pos_of(text, idx)
}
fn at(text: &str, needle: &str, nth: usize) -> Pos { at_skip(text, needle, nth, 0) }

#[test] fn role_of_binding_param_and_branch() {
    let (reg, text) = doc(); let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let body = d.ident_at(at(&text, "Body = ", 0)).unwrap();
    assert!(matches!(d.role_of(body), Role::BoundBy{..}));
    let req0 = d.ident_at(at(&text, "Req0, Opts", 0)).unwrap();
    assert!(matches!(d.role_of(req0), Role::Param{index:0,..}));
    let v = d.ident_at(at_skip(&text, "{ok, V}", 0, 5)).unwrap();   // the V
    assert!(matches!(d.role_of(v), Role::BranchPattern{..}));
}
#[test] fn returns_of_handle_goes_through_case() {
    let (reg, text) = doc(); let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let f = d.enclosing_function(d.ident_at(at(&text, "Body = ", 0)).unwrap()).unwrap();
    let rs: Vec<&str> = d.returns_of(&f).iter().map(|n| d.text_of(*n)).collect();
    assert_eq!(rs, vec!["{V, R}", "{0, R}"]);
}
#[test] fn call_site_args_and_callee() {
    let (reg, text) = doc(); let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let limit_ident = d.ident_at(at(&text, "Limit = ", 0)).unwrap();
    let Role::BoundBy{value, ..} = d.role_of(limit_ident) else { panic!() };
    let call = d.call_at(value).unwrap();
    assert_eq!(d.callee_text(&call), "maps:get");
    assert_eq!(call.args.len(), 3);
    assert_eq!(d.arg_index(&call, call.args[1]), Some(1));
}
#[test] fn destructure_tuple_and_record() {
    let (reg, text) = doc(); let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    // pattern {ok, V} against value {ok, N * 2} → N * 2
    let v = d.ident_at(at(&text, "V} ->", 0)).unwrap();
    let Role::BranchPattern{pattern, ..} = d.role_of(v) else { panic!() };
    let pick_ret = d.ident_at(at(&text, "N * 2", 0)).unwrap();          // ident N
    let value = pick_ret.0.parent().unwrap().parent().unwrap();         // the tuple {ok, N * 2}
    assert_eq!(d.text_of(d.destructure(pattern, v, whence::syntax::N(value)).unwrap()), "N * 2");
    // field access
    let peer = d.ident_at(at(&text, "Peer = ", 0)).unwrap();
    let Role::BoundBy{value, ..} = d.role_of(peer) else { panic!() };
    let (cont, field) = d.field_access(value).unwrap();
    assert_eq!((d.text_of(cont), field.as_str()), ("Req0", "peer"));
    // construct_field on R = #req{...}
    let r = d.ident_at(at(&text, "R = #req", 0)).unwrap();
    let Role::BoundBy{value, ..} = d.role_of(r) else { panic!() };
    assert_eq!(d.text_of(d.construct_field(value, "peer").unwrap()), "Peer");
}
#[test] fn literal_and_opaque() {
    let (reg, text) = doc(); let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let limit = d.ident_at(at(&text, "Limit = ", 0)).unwrap();
    let Role::BoundBy{value, ..} = d.role_of(limit) else { panic!() };
    let call = d.call_at(value).unwrap();
    assert!(d.is_literal(call.args[2]));         // 10
    assert!(d.is_literal(call.args[0]));         // limit (atom)
    assert!(!d.is_literal(call.args[1]));        // Opts
}
```

- [ ] **Step 2: Run** `cargo test --test syntax_erlang` — FAIL.

- [ ] **Step 3: Implement** `syntax.rs`. `Doc::parse` runs the query once over the whole tree and stores `(capture_index, node)` pairs; `has_cap` checks membership by `node.id()`. `role_of` walks ancestors of the ident: the first ancestor that is a `@binding.pattern` (and whose parent is the `@binding`) → `BoundBy`; that is inside `@function.params` → `Param` with index = position of the top-level param child containing the ident; `@branch.pattern` → `BranchPattern` with subject = `@branch.subject` capture under the branch's parent; any `@opaque` ancestor before reaching the function body → `Opaque`. Order matters: check `@opaque` first only if the opaque node is *below* the pattern/params ancestor (an ident inside a fun inside a match RHS is `Use`, not `Opaque` — opaqueness is decided at step time on the value, not here).

- [ ] **Step 4: Run** — PASS. **Step 5: Commit** `feat(engine): structural questions over captured syntax [<id>]`

---

### Task 8: Trace core

**Files:**
- Create: `engine/src/trace/mod.rs`, `engine/src/trace/step.rs`, `engine/src/trace/frame.rs`
- Create fixtures: `engine/tests/fixtures/erlang/{local_chain,param_callers,call_result,external_and_entry,limits}/`
- Test: `engine/tests/replay.rs`

**Interfaces:**
- Produces:
  ```rust
  pub struct TraceRequest { pub root: PathBuf, pub file: PathBuf, pub pos: Pos, pub limits: Limits }
  #[derive(Debug, thiserror::Error)] pub enum TraceError {
      #[error("no language for {0}")] NoLanguage(PathBuf),
      #[error("cursor is not on an identifier")] NotIdentifier,
      #[error(transparent)] Host(#[from] HostError),
      #[error(transparent)] Io(#[from] std::io::Error) }
  pub fn trace(host: &mut dyn Host, reg: &Registry, req: &TraceRequest) -> Result<Tree, TraceError>
  ```
- Internal:
  ```rust
  // frame.rs
  pub struct Frame { pub func_id: String /*file:name:arity*/, pub args: Vec<ExprRef> }  // ExprRef = (PathBuf, Pos) of the argument expression
  pub struct Ctx<'a> { pub host: &'a mut dyn Host, pub reg: &'a Registry, pub root: &'a Path,
                       pub docs: HashMap<PathBuf, Doc<'a>>, pub frames: Vec<Frame>,
                       pub visited: HashSet<(PathBuf, Pos, u64)>, pub limits: Limits,
                       pub deadline: Instant, pub node_count: u32, pub truncated: u32 }
  impl Ctx { pub fn doc(&mut self, file: &Path) -> Result<&Doc, TraceError>  // fetch via host.text, parse, cache
             pub fn frame_hash(&self) -> u64
             pub fn in_root(&self, file: &Path) -> bool }
  // step.rs
  pub enum Expr { Ident(PathBuf, Pos), Value(PathBuf, Pos) }   // Ident = a variable occurrence; Value = any expression node
  pub fn expand(ctx: &mut Ctx, e: &Expr, depth: u32) -> Node    // builds the node for e and, if budget allows, its children
  ```
- Algorithm (`expand`), following spec §5.2 exactly:
  1. Budget check: `depth >= limits.depth` → stop `limit: depth`; `node_count >= limits.nodes` → stop `limit: nodes`; `Instant::now() > deadline` → stop `limit: time`. Increment `node_count` when a node is created.
  2. Cycle check on `(file, pos, frame_hash)` → stop `unresolved: recursion`.
  3. `Expr::Ident` (variable use): `host.definition(file, pos)`. Empty → stop `unresolved: no definition from language server`. Take the first location in the same file, else the first. Load its doc, `ident_at(def_pos)` → if none, stop `unresolved: definition is not an identifier`. If the definition is the same position as the use, treat the use as the definition site. Then `role_of`:
     - `BoundBy{pattern,value}` → node `binding` `via: match`, label ident text, loc = def pos; children = `[expand(Value(destructure(pattern, ident, value) or value))]`.
     - `Param{func,index}` → if `frames.last()` has `func_id == func` → node `param` `via: arg`, children = `[expand(Value(frame.args[index]))]` (no host call). Else `host.references(file, name_pos_of(func), false)`; filter to locations in root; for each, load doc, `calls_containing(loc.pos)` → innermost call whose callee text ends with the function name and `args.len() == params.len()`; collect `(file, args[index])`. Zero → stop `entry_point: no call sites of name/arity`. Apply fan-out: keep the first `limits.fanout` (same-file first, then path, then position), set `truncated` to the rest. Node `param`, children = one `expand(Value(arg))` per kept call site, each child `via: arg`.
     - `BranchPattern{pattern, subject}` → node `binding` `via: match`, children = `[expand(Value(destructure(pattern, ident, subject) or subject))]`.
     - `Opaque(n)` → stop `unresolved: bound inside <kind>` (e.g. `bound inside anonymous_fun`).
     - `Use` → stop `unresolved: definition site not recognised`.
  4. `Expr::Value(node)`:
     - `is_literal` → stop `literal`.
     - `has_cap(IDENT)` → delegate to the Ident rule (a value that is just a variable).
     - `is_opaque` → stop `unresolved: <kind>`.
     - `field_access` → `(container, field)`: search the current function backwards from this node for a `@binding` whose pattern is the container ident and whose value is a `@construct` with `construct_field(value, field)` → node `field` `via: field_set`, child `expand(Value(that))`; else stop `unresolved: field <field> of <container>`.
     - `call_at` → `host.definition(callee_name_pos)`. Empty → stop `unresolved: callee not found`. If any definition file is outside root → stop `external: <callee_text>`. Else load the definition doc, `enclosing_function` of the definition position (all clauses: every `FnDecl` in that file with the same name and arity — clauses are separate `@function` matches). Push `Frame{func_id, args}`; for each clause, for each `returns_of(clause)` → child `expand(Value(ret))` `via: return`; pop frame. Fan-out applies across the combined return list. Node kind `call_result`, label `callee_text`.
     - `@construct` with variable parts → node `binding`? No: a constructor is not itself traceable as one value. Emit stop `unresolved: constructed value <kind>` — user picks a component. (Documented limitation; the honesty rule forbids flattening.)
     - Otherwise (binary op, comparison, etc.) → stop `unresolved: <kind>`.
  5. Snippet = `doc.line_of(node)`, label = ident text or trimmed expression ≤ 40 chars with `…`.
- `trace()`: resolve language for `req.file` (else `NoLanguage`), `ident_at(pos)` (else `NotIdentifier`), root = `expand(Ident, 0)`, `stats` from ctx and `host.request_count()`, elapsed ms.

- [ ] **Step 1: Fixtures** — each dir has `.erl` files, `host.json`, `expected.json`. Write the sources first; produce `host.json` by hand for M1 (positions are what `erlang_ls` would return: variable definition = first occurrence in the clause; function definition = the clause head; references = call sites' function-name positions). `expected.json` is generated on the first green run by `whence replay <dir> <pos> --json > expected.json` after manual inspection — the test compares the whole tree.

`local_chain/a.erl` (trace `Z` on the last line):
```erlang
-module(a).
-export([f/1]).
f(X) ->
    Y = X,
    Z = Y,
    Z.
```
Expected shape: `binding Z (via match)` → `binding Y` → `param X` → `stop entry_point` (references of `f` return only the export? no: `host.json` references for `f/1` = `[]`).

`param_callers/` two files: `b.erl` defines `g(A) -> A.`; `c.erl` calls `g(1)`, `g(Val)` where `Val = os:getenv("V")`. Trace `A` in `g`: `param A` with 2 children (`literal 1`, `binding Val → call_result os:getenv → stop external`). `host.json`: references for `g` at both call sites; definition of `os:getenv` → `/usr/lib/erlang/lib/kernel/src/os.erl` (outside root).

`call_result/` : `h() -> R = pick(3), R.` and `pick(N) when N > 5 -> {ok, N}; pick(_) -> error.` Trace `R`: `call_result pick` with children `unresolved: constructed value tuple` and `literal error` — and, to check the frame, a variant `k() -> R = id(7), R.` `id(N) -> N.` → `call_result id` → `param N (via arg)` → `literal 7`, with `stats.host_requests` proving no `references` call was made (the replay `host.json` simply does not record references for `id`, so a wrong implementation errors).

`external_and_entry/`: `handle(Req) -> B = cowboy_req:body(Req), B.` — `cowboy_req` definition outside root → `external: cowboy_req:body`; `Req` param → `entry_point`.

`limits/`: `p(A) -> A.` with 12 recorded call sites, request `limits.fanout = 3` → `param A` has 3 children and `truncated: 9`; a self-recursive `loop(S) -> loop(S).` trace from `S` → `unresolved: recursion`.

- [ ] **Step 2: Test harness** `engine/tests/replay.rs`

```rust
use std::path::{Path, PathBuf};
use whence::{host_replay::ReplayHost, lang::Registry, pos::Pos, trace::{trace, TraceRequest}, tree::Limits};

struct Case { dir: &'static str, file: &'static str, pos: (u32, u32), limits: Limits }

fn run(c: &Case) -> serde_json::Value {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/erlang").join(c.dir);
    let mut host = ReplayHost::load(&dir).unwrap();
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest { root: dir.clone(), file: dir.join(c.file), pos: Pos{line:c.pos.0, col:c.pos.1}, limits: c.limits };
    let tree = trace(&mut host, &reg, &req).unwrap();
    let mut v = serde_json::to_value(&tree).unwrap();
    // normalise: strip absolute prefix and volatile stats
    relativise(&mut v, &dir); v["stats"]["ms"] = 0.into();
    v
}
fn relativise(v: &mut serde_json::Value, dir: &Path) { /* walk; for "file" strings strip dir prefix */ }

fn check(c: Case) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/erlang").join(c.dir);
    let got = run(&c);
    let exp_path = dir.join("expected.json");
    if std::env::var("UPDATE_EXPECTED").is_ok() { std::fs::write(&exp_path, serde_json::to_string_pretty(&got).unwrap()).unwrap(); }
    let exp: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&exp_path).expect("expected.json — run with UPDATE_EXPECTED=1 after inspecting output")).unwrap();
    pretty_assertions::assert_eq!(got, exp);
}

#[test] fn local_chain()        { check(Case{dir:"local_chain", file:"a.erl", pos:(5,4), limits:Limits::default()}) }
#[test] fn param_callers()      { check(Case{dir:"param_callers", file:"b.erl", pos:(2,8), limits:Limits::default()}) }
#[test] fn call_result()        { check(Case{dir:"call_result", file:"d.erl", pos:(2,20), limits:Limits::default()}) }
#[test] fn call_result_frame()  { let v = run(&Case{dir:"call_result", file:"d.erl", pos:(4,20), limits:Limits::default()});
                                  assert_eq!(v["root"]["children"][0]["children"][0]["via"], "arg");
                                  assert_eq!(v["root"]["children"][0]["children"][0]["children"][0]["stop"]["reason"], "literal"); }
#[test] fn external_and_entry() { check(Case{dir:"external_and_entry", file:"e.erl", pos:(2,30), limits:Limits::default()}) }
#[test] fn fanout_truncates()   { let v = run(&Case{dir:"limits", file:"l.erl", pos:(2,8), limits:Limits{fanout:3, ..Default::default()}});
                                  assert_eq!(v["root"]["children"].as_array().unwrap().len(), 3);
                                  assert_eq!(v["root"]["truncated"], 9); }
#[test] fn recursion_stops()    { let v = run(&Case{dir:"limits", file:"l.erl", pos:(4,12), limits:Limits::default()});
                                  let s = serde_json::to_string(&v).unwrap(); assert!(s.contains("recursion")); }
#[test] fn depth_limit_stops()  { let v = run(&Case{dir:"local_chain", file:"a.erl", pos:(5,4), limits:Limits{depth:1, ..Default::default()}});
                                  assert_eq!(v["root"]["children"][0]["stop"]["reason"], "limit"); }
```
(The `pos` tuples above are the cursor positions of the traced identifiers in the fixture files; recompute them against the files you actually write.)

- [ ] **Step 3: Run** `cargo test --test replay` — FAIL. **Step 4: Implement** `trace/` per the algorithm. **Step 5: Run** until every test passes; inspect each generated `expected.json` by eye against the shapes described in Step 1 before committing it. **Step 6: Commit** `feat(engine): trace core with replay fixtures [<id>]`

---

### Task 9: Server loop and CLI

**Files:**
- Create: `engine/src/server.rs`; Modify: `engine/src/main.rs`
- Test: `engine/tests/server.rs` (drives the binary over pipes)

**Interfaces:**
- Produces: `whence serve` (stdio JSON-RPC), `whence replay <fixture-dir> <file:line:col> [--json] [--fanout N] [--depth N]`, `whence replay --serve <fixture-dir>` (serves stdio but answers host requests itself from the fixture — for Neovim plugin tests), `whence --version`.
- Methods handled: `initialize` → `{version, languages}`; `whence/trace` → tree or error (`E_NO_LANGUAGE`, `E_NOT_IDENTIFIER`, `E_HOST`, `E_INVALID_PARAMS`); `shutdown` → `{}` then exit after the response is flushed; unknown → `E_METHOD_NOT_FOUND`. Requests arriving while a trace is running are answered `E_INVALID_REQUEST` "busy" (single-flight; the Neovim host serialises calls anyway).
- Human-readable replay output: one node per line, `"  " * depth + label + "  ← via" + "  file:line:col" + (stop ? "  [reason: detail]" : "") + (truncated ? "  … N more" : "")`. This format is shared with the panel renderer conceptually but implemented separately (Lua).

- [ ] **Step 1: Failing test** `engine/tests/server.rs`

```rust
use std::io::{BufReader, Write};
use std::process::{Command, Stdio};
use whence::protocol::*;

fn bin() -> Command { Command::new(env!("CARGO_BIN_EXE_whence")) }

#[test] fn initialize_trace_and_shutdown_over_stdio_with_replay_host() {
    let fx = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/erlang/local_chain");
    let mut child = bin().args(["replay", "--serve", fx]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    write_message(&mut stdin, &Message::Request(Request{ id: Id::Num(1), method: "initialize".into(),
        params: serde_json::json!({"root": fx, "capabilities": {"documentHighlight": false}}) })).unwrap();
    let Message::Response(r) = read_message(&mut stdout).unwrap().unwrap() else { panic!() };
    assert!(r.result.unwrap()["languages"].as_array().unwrap().iter().any(|l| l == "erlang"));
    write_message(&mut stdin, &Message::Request(Request{ id: Id::Num(2), method: "whence/trace".into(),
        params: serde_json::json!({"file": format!("{fx}/a.erl"), "line": 5, "col": 4}) })).unwrap();
    let Message::Response(r) = read_message(&mut stdout).unwrap().unwrap() else { panic!() };
    assert_eq!(r.result.unwrap()["root"]["label"], "Z");
    write_message(&mut stdin, &Message::Request(Request{ id: Id::Num(3), method: "shutdown".into(), params: serde_json::json!({}) })).unwrap();
    let _ = read_message(&mut stdout).unwrap().unwrap();
    assert!(child.wait().unwrap().success());
}

#[test] fn replay_cli_prints_tree() {
    let fx = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/erlang/local_chain");
    let out = bin().args(["replay", fx, "a.erl:5:4"]).output().unwrap();
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(out.status.success());
    assert!(s.lines().next().unwrap().starts_with("Z"));
    assert!(s.contains("[entry_point"));
}
```

- [ ] **Step 2: Run** — FAIL. **Step 3: Implement.** `server::serve(reader, writer, host_factory)`: a reader thread pushes messages to an `mpsc` channel; the main loop pops messages; for `whence/trace` it constructs the host (`RpcHost` sharing the writer via `Arc<Mutex<W>>` and the same receiver — pass the `Receiver` by `&mut` so `RpcHost` consumes responses while the trace runs, then control returns to the loop) or, in `--serve` replay mode, the `ReplayHost`. `main.rs` with clap subcommands `Serve`, `Replay { dir, target: Option<String>, serve: bool, json: bool, fanout, depth }`. Parse `file:line:col` where line/col are **1-based** on the CLI (human) and converted to 0-based. **Step 4: Run** — PASS. **Step 5: Commit** `feat(engine): stdio server and replay cli [<id>]`

---

### Task 10: Neovim — engine process and host answers

**Files:**
- Create: `nvim/lua/whence/engine.lua`, `nvim/lua/whence/host.lua`, `nvim/lua/whence/host_replay.lua`, `nvim/lua/whence/init.lua`, `nvim/plugin/whence.lua`
- Test: `nvim/tests/minimal_init.lua`, `nvim/tests/engine_spec.lua`

**Interfaces:**
- Produces:
  ```lua
  -- engine.lua
  M.start(opts) -> client | nil, err      -- opts.cmd (list), opts.root; uses vim.lsp.rpc.start
  M.trace(client, params, cb)              -- params = {file,line,col,limits?}; cb(err, tree)
  M.stop(client)
  -- host.lua  (all synchronous; called from vim.lsp.rpc's server_request dispatcher)
  M.handle(method, params) -> result, err  -- dispatches host/text, host/definition, host/references, host/documentHighlight
  M.bufnr_for(file) -> bufnr               -- loads hidden buffer, waits ≤ 2 s for an LSP client to attach
  -- init.lua
  M.setup(opts)   -- opts: { bin = nil, limits = {}, panel = { width = 60 } }
  M.trace()       -- at cursor in current buffer
  M.trace_at(file, line0, col0)   -- 0-based; used by tests and by `R` in the panel
  ```
- Position conversion: engine speaks UTF-16 columns; Neovim's LSP clients speak their negotiated `offset_encoding`. `host.lua` converts the incoming UTF-16 column to the client's encoding with `vim.str_byteindex(line_text, "utf-16", col)` then `vim.str_utfindex(line_text, client.offset_encoding, byte)`, and converts results back. Results from several clients are concatenated and de-duplicated by `(uri, range)`.
- `server_request` in `vim.lsp.rpc` must return synchronously, so `host.lua` uses `vim.lsp.buf_request_sync(bufnr, method, params, 5000)`; a `nil` (timeout) becomes a JSON-RPC error `-32000 "timeout"`. `host/text` returns `table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n") .. "\n"` when the buffer is loaded, else `vim.fn.readfile`.

- [ ] **Step 1: Test scaffolding**

`nvim/tests/minimal_init.lua`:
```lua
vim.opt.rtp:prepend(vim.fn.getcwd() .. "/nvim")
local plenary = os.getenv("PLENARY_DIR") or vim.fn.stdpath("data") .. "/lazy/plenary.nvim"
vim.opt.rtp:prepend(plenary)
vim.cmd("runtime plugin/plenary.vim")
vim.g.whence_bin = vim.fn.getcwd() .. "/target/debug/whence"
```

`nvim/tests/engine_spec.lua`:
```lua
local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"
describe("engine", function()
  it("initializes and traces through replay server", function()
    local engine = require("whence.engine")
    local client = assert(engine.start({ cmd = { vim.g.whence_bin, "replay", "--serve", fx }, root = fx }))
    local done, tree, err = false, nil, nil
    engine.trace(client, { file = fx .. "/a.erl", line = 5, col = 4 }, function(e, t) err, tree, done = e, t, true end)
    vim.wait(5000, function() return done end)
    assert.is_nil(err)
    assert.equals("Z", tree.root.label)
    engine.stop(client)
  end)
end)
```

- [ ] **Step 2: Run** `make test-nvim` — FAIL (module not found).

- [ ] **Step 3: Implement**

`engine.lua`:
```lua
local M = {}
local host = require("whence.host")

function M.start(opts)
  local client = vim.lsp.rpc.start(opts.cmd, {
    server_request = function(method, params)
      local ok, result, err = pcall(host.handle, method, params)
      if not ok then return nil, { code = -32000, message = tostring(result) } end
      return result, err
    end,
    notification = function() end,
    on_error = function(code, err) vim.schedule(function() vim.notify("whence: rpc error " .. tostring(code) .. " " .. vim.inspect(err), vim.log.levels.ERROR) end) end,
    on_exit = function(code) if code ~= 0 then vim.schedule(function() vim.notify("whence: engine exited " .. code, vim.log.levels.WARN) end) end end,
  }, { cwd = opts.root })
  if not client then return nil, "failed to start " .. table.concat(opts.cmd, " ") end
  local done, ierr = false, nil
  client.request("initialize", { root = opts.root, capabilities = { documentHighlight = true } }, function(e) ierr, done = e, true end)
  vim.wait(5000, function() return done end)
  if ierr then return nil, vim.inspect(ierr) end
  return client
end

function M.trace(client, params, cb)
  client.request("whence/trace", params, function(err, result) cb(err, result) end)
end

function M.stop(client) client.request("shutdown", {}, function() client.terminate() end) end
return M
```

`host.lua` — `handle` maps methods to LSP methods `textDocument/definition`, `textDocument/references` (params `context = { includeDeclaration = params.includeDeclaration }`), `textDocument/documentHighlight`; builds `{ textDocument = { uri = vim.uri_from_fname(file) }, position = { line, character } }` per client encoding; flattens `Location | Location[] | LocationLink[]` into `{ file, range }` with `col` converted back to UTF-16; highlight `kind` 1/2/3 → `"text"/"read"/"write"`.

`bufnr_for(file)`: `local b = vim.fn.bufadd(file); vim.fn.bufload(b); vim.bo[b].buflisted = false;` then `vim.wait(2000, function() return #vim.lsp.get_clients({ bufnr = b }) > 0 end)`. Files outside any LSP root simply get no clients → the request returns `{}`.

`host_replay.lua` (test support, also created in this task): `M.handle(fixture_dir)` returns a `handle(method, params)` function that answers from `fixture_dir/host.json` with the Task 3 key scheme and reads `host/text` from disk. `setup({ _replay = dir })` swaps `host.handle` for it, so Lua-level tests exercise the real `whence serve` path and the real dispatcher without an LSP. (`whence replay --serve` is for the engine's own tests only.)

`init.lua`: lazily starts one engine per `root` (root = `vim.fs.root(0, {"rebar.config","Cargo.toml","go.mod",".git"}) or cwd`); `trace()` reads cursor (`nvim_win_get_cursor` → line-1, byte col → UTF-16 via `vim.str_utfindex(line, "utf-16", byte)`), calls `engine.trace`, hands the tree to `panel.show` (Task 11). Binary resolution order: `opts.bin` / `vim.g.whence_bin` → `vim.fn.exepath("whence")` → `stdpath("data") .. "/whence/bin/whence"`; if none, `vim.notify` pointing at `:WhenceInstall`.

`plugin/whence.lua`: `vim.api.nvim_create_user_command("Whence", function() require("whence").trace() end, {})` and the lowercase alias via `cabbrev whence Whence` guarded so it only expands at the start of the command line.

- [ ] **Step 4: Run** `cargo build && make test-nvim` — PASS. **Step 5: Commit** `feat(nvim): engine client and lsp-backed host [<id>]`

---

### Task 11: Neovim — panel

**Files:**
- Create: `nvim/lua/whence/panel.lua`, `nvim/syntax/whence.vim` (or `nvim/queries/…` not needed — plain syntax matches)
- Test: `nvim/tests/panel_spec.lua`

**Interfaces:**
- Produces:
  ```lua
  M.show(tree, ctx)     -- ctx = { source_win = winid }
  M.render(tree) -> lines, index   -- pure: lines[i] = string, index[i] = node  (unit-testable)
  M.jump_current()      -- <CR>
  M.preview_current()   -- p
  M.rerun_current()     -- R  → require("whence").trace_at(node.loc.file, node.loc.line, node.loc.col)
  ```
- Line format (`render`): `string.rep("  ", depth) .. marker .. label .. viaText .. "  " .. relfile .. ":" .. (line+1) .. ":" .. (col16+1) .. stopText`, where `marker` is `"● "` for value nodes and `"■ "` for stop nodes, `viaText` is `"  ← match|rebind|mutation|arg|return|field_set"` or `""`, `stopText` is `"  [external: cowboy_req:body]"` etc. After a node's children, if `truncated > 0`, add a line `string.rep("  ", depth+1) .. "… " .. truncated .. " more"`. Second line under each value node? No — keep one line per node; the snippet goes into `p` preview and a virtual-text `EOL` extmark (highlight `Comment`), so lines stay short.
- Buffer options: `buftype=nofile bufhidden=hide swapfile=false modifiable=false filetype=whence foldmethod=indent shiftwidth=2 foldlevel=99`. Highlights: `WhenceStop` → `DiagnosticWarn`, `WhenceTrunc` → `Comment`, `WhenceLoc` → `Directory`, linked by default; `syntax/whence.vim` matches `\[\w\+:.*\]$`, `… \d\+ more`, and `\S\+:\d\+:\d\+`.
- Jump: `<CR>` → pick target window: `ctx.source_win` if valid else the previous window (`vim.fn.win_getid(vim.fn.winnr("#"))`), `vim.api.nvim_set_current_win`, `vim.cmd.edit(vim.fn.fnameescape(file))`, cursor at `(line+1, byte col)` with byte col from `vim.str_byteindex(linetext, "utf-16", col)`. `p` does the same without leaving the panel and centres with `zz`. `q` closes the window.

- [ ] **Step 1: Failing test** `nvim/tests/panel_spec.lua`

```lua
describe("panel.render", function()
  it("renders nodes, via, location and stops", function()
    local tree = { root = { id="a", kind="binding", label="Z", loc={file="/p/a.erl",line=5,col=4}, via="match", snippet="Z.", stop=vim.NIL, truncated=0,
      children = { { id="b", kind="param", label="X", loc={file="/p/a.erl",line=2,col=2}, via="match", snippet="f(X) ->", stop=vim.NIL, truncated=2,
        children = { { id="c", kind="stop", label="X", loc={file="/p/a.erl",line=2,col=2}, via=vim.NIL, snippet="f(X) ->",
                       stop={reason="entry_point", detail="no call sites of f/1"}, truncated=0, children={} } } } } } }
    local lines, index = require("whence.panel").render(tree, "/p")
    assert.equals("● Z  ← match  a.erl:6:5", lines[1])
    assert.equals("  ● X  ← match  a.erl:3:3", lines[2])
    assert.equals("    ■ X  a.erl:3:3  [entry_point: no call sites of f/1]", lines[3])
    assert.equals("    … 2 more", lines[4])
    assert.equals("c", index[3].id)
    assert.is_nil(index[4])
  end)
end)
describe("panel.show", function()
  it("opens a whence buffer and jumps on <CR>", function()
    local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"
    vim.cmd.edit(fx .. "/a.erl")
    require("whence").setup({ bin = vim.g.whence_bin, _replay = fx })   -- _replay: test hook making init start `replay --serve`
    require("whence").trace_at(fx .. "/a.erl", 5, 4)
    vim.wait(5000, function() return vim.bo.filetype == "whence" end)
    assert.equals("whence", vim.bo.filetype)
    vim.api.nvim_win_set_cursor(0, { 2, 0 })
    vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<CR>", true, false, true), "x", false)
    assert.equals("erlang", vim.bo.filetype)
    assert.equals(4, vim.api.nvim_win_get_cursor(0)[1])   -- Y = X, is line 4 (1-based)
  end)
end)
```

- [ ] **Step 2: Run** — FAIL. **Step 3: Implement** `panel.lua` as specified; `render` takes `(tree, root)` and relativises `loc.file` against `root` for display. `show` reuses a single panel buffer (`vim.b.whence_index` holds the node index; the window is found by buffer, created with `vim.cmd("botright vsplit")` + width from config otherwise). Keymaps buffer-local via `vim.keymap.set("n", "<CR>", M.jump_current, { buffer = buf })` etc. Wire `init.lua` to call `panel.show(tree, { source_win = ..., root = ... })`. **Step 4: Run** — PASS. **Step 5: Commit** `feat(nvim): provenance panel with jump, preview, rerun [<id>]`

---

### Task 12: Neovim — recorder and installer

**Files:**
- Create: `nvim/lua/whence/record.lua`, `nvim/lua/whence/install.lua`, `nvim/lua/whence/version.lua`
- Modify: `nvim/plugin/whence.lua` (add `:WhenceRecord {dir}`, `:WhenceInstall`), `nvim/lua/whence/host.lua` (optional recording hook)
- Test: `nvim/tests/record_spec.lua`

**Interfaces:**
- `record.lua`: `M.begin(dir)` installs a wrapper around `host.handle` that appends each `(method, params) → result` to an in-memory log and copies every file served through `host/text` into `dir` (relative to the engine root); `M.finish()` writes `dir/host.json` in the Task 3 format (keys `rel:line:col[|decl|nodecl]`, `Location.file` relativised when under root, left absolute otherwise — that is how `external` definitions are recorded) and removes the wrapper. `:WhenceRecord {dir}` = `begin`, run `trace()` at the cursor, on completion `finish` and print the replay command `whence replay {dir} {rel}:{line+1}:{col+1}`.
- `install.lua`: `M.install(cb)` downloads `https://github.com/{repo}/releases/download/v{version}/whence-{target}.tar.gz` and `SHA256SUMS` with `vim.system({"curl","-fsSL",…})` into `stdpath("data")/whence/`, verifies with `vim.fn.sha256` against the listed sum, extracts with `tar -xzf`, `chmod +x`. `repo` = `vim.g.whence_repo or "lattenwald/whence"`, `version` from `version.lua` (bumped by the release process), `target` from `jit.os`/`jit.arch` → `x86_64-unknown-linux-gnu | aarch64-unknown-linux-gnu | x86_64-apple-darwin | aarch64-apple-darwin | x86_64-pc-windows-msvc`.

- [ ] **Step 1: Failing test** `record_spec.lua`

```lua
describe("record", function()
  it("writes a replayable fixture", function()
    local fx = vim.fn.getcwd() .. "/engine/tests/fixtures/erlang/local_chain"
    local out = vim.fn.tempname(); vim.fn.mkdir(out, "p")
    require("whence").setup({ bin = vim.g.whence_bin, _replay = fx })
    local record = require("whence.record")
    record.begin(out, fx)
    require("whence").trace_at(fx .. "/a.erl", 5, 4)
    vim.wait(5000, function() return vim.bo.filetype == "whence" end)
    record.finish()
    local host = vim.json.decode(table.concat(vim.fn.readfile(out .. "/host.json"), "\n"))
    assert.is_truthy(host.definition["a.erl:5:4"])
    assert.equals(1, vim.fn.filereadable(out .. "/a.erl"))
    -- and the engine can replay it
    local res = vim.system({ vim.g.whence_bin, "replay", out, "a.erl:6:5" }):wait()
    assert.equals(0, res.code)
    assert.is_truthy(res.stdout:find("^Z"))
  end)
end)
```
(Works because `_replay` from Task 10 routes answers through `host.handle`, which is what the recorder wraps.)

- [ ] **Step 2: Run** — FAIL. **Step 3: Implement** `record.lua`, `install.lua`, commands. **Step 4: Run** — PASS (`install` is not unit-tested; verify manually once the first release exists). **Step 5: Commit** `feat(nvim): fixture recorder and release installer [<id>]`

---

### Task 13: Docs and CI

**Files:**
- Create: `docs/PROTOCOL.md`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- Modify: `CLAUDE.md` (add the two commands: `make test`, `make test-nvim`), `README.md` unchanged

- [ ] **Step 1: `docs/PROTOCOL.md`** — the normative text of spec §4 plus: framing, the `Tree` JSON with one full example (copy the `local_chain` `expected.json`), error codes table from Task 2, the `host.json` fixture format from Task 3, and a "Writing a host" checklist (spawn `whence serve` with cwd = root; answer requests synchronously or with the request id; buffer text must include unsaved edits; positions UTF-16; dedupe multi-client results). Preamble as a bullet list (status, version, source of truth = spec).

- [ ] **Step 2: `ci.yml`** — on push/PR: `dtolnay/rust-toolchain@1.98`, `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`; a second job installs Neovim stable (`rhysd/action-setup-vim`) and plenary (`git clone --depth 1 https://github.com/nvim-lua/plenary.nvim`), builds debug binary, runs `PLENARY_DIR=… make test-nvim`.

- [ ] **Step 3: `release.yml`** — on tag `v*`: matrix over the five targets from Task 12 (`ubuntu-latest` ×2 with `cross` for aarch64, `macos-latest` ×2, `windows-latest`), `cargo build --release --target`, tar/zip as `whence-<target>.tar.gz` containing the binary, upload; final job assembles `SHA256SUMS` and creates the GitHub release with `softprops/action-gh-release`. Version comes from `engine/Cargo.toml`; a `make release-check` target asserts `nvim/lua/whence/version.lua` matches it.

- [ ] **Step 4: Verify** — `act` is not required; push a branch and confirm `ci.yml` is green; tag `v0.1.0` on the M1 completion commit and confirm assets appear; run `:WhenceInstall` on a machine without the binary.

- [ ] **Step 5: Commit** `docs: host protocol reference; ci: test and release workflows [<id>]`

---

## M1 exit check (spec §11)

On a real Erlang project with `erlang_ls` attached: `:Whence` on a request-handler variable yields a panel whose leaves are `external`/`entry_point`/`literal` stops or explicit `unresolved` reasons, `<CR>` lands on the right identifier, `R` re-traces from a node, and every edge shown is verified correct by reading the code. Record that session with `:WhenceRecord` into `engine/tests/fixtures/erlang/dogfood_<name>/` and add it to `replay.rs` — this is the first fixture whose `host.json` comes from a language server rather than by hand.

## Self-review notes

- Spec coverage: §3 architecture (T1, T9, T10), §4 protocol (T2, T3, T9, T13), §5.1 tree (T4), §5.2–5.5 algorithm/bounds/honesty (T7, T8), §6 language data + build embedding (T5, T6), §7 Neovim (T10–T12), §9 testing (T3 replay host, T8 fixtures, T10–T12 plenary, T12 recorder), §10 distribution (T12 installer, T13 release), §12 tickets (T1). §8 VS Code is M3, not in this plan.
- Known simplifications, deliberate: `host.json` fixtures for T8 are hand-written (the recorder that replaces this arrives in T12; the exit check adds the first recorded one). Constructed values (`{A, B}`) stop as `unresolved: constructed value` rather than fanning into components — revisit in M2 if it proves annoying in practice.
- Interface names used consistently across tasks: `Host`, `ReplayHost::load`, `RpcHost::new`, `Registry::embedded/for_file/by_name`, `Doc::parse/ident_at/role_of/…`, `trace(host, reg, req)`, `Limits`, `Node::stop`, Lua `engine.start/trace/stop`, `host.handle`, `panel.render/show`, `require("whence").trace_at`.
