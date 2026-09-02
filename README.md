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

The plugin is the `nvim/` directory of this repository. Clone the repository
and point your plugin manager at that directory; with lazy.nvim:

```lua
{
  dir = "/path/to/whence/nvim",
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
