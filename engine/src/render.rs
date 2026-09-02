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
    if let Some(stop) = &node.stop {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{Loc, Node, NodeKind, Stats, StopReason, Via};
    use std::path::Path;

    fn loc(line: u32, col: u32) -> Loc {
        Loc {
            file: "/root/a.erl".into(),
            line,
            col,
        }
    }

    #[test]
    fn renders_indented_lines_with_via_stop_and_truncation() {
        let child = Node::stop(
            0,
            Path::new("a.erl"),
            loc(3, 2),
            "X",
            "f(X) ->",
            StopReason::EntryPoint,
            "no call sites",
            0,
        );
        let root = Node {
            id: "id".into(),
            kind: NodeKind::Binding,
            label: "Z".into(),
            loc: loc(5, 4),
            via: Some(Via::Match),
            snippet: "Z = Y,".into(),
            stop: None,
            children: vec![child],
            truncated: 2,
        };
        let tree = Tree {
            root,
            stats: Stats {
                nodes: 2,
                truncated: 2,
                host_requests: 0,
                ms: 0,
            },
        };
        let text = render_text(&tree, Path::new("/root"));
        assert_eq!(
            text,
            "Z  ← match  a.erl:6:5  … 2 more\n  X  a.erl:4:3  [entry_point: no call sites]\n"
        );
    }
}
