---
id: whe-vwc5
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
# Go zero value: capture the type as @binding.value @literal instead of @literal on the binding node

languages/go/whence.scm marks the whole var_spec @binding @literal and the engine reads it through Role::Declared { literal }, a role that exists for one language and bypasses value(). Capturing the type as the value ((var_spec name: (identifier) @binding.pattern type: (_) @binding.value @literal !value) @binding) lets Role::BoundBy and the generic literal stop handle it; Role::Declared loses its flag. Changes the golden detail from 'zero value' to the type node, so decide the wording first.

