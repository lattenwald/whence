---
id: whe-colm
status: open
deps: []
links: []
created: 2026-09-04T16:25:17Z
type: chore
priority: 4
assignee: Alexander Q
tags: [perf]
---
# bucket captures by id at parse time

Doc::caps is one flat Vec sorted by span, so caps_within does a partition_point then walks every cap in the range whatever its capture id (six such walks per fn_decl), and caps_containing (covers, node_at, ident_at) is a full linear scan with no binary search. Parse into by_cap: Vec<Vec<Span>> indexed by capture id, each sorted by start: caps_within becomes a binary search plus a slice of one bucket, caps_containing scans one bucket, and the capture-name lookup happens once per call instead of per cap. caps_within also resolves a tree_sitter node for every hit before callers filter or take .first() — yielding spans and resolving only survivors would drop that too. Subsumes the per-language capture_ids HashMap added for whe-r5uh, and could go further: vocab as an enum with caps: [Option<u32>; N] on Language removes the hashing in has_cap entirely.

