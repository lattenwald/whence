---
id: whe-45c5
status: open
deps: []
links: []
created: 2026-09-04T13:10:19Z
type: chore
priority: 3
assignee: Alexander Q
parent: whe-ooar
tags: [m2]
---
# record-fixture.lua should write recorded_with and a relative root itself

The recorder writes an absolute root and no recorded_with; both are normalised by hand after each recording. The script knows the project and can run rust-analyzer --version / gopls version.

