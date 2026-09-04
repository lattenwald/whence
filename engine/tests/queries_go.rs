use std::collections::BTreeMap;
use whence::lang::vocab;

mod common;
use common::has;

const SAMPLE: &str = include_str!("fixtures/go/queries/sample.go");

fn captures(src: &str) -> BTreeMap<String, Vec<String>> {
    common::captures("go", src)
}

#[test]
fn multi_value_bindings_and_lists() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::BINDING_PATTERN, "q, r"));
    assert!(has(&c, vocab::BINDING_VALUE, "g(v)"));
    assert!(has(&c, vocab::CONSTRUCT, "q, r"));
    assert!(has(&c, vocab::THROUGH_INNER, "g(v)"));
    assert!(has(&c, vocab::THROUGH, "g(v)"));
    assert!(!has(&c, vocab::THROUGH, "q, r"));
}

#[test]
fn receivers_params_abstract_and_zero_values() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::FUNCTION_RECEIVER_MUTABLE, "s *S"));
    assert!(!has(&c, vocab::FUNCTION_RECEIVER_MUTABLE, "s S"));
    assert!(has(&c, vocab::FUNCTION_RECEIVER, "s S"));
    assert!(has(&c, vocab::PARAM_MUTABLE, "p *int"));
    assert!(!has(&c, vocab::PARAM_MUTABLE, "d int"));
    assert!(has(&c, vocab::FUNCTION_ABSTRACT, "Abs() int"));
    assert!(has(&c, vocab::LITERAL, "z int"));
    assert!(!has(&c, vocab::LITERAL, "w int = v"));
}

#[test]
fn writes_escapes_and_returns() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::ASSIGN_COMPOUND, "v++"));
    assert!(has(&c, vocab::ASSIGN_COMPOUND, "s.X += d"));
    assert!(!has(&c, vocab::ASSIGN_COMPOUND, "v = v + 1"));
    assert!(has(&c, vocab::ASSIGN_TARGET, "xs[0]"));
    assert!(has(&c, vocab::PLACE_BASE, "xs"));
    assert!(has(&c, vocab::ESCAPE, "v"));
    assert!(has(&c, vocab::RETURN, "fn(v)"));
    assert!(has(&c, vocab::RETURN, "return"));
    assert!(has(&c, vocab::BINDING_PATTERN, "k, e"));
    assert!(has(&c, vocab::BINDING_ELEMENT, "k, e := range xs"));
    assert!(c[vocab::OPAQUE].iter().any(|s| s.starts_with("func(")));
}
