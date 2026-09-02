use std::collections::BTreeMap;
use tree_sitter::StreamingIterator;
use whence::lang::{Registry, vocab};

const SAMPLE: &str = include_str!("fixtures/erlang/queries/sample.erl");

fn captures(src: &str) -> BTreeMap<String, Vec<String>> {
    let reg = Registry::embedded().unwrap();
    let lang = reg.by_name("erlang").unwrap();
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
    assert!(has(&c, vocab::LITERAL, "10"));
    assert!(has(&c, vocab::IDENT, "Opts"));
    assert!(has(&c, vocab::OPAQUE, "fun(X) -> X end"));
}
