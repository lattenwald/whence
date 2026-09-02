(var) @ident

(match_expr lhs: (_) @binding.pattern rhs: (_) @binding.value) @binding

(call expr: (_) @call.callee args: (expr_args) @call.args) @call

(function_clause
  name: (_) @function.name
  args: (expr_args) @function.params
  body: (clause_body) @function.body) @function

(clause_body (_) @return.value .)

[(integer) (float) (string) (atom) (char)] @literal
