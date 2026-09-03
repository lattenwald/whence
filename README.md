# whence

Where does this value come from?

Put the cursor on a variable and ask. `whence` walks back through
assignments, rebindings, mutations, call-site arguments and callee returns,
and shows the trail as a tree you can jump around in. The trail ends where
the value enters the program from outside — or where the tool can no longer
follow, in which case it says so rather than guessing.

It runs inside your editor, on top of the language servers you already have.

See [docs/INTENT.md](docs/INTENT.md) for what it is meant to do and
[docs/superpowers/specs](docs/superpowers/specs) for how it is built.

## Neovim

Needs Neovim 0.11+, a language server attached to the buffer (Erlang: `elp`
or `erlang_ls`), and the `whence` engine binary.

Install the plugin from GitHub; with lazy.nvim:

```lua
{
  "lattenwald/whence",
  config = function()
    require("whence").setup({
      -- bin = "/path/to/whence",   -- engine binary; see below
      -- root = "/path/to/project", -- default: the language server's root
      -- limits = { depth = 64, fanout = 8, nodes = 400, time_ms = 10000, split = true },
      -- panel = { width = 60 },
    })
    vim.keymap.set("n", "<leader>w", "<Plug>(whence)")
  end,
}
```

The engine is a Rust binary. Either run `:WhenceInstall`, which downloads the
release matching the plugin version into Neovim's data directory, or build it
yourself and point the plugin at it:

```sh
cargo build --release --manifest-path engine/Cargo.toml
```

```lua
vim.g.whence_bin = "/path/to/whence/target/release/whence"
```

The plugin looks for the binary in `setup({ bin = ... })`, `vim.g.whence_bin`,
`$PATH`, then the `:WhenceInstall` location, in that order.

Commands: `:Whence` traces the identifier under the cursor into a side panel
(`<CR>` jumps, `p` previews, `R` re-runs from a node, `q` closes).
`:WhenceRecord <dir>` records the same trace as a replay fixture.

## VS Code

The extension is not on the Marketplace yet; install it from a VSIX.
Download the one for your platform from the
[latest release](https://github.com/lattenwald/whence/releases/latest):

| Platform | File |
|---|---|
| Linux x64 | `whence-linux-x64-<version>.vsix` |
| Linux arm64 | `whence-linux-arm64-<version>.vsix` |
| macOS Intel | `whence-darwin-x64-<version>.vsix` |
| macOS Apple silicon | `whence-darwin-arm64-<version>.vsix` |
| Windows x64 | `whence-win32-x64-<version>.vsix` |

Then either run **Extensions: Install from VSIX…** from the command palette or

```sh
code --install-extension whence-linux-x64-<version>.vsix
```

The engine binary is bundled inside the VSIX, so nothing else needs
installing. A language server for the file's language must be active
(Erlang: `elp` or `erlang_ls`); the extension asks it for definitions and
references. To upgrade, install the newer VSIX over the old one.

### Usage

Put the cursor on a variable and run **Whence: Trace Variable**
(`Ctrl+Alt+W`, `Cmd+Alt+W` on macOS; also in the editor context menu). The
tree opens in a **Whence** tab in the Panel (next to Terminal and Problems)
and the traced locations are highlighted in the editor.

In the tree:

- click a node to preview its location; `Enter` opens it;
- **Re-run From Here** (inline icon or right-click) restarts the trace from
  that node;
- the tab's title bar has **Re-run Last Trace** and **Clear**, also
  available from the command palette while a result is shown.

**Whence: Record Fixture** writes a replay fixture for the trace under the
cursor, for reproducing and reporting engine issues.

### Building from source

Needs Node.js and a Rust toolchain.

```sh
make vscode-deps                                         # npm ci
cargo build --release --target x86_64-unknown-linux-gnu  # your triple
make vsix TARGET=x86_64-unknown-linux-gnu
```

This writes `vscode/whence-<platform>-<version>.vsix`, which installs like a
downloaded one. `make test-vscode` runs the extension tests.

## License

[MIT](LICENSE).
