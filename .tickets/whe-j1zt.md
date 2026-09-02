---
id: whe-j1zt
status: open
deps: []
links: []
created: 2026-09-02T11:29:06Z
type: task
priority: 3
assignee: Alexander Q
tags: [later]
---
# node ids collide for identical expressions reached via different paths (diamond)

Seen in engine/tests/fixtures/erlang/diamond/expected.json: both literal 3 leaves share id db9f3c11d80cab00 (same rel path, pos, kind, frame hash after the caller frame is popped). Harmless for the one-shot panel; blocks keying UI state or lazy expansion (whence/expand {id}) by id. Options: include the parent id in the hash (path-dependent, still stable), or a per-tree ordinal suffix.

