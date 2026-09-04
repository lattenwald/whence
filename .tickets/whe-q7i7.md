---
id: whe-q7i7
status: open
deps: []
links: []
created: 2026-09-04T10:18:48Z
type: bug
priority: 3
assignee: Alexander Q
parent: whe-ooar
tags: [m2]
---
# call_result: an implementation that is itself abstract stops as 'not a function'

gopls lists interfaces that embed/satisfy an interface method among textDocument/implementation results; such a location is a @function.abstract declaration, and the target loop reports 'definition of X is not a function'. Honest but misleading: check declares_abstract in the target loop and word it 'abstract implementation of X' (or expand it recursively with a depth cut).

