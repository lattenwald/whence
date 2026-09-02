use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::host::{Host, Location};
use crate::lang::Registry;
use crate::pos::Pos;
use crate::syntax::{Doc, Span};
use crate::trace::TraceError;
use crate::tree::Limits;

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
    /// Host answers are facts about the snapshot in `docs`, so they live exactly as long.
    defs: HashMap<(PathBuf, Pos), Vec<Location>>,
    refs: HashMap<(PathBuf, Pos, bool), Vec<Location>>,
    pub frames: Vec<Frame>,
    /// Path keys of the nodes being expanded, root first; ids derive from the top.
    pub path: Vec<u64>,
    /// The expressions on the *current* expansion path, not everything seen (spec §5.4).
    pub visited: HashSet<(PathBuf, Span, u64)>,
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
            defs: HashMap::new(),
            refs: HashMap::new(),
            frames: Vec::new(),
            path: Vec::new(),
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

    pub fn definition(&mut self, file: &Path, pos: Pos) -> Result<Vec<Location>, TraceError> {
        let key = (file.to_path_buf(), pos);
        if let Some(v) = self.defs.get(&key) {
            return Ok(v.clone());
        }
        let v = self.host.definition(file, pos)?;
        self.defs.insert(key, v.clone());
        Ok(v)
    }

    pub fn references(
        &mut self,
        file: &Path,
        pos: Pos,
        include_decl: bool,
    ) -> Result<Vec<Location>, TraceError> {
        let key = (file.to_path_buf(), pos, include_decl);
        if let Some(v) = self.refs.get(&key) {
            return Ok(v.clone());
        }
        let v = self.host.references(file, pos, include_decl)?;
        self.refs.insert(key, v.clone());
        Ok(v)
    }

    pub fn parent(&self) -> u64 {
        self.path.last().copied().unwrap_or(0)
    }

    pub fn frame_hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        for f in &self.frames {
            f.func_id.hash(&mut h);
            for (file, span) in &f.args {
                self.rel(file).hash(&mut h);
                span.start.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Ids and hashes use this, so the same workspace traces alike at any checkout path.
    pub fn rel<'p>(&self, file: &'p Path) -> &'p Path {
        file.strip_prefix(self.root).unwrap_or(file)
    }

    /// `<relpath>:<name>/<arity>`: what frames and the recursion cut match on.
    pub fn func_id(&self, file: &Path, name: &str, arity: usize) -> String {
        format!("{}:{name}/{arity}", self.rel(file).display())
    }

    pub fn in_root(&self, file: &Path) -> bool {
        file.starts_with(self.root)
    }
}
