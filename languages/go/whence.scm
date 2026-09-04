(identifier) @ident

(short_var_declaration
  left: (expression_list) @binding.pattern
  right: (expression_list) @binding.value) @binding
;; `var x T` binds the zero value of the type; `var x T = v` binds the value
(var_spec name: (identifier) @binding.pattern type: (_) @binding.value @literal !value) @binding
(var_spec
  name: (identifier) @binding.pattern
  value: (expression_list) @binding.value) @binding @binding.positional
(range_clause left: (expression_list) @binding.pattern right: (_) @binding.value) @binding @binding.element

(assignment_statement
  left: (expression_list) @assign.target
  right: (expression_list) @assign.value) @assign
((assignment_statement operator: _ @op) @assign.compound
 (#not-eq? @op "="))
((inc_statement (_) @assign.target) @assign @assign.compound)
((dec_statement (_) @assign.target) @assign @assign.compound)
;; what an index or deref target writes through
(index_expression operand: (_) @place.base)
(unary_expression operator: "*" operand: (_) @place.base)

(call_expression
  function: (identifier) @call.callee
  arguments: (argument_list) @call.args) @call
;; a package qualifier lands in @call.receiver and is ignored: the callee is not a method
(call_expression
  function: (selector_expression operand: (_) @call.receiver field: (_) @call.callee)
  arguments: (argument_list) @call.args) @call

(function_declaration
  name: (_) @function.name
  parameters: (_) @function.params
  body: (_) @function.body) @function
(method_declaration
  receiver: (parameter_list (parameter_declaration) @function.receiver)
  name: (_) @function.name
  parameters: (_) @function.params
  body: (_) @function.body) @function
;; writes through a pointer receiver reach the caller
(method_declaration
  receiver: (parameter_list (parameter_declaration type: (pointer_type)) @function.receiver.mutable))
(parameter_declaration name: (identifier) @param)
(parameter_declaration
  type: [(pointer_type) (slice_type) (map_type) (channel_type)]) @param.mutable
;; `c ...int` is one name for many arguments: no position of its own
(variadic_parameter_declaration) @opaque
;; and it leaves the declaration's arity open
(function_declaration parameters: (parameter_list (variadic_parameter_declaration))) @function.variadic
(method_declaration parameters: (parameter_list (variadic_parameter_declaration))) @function.variadic
(method_elem parameters: (parameter_list (variadic_parameter_declaration))) @function.variadic
(method_elem name: (_) @function.name parameters: (_) @function.params) @function @function.abstract

(return_statement (expression_list) @return)
;; a bare `return` in a named-results function yields the results, not an expression
((return_statement) @return (#eq? @return "return"))

(selector_expression operand: (_) @field.container field: (_) @field.name) @field

(composite_literal body: (literal_value) @through.inner) @through
(literal_value) @construct
(keyed_element key: (literal_element) @construct.field.name value: (literal_element) @construct.field.value)
(literal_element (_) @through.inner) @through

(expression_list) @construct
;; a one-element list is the expression it holds
(expression_list . (_) @through.inner .) @through

(parenthesized_expression (_) @through.inner) @through
;; `&e`: a reference is its referent, and the callee may write through it
(unary_expression operator: "&" operand: (_) @escape @through.inner) @through

[(int_literal) (float_literal) (imaginary_literal) (rune_literal)
 (interpreted_string_literal) (raw_string_literal)
 (true) (false) (nil) (iota)] @literal

;; @opaque: the engine never looks inside these
[(func_literal) (go_statement) (defer_statement)] @opaque
;; a function type's own parameters belong to no declaration
(function_type parameters: (_) @opaque)
