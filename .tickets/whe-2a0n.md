---
id: whe-2a0n
status: open
deps: []
links: []
created: 2026-09-04T16:49:22Z
type: bug
priority: 2
assignee: Alexander Q
tags: [m2, go]
---
# Go method expression T.M(s, x) binds the receiver twice

languages/go/whence.scm captures the operand of a selector callee as @call.receiver, so for a method expression T.M(s, x) the type name T becomes the receiver and s, x stay as arguments. receiver_shift cannot correct it (it fires only when the call has no @call.receiver, i.e. Rust T::m(s, x)), so call_result builds a frame with receiver=T against a declaration with one parameter: tracing the method's receiver reaches the type name and tracing its first parameter is off by one — wrong edges, not missing ones. Reported by /code-review on 2026-09-04 (v0.3.0..HEAD); confirm with a syntax test over call_using first. The fix is in the query or the vocabulary, not per-language Rust: a method expression's operand is a type, not a value, and the engine has no capture that says so.

