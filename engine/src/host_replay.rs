use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::host::{Highlight, Host, HostError, Location};
use crate::pos::Pos;

#[derive(Deserialize)]
struct Recorded {
    #[serde(default)]
    definition: HashMap<String, Vec<Location>>,
    #[serde(default)]
    references: HashMap<String, Vec<Location>>,
    #[serde(default, rename = "documentHighlight")]
    document_highlight: HashMap<String, Vec<Highlight>>,
}

pub struct ReplayHost {
    dir: PathBuf,
    recorded: Recorded,
    count: u32,
}

impl ReplayHost {
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join("host.json");
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let mut recorded: Recorded =
            serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        for locs in recorded
            .definition
            .values_mut()
            .chain(recorded.references.values_mut())
        {
            for l in locs {
                if let Ok(rest) = l.file.strip_prefix("$HOME") {
                    l.file = std::env::home_dir().unwrap_or_default().join(rest);
                } else if l.file.is_relative() {
                    l.file = dir.join(&l.file);
                }
            }
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            recorded,
            count: 0,
        })
    }

    pub fn reset(&mut self) {
        self.count = 0;
    }

    fn key(&self, file: &Path, pos: Pos) -> String {
        let rel = file.strip_prefix(&self.dir).unwrap_or(file);
        format!("{}:{}:{}", rel.display(), pos.line, pos.col)
    }
}

fn unrecorded(method: &str) -> HostError {
    HostError::Rpc {
        method: method.to_string(),
        message: "unrecorded".to_string(),
    }
}

impl Host for ReplayHost {
    fn text(&mut self, file: &Path) -> Result<String, HostError> {
        self.count += 1;
        Ok(std::fs::read_to_string(file)?)
    }

    fn definition(&mut self, file: &Path, pos: Pos) -> Result<Vec<Location>, HostError> {
        self.count += 1;
        let hit = self.recorded.definition.get(&self.key(file, pos));
        hit.cloned().ok_or_else(|| unrecorded("host/definition"))
    }

    fn references(
        &mut self,
        file: &Path,
        pos: Pos,
        include_decl: bool,
    ) -> Result<Vec<Location>, HostError> {
        self.count += 1;
        let key = format!(
            "{}|{}",
            self.key(file, pos),
            if include_decl { "decl" } else { "nodecl" }
        );
        let hit = self.recorded.references.get(&key);
        hit.cloned().ok_or_else(|| unrecorded("host/references"))
    }

    fn document_highlight(&mut self, file: &Path, pos: Pos) -> Result<Vec<Highlight>, HostError> {
        self.count += 1;
        let hit = self.recorded.document_highlight.get(&self.key(file, pos));
        hit.cloned()
            .ok_or_else(|| unrecorded("host/documentHighlight"))
    }

    fn request_count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("a.erl"), "f(X) -> X.\n").unwrap();
        std::fs::write(
            d.path().join("host.json"),
            r#"{
          "definition": { "a.erl:0:8": [ {"file":"a.erl","range":{"start":{"line":0,"col":2},"end":{"line":0,"col":3}}} ] },
          "references": { "a.erl:0:2|nodecl": [ {"file":"/outside/b.erl","range":{"start":{"line":9,"col":0},"end":{"line":9,"col":1}}} ] },
          "documentHighlight": { "a.erl:0:8": [ {"range":{"start":{"line":0,"col":8},"end":{"line":0,"col":9}},"kind":"read"} ] } }"#,
        )
        .unwrap();
        d
    }

    #[test]
    fn answers_recorded_definition_with_absolute_paths() {
        let d = fixture();
        let mut h = ReplayHost::load(d.path()).unwrap();
        let locs = h
            .definition(&d.path().join("a.erl"), Pos { line: 0, col: 8 })
            .unwrap();
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].file, d.path().join("a.erl"));
        assert_eq!(h.request_count(), 1);
    }

    #[test]
    fn unrecorded_request_is_error() {
        let d = fixture();
        let mut h = ReplayHost::load(d.path()).unwrap();
        assert!(
            h.references(&d.path().join("a.erl"), Pos { line: 0, col: 0 }, true)
                .is_err()
        );
    }

    #[test]
    fn reference_keys_distinguish_include_declaration() {
        let d = fixture();
        let mut h = ReplayHost::load(d.path()).unwrap();
        let p = d.path().join("a.erl");
        let locs = h.references(&p, Pos { line: 0, col: 2 }, false).unwrap();
        assert_eq!(locs[0].file, PathBuf::from("/outside/b.erl"));
        assert!(h.references(&p, Pos { line: 0, col: 2 }, true).is_err());
    }

    #[test]
    fn home_placeholder_expands_to_this_machine() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("host.json"),
            r#"{"definition":{"a.erl:0:0":[{"file":"$HOME/lib/os.erl","range":{"start":{"line":0,"col":0},"end":{"line":0,"col":1}}}]},"references":{}}"#,
        )
        .unwrap();
        let mut h = ReplayHost::load(d.path()).unwrap();
        let locs = h
            .definition(&d.path().join("a.erl"), Pos { line: 0, col: 0 })
            .unwrap();
        assert_eq!(
            locs[0].file,
            std::env::home_dir().unwrap().join("lib/os.erl")
        );
    }
}
