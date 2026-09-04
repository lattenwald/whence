(var) @ident

(match_expr lhs: (_) @binding.pattern rhs: (_) @binding.value) @binding

(call expr: (_) @call.callee args: (expr_args) @call.args) @call
;; the grammar nests a remote call as remote(module, fun: call(...)): @call.callee is the bare name
(remote module: (remote_module (_) @callee.module) fun: (call expr: (_) @callee.name))
;; @through: classify the node by its @through.inner child
(remote fun: (call) @through.inner) @through

;; one @function per clause: multi-clause functions yield several matches
(function_clause
  name: (_) @function.name
  args: (expr_args) @function.params
  body: (clause_body) @function.body) @function
(function_clause args: (expr_args (_) @param))

;; the grammar gives each clause its own fun_decl; clauses are grouped by the module they share
(source_file) @function.group

(function_clause body: (clause_body (_) @return .))
(clause_body (_) @return.value .)
(block_expr (_) @return.value .)
(paren_expr expr: (_) @return.value)
(try_expr exprs: (_) @return.value . catch: (catch_clause))
(try_expr exprs: (_) @return.value . after: (_))
[(case_expr) (if_expr) (try_expr) (receive_expr) (block_expr) (paren_expr)] @return.container

(case_expr expr: (_) @branch.subject)
(cr_clause pat: (_) @branch.pattern) @branch

[(tuple) (list) (record_expr) (record_update_expr) (map_expr) (map_expr_update)] @construct
(record_field name: (_) @construct.field.name expr: (field_expr (_) @construct.field.value))
(map_field key: (_) @construct.field.name value: (_) @construct.field.value)
(record_update_expr expr: (_) @construct.base)
(map_expr_update expr: (_) @construct.base)
(record_field_expr expr: (_) @field.container field: (record_field_name (_) @field.name)) @field
;; `[H | T]`: the pipe tail is not a positional element
(list (pipe)) @construct.cons

[(integer) (float) (string) (atom) (char)] @literal

;; @opaque: the engine never looks inside these
[(receive_expr) (anonymous_fun) (macro_call_expr) (list_comprehension)
 (binary_comprehension) (map_comprehension) (try_after)
 (catch_expr) (maybe_expr)] @opaque
