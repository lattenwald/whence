use std::collections::BTreeMap;
use whence::lang::vocab;

mod common;
use common::has;

const SAMPLE: &str = include_str!("fixtures/erlang/queries/sample.erl");

fn captures(src: &str) -> BTreeMap<String, Vec<String>> {
    common::captures("erlang", src)
}

#[test]
fn bindings_calls_functions() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::BINDING_PATTERN, "Body"));
    assert!(has(&c, vocab::BINDING_VALUE, "read_body(Req0)"));
    assert!(has(&c, vocab::CALL_CALLEE, "get"));
    assert!(has(&c, vocab::CALL_ARGS, "(limit, Opts, 10)"));
    assert!(has(&c, vocab::CALLEE_MODULE, "maps"));
    assert!(has(&c, vocab::CALLEE_NAME, "get"));
    assert!(has(&c, vocab::THROUGH, "maps:get(limit, Opts, 10)"));
    assert!(has(&c, vocab::THROUGH_INNER, "get(limit, Opts, 10)"));
    assert_eq!(
        c[vocab::FUNCTION_NAME]
            .iter()
            .filter(|n| *n == "pick")
            .count(),
        2,
        "one @function per clause"
    );
}

#[test]
fn tail_returns_and_branches() {
    let c = captures(SAMPLE);
    assert!(
        c[vocab::RETURN_CONTAINER]
            .iter()
            .any(|s| s.starts_with("case pick(Limit)"))
    );
    assert!(has(&c, vocab::RETURN_VALUE, "{V, R}"));
    assert!(has(&c, vocab::RETURN_VALUE, "error"));
    assert!(has(&c, vocab::RETURN_VALUE, "42"));
    assert!(has(&c, vocab::RETURN_VALUE, "43"));
    assert!(!has(&c, vocab::RETURN_VALUE, "io:format(\"y\")"));
    assert!(has(&c, vocab::BRANCH_SUBJECT, "pick(Limit)"));
    assert!(has(&c, vocab::BRANCH_PATTERN, "{ok, V}"));
}

#[test]
fn fields_constructs_literals_opaque() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::FIELD_CONTAINER, "Req0"));
    assert!(has(&c, vocab::FIELD_NAME, "peer"));
    assert!(has(&c, vocab::CONSTRUCT_FIELD_NAME, "body"));
    assert!(has(&c, vocab::CONSTRUCT_FIELD_VALUE, "Body"));
    assert!(has(&c, vocab::CONSTRUCT, "{ok, V}"));
    assert!(has(&c, vocab::CONSTRUCT_CONS, "[H | T]"));
    assert!(!has(&c, vocab::CONSTRUCT_CONS, "[1, 2, 3]"));
    assert!(has(&c, vocab::LITERAL, "10"));
    assert!(has(&c, vocab::IDENT, "Opts"));
    assert!(has(&c, vocab::OPAQUE, "fun(X) -> X end"));
}
