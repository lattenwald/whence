---
id: whe-y3sm
status: open
deps: []
links: []
created: 2026-09-02T12:32:30Z
type: task
priority: 3
assignee: Alexander Q
tags: [later]
---
# list stray (non-call-site) references as extra siblings when call sites also exist

step.rs param row: strays are reported only when no call sites exist (spec §5.2 narrowed accordingly). Emitting them as extra unresolved siblings would show callback-style feeds (fun f/N passed to lists:map) alongside direct calls.

