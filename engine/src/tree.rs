use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Binding,
    Branch,
    Param,
    CallResult,
    Field,
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
    Field,
    Element,
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

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopTag {
    #[default]
    Stop,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Null;

impl Serialize for Null {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_none()
    }
}

impl<'de> Deserialize<'de> for Null {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Option::<()>::deserialize(d).map(|_| Null)
    }
}

/// One field behind the wire's `kind` and `stop`, so they cannot disagree.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Outcome {
    Stop { kind: StopTag, stop: Stop },
    Construct { kind: NodeKind, stop: Null },
}

impl Outcome {
    pub fn kind(&self) -> Option<&NodeKind> {
        match self {
            Outcome::Construct { kind, .. } => Some(kind),
            Outcome::Stop { .. } => None,
        }
    }

    pub fn stop(&self) -> Option<&Stop> {
        match self {
            Outcome::Stop { stop, .. } => Some(stop),
            Outcome::Construct { .. } => None,
        }
    }

    fn id_tag(&self) -> String {
        match self {
            Outcome::Construct { kind, .. } => format!("{kind:?}"),
            Outcome::Stop { .. } => "Stop".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(flatten)]
    pub outcome: Outcome,
    pub label: String,
    pub loc: Loc,
    pub via: Option<Via>,
    pub snippet: String,
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
    /// `false`: wherever the tree would fork, stop with one `unresolved` naming the candidates.
    pub split: bool,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            depth: 64,
            fanout: 8,
            nodes: 400,
            time_ms: 10_000,
            split: true,
        }
    }
}

impl Node {
    /// `id_path`: the root-relative path, so ids do not depend on the checkout location.
    /// `nth`: which of several stops at the same place under the same parent this is.
    #[allow(clippy::too_many_arguments)]
    pub fn stop(
        parent: u64,
        id_path: &Path,
        loc: Loc,
        label: &str,
        snippet: &str,
        reason: StopReason,
        detail: impl Into<String>,
        nth: u32,
    ) -> Node {
        let outcome = Outcome::Stop {
            kind: StopTag::Stop,
            stop: Stop {
                reason,
                detail: detail.into(),
            },
        };
        Node {
            id: node_id(parent, id_path, loc.line, loc.col, &outcome, nth),
            outcome,
            label: label.to_string(),
            loc,
            via: None,
            snippet: snippet.to_string(),
            children: Vec::new(),
            truncated: 0,
        }
    }

    pub fn construct(kind: NodeKind) -> Outcome {
        Outcome::Construct { kind, stop: Null }
    }

    pub fn count(&self) -> u32 {
        1 + self.children.iter().map(Node::count).sum::<u32>()
    }
}

/// Path-dependent (spec §5.1): one place under two parents is two nodes.
pub fn node_id(
    parent: u64,
    file: &Path,
    line: u32,
    col: u32,
    outcome: &Outcome,
    nth: u32,
) -> String {
    let digest = Sha256::digest(format!(
        "{parent:016x}:{}:{line}:{col}:{}:{nth}",
        file.display(),
        outcome.id_tag()
    ));
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// The path key a node hands to its children: its parent's key plus its own place.
pub fn path_id(parent: u64, file: &Path, line: u32, col: u32) -> u64 {
    let digest = Sha256::digest(format!("{parent:016x}:{}:{line}:{col}", file.display()));
    u64::from_be_bytes(digest[..8].try_into().expect("8 bytes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_spec_shape() {
        let n = Node::stop(
            0,
            Path::new("a.erl"),
            Loc {
                file: "/root/a.erl".into(),
                line: 1,
                col: 2,
            },
            "X",
            "X = 1",
            StopReason::Literal,
            "integer",
            0,
        );
        let v = serde_json::to_value(&n).unwrap();
        assert_eq!(v["kind"], "stop");
        assert_eq!(v["stop"]["reason"], "literal");
        assert_eq!(serde_json::from_value::<Node>(v.clone()).unwrap(), n);
        assert_eq!(v["via"], serde_json::Value::Null);
        assert_eq!(v["truncated"], 0);
        assert_eq!(v["children"], serde_json::json!([]));
        assert_eq!(v["id"].as_str().unwrap().len(), 16);
    }

    #[test]
    fn kind_and_stop_cannot_disagree() {
        let bad = r#"{"id":"x","kind":"stop","stop":null,"label":"","loc":{"file":"a","line":0,"col":0},
                      "via":null,"snippet":"","children":[],"truncated":0}"#;
        assert!(serde_json::from_str::<Node>(bad).is_err());
        let bad = bad.replace(
            r#""kind":"stop","stop":null"#,
            r#""kind":"param","stop":{"reason":"literal","detail":""}"#,
        );
        assert!(serde_json::from_str::<Node>(&bad).is_err());
        let good = bad.replace(
            r#""stop":{"reason":"literal","detail":""}"#,
            r#""stop":null"#,
        );
        let n: Node = serde_json::from_str(&good).unwrap();
        assert_eq!(n.outcome.kind(), Some(&NodeKind::Param));
        let v = serde_json::to_value(&n).unwrap();
        assert_eq!(v["stop"], serde_json::Value::Null);
    }

    #[test]
    fn limits_default_and_partial_override() {
        let l: Limits = serde_json::from_str(r#"{"fanout":3}"#).unwrap();
        assert_eq!((l.depth, l.fanout, l.nodes, l.split), (64, 3, 400, true));
    }

    #[test]
    fn stop_id_ignores_the_absolute_location() {
        let stop = |dir: &str| {
            Node::stop(
                0,
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
                0,
            )
            .id
        };
        assert_eq!(stop("/home/one"), stop("/build/two"));
    }

    #[test]
    fn node_id_is_stable_and_path_sensitive() {
        let p = Path::new("/a.erl");
        let (param, binding) = (
            Node::construct(NodeKind::Param),
            Node::construct(NodeKind::Binding),
        );
        let a = node_id(0, p, 1, 2, &param, 0);
        assert_eq!(a, node_id(0, p, 1, 2, &param, 0));
        assert_ne!(a, node_id(1, p, 1, 2, &param, 0));
        assert_ne!(a, node_id(0, p, 1, 2, &param, 1));
        assert_ne!(a, node_id(0, p, 1, 2, &binding, 0));
        assert_ne!(path_id(0, p, 1, 2), path_id(path_id(0, p, 1, 2), p, 1, 2));
    }
}
