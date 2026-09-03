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

Download the `.vsix` for your platform from the
[latest release](https://github.com/lattenwald/whence/releases/latest)
(`linux-x64`, `linux-arm64`, `darwin-x64`, `darwin-arm64`, `win32-x64`) and
install it with **Extensions: Install from VSIX…** or

```sh
code --install-extension whence-linux-x64-<version>.vsix
```

The engine binary is inside the VSIX; nothing else to install. A language
server for the file's language must be active (Erlang: `elp` or
`erlang_ls`).

**Whence: Trace Variable** (`Ctrl+Alt+W`, `Cmd+Alt+W` on macOS, also in the
editor context menu) opens the tree in the Panel. Click a node to preview
its location, press Enter to open it, use the inline action to re-run from
that node. **Whence: Record Fixture** writes a replay fixture for the trace
under the cursor.

To build the extension from source: `make vsix TARGET=<rust triple>` after
`cargo build --release --target <rust triple>`.

## License

[MIT](LICENSE).
