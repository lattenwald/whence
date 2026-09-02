use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::host::Host;
use crate::lang::Registry;
use crate::pos::Pos;
use crate::syntax::{Doc, N};
use crate::trace::TraceError;
use crate::tree::Limits;

/// Names a node across borrows: `tree_sitter` node ids are valid for one tree borrow only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind_id: u16,
}

impl Span {
    pub fn of(n: N) -> Span {
        Span {
            start: n.0.start_byte(),
            end: n.0.end_byte(),
            kind_id: n.0.kind_id(),
        }
    }
}

pub fn node_at<'d>(doc: &'d Doc<'_>, span: Span) -> Option<N<'d>> {
    let mut n = doc
        .tree
        .root_node()
        .descendant_for_byte_range(span.start, span.end)?;
    loop {
        if n.start_byte() == span.start && n.end_byte() == span.end && n.kind_id() == span.kind_id {
            return Some(N(n));
        }
        n = n.parent()?;
    }
}

pub type ExprRef = (PathBuf, Span);

pub struct Frame {
    /// `<file>:<name>/<arity>` of the callee we descended into.
    pub func_id: String,
    pub args: Vec<ExprRef>,
}

pub struct Ctx<'a> {
    pub host: &'a mut dyn Host,
    pub reg: &'a Registry,
    pub root: &'a Path,
    pub docs: HashMap<PathBuf, Rc<Doc<'a>>>,
    pub frames: Vec<Frame>,
    pub visited: HashSet<(PathBuf, Pos, u64)>,
    pub limits: Limits,
    pub deadline: Instant,
    pub node_count: u32,
    pub truncated: u32,
}

impl<'a> Ctx<'a> {
    pub fn new(
        host: &'a mut dyn Host,
        reg: &'a Registry,
        root: &'a Path,
        limits: Limits,
        start: Instant,
    ) -> Ctx<'a> {
        Ctx {
            host,
            reg,
            root,
            docs: HashMap::new(),
            frames: Vec::new(),
            visited: HashSet::new(),
            limits,
            deadline: start + Duration::from_millis(limits.time_ms),
            node_count: 0,
            truncated: 0,
        }
    }

    pub fn doc(&mut self, file: &Path) -> Result<Rc<Doc<'a>>, TraceError> {
        if let Some(d) = self.docs.get(file) {
            return Ok(d.clone());
        }
        let lang = self
            .reg
            .for_file(file)
            .ok_or_else(|| TraceError::NoLanguage(file.to_path_buf()))?;
        let text = self.host.text(file)?;
        let doc = Rc::new(Doc::parse(lang, file.to_path_buf(), text));
        self.docs.insert(file.to_path_buf(), doc.clone());
        Ok(doc)
    }

    pub fn frame_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        for f in &self.frames {
            f.func_id.hash(&mut h);
            for (file, span) in &f.args {
                file.hash(&mut h);
                span.start.hash(&mut h);
            }
        }
        h.finish()
    }

    pub fn in_root(&self, file: &Path) -> bool {
        file.starts_with(self.root)
    }
}
