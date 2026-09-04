---
id: whe-4ufr
status: closed
deps: []
links: []
created: 2026-09-04T15:42:03Z
type: chore
priority: 4
assignee: Alexander Q
tags: [release]
---
# one source of truth for the version number

A release edits the version in five places (engine/Cargo.toml, Cargo.lock, nvim/lua/whence/version.lua, vscode/package.json, vscode/package-lock.json) and 'make release-check' only verifies afterwards that they agree. Consider deriving the plugin and extension versions from engine/Cargo.toml — a small 'make bump VERSION=x.y.z' target that rewrites them (npm version for the vscode pair, cargo set-version or sed for the engine), or generating nvim/lua/whence/version.lua at build time — so a release touches one number.

