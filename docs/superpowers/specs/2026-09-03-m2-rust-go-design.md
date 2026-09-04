# whence — Rust and Go (M2) design

- Status: implemented
- Date: 2026-09-03
- Parent: [whence design](2026-09-01-whence-design.md) §4 (protocol), §5 (trace), §6 (language data), §9 (testing), §11 (milestones)
- Protocol reference: [docs/PROTOCOL.md](../../PROTOCOL.md)

## 1. Summary

M2 adds Rust and Go as language data and grows the engine's generic
machinery where those languages need it: writes to a variable after its
binding, places a variable may be written through a reference, calls to
methods that are declared abstractly, multi-value bindings, loop variables,
and functions that share a name. No per-language Rust code is added; every
new construct is a capture in the vocabulary plus a generic rule in the step
function. The exit criterion mirrors M1: on a real Rust project and a real
Go project, tracing a request-handler variable yields a tree that ends in
`external`/`entry_point` stops, and every edge shown is correct on
inspection.

## 2. Decisions

| Topic | Decision | Why |
|---|---|---|
| Writes after the binding | Occurrences of the variable in the same function, textually between binding and use, classified by syntax (`@assign`) into rebind/mutation nodes, newest first, the original binding last. | The spec's M1 parenthetical. Syntax classifies; the server only lists occurrences. |
| Occurrence source | `host/documentHighlight` when the host declared it; else `host/references` filtered to the use's file. One code path after the fetch. | Highlight is cheaper and file-local; references are the fallback every host already has. The kinds (`read`/`write`) are not needed, so nothing depends on how a server labels occurrences. |
| Reference escapes | An occurrence inside `&mut x` / `&x`, as the receiver of a method whose declaration has a mutable receiver, as an argument to a parameter declared `&mut`/pointer, or as the receiver of a method the engine cannot see (outside root) → `stop: unresolved: may be written by <call>`. | Honesty rule (§5.5): whether the callee wrote is unknowable without descending, so it is a candidate, never an edge. |
| Abstract methods | A callee whose definition is a bodiless declaration (`@function.abstract`) triggers `host/implementation`; every in-root implementation is a callee. A trait method with a default body is abstract *and* a candidate itself. | Choosing the default or stopping would both be guesses; listing implementations is what the server knows. New request, both hosts. |
| Multi-value bindings | A positional pattern against a call or other non-constructor value pushes a pending *index* projection, reusing the pending-field mechanism. | `a, b := f()` and `let (a, b) = f()` are the same shape as `R#r.a` with `f()` as the container. |
| Loop variables | `for x in xs` / `for i, e := range xs` bind through `via: element` to the iterable. | The value is derived from the iterable and nothing else the engine can see; `via: match` would claim the variable *is* the iterable. |
| Same-name functions | A function's clause set is the `@function` nodes under its nearest `@function.group` ancestor; without a group, the one node. `FuncId` carries the group's position, not just name/arity. | Rust `impl A { fn new() }` / `impl B { fn new() }` and Go methods on different types collide by name and arity; M1's name/arity collapse was an Erlang assumption. |
| Methods and receivers | `@call.receiver` at the call site, `@function.receiver` (and `.mutable`) on the declaration; the receiver is excluded from the positional parameters and travels in the frame separately. A path-style call to a method (`T::m(x)`) passes the receiver as its first argument. | Rust's `self` sits inside `parameters`; Go's sits outside. One model covers both. |
| Transparent wrappers | `@through`/`@through.inner` is applied at the start of `value`, `is_literal` and `destructure`, not only in `call_at`, and to a fixpoint, so nested wrappers (`(&x)`, a one-element `expression_list` around a composite literal) unwrap in one step; a wrapper around a bare identifier is a variable use at that identifier. | Rust `(e)`, `e?`, `e.await`, struct-expression bodies and Go one-element `expression_list`s wrap a value without changing it; one generic rule beats a capture per wrapper. |
| `lang.toml` quirks | `single_assignment` is kept and read (skips the occurrence step); `multi_assign` and `mutable_ref_markers` are deleted. | Both are expressed by captures now. A flag nothing reads is a lie in the data. |
| Fixture projects | Small hand-written Rust and Go projects under `engine/tests/fixtures/{rust,go}/<case>/`, recorded with `:WhenceRecord` against rust-analyzer and gopls. | Fixtures are recorded, not written. Nothing from a private codebase enters the repo. |

## 3. Trace model changes (parent §5.2)

### 3.1 Variable use

Unchanged first: `host/definition`, de-duplicated by (file, range). Then, unless
the language declares `single_assignment = true`, the engine fetches the
variable's *occurrences* in the use's file: `host/documentHighlight` at the
use when the host declared the capability, otherwise `host/references`
(`includeDeclaration: false`) filtered to that file. An occurrence is kept
when it lies inside the `@function` that owns the use, its start is after the
definition's start and before the use's start, and it is not the definition
itself. Definitions outside the use's file (globals, statics) get no
occurrence step.

Each kept occurrence `O` is classified in this order:

1. **Write.** `O` is inside the `@assign.target` of an `@assign` node `A` and
   the write reaches it. The target is taken through `@through` and, when it
   is a positional `@construct` (Go's `expression_list`), narrowed to the one
   element containing `O`: call that the *place*. The write reaches `O` when
   the place's chain of containers — each step a `@field`'s `@field.container`
   or a `@place.base` — contains `O`. The index of `x[i] = e` and the key of
   `m[k] = v` are therefore reads, not writes.
   Child: a `binding` node at `O`, `via: rebind` when the place is `O` itself
   and `A` is not `@assign.compound`, else `via: mutation`. Its one child is
   `A`'s `@assign.value`, narrowed through the target pattern like a binding
   (Go `a, b = b, a`); an `@assign` without a value (`x++`) gets a `stop:
   literal` child at `A`. With a pending projection `p`: a place `O.p…` sets
   it, so the child is the value `via: field_set` and `p` is consumed (a
   `Field` matches a named field, an `Index(i)` a `@field.name.index` reading
   `i`); a place that projects `O` elsewhere does not affect `p` and the
   occurrence is dropped; a write to `O` itself is kept whatever `p` is.
2. **Escape.** `O` is inside an `@escape` node → `stop: unresolved: may be
   written by <text of the enclosing statement or call>`, located at `O`.
3. **Mutable receiver.** `O` is inside the `@call.receiver` of a call `C`.
   `host/definition` on `C`'s callee: a declaration in root carrying
   `@function.receiver.mutable` → the same stop, detail naming `C`; a
   declaration outside root → the same stop with detail `may be written by
   external method <callee>`; in root without a mutable receiver → dropped.
4. **Mutable parameter.** `O` is inside argument `i` of a call `C` whose
   callee declaration is in root and whose parameter `i` carries
   `@param.mutable` → the same stop. Outside root → dropped: passing a plain
   value to unknown code is not evidence of a write.
5. Otherwise dropped (a plain read).

When nothing survives, the tree is exactly M1's: the definition node (or the
M1 `branch` over several definitions) stands for the use. Otherwise the use
becomes a `branch` node (`via: match`, label = the variable) whose children
are the classified occurrences newest first, then the definition node(s).
The fan-out bound and `split` apply to the occurrence list through
`candidates`; the definition nodes are appended after and are never cut, so
the original binding is always visible.

Known gap, stated rather than guessed at: a write textually after the use
that reaches it through a loop is not shown. Passing a Go slice, map or
pointer *variable* by value to code that writes through it is not detected;
only `&x` and pointer/`&mut` parameters are.

### 3.2 Binding site

`Role::BoundBy { pattern, value }` is joined by `Role::Declared` for a
declaration without a value. Rust `let x;` → `stop: unresolved: declared
without a value`. Go `var x T` binds instead: the query captures the type as
the `@binding.value`, marked `@literal`, so the generic literal stop reports
the type node — no zero-value rule in the engine.

Destructuring drops the requirement that pattern and value share a node
kind: Rust `tuple_pattern` vs `tuple_expression` and `struct_pattern` vs
`field_initializer_list` are different kinds of the same shape. The rule is
now: both `@construct`, both keyed (matched by `@construct.field.name` text)
or both positional with equal arity (matched by index). A positional pattern
of one or more elements against a value that is not a matching
construct (a call, an index expression) pushes `Proj::Index(i)` for the
identifier's position and traces the whole value; see §3.5.

`Role::ElementOf { value }` (a `@binding` marked `@binding.element`) yields a
`binding` node whose child is the iterable `via: element`. No destructuring.

### 3.3 Call to an abstract method

`call_result` accepts a definition that is `@function.abstract`. For each
such definition it sends `host/implementation` at the declaration's name and
treats every returned location like a definition: outside root counts toward
`external`, inside root must declare a function. An implementation may itself
be abstract — gopls lists the interfaces that embed a method among its
implementations — so the expansion repeats to a fixpoint. A declaration
already expanded contributes that expansion again (two interfaces embedding
one method both reach it); one whose expansion is still in flight is a cycle
and contributes nothing. An abstract declaration that has
a body (a trait default) is one more callee alongside its implementations. No
implementations and no body → `stop: unresolved: no implementation of
<name>`. A host without the `implementation` capability → `stop: unresolved:
abstract method <name>`.

### 3.4 Methods

A `CallSite` carries `receiver: Option<N>` from `@call.receiver`; a `FnDecl`
carries `receiver: Option<N>` from `@function.receiver`, and its `params`
exclude that node. A `Frame` carries `receiver: Option<ExprRef>` beside
`args`. `Role::Param { func, slot: Slot::Receiver }` is the role of an
identifier inside the declaration's receiver; with a frame it resolves to the
frame's receiver expression, without one it goes through `host/references`
like a parameter and takes each call site's `@call.receiver`. A call with no
`@call.receiver` to a declaration with a receiver, whose argument count is one
more than the parameter count, passes its first argument as the receiver
(`T::m(x)`).

`FnDecl.params` are the `@param` nodes owned by `@function.params`: one Go
`parameter_declaration` may name several parameters, and a Rust `parameter`
binds a pattern, so neither is a position. An argument is destructured against
the parameter node, which is then the pattern the name sits in.

### 3.5 Projection

`ctx.proj: Vec<String>` becomes `Vec<Proj>` with `Proj::Field(String)` and
`Proj::Index(usize)`. A field access pushes whichever its `@field.name` says:
`Index` when the name carries `@field.name.index` (Rust `t.0`), else `Field`. On a
keyed `@construct`, `Field(f)` selects the entry named `f` as today; on a
positional `@construct`, `Index(i)` selects the `i`-th element. The engine
never reads a field name as a number: only the query decides that a field
addresses a position. A one-element positional construct is transparent (Go's
`expression_list` around a single call). Anything else → `stop:
unresolved: no element <i> in this <kind>` / `no field <f> …` as today.

### 3.6 Function identity

`FuncId { file, group: usize, name, arity }` where `group` is the start byte
of the nearest `@function.group` ancestor of the declaring `@function`, else
of the `@function` itself. The clause set of a callee is every `@function`
whose group start, name and arity equal the callee's. `describe()` is
unchanged. Erlang marks `(source_file) @function.group` — the grammar gives
each clause its own `fun_decl`, so the module is the node its clauses share;
Rust and Go mark none.

## 4. Tree model (parent §5.1)

`via` gains `"element"`. Everything else is unchanged; node ids are
unaffected for existing trees because occurrence-free uses produce the same
nodes as before.

## 5. Protocol (parent §4, `docs/PROTOCOL.md`)

```
initialize            { root, capabilities: { documentHighlight: bool, implementation: bool } }
host/implementation   { file, line, col } → [Location]     optional, declared in initialize
```

`host/documentHighlight` is no longer "unused"; the doc text is updated to say
what §3.1 uses it for and that references are the fallback. Replay fixtures
gain an `implementation` section keyed like `definition`; the recorders write
it and the replay hosts (engine and VS Code) read it, relativising paths like
the other location sections.

Neovim answers `host/implementation` with `textDocument/implementation`
through the existing `locations` helper and declares the capability. VS Code
answers with `vscode.executeImplementationProvider` through the existing
`locations` helper and declares the capability.

## 6. Vocabulary (parent §6)

Additions, all read by generic code:

```
@assign  @assign.target  @assign.value  @assign.compound
@place.base
@escape
@param  @param.mutable
@call.receiver
@function.receiver  @function.receiver.mutable  @function.abstract  @function.group
@binding.element
@field.name.index
```

A capture named `<base>.<marker>` and co-captured on a `<base>` node is a
**marker**: the base keeps every obligation it had and is read through the
same code as any other, and the marker only adds meaning. `@assign.compound`,
`@function.receiver.mutable`, `@function.abstract`, `@field.name.index` and
`@binding.element` are all of this shape; each entry below says only what its
marker means.

- `@assign`: a write to an existing place. `.target`/`.value` as for
  `@binding`; `.compound` for `+=`, `++`, etc.
- `@place.base`: the expression an index or deref target writes through (`x`
  in `x[i] = e`, `*x = e`), so that the index is not mistaken for a write.
- `@escape`: an expression whose address or mutable reference is taken
  (`&mut e`, `&e` in Go).
- `@param`: one parameter name, required of every language. A function's
  positional parameters are these nodes rather than the named children of
  `@function.params`, so that a declaration naming several parameters
  (`a, b int`) or binding a pattern (`(a, b): (i32, i32)`) is counted correctly.
- `@param.mutable`: a parameter declaration through which the callee may
  write (`x: &mut T`, `p *T`); it may sit on the declaration containing the
  `@param` name.
- `@call.receiver`: the receiver expression of a method call.
- `@function.receiver`: the receiver parameter of a method declaration;
  `.mutable` co-captured when writes through it reach the caller (`&mut
  self`, `(s *T)`); a by-value `mut self` is not mutable in this sense.
- `@function.abstract`: the function has no body, or is a trait default;
  implementations are asked for. It is the one exemption from `@function.body`,
  which every other `@function` must carry.
- `@function.group`: the node grouping the clauses of one function.
- `@field.name.index`: the field addresses a position, not a name (Rust
  `t.0`); the projection is `Index` and the engine never parses a field name
  as a number.
- `@binding.element`: the `@binding.value` is an iterable, not the bound
  value, so it is not destructured against the pattern.

Semantics change for two existing captures: `@through` is applied wherever a
value is classified (§2) and repeated to a fixpoint, and `@construct` positional matching no longer
requires equal node kinds (§3.2). `is_literal` on a keyed construct skips
`@construct.field.name` nodes and checks each entry's `@construct.field.value`.

`required()` loses `@return.value`, which is read only inside a
`@return.container` and so cannot be required of a language whose branches are
statements; a language that defines `@return.container` must still define it.
It gains `@param`: without it a function has no positional parameters. The
other new captures are used only where present, so the Erlang query gains only
`@param` and `@function.group`.

`lang.toml`: `[quirks] single_assignment = true|false` is the only key.

### 6.1 Rust (`languages/rust/whence.scm`, tree-sitter-rust 0.24)

Verified against the grammar's parse trees. Query patterns use the
grammar's `_expression` supertype where "any expression" is meant.

| Construct | Pattern |
|---|---|
| identifiers | `[(identifier) (self) (shorthand_field_identifier)] @ident` |
| `let p = v` / `let x;` | `(let_declaration pattern: (_) @binding.pattern value: (_)? @binding.value) @binding` (the optional value yields `BoundBy { value: None }`) |
| `if let` / `while let` | `(let_condition pattern: (_) @binding.pattern value: (_) @binding.value) @binding` |
| `for p in v` | `(for_expression pattern: (_) @binding.pattern value: (_) @binding.value) @binding @binding.element` |
| `a = b` | `(assignment_expression left: (_) @assign.target right: (_) @assign.value) @assign` |
| `a += b` | `(compound_assignment_expr left: (_) @assign.target right: (_) @assign.value) @assign @assign.compound` |
| place bases | `(index_expression . (_) @place.base)`; `(unary_expression "*" (_) @place.base)` |
| call | `(call_expression function: [(identifier) (scoped_identifier) (generic_function)] @call.callee arguments: (arguments) @call.args) @call` |
| method call | `(call_expression function: (field_expression value: (_) @call.receiver field: (_) @call.callee) arguments: (arguments) @call.args) @call` |
| function | `(function_item name: (_) @function.name parameters: (_) @function.params body: (_) @function.body) @function` |
| receiver | `(self_parameter) @function.receiver`; `(self_parameter "&" (mutable_specifier)) @function.receiver.mutable` |
| parameter | `(parameter pattern: (_) @param)` (`self_parameter` carries no pattern and stays the receiver) |
| mutable param | `(parameter type: (reference_type (mutable_specifier))) @param.mutable` |
| trait method, no body | `(function_signature_item name: (_) @function.name parameters: (_) @function.params) @function @function.abstract` |
| trait default | `(trait_item body: (declaration_list (function_item) @function.abstract))` |
| body tail | `(function_item body: (block (_expression) @return .))` |
| `return e` | `(return_expression (_expression) @return)` |
| containers | `[(block) (if_expression) (match_expression) (unsafe_block)] @return.container`; `(block (_expression) @return.value .)`; `(if_expression consequence: (_) @return.value)`; `(else_clause (_) @return.value)`; `(match_arm value: (_) @return.value)`; `(unsafe_block (block) @return.value)` |
| match | `(match_expression value: (_) @branch.subject)`; `(match_arm pattern: (_) @branch.pattern) @branch` |
| escapes | `(reference_expression (mutable_specifier) value: (_) @escape)` |
| field access | `(field_expression value: (_) @field.container field: (_) @field.name) @field`; `(field_expression field: (integer_literal) @field.name.index)` (only when not a call's callee: the callee pattern above claims the `field_expression` first; the engine checks `@call.callee` before `@field`) |
| struct literal | `(struct_expression body: (field_initializer_list) @through.inner) @through`; `(field_initializer_list) @construct`; `(field_initializer field: (_) @construct.field.name value: (_) @construct.field.value)`; `(shorthand_field_initializer (identifier) @construct.field.name @construct.field.value)`; `(base_field_initializer (_expression) @through.inner) @through @construct.base` (the capture sits on the construct's own child, which is transparent) |
| tuples, arrays | `[(tuple_expression) (array_expression) (tuple_pattern) (slice_pattern)] @construct`; `(tuple_pattern (remaining_field_pattern)) @construct.cons`; `(slice_pattern (remaining_field_pattern)) @construct.cons`. `tuple_struct_pattern` is not a construct: its `type:` child would count as an element, so `Some(z) = e` traces the whole `e`. |
| struct pattern | `(struct_pattern) @construct`; `(field_pattern name: (_) @construct.field.name pattern: (_) @construct.field.value)`; `(field_pattern name: (shorthand_field_identifier) @construct.field.name @construct.field.value)` |
| wrappers | `(parenthesized_expression (_expression) @through.inner) @through`; `(try_expression (_expression) @through.inner) @through`; `(await_expression (_expression) @through.inner) @through`; `(reference_expression value: (_expression) @through.inner) @through` (a reference is its referent, so a trace across a by-reference argument reaches the variable); `(return_expression (_expression) @through.inner) @through` (a `return e` block tail is captured `@return` twice — once as the tail, once as the operand — and the wrapper being transparent leaves one) |
| literals | `[(integer_literal) (float_literal) (string_literal) (raw_string_literal) (char_literal) (boolean_literal) (unit_expression)] @literal` |
| function-type params | `(function_type parameters: (_) @opaque)` — `cb: fn(a: i32)` nests a parameter list of its own; `@opaque` stops the `@param` climb there, so `a` is not a parameter of the enclosing function |
| opaque | `[(closure_expression) (macro_invocation) (async_block)] @opaque`. Loops are not opaque: `owning_function` stops at `@opaque`, and a `return` inside a loop body is still the function's. A loop used as a value falls to the default `unresolved: <kind>`. |

Notes verified in the parse trees: `&mut self` is `self_parameter` with a
`mutable_specifier` child and a `"&"` token; `mut self` has the specifier
without the token. A trait method without a body is `function_signature_item`.
A block's tail is a bare `_expression` as the last named child; a trailing
`;` makes it an `expression_statement`, which the `_expression` supertype
does not match. `..base` is `base_field_initializer`. Method-call callees are
`field_expression`, so `@call.callee` is the `field:` identifier and the
callee text is that name.

### 6.2 Go (`languages/go/whence.scm`, tree-sitter-go 0.25)

| Construct | Pattern |
|---|---|
| identifiers | `(identifier) @ident` |
| `a, b := v` | `(short_var_declaration left: (expression_list) @binding.pattern right: (expression_list) @binding.value) @binding` |
| `var x T = v` / `var x T` | `(var_spec name: (identifier) @binding.pattern value: (expression_list) @binding.value) @binding`; `(var_spec name: (identifier) @binding.pattern type: (_) @binding.value @literal !value) @binding` (the zero value is the type, §3.2) |
| `for k, v := range xs` | `(range_clause left: (expression_list) @binding.pattern right: (_) @binding.value) @binding @binding.element` |
| `a, b = v` / `a += v` | `(assignment_statement left: (expression_list) @assign.target right: (expression_list) @assign.value) @assign`; `((assignment_statement operator: _ @op) @assign.compound (#not-eq? @op "="))` |
| `x++` / `x--` | `((inc_statement (_) @assign.target) @assign @assign.compound)`; `((dec_statement (_) @assign.target) @assign @assign.compound)` |
| place bases | `(index_expression operand: (_) @place.base)`; `(unary_expression operator: "*" operand: (_) @place.base)` |
| call | `(call_expression function: (identifier) @call.callee arguments: (argument_list) @call.args) @call` |
| method / package call | `(call_expression function: (selector_expression operand: (_) @call.receiver field: (_) @call.callee) arguments: (argument_list) @call.args) @call` (a package qualifier is captured as a receiver and ignored when the callee is not a method) |
| function | `(function_declaration name: (_) @function.name parameters: (_) @function.params body: (_) @function.body) @function` |
| method | `(method_declaration receiver: (parameter_list (parameter_declaration) @function.receiver) name: (_) @function.name parameters: (_) @function.params body: (_) @function.body) @function`; `(method_declaration receiver: (parameter_list (parameter_declaration type: (pointer_type)) @function.receiver.mutable))` |
| parameter | `(parameter_declaration name: (identifier) @param)` (one declaration may name several) |
| mutable param | `(parameter_declaration type: [(pointer_type) (slice_type) (map_type) (channel_type)]) @param.mutable` (not the variadic declaration: it is `@opaque` and names no `@param`) |
| variadic param | `(variadic_parameter_declaration) @opaque` — `c ...int` is one name for any number of arguments, so it holds no argument position |
| interface method | `(method_elem name: (_) @function.name parameters: (_) @function.params) @function @function.abstract` |
| `return a, b` | `(return_statement (expression_list) @return)`; bare `return` → `((return_statement) @return (#eq? @return "return"))` |
| expression lists | `(expression_list) @construct`; `(expression_list . (_) @through.inner .) @through` |
| escapes | `(unary_expression operator: "&" operand: (_) @escape @through.inner) @through` (one pattern: the referent both escapes and is what the reference is) |
| field access | `(selector_expression operand: (_) @field.container field: (_) @field.name) @field` |
| composite literal | `(composite_literal body: (literal_value) @through.inner) @through`; `(literal_value) @construct`; `(keyed_element key: (literal_element) @construct.field.name value: (literal_element) @construct.field.value)`; `(literal_element (_) @through.inner) @through` |
| wrappers | `(parenthesized_expression (_) @through.inner) @through` (the `&` wrapper is the escape row above) |
| literals | `[(int_literal) (float_literal) (imaginary_literal) (rune_literal) (interpreted_string_literal) (raw_string_literal) (true) (false) (nil) (iota)] @literal` |
| function-type params | `(function_type parameters: (_) @opaque)` — `f func(x int) int` nests a parameter list of its own; `@opaque` stops the `@param` climb there, so `x` is not a parameter of the enclosing function |
| opaque | `[(func_literal) (go_statement) (defer_statement)] @opaque` (`select_statement` is not opaque: a `return` in one of its cases is the function's) |

Verified in the parse trees: `q, r := g(v)` is an `expression_list` of two
identifiers against an `expression_list` of one `call_expression`; a pointer
receiver is `parameter_declaration type: (pointer_type)` under the
`receiver:` field; interface methods are `method_elem`; `keyed_element` has
`key:`/`value:` fields of `literal_element`; the assignment operator is an
anonymous node under `operator:`. Go has no expression-level branches, so
`@return.container`, `@return.value` and `@branch.*` are absent; a
`switch`/`if` never appears as a value. In `var u, w = f()` each `name:` child is
its own `@binding.pattern` against the whole `@binding.value` list, so a name
carries no position: the trace continues into the list and ends where the list
ends, never on a wrong element.

### 6.3 Erlang

Adds `(source_file) @function.group` and
`(function_clause args: (expr_args (_) @param))` — the clause's argument
patterns, which is what the named children of `@function.params` were.
`lang.toml` loses the two unused keys.

## 7. Engine changes by file

- `engine/src/lang/vocab.rs`: the constants above.
- `engine/src/lang/mod.rs`: `Quirks { single_assignment }` only; `Language.quirks` read by the step function.
- `engine/src/syntax.rs`: `FnDecl.receiver`, `CallSite.receiver`, `Role::Param { func, slot: Slot }` (`Slot::Receiver` or `Slot::Arg(i)`), `Role::ElementOf`, `BoundBy.value: Option`; `assign_at(occurrence) -> Option<Assign { node, target, value: Option<N>, compound: bool }>`; `escapes(occurrence) -> bool`; `is_abstract(FnDecl)`; `function_group(FnDecl) -> usize`; `Proj` and `field_access -> (container, Proj)`; `through` applied in `value`-facing helpers; `destructure` without the kind check and with the positional/keyed rule; `construct_element(construct, i)`.
- `engine/src/trace/frame.rs`: `FuncId.group`, `Frame.receiver`, `Ctx.proj: Vec<Proj>`, `Ctx.highlights`/`Ctx.implementation` caches beside `defs`/`refs`, `Ctx.occurrences(file, pos)` implementing the highlight-or-references choice.
- `engine/src/trace/step.rs`: `ident` gains the occurrence step (§3.1) and builds the `branch`; `definition` handles the new roles; `call_result` handles `@function.abstract`, receivers and groups; `project` handles `Proj::Index`.
- `engine/src/host.rs`, `host_rpc.rs`, `host_replay.rs`, `server.rs`: `implementation` request and capability; replay section.
- `engine/src/main.rs`: `whence replay` unchanged in interface; the `--serve` replay host declares both capabilities as recorded sections allow.
- `engine/build.rs`: no change; it embeds every `languages/*` directory. `engine/Cargo.toml` adds `tree-sitter-rust = "0.24"`, `tree-sitter-go = "0.25"`.
- `languages/rust/`, `languages/go/`: `whence.scm`, `lang.toml`.

## 8. Hosts

- Neovim: `host.lua` adds `["host/implementation"]` via `locations("textDocument/implementation", …)` and declares `implementation = true`; `record.lua` `SECTION` gains the method. `:WhenceInstall` unchanged.
- VS Code: `host.ts` adds `implementation` via `locations("vscode.executeImplementationProvider", …)` and declares the capability in `initialize`; `hostReplay.ts` `SECTION`/`Sections` and `loadFixture` gain the section; `record.ts` follows through `SECTION`.
- `docs/PROTOCOL.md`: the request, the capability, the fixture section, and the corrected paragraph on `documentHighlight`.

## 9. Testing (parent §9)

- **Query tests** `engine/tests/queries_rust.rs`, `queries_go.rs` over
  `fixtures/{rust,go}/queries/sample.{rs,go}`: pin the captures the engine's
  rules depend on and that a grammar update could silently break — tail vs
  statement, receiver mutability, abstract vs default, compound assignment,
  one-element list transparency. No capture-by-capture inventory.
- **Syntax tests** for the new generic rules against the Rust and Go
  samples: `destructure` across pattern/value kinds and with an index
  projection, `Role::Param { slot: Slot::Receiver }`, `assign_at`
  classification, `function_group`.
  These test engine logic, not the grammar.
- **Replay fixtures**, recorded with `:WhenceRecord` from the sample projects
  (rust-analyzer, gopls), one directory per case under
  `fixtures/rust/` and `fixtures/go/`; `replay.rs` takes `<lang>/<case>`.
  Cases: rebind chain; compound mutation with a pending field; `&mut`
  escape plus in-root mutable receiver plus external method; mutable
  parameter; abstract method with two implementations (and, in Rust, a
  default); multi-value binding through a call; loop variable; same-name
  methods on two types; `let x;` then assignment. Each golden is inspected
  before commit.
- **Protocol/host tests**: the replay host answers `implementation` and
  relativises its paths; `RpcHost` refuses it without the capability. One
  test per host that the recorder writes the section (extends the existing
  fixture round-trip tests; no new files).
- Erlang goldens are regenerated only if `@function.group` changes an id,
  which it must not: `describe()` and node ids do not include the group.

## 10. Milestone exit and dogfooding

Manual, on the two dogfood projects, one Rust and one Go, with both editors.
Nothing recorded from them is committed. Findings become `bug` tickets under
the M2 epic. The exit criterion is the M1 wording applied to both.

## 11. Out of scope

Curated input-source lists, lazy expansion, runtime language loading
(parent §11 "Later"). Loop-carried writes after the use. Writes through
Go slices/maps/pointer variables passed by value. Trait dispatch on generic
type parameters (rust-analyzer's `implementation` answers what it can; the
rest is `external` or `unresolved`).
