# Variable provenance tool — intent

## Problem

When reading unfamiliar code (Erlang, Rust, Go, others later), I need to answer
"where does this value come from?" — through assignments, rebindings,
mutations, function parameters and call sites — back to the point where the
value enters the program from outside (user input, environment, network,
config, storage, external library).

Doing this by hand with go-to-definition / find-references loses the trail after
a few hops.

## What it should do

Given a variable at the cursor, show a walkable list or tree of where its value
came from: assignments, mutations, and the call sites feeding a parameter.

The trace goes as far as it can toward the program boundary. Where it cannot
continue — external code, a known input source, or something it can't follow —
it stops and says so. From that point the user takes over: jumps there, picks
the preceding variable or function, and invokes the tool again.

The tool should be honest about uncertainty. A visible "couldn't follow this"
is more useful than a plausible-looking wrong edge.

## Interface

- Called from Neovim first. Shows the result in a panel the user can navigate
  and jump from.
- Should later be usable from VS Code or standalone, to share with the team.
  Keep the editor-specific part thin; the rest should not depend on Neovim.

## Constraints

- Work on top of existing language servers rather than writing per-language
  analysis. Supplement with whatever else is practical.
- Multi-language from the start; Erlang, Rust and Go are the ones I use.
- Bound the output size. A parameter called from many places should not
  produce an unreadable tree.

## Not now

- Graphical rendering (HTML, SVG, diagrams). Possibly later, on top of the
  same data.
- Forward tracing.
- Handling every language quirk (message passing, aliasing, dynamic dispatch).
  These can simply stop the trace.

Everything not stated here is open. Choose the architecture, language,
protocol, and representation as you see fit, and note the trade-offs.
