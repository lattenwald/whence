---
id: whe-61if
status: closed
deps: []
links: []
created: 2026-09-04T14:49:46Z
type: chore
priority: 4
assignee: Alexander Q
parent: whe-ooar
tags: [m2, simplify]
---
# receiver shift is encoded three ways in step.rs

classify maps a caller slot through match (slot, shift) with i-1; param_like maps back with i+shift and (shift==1).then(..); call_result normalises with args.remove(0). One helper shifted(call, decl) -> (receiver, args) that all three read slots from uniformly would remove the (Slot::Arg(0), true) special case. Not done during /simplify because the region is subtle and the change is not mechanically behaviour-preserving.

