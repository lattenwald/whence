---
id: whe-6b1p
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
# tuple index projection should come from a capture, not from parsing the field name as an integer

step.rs project()/sets_projection infer Proj::Index for Rust t.0 by parsing the @field.name text as usize. That reads a Rust-specific fact out of text; any language with numeric-looking keys inherits the guess. The grammar distinguishes field: (integer_literal) from field: (field_identifier): add a capture (e.g. @field.index) so field_access returns Proj::Field or Proj::Index directly.

