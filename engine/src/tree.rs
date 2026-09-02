use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Binding,
    Param,
    CallResult,
    Field,
    Stop,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    Match,
    Rebind,
    Mutation,
    Arg,
    Return,
    FieldSet,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    External,
    EntryPoint,
    Literal,
    Unresolved,
    Limit,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Stop {
    pub reason: StopReason,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Loc {
    pub file: PathBuf,
    pub line: u32,
    pub col: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub loc: Loc,
    pub via: Option<Via>,
    pub snippet: String,
    pub stop: Option<Stop>,
    pub children: Vec<Node>,
    pub truncated: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Stats {
    pub nodes: u32,
    pub truncated: u32,
    pub host_requests: u32,
    pub ms: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Tree {
    pub root: Node,
    pub stats: Stats,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(default)]
pub struct Limits {
    pub depth: u32,
    pub fanout: u32,
    pub nodes: u32,
    pub time_ms: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            depth: 64,
            fanout: 8,
            nodes: 400,
            time_ms: 10_000,
        }
    }
}

impl Node {
    pub fn stop(
        loc: Loc,
        label: &str,
        snippet: &str,
        reason: StopReason,
        detail: impl Into<String>,
    ) -> Node {
        Node::stop_rel(&loc.file.clone(), loc, label, snippet, reason, detail)
    }

    /// `id_path`: the root-relative path, so ids do not depend on the checkout location.
    pub fn stop_rel(
        id_path: &Path,
        loc: Loc,
        label: &str,
        snippet: &str,
        reason: StopReason,
        detail: impl Into<String>,
    ) -> Node {
        let kind = NodeKind::Stop;
        Node {
            id: node_id(id_path, loc.line, loc.col, &kind, 0),
            kind,
            label: label.to_string(),
            loc,
            via: None,
            snippet: snippet.to_string(),
            stop: Some(Stop {
                reason,
                detail: detail.into(),
            }),
            children: Vec::new(),
            truncated: 0,
        }
    }

    pub fn count(&self) -> u32 {
        1 + self.children.iter().map(Node::count).sum::<u32>()
    }
}

pub fn node_id(file: &Path, line: u32, col: u32, kind: &NodeKind, frame_hash: u64) -> String {
    let digest = Sha256::digest(format!(
        "{}:{line}:{col}:{kind:?}:{frame_hash}",
        file.display()
    ));
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_spec_shape() {
        let n = Node::stop(
            Loc {
                file: "/root/a.erl".into(),
                line: 1,
                col: 2,
            },
            "X",
            "X = 1",
            StopReason::Literal,
            "integer",
        );
        let v = serde_json::to_value(&n).unwrap();
        assert_eq!(v["kind"], "stop");
        assert_eq!(v["stop"]["reason"], "literal");
        assert_eq!(v["via"], serde_json::Value::Null);
        assert_eq!(v["truncated"], 0);
        assert_eq!(v["children"], serde_json::json!([]));
        assert_eq!(v["id"].as_str().unwrap().len(), 16);
        assert_eq!(n.count(), 1);
    }

    #[test]
    fn limits_default_and_partial_override() {
        let l: Limits = serde_json::from_str(r#"{"fanout":3}"#).unwrap();
        assert_eq!((l.depth, l.fanout, l.nodes), (64, 3, 400));
    }

    #[test]
    fn stop_id_ignores_the_absolute_location() {
        let stop = |dir: &str| {
            Node::stop_rel(
                Path::new("a.erl"),
                Loc {
                    file: format!("{dir}/a.erl").into(),
                    line: 1,
                    col: 2,
                },
                "X",
                "X = 1",
                StopReason::Literal,
                "integer",
            )
            .id
        };
        assert_eq!(stop("/home/one"), stop("/build/two"));
    }

    #[test]
    fn node_id_is_stable_and_frame_sensitive() {
        let p = Path::new("/a.erl");
        let a = node_id(p, 1, 2, &NodeKind::Param, 0);
        assert_eq!(a, node_id(p, 1, 2, &NodeKind::Param, 0));
        assert_ne!(a, node_id(p, 1, 2, &NodeKind::Param, 1));
        assert_ne!(a, node_id(p, 1, 2, &NodeKind::Binding, 0));
    }
}
