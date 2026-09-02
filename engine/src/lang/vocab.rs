//! The only capture names the engine reads; extend here, never per language.

pub const BINDING: &str = "binding";
pub const BINDING_PATTERN: &str = "binding.pattern";
pub const BINDING_VALUE: &str = "binding.value";

pub const CALL: &str = "call";
pub const CALL_CALLEE: &str = "call.callee";
pub const CALL_ARGS: &str = "call.args";

pub const FUNCTION: &str = "function";
pub const FUNCTION_NAME: &str = "function.name";
pub const FUNCTION_PARAMS: &str = "function.params";
pub const FUNCTION_BODY: &str = "function.body";

pub const RETURN_VALUE: &str = "return.value";
pub const RETURN_CONTAINER: &str = "return.container";
pub const LITERAL: &str = "literal";

pub const BRANCH: &str = "branch";
pub const BRANCH_PATTERN: &str = "branch.pattern";
pub const BRANCH_SUBJECT: &str = "branch.subject";

pub const CALLEE_MODULE: &str = "callee.module";
pub const CALLEE_NAME: &str = "callee.name";

pub const OPAQUE: &str = "opaque";

pub const FIELD: &str = "field";
pub const FIELD_CONTAINER: &str = "field.container";
pub const FIELD_NAME: &str = "field.name";

pub const CONSTRUCT: &str = "construct";
pub const CONSTRUCT_FIELD_NAME: &str = "construct.field.name";
pub const CONSTRUCT_FIELD_VALUE: &str = "construct.field.value";

pub const IDENT: &str = "ident";

/// Captures every language must define; the rest are used only where present.
pub fn required() -> &'static [&'static str] {
    &[
        BINDING,
        BINDING_PATTERN,
        BINDING_VALUE,
        CALL,
        CALL_CALLEE,
        CALL_ARGS,
        FUNCTION,
        FUNCTION_NAME,
        FUNCTION_PARAMS,
        FUNCTION_BODY,
        RETURN_VALUE,
        LITERAL,
        IDENT,
    ]
}
