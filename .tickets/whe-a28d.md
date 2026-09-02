---
id: whe-a28d
status: open
deps: []
links: []
created: 2026-09-02T12:32:30Z
type: task
priority: 4
assignee: Alexander Q
tags: [later]
---
# call_result children can exceed fanout when recursive-target stops are appended

step.rs: fanout truncates the clause plan, then recursive-target stops are appended, so children.len() can exceed limits.fanout.

