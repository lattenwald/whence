---
id: whe-plg5
status: open
deps: []
links: []
created: 2026-09-04T16:49:22Z
type: chore
priority: 4
assignee: Alexander Q
tags: [m2]
---
# a zero value's stop reads as a claim about the type node

Since whe-vwc5 the Go zero value is the declared type captured @binding.value @literal, so 'var p *T' traces to a literal stop labelled *T with detail 'pointer_type' — honest about the node, but it reads as a claim that the value is *T. A pending projection now lands there too: 'var s S; s.f' reports 'unresolved: no field f in a literal' at S where the old role gave a clean literal stop. Wants a detail that says the node is a type standing for its zero value; overlaps whe-rc6c (stop.detail shows raw tree-sitter kinds), so decide the wording for both at once.

