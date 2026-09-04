[(identifier) (self) (shorthand_field_identifier)] @ident

;; the optional value yields a declaration without one
(let_declaration pattern: (_) @binding.pattern value: (_)? @binding.value) @binding
(let_condition pattern: (_) @binding.pattern value: (_) @binding.value) @binding
(for_expression pattern: (_) @binding.element value: (_) @binding.value) @binding

(assignment_expression left: (_) @assign.target right: (_) @assign.value) @assign
(compound_assignment_expr
  left: (_) @assign.target
  right: (_) @assign.value) @assign @assign.compound
;; what an index or deref target writes through
(index_expression . (_) @place.base)
(unary_expression "*" (_) @place.base)

(call_expression
  function: [(identifier) (scoped_identifier) (generic_function)] @call.callee
  arguments: (arguments) @call.args) @call
;; a method callee is the field name; the receiver travels separately
(call_expression
  function: (field_expression value: (_) @call.receiver field: (_) @call.callee)
  arguments: (arguments) @call.args) @call

(function_item
  name: (_) @function.name
  parameters: (_) @function.params
  body: (_) @function.body) @function
(self_parameter) @function.receiver
;; `&mut self` writes reach the caller; `mut self` does not
(self_parameter "&" (mutable_specifier)) @function.receiver.mutable
(parameter pattern: (_) @param)
(parameter type: (reference_type (mutable_specifier))) @param.mutable
(function_signature_item
  name: (_) @function.name
  parameters: (_) @function.params) @function.abstract
;; a trait default: implementations are asked for alongside the body
(trait_item body: (declaration_list (function_item) @function.abstract))

;; a block's tail is a bare expression; a trailing `;` makes it a statement
(function_item body: (block (_expression) @return .))
(return_expression (_expression) @return)
[(block) (if_expression) (match_expression) (unsafe_block)] @return.container
(block (_expression) @return.value .)
(if_expression consequence: (_) @return.value)
(else_clause (_) @return.value)
(match_arm value: (_) @return.value)
(unsafe_block (block) @return.value)

(match_expression value: (_) @branch.subject)
(match_arm pattern: (_) @branch.pattern) @branch

;; `&mut e`: the callee may write through it
(reference_expression (mutable_specifier) value: (_) @escape)

;; only when not a call's callee: the engine checks @call.callee first
(field_expression value: (_) @field.container field: (_) @field.name) @field

(struct_expression body: (field_initializer_list) @through.inner) @through
(field_initializer_list) @construct
(field_initializer field: (_) @construct.field.name value: (_) @construct.field.value)
(shorthand_field_initializer (identifier) @construct.field.name @construct.field.value)
(base_field_initializer (_expression) @through.inner) @through @construct.base

[(tuple_expression) (array_expression) (tuple_pattern) (slice_pattern)] @construct
;; `..` swallows an unknown number of elements: the rest are not positional
(tuple_pattern (remaining_field_pattern)) @construct.cons
(slice_pattern (remaining_field_pattern)) @construct.cons

(struct_pattern) @construct
(field_pattern name: (_) @construct.field.name pattern: (_) @construct.field.value)
(field_pattern name: (shorthand_field_identifier) @construct.field.name @construct.field.value)

;; @through: classify the node by its @through.inner child
(parenthesized_expression (_expression) @through.inner) @through
(try_expression (_expression) @through.inner) @through
(await_expression (_expression) @through.inner) @through
(reference_expression value: (_expression) @through.inner) @through
(return_expression (_expression) @through.inner) @through

[(integer_literal) (float_literal) (string_literal) (raw_string_literal)
 (char_literal) (boolean_literal) (unit_expression)] @literal

;; @opaque: the engine never looks inside these
[(closure_expression) (macro_invocation) (async_block)] @opaque
