//! Faked host answers: no real server sends a definition into a grammarless file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use whence::{
    host::{Highlight, Host, HostError, Location, Range},
    host_replay::ReplayHost,
    lang::Registry,
    pos::Pos,
    trace::{TraceRequest, trace},
    tree::Limits,
};

struct FakeHost {
    text: String,
    definitions: HashMap<Pos, Location>,
}

impl Host for FakeHost {
    fn text(&mut self, _: &Path) -> Result<String, HostError> {
        Ok(self.text.clone())
    }

    fn definition(&mut self, _: &Path, pos: Pos) -> Result<Vec<Location>, HostError> {
        Ok(self.definitions.get(&pos).cloned().into_iter().collect())
    }

    fn references(&mut self, _: &Path, _: Pos, _: bool) -> Result<Vec<Location>, HostError> {
        Ok(Vec::new())
    }

    fn document_highlight(&mut self, _: &Path, _: Pos) -> Result<Vec<Highlight>, HostError> {
        Err(HostError::Unsupported("documentHighlight"))
    }

    fn implementation(&mut self, _: &Path, _: Pos) -> Result<Vec<Location>, HostError> {
        Err(HostError::Unsupported("implementation"))
    }

    fn request_count(&self) -> u32 {
        0
    }
}

fn pos(line: u32, col: u32) -> Pos {
    Pos { line, col }
}

fn at(file: &str, line: u32, col: u32) -> Location {
    Location {
        file: PathBuf::from(file),
        range: Range {
            start: pos(line, col),
            end: pos(line, col + 1),
        },
    }
}

/// `body` binds `Y` at 1:4 to a value whose definition is faked into a grammarless file.
fn trace_into_grammarless(body: &str) -> String {
    static REG: OnceLock<Registry> = OnceLock::new();
    let reg = REG.get_or_init(|| Registry::embedded().unwrap());
    let mut host = FakeHost {
        text: body.to_string(),
        definitions: HashMap::from([
            (pos(2, 4), at("/r/a.erl", 1, 4)),
            (pos(1, 8), at("/r/x.escript", 0, 0)),
        ]),
    };
    let req = TraceRequest {
        root: "/r".into(),
        file: "/r/a.erl".into(),
        pos: pos(2, 4),
        limits: Limits::default(),
    };
    serde_json::to_string(&trace(&mut host, reg, &req).unwrap()).unwrap()
}

#[test]
fn a_definition_in_a_file_without_a_grammar_stops_that_node_only() {
    let out = trace_into_grammarless("f(X) ->\n    Y = X,\n    Y.\n");
    assert!(out.contains("\"label\":\"Y\""), "{out}");
    assert!(out.contains("no language for x.escript"), "{out}");
}

#[test]
fn a_callee_defined_in_a_file_without_a_grammar_stops_that_node_only() {
    let out = trace_into_grammarless("g() ->\n    Y = mk(),\n    Y.\n");
    assert!(out.contains("no language for x.escript"), "{out}");
}

struct Counting {
    inner: ReplayHost,
    highlights: u32,
}

impl Host for Counting {
    fn text(&mut self, file: &Path) -> Result<String, HostError> {
        self.inner.text(file)
    }

    fn definition(&mut self, file: &Path, pos: Pos) -> Result<Vec<Location>, HostError> {
        self.inner.definition(file, pos)
    }

    fn references(
        &mut self,
        file: &Path,
        pos: Pos,
        include_decl: bool,
    ) -> Result<Vec<Location>, HostError> {
        self.inner.references(file, pos, include_decl)
    }

    fn document_highlight(&mut self, file: &Path, pos: Pos) -> Result<Vec<Highlight>, HostError> {
        self.highlights += 1;
        self.inner.document_highlight(file, pos)
    }

    fn implementation(&mut self, file: &Path, pos: Pos) -> Result<Vec<Location>, HostError> {
        self.inner.implementation(file, pos)
    }

    fn request_count(&self) -> u32 {
        self.inner.request_count()
    }
}

#[test]
fn a_single_assignment_language_asks_for_no_highlights() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/erlang/local_chain");
    let mut host = Counting {
        inner: ReplayHost::load(&dir).unwrap(),
        highlights: 0,
    };
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest {
        file: dir.join("a.erl"),
        root: dir,
        pos: pos(6, 4),
        limits: Limits::default(),
    };
    trace(&mut host, &reg, &req).unwrap();
    assert_eq!(host.highlights, 0);
}
