---
id: whe-kj9v
status: closed
deps: []
links: []
created: 2026-09-04T14:49:46Z
type: chore
priority: 3
assignee: Alexander Q
parent: whe-ooar
tags: [m2, simplify]
---
# @function.abstract as a marker on @function, not a parallel kind

Bodiless declarations (Rust trait signatures, Go interface methods) are captured as @function.abstract alone, so every owner lookup names both captures (caps_owned_by with [FUNCTION, FUNCTION_ABSTRACT], nearest_ancestor_with_any, declares_abstract next to declares_function), and enclosing_function/role_of still only look for @function: a parameter of a bodiless declaration falls to Role::Use. Capture them as @function @function.abstract on one node (body optional when abstract), like @assign.compound and @function.receiver.mutable; then abstractness is has_cap(f, FUNCTION_ABSTRACT) and the doubled API collapses. Update both query files, vocab docs and the M2 spec §6.

