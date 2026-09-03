# whence — VS Code extension (M3) design

- Status: approved design, not yet implemented
- Date: 2026-09-03
- Parent: [whence design](2026-09-01-whence-design.md) §4 (protocol), §8 (this milestone), §9 (testing), §10 (distribution)
- Protocol reference: [docs/PROTOCOL.md](../../PROTOCOL.md)

## 1. Summary

A VS Code extension that plays the same host role the Neovim plugin plays:
spawn the engine binary, answer `host/*` requests from VS Code's own
providers, render the resulting tree, and reveal locations. It contains no
analysis logic. It ships as one platform-specific VSIX per release target
with the engine binary inside; nothing is downloaded at runtime.

## 2. Decisions

| Topic | Decision | Why |
|---|---|---|
| Binary delivery | Bundled in a platform-specific VSIX (`vsce package --target`). No download path, no path setting. | Zero network, no extension/engine version drift; the marketplace-supported mechanism. |
| Result view | A `TreeView` contributed to the bottom Panel. | The most common and idiomatic VS Code view for on-demand results (References, Call Hierarchy); native theming, keyboard navigation, accessibility. Users can drag it elsewhere. |
| RPC | `vscode-jsonrpc` over the child's stdio. | The library VS Code's LSP client is built on; handles framing and reverse requests. `vscode-languageclient` assumes an LSP server; hand-rolled framing re-implements it. |
| Recorder | `Whence: Record Fixture` command writing the Neovim recorder's format. | Fixtures are recorded, not written; VS Code's providers can answer differently from `vim.lsp`, and those differences belong in goldens. |
| Publishing | VSIX files attached to the GitHub Release. | No publisher account or secrets yet; marketplace and Open VSX are one CI step each, later. |
| Language-server readiness | No waiting or polling. | VS Code runs the providers registered at that moment. An early trace yields `unresolved` stops, which is honest; rerunning fixes it. Polling guesses at readiness. |
| Workspace-less window | The trace command reports "open a folder" and does nothing. | The engine needs a root; guessing one from the file path would make node ids and `external` stops depend on the guess. |

## 3. Layout and toolchain

```
vscode/
  package.json          manifest: commands, view, menus, keybinding, engines
  tsconfig.json         strict, ES2022, Node 20 types
  esbuild.mjs           bundles src/extension.ts → dist/extension.js
  eslint.config.mjs     @typescript-eslint
  .vscodeignore         ships dist/, bin/, package.json, README, LICENSE only
  bin/                  engine binary for the target being packaged (git-ignored)
  src/
    extension.ts        activation, command registration, wiring
    engine.ts           spawn, lifecycle, single-flight trace, RPC connection
    host.ts             host/text, host/definition, host/references, host/documentHighlight
    tree.ts             TreeDataProvider, tree items, reveal
    decorations.ts      editor decorations for traced locations
    record.ts           fixture writer
    targets.ts          Rust target triple ↔ VSIX target mapping
  test/                 @vscode/test-cli + Mocha specs
```

- `engines.vscode` is `^1.96.0`. Runtime dependency: `vscode-jsonrpc` only.
  Dev dependencies: `typescript`, `esbuild`, `@types/vscode`, `@types/node`,
  `@vscode/vsce`, `@vscode/test-cli`, `@vscode/test-electron`, `mocha`,
  `eslint`, `typescript-eslint`.
- Activation events are exactly the contributed commands and the view;
  never `*`.
- The extension version is `vscode/package.json`'s `version`; `make
  release-check` compares it with `engine/Cargo.toml` alongside the Neovim
  version file.
- Top-level `Makefile` gains `vscode-deps` (npm ci), `test-vscode`, and
  `vsix` (`TARGET=<rust triple>`; copies the matching binary into
  `vscode/bin/` and packages). `bin/` is git-ignored.

## 4. Engine lifecycle (`engine.ts`)

- The engine is spawned lazily on the first trace, with `cwd` at the
  workspace folder containing the active file, and reused until the
  extension deactivates or the process exits. On exit, pending traces are
  failed with an error and the next trace respawns. Exit with a non-zero code
  raises one error notification.
- Binary path: `<extensionPath>/bin/whence` (`whence.exe` on Windows). If it
  is missing (a `vsce package` without the binary, or running from source
  without `make vsix`), the trace command fails with a message naming the
  expected path.
- `initialize { root, capabilities: { documentHighlight: true } }` is sent
  once per spawn.
- Traces are single-flight in the extension as well: a second trace while
  one runs is rejected with an information message. A trace runs under
  `window.withProgress` in the notification area; the engine's time budget
  bounds it, so there is no cancel button.
- `shutdown` then `exit` on deactivate, with a short timeout before `kill`.
- All engine stderr and RPC errors go to a `LogOutputChannel` named "Whence".

## 5. Host answers (`host.ts`)

All positions are already UTF-16 in the VS Code API, so no conversion.

| Method | Implementation |
|---|---|
| `host/text` | The open `TextDocument`'s text if `workspace.textDocuments` has the file (unsaved edits included), else `workspace.fs.readFile` decoded as UTF-8. Mirrors Neovim: buffer first, disk second. |
| `host/definition` | `workspace.openTextDocument(uri)` (loads without showing an editor), then `vscode.executeDefinitionProvider`. |
| `host/references` | `openTextDocument`, then `vscode.executeReferenceProvider`. VS Code always asks providers with `includeDeclaration: true` and offers no way to change it, so the list is passed through regardless of the engine's flag. This is safe: the engine drops any reference that sits on a function declaration before counting call sites (`step.rs`, `declares_function`). Recorded fixtures therefore key the answer under the engine's flag, exactly as Neovim's do. |
| `host/documentHighlight` | `openTextDocument`, then `vscode.executeDocumentHighlights`; kinds mapped `Text → "text"`, `Read → "read"`, `Write → "write"`. |

- Results are flattened the way Neovim does: a `Location` is taken as is; a
  `LocationLink` contributes `targetUri` and `targetSelectionRange`.
  Duplicates by (file, range) are removed; VS Code has already merged
  providers, so this only catches a provider returning the same place twice.
- An empty result is returned as `[]`, never as an error. Only a thrown
  exception (a provider crash, an unreadable file) becomes a JSON-RPC error,
  which aborts the trace as the protocol prescribes.
- `openTextDocument` on a file outside the workspace still works; the
  engine's `external` stop decides what to do with it.

## 6. Tree view (`tree.ts`) and decorations (`decorations.ts`)

- View id `whence.tree`, name "Whence", contributed to the `panel` container.
  Hidden (`when: whence.hasResult`) until the first trace, then revealed with
  `reveal(root, { expand: true })` and every node expanded, since a bounded
  tree is meant to be read whole.
- Item mapping, from the protocol `Node`:
  - `label`: `node.label`.
  - `description`: `via` (when present) followed by the snippet, e.g.
    `match · Z = f(X).`.
  - `tooltip`: Markdown with root-relative path and 1-based `line:col`, the
    `kind`, and for stops `reason: detail`.
  - `iconPath`: `ThemeIcon` per kind: `symbol-variable` (binding),
    `git-branch` (branch), `symbol-parameter` (param), `symbol-method`
    (call_result), `symbol-field` (field), `circle-slash` (stop), the stop
    icon colored with `ThemeColor("problemsWarningIcon.foreground")` for
    `external`/`entry_point`/`literal`, `problemsErrorIcon.foreground` for
    `unresolved`/`limit`.
  - `contextValue`: `whence.node`, or `whence.stop` for stops, so menus can
    differ.
  - A `truncated > 0` node gets a last child item `… N more` with
    `contextValue: whence.truncated`, no command, `disabled` styling via a
    `ThemeIcon("ellipsis")` and dimmed description.
  - `id`: the engine's `node.id`, which is stable across reruns of the same
    tree, so selection and expansion survive a refresh.
- Interactions follow the References view:
  - single click: `command: whence.preview` → open the location in a
    preview editor beside the panel with `preserveFocus: true`;
  - Enter or double click: `whence.open` → same, `preserveFocus: false`,
    `preview: false`;
  - inline action and context menu on nodes: `whence.rerunFromNode`
    (calls the trace with the node's `loc`);
  - view title: `whence.rerun` (last trace again), `whence.clear`,
    built-in `collapseAll`.
- Editor commands: `whence.trace` ("Whence: Trace Variable") in the Command
  Palette and the editor context menu, enabled `when: editorTextFocus`.
  Default keybinding `ctrl+alt+w` (`cmd+alt+w` on macOS). The command takes
  the active editor's selection start; the engine decides whether it is on
  an identifier and reports otherwise.
- Decorations: while a result is held, each node location gets an
  `editor.wordHighlightBackground` decoration in every visible editor
  showing that file, and the selected node's location gets
  `editor.wordHighlightStrongBackground`. Both are `ThemeColor`s so every
  theme applies. Cleared on `whence.clear` and replaced on a new trace.
- Context key: `whence.hasResult`.

## 7. Recorder (`record.ts`)

- Command `whence.record` ("Whence: Record Fixture"). Asks for an empty
  output directory with `showOpenDialog` (folder), refuses a non-empty one,
  then runs a trace from the cursor with recording on.
- Output is byte-compatible with the Neovim recorder, so the engine's
  replay tests and `whence replay` read it unchanged:
  - `host.json`: `{ definition, references, documentHighlight }` sections
    keyed `<root-relative file>:<line>:<col>` (`|decl` / `|nodecl` suffix
    for references), values as returned to the engine but with file paths
    made portable: root-relative when under the root, `$HOME/…` when under
    the home directory, absolute otherwise.
  - every in-root file the engine asked `host/text` for, copied verbatim
    under its root-relative path;
  - `whence-record.json`: `{ root, file, line, col, conflicts }` where
    `file:line:col` is the cursor the trace started from and `conflicts`
    lists keys that received two differing answers (first answer kept).
- The recorder wraps the host handler for the duration of one trace, so
  `host.ts` does not know about it.

## 8. Packaging and CI

- `targets.ts` (also imported by `esbuild.mjs` for the packaging script)
  maps release triples to VSIX targets:
  `x86_64-unknown-linux-gnu → linux-x64`, `aarch64-unknown-linux-gnu →
  linux-arm64`, `x86_64-apple-darwin → darwin-x64`, `aarch64-apple-darwin →
  darwin-arm64`, `x86_64-pc-windows-msvc → win32-x64`. A test asserts this
  set equals the release workflow's matrix, as the Neovim install test does.
- `release.yml` gains a `vsix` job after `build`, one matrix entry per
  target: download that target's binary artifact, unpack into `vscode/bin/`,
  `npm ci`, `npx vsce package --target <vsix target>`, upload the VSIX as an
  artifact. The `publish` job attaches `whence-<vsix target>.vsix` files
  next to the tarballs and includes them in `SHA256SUMS`.
- `make release-check` also compares `vscode/package.json` with the crate.
- Install for users: download the VSIX for their platform from the release
  and use "Extensions: Install from VSIX…". Documented in `README.md`.

## 9. Testing

- `@vscode/test-cli` (`.vscode-test.mjs`) runs Mocha specs in an Extension
  Development Host with the workspace set to a replay fixture directory. The
  engine binary is `target/debug/whence` from `cargo build`, pointed at
  through the environment variable `WHENCE_TEST_BIN`, which `engine.ts`
  honours only when `extensionMode` is `Test`; released builds ignore it.
- Specs, one file per module, following the same theater rules as the
  rest of the repo (no tests of VS Code APIs, no display text assertions,
  no restated fixtures):
  - `engine`: spawn, `initialize`, a trace over `whence replay --serve`,
    single-flight rejection, failure of a pending trace when the process
    dies, respawn afterwards;
  - `host`: text from an unsaved open document vs. disk; definition and
    references answered through providers registered by the test itself
    (a stub provider returning fixed locations and `LocationLink`s, so the
    flattening and de-duplication are exercised against real VS Code
    plumbing, not a mock); an empty result is `[]`, a throwing provider
    becomes an error;
  - `tree`: every node maps to an item whose command carries that node's
    location; `… N more` items carry no command; preview keeps focus in the
    panel and open moves it; rerun from a node calls the trace with the
    node's location;
  - `record`: a recording produces a directory that `whence replay`
    accepts and that yields the same tree as the live trace;
  - `targets`: mapping equals the release matrix;
  - version: `package.json` version equals `engine/Cargo.toml`'s.
- `make test-vscode` runs `npm ci` if needed, `cargo build`, then the specs.
  CI runs it on `ubuntu-latest` with `xvfb-run`.

## 10. Out of scope

- Marketplace and Open VSX publishing (one CI step each, once a publisher
  exists).
- Lazy expansion, a web extension build, remote (SSH/WSL) specifics beyond
  what a platform VSIX already provides: VS Code installs the extension on
  the remote host, which is where the binary runs and where the workspace
  is, so nothing extra is needed.
- Any change to the engine or the protocol. If implementation finds one
  necessary, that is a parent-spec change made in the same commit.
