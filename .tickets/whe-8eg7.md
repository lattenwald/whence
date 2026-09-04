---
id: whe-8eg7
status: closed
deps: []
links: []
created: 2026-09-04T16:19:06Z
type: bug
priority: 2
assignee: Alexander Q
tags: [m2, go]
---
# Go: var c, d = 1, 2 gives every name the whole value list

var_spec has no container node for the declared names (repeated name: fields), so (var_spec name: (identifier) @binding.pattern value: (expression_list) @binding.value) @binding matches once per name with the same @binding.value: 'c, d = 1, 2' yields pattern=c value='1, 2' and pattern=d value='1, 2'. destructure returns early because pattern == ident, so pattern_index is never consulted and d traces to the whole list — a wrong edge, not a missing one (short_var_declaration is unaffected: its left is an expression_list). Reported by a /simplify reviewer that ran the pattern against the grammar on 2026-09-04; confirm with a syntax test first. The fix is vocabulary, not a Go tweak: the engine needs a way to say the nth name of a declaration aligns with the nth value when the grammar gives the names no container — a per-name index capture, or letting positional() read the repeated fields.

