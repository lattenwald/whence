use std::collections::BTreeMap;
use tree_sitter::StreamingIterator;
use whence::lang::{Registry, vocab};

const SAMPLE: &str = include_str!("fixtures/go/queries/sample.go");

fn captures(src: &str) -> BTreeMap<String, Vec<String>> {
    let reg = Registry::embedded().unwrap();
    let lang = reg.by_name("go").unwrap();
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
    assert!(has(&c, vocab::BINDING_ELEMENT, "k, e"));
    assert!(c[vocab::OPAQUE].iter().any(|s| s.starts_with("func(")));
}
