use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;

use crate::tree::{Node, Tree};

pub fn render_text(tree: &Tree, root: &Path) -> String {
    let mut out = String::new();
    line(&mut out, &tree.root, 0, root);
    out
}

fn line(out: &mut String, node: &Node, depth: usize, root: &Path) {
    let file = node.loc.file.strip_prefix(root).unwrap_or(&node.loc.file);
    let _ = write!(out, "{}{}", "  ".repeat(depth), node.label);
    if let Some(via) = node.via {
        let _ = write!(out, "  ← {}", tag(&via));
    }
    // Locations are 0-based on the wire and 1-based for humans.
    let _ = write!(
        out,
        "  {}:{}:{}",
        file.display(),
        node.loc.line + 1,
        node.loc.col + 1
    );
    if let Some(stop) = node.outcome.stop() {
        let _ = write!(out, "  [{}: {}]", tag(&stop.reason), stop.detail);
    }
    if node.truncated > 0 {
        let _ = write!(out, "  … {} more", node.truncated);
    }
    out.push('\n');
    for child in &node.children {
        line(out, child, depth + 1, root);
    }
}

fn tag<T: Serialize>(v: &T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        other => format!("{other:?}"),
    }
}
