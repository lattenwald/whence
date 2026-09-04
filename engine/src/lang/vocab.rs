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

/// Where a value leaves the function: a body tail, a `return` operand.
pub const RETURN: &str = "return";
pub const RETURN_VALUE: &str = "return.value";
pub const RETURN_CONTAINER: &str = "return.container";
pub const LITERAL: &str = "literal";

pub const BRANCH: &str = "branch";
pub const BRANCH_PATTERN: &str = "branch.pattern";
pub const BRANCH_SUBJECT: &str = "branch.subject";

pub const CALLEE_MODULE: &str = "callee.module";
pub const CALLEE_NAME: &str = "callee.name";

pub const OPAQUE: &str = "opaque";

pub const THROUGH: &str = "through";
pub const THROUGH_INNER: &str = "through.inner";

pub const FIELD: &str = "field";
pub const FIELD_CONTAINER: &str = "field.container";
pub const FIELD_NAME: &str = "field.name";

pub const CONSTRUCT: &str = "construct";
pub const CONSTRUCT_FIELD_NAME: &str = "construct.field.name";
pub const CONSTRUCT_FIELD_VALUE: &str = "construct.field.value";
/// On a construct whose elements are not positionally addressable (a cons/spread tail).
pub const CONSTRUCT_CONS: &str = "construct.cons";
/// The value an update construct starts from: fields it does not set come from here.
pub const CONSTRUCT_BASE: &str = "construct.base";

pub const IDENT: &str = "ident";

/// A write to an existing place: `x = e`, `x += e`, `x++`.
pub const ASSIGN: &str = "assign";
pub const ASSIGN_TARGET: &str = "assign.target";
pub const ASSIGN_VALUE: &str = "assign.value";
/// Co-captured on an `@assign` that reads the old value (`+=`, `++`).
pub const ASSIGN_COMPOUND: &str = "assign.compound";

/// The expression written through by an index or deref target (x in x[i], *x).
pub const PLACE_BASE: &str = "place.base";

/// An expression whose address or mutable reference is taken.
pub const ESCAPE: &str = "escape";
/// A parameter the callee may write through (`&mut T`, `*T`).
pub const PARAM_MUTABLE: &str = "param.mutable";

pub const CALL_RECEIVER: &str = "call.receiver";
pub const FUNCTION_RECEIVER: &str = "function.receiver";
/// Co-captured on a receiver whose writes reach the caller (`&mut self`, `(s *T)`).
pub const FUNCTION_RECEIVER_MUTABLE: &str = "function.receiver.mutable";
/// A function declared without a body, or a trait default: implementations are asked for.
pub const FUNCTION_ABSTRACT: &str = "function.abstract";
/// Groups the clauses of one function; absent where a function is one node.
pub const FUNCTION_GROUP: &str = "function.group";

/// The pattern of a loop binding; `@binding.value` is the iterable.
pub const BINDING_ELEMENT: &str = "binding.element";

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
        RETURN,
        RETURN_VALUE,
        LITERAL,
        IDENT,
    ]
}
