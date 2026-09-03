# Deviations

Deviations from the plan being executed, and why.

## M3 — VS Code extension (`docs/superpowers/plans/2026-09-03-m3-vscode.md`)

### Task 1 (`whe-jw7g`)

- `lint` script is `eslint .`, not `eslint src test scripts`: eslint 10 fails
  with "No files matching the pattern" until `scripts/` exists (Task 6).
- `.gitignore` also ignores `vscode/.vscode-test/`, the VS Code build the test
  runner downloads.
- `npm install` resolved TypeScript 6 and eslint 10 rather than the plan's
  TypeScript 5; both compile and lint the sources clean.

### Task 2 (`whe-gakf`)

- `vscode/eslint.config.mjs` gained an `ignores` entry for `.vscode-test/`,
  `out/` and `dist/`. `eslint .` does not read `.gitignore`, so once the test
  runner had downloaded VS Code it tried to lint the bundled extensions' own
  configs and died on their missing dev dependencies.
