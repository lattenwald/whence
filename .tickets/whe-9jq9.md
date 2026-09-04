---
id: whe-9jq9
status: open
deps: []
links: []
created: 2026-09-04T11:27:10Z
type: bug
priority: 2
assignee: Alexander Q
parent: whe-ooar
tags: [m2]
---
# A trace stops at a reference expression instead of its operand

Recorded in engine/tests/fixtures/rust/mut_param: tracing the parameter of run/1 to its call site reaches the argument '&mut xs' and stops with 'unresolved: reference_expression'. Go's go/receiver shows the same shape for '&s'. Rust captures '&mut e' as @escape and Go '&e' likewise, but neither language marks the reference as @through, so the operand is never reached and a trace across a by-reference argument ends one node short of the variable it points at.

## Acceptance Criteria

Tracing a value bound to '&mut x' / '&x' continues into 'x'; the escape classification of the same node is unaffected.

