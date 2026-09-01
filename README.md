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
