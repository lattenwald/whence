use std::collections::BTreeMap;
use whence::lang::vocab;

mod common;
use common::has;

const SAMPLE: &str = include_str!("fixtures/rust/queries/sample.rs");

fn captures(src: &str) -> BTreeMap<String, Vec<String>> {
    common::captures("rust", src)
}

#[test]
fn tails_returns_and_statements_are_told_apart() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::RETURN, "x"));
    assert!(has(&c, vocab::RETURN, "(m, p.y)"));
    assert!(!has(&c, vocab::RETURN, "self.x += d"));
    assert!(has(&c, vocab::RETURN_VALUE, "p.x"));
    assert!(has(&c, vocab::RETURN_VALUE, "q"));
}

#[test]
fn receivers_params_and_abstract_declarations() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::FUNCTION_RECEIVER_MUTABLE, "&mut self"));
    assert!(!has(&c, vocab::FUNCTION_RECEIVER_MUTABLE, "&self"));
    assert!(!has(&c, vocab::FUNCTION_RECEIVER_MUTABLE, "mut self"));
    assert!(has(&c, vocab::PARAM_MUTABLE, "b: &mut Vec<i32>"));
    assert!(has(&c, vocab::FUNCTION_ABSTRACT, "fn abs(&self) -> i32;"));
    assert!(
        c[vocab::FUNCTION_ABSTRACT]
            .iter()
            .any(|s| s.starts_with("fn dflt"))
    );
    assert!(has(&c, vocab::CALL_RECEIVER, "b"));
    assert!(has(&c, vocab::CALL_CALLEE, "push"));
}

#[test]
fn writes_escapes_and_wrappers() {
    let c = captures(SAMPLE);
    assert!(has(&c, vocab::ASSIGN_TARGET, "v"));
    assert!(has(&c, vocab::ASSIGN_COMPOUND, "self.x += d"));
    assert!(!has(&c, vocab::ASSIGN_COMPOUND, "v = v + 1"));
    assert!(has(&c, vocab::PLACE_BASE, "b"));
    assert!(has(&c, vocab::ESCAPE, "e"));
    assert!(has(&c, vocab::THROUGH_INNER, "n"));
    assert!(has(&c, vocab::CONSTRUCT_BASE, "..base()"));
    assert!(has(&c, vocab::CONSTRUCT_CONS, "(a, .., zz)"));
    assert!(!has(&c, vocab::CONSTRUCT_CONS, "(q, r)"));
    assert!(has(&c, vocab::BINDING_PATTERN, "i"));
    assert!(
        c[vocab::BINDING_ELEMENT]
            .iter()
            .any(|s| s.starts_with("for i in 0..3"))
    );
    assert!(has(&c, vocab::OPAQUE, "vec![]"));
}
