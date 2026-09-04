use std::collections::BTreeMap;
use std::sync::OnceLock;

use tree_sitter::StreamingIterator;
use whence::{lang::Registry, pos::Pos, syntax::Doc};

#[allow(dead_code)]
pub fn captures(lang: &str, src: &str) -> BTreeMap<String, Vec<String>> {
    let reg = Registry::embedded().unwrap();
    let lang = reg.by_name(lang).unwrap();
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

#[allow(dead_code)]
pub fn has(c: &BTreeMap<String, Vec<String>>, cap: &str, text: &str) -> bool {
    c.get(cap).is_some_and(|v| v.iter().any(|s| s == text))
}

#[allow(dead_code)]
pub fn parse(lang: &str, path: &str, text: &str) -> Doc<'static> {
    static REG: OnceLock<Registry> = OnceLock::new();
    let reg = REG.get_or_init(|| Registry::embedded().unwrap());
    Doc::parse(reg.by_name(lang).unwrap(), path.into(), text.to_string())
}

/// Position of the `nth` (0-based) occurrence of `needle`, offset by `skip` bytes into it.
#[allow(dead_code)]
pub fn at_skip(text: &str, needle: &str, nth: usize, skip: usize) -> Pos {
    let mut from = 0;
    for _ in 0..nth {
        from += text[from..].find(needle).unwrap() + needle.len();
    }
    let idx = from + text[from..].find(needle).unwrap() + skip;
    whence::pos::pos_of(text, idx)
}

#[allow(dead_code)]
pub fn at(text: &str, needle: &str, nth: usize) -> Pos {
    at_skip(text, needle, nth, 0)
}
