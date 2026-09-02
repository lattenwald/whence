---
id: whe-rc6c
status: open
deps: []
links: []
created: 2026-09-02T14:06:25Z
type: task
priority: 3
assignee: Alexander Q
tags: [later, ui]
---
# stop.detail shows raw tree-sitter kind ids (macro_call_expr, map_expr_update)

The panel prints stops as [reason: detail]; six places in trace/step.rs put a grammar kind id in detail: literal stops (integer/atom), @opaque values (macro_call_expr, anonymous_fun), 'bound inside <kind>', 'constructed value <kind>', containers with no tails, and the fallback. The vocabulary is the grammar author's and drifts per language. Preferred fix: humanise the kind id generically in the engine (split underscores, drop a trailing expr/expression/literal), keeping engine-owned phrasing around it ('constructed value: map update'); per-language label tables or @opaque.* sub-captures stay available if a grammar's names are unreadable. Deferred until the current output has been seen in the panel on real code. Changes detail in most goldens; check panel_spec for a pinned literal detail.

