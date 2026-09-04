use std::collections::BTreeMap;
use tree_sitter::StreamingIterator;
use whence::lang::{Registry, vocab};

const SAMPLE: &str = include_str!("fixtures/rust/queries/sample.rs");

fn captures(src: &str) -> BTreeMap<String, Vec<String>> {
    let reg = Registry::embedded().unwrap();
    let lang = reg.by_name("rust").unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.ts).unwrap();
    let tree = parser.parse(src, None).unwrap();
    let mut cur = tree_sitter::QueryCursor::new();
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let names = lang.query.capture_names();
    let mut it = cur.matches(&lang.query, tree.root_node(), src.as_bytes());
    while let Some(m) = it.next() {
        for c in m.captures() {
            out.entry(names[c.index as usize].to_string())
                .or_default()
                .push(c.node.utf8_text(src.as_bytes()).unwrap().to_string());
        }
    }
    out
}

fn has(c: &BTreeMap<String, Vec<String>>, cap: &str, text: &str) -> bool {
    c.get(cap).is_some_and(|v| v.iter().any(|s| s == text))
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
    assert!(has(&c, vocab::BINDING_ELEMENT, "i"));
    assert!(has(&c, vocab::OPAQUE, "vec![]"));
}
