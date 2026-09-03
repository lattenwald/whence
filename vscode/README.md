# Whence for VS Code

Where does this value come from? Put the cursor on a variable and run
**Whence: Trace Variable** (`Ctrl+Alt+W`, `Cmd+Alt+W` on macOS). The
provenance tree opens in the Panel: click a node to preview its location,
press Enter to open it, use the inline action to re-run from that node.

Requires a language server for the file's language (Erlang: `elp` or
`erlang_ls`). The engine binary is bundled; install the VSIX for your
platform from the GitHub release.

See the [project readme](https://github.com/lattenwald/whence) for what it
does and does not do.
