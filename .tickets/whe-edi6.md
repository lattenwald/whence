---
id: whe-edi6
status: open
deps: []
links: []
created: 2026-09-04T14:49:46Z
type: chore
priority: 4
assignee: Alexander Q
parent: whe-ooar
tags: [m2, simplify]
---
# @binding.element should mark the binding, not replace @binding.pattern

Loop bindings use @binding.element instead of @binding.pattern, so role_of tests has_cap(PATTERN) || has_cap(ELEMENT) and binding_parts (which only knows @binding.pattern) cannot see a loop binding. Keep @binding.pattern/@binding.value for loops and co-capture the marker on the binding node (@binding @binding.element), as .compound/.mutable do.

