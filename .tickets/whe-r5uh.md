---
id: whe-r5uh
status: open
deps: []
links: []
created: 2026-09-04T14:49:46Z
type: chore
priority: 4
assignee: Alexander Q
parent: whe-ooar
tags: [m2, perf]
---
# clauses_of builds every FnDecl in the file to find one function's clauses

syntax.rs clauses_of goes through functions(), which runs fn_decl (several cap scans each) for every @function in the file, then filters by name/arity and walks function_group per survivor. Start from @function.name captures whose text matches, then enclosing_function on those hits only. Also cap_index resolves capture names by linear string search on every has_cap call; resolving once per Language at Registry::embedded() would remove that.

