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

struct Interfaces {
    text: String,
    definitions: HashMap<Pos, Location>,
    implementations: HashMap<Pos, Vec<Location>>,
}

impl Host for Interfaces {
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

    fn implementation(&mut self, _: &Path, pos: Pos) -> Result<Vec<Location>, HostError> {
        Ok(self.implementations.get(&pos).cloned().unwrap_or_default())
    }

    fn request_count(&self) -> u32 {
        0
    }
}

/// gopls lists the interfaces that embed a method among its implementations, so one
/// concrete method is reached twice: through `I.M` directly and again through `J.M`.
#[test]
fn a_method_reached_through_two_interfaces_is_still_a_callee() {
    let text = "package p\n\ntype I interface{ M() int }\ntype J interface{ M() int }\n\n\
                type T struct{}\n\nfunc (t T) M() int { return 7 }\n\n\
                func run(i I) int {\n\tv := i.M()\n\treturn v\n}\n";
    let (concrete, embedding) = (at("/r/p.go", 7, 11), at("/r/p.go", 3, 18));
    let mut host = Interfaces {
        text: text.to_string(),
        definitions: HashMap::from([
            (pos(10, 1), at("/r/p.go", 10, 1)),
            (pos(10, 8), at("/r/p.go", 2, 18)),
        ]),
        implementations: HashMap::from([
            (pos(2, 18), vec![concrete.clone(), embedding]),
            (pos(3, 18), vec![concrete]),
        ]),
    };
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest {
        root: "/r".into(),
        file: "/r/p.go".into(),
        pos: pos(10, 1),
        limits: Limits::default(),
    };
    let out = serde_json::to_string(&trace(&mut host, &reg, &req).unwrap()).unwrap();
    assert!(!out.contains("no implementation"), "{out}");
    assert!(out.contains("\"label\":\"7\""), "{out}");
}

#[test]
fn an_unimplemented_interface_beside_a_concrete_method_keeps_the_method() {
    let text = "package p\n\ntype I interface{ M() int }\ntype J interface{ M() int }\n\n\
                type T struct{}\n\nfunc (t T) M() int { return 7 }\n\n\
                func run(i I) int {\n\tv := i.M()\n\treturn v\n}\n";
    let mut host = Interfaces {
        text: text.to_string(),
        definitions: HashMap::from([
            (pos(10, 1), at("/r/p.go", 10, 1)),
            (pos(10, 8), at("/r/p.go", 2, 18)),
        ]),
        implementations: HashMap::from([(
            pos(2, 18),
            vec![at("/r/p.go", 7, 11), at("/r/p.go", 3, 18)],
        )]),
    };
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest {
        root: "/r".into(),
        file: "/r/p.go".into(),
        pos: pos(10, 1),
        limits: Limits::default(),
    };
    let out = serde_json::to_string(&trace(&mut host, &reg, &req).unwrap()).unwrap();
    assert!(out.contains("\"label\":\"7\""), "{out}");
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

/// Go's method expression names the type, not a receiver: `T.M(s, x)` passes the
/// receiver as its first argument, the way Rust's `T::m(s, x)` does.
#[test]
fn a_method_expression_passes_the_receiver_as_the_first_argument() {
    let text = "package p\n\ntype T struct{ f int }\n\n\
                func (t T) M(x int) int { return x }\n\n\
                func run(s T) int {\n\tv := T.M(s, 7)\n\treturn v\n}\n";
    let mut host = Interfaces {
        text: text.to_string(),
        definitions: HashMap::from([
            (pos(7, 1), at("/r/p.go", 7, 1)),
            (pos(7, 8), at("/r/p.go", 4, 11)),
            (pos(4, 33), at("/r/p.go", 4, 13)),
        ]),
        implementations: HashMap::new(),
    };
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest {
        root: "/r".into(),
        file: "/r/p.go".into(),
        pos: pos(7, 1),
        limits: Limits::default(),
    };
    let out = serde_json::to_string(&trace(&mut host, &reg, &req).unwrap()).unwrap();
    assert!(out.contains("\"label\":\"7\""), "{out}");
}

/// A variadic method takes any number of arguments, so one more than it declares
/// is not a method expression and the receiver stays where it is written.
#[test]
fn an_extra_argument_to_a_variadic_method_is_not_a_receiver() {
    let text = "package p\n\ntype T struct{ f int }\n\n\
                func (t T) N(x int, xs ...int) int { return x }\n\n\
                func run(s T) int {\n\tv := s.N(7, 8)\n\treturn v\n}\n";
    let mut host = Interfaces {
        text: text.to_string(),
        definitions: HashMap::from([
            (pos(7, 1), at("/r/p.go", 7, 1)),
            (pos(7, 8), at("/r/p.go", 4, 11)),
            (pos(4, 44), at("/r/p.go", 4, 13)),
        ]),
        implementations: HashMap::new(),
    };
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest {
        root: "/r".into(),
        file: "/r/p.go".into(),
        pos: pos(7, 1),
        limits: Limits::default(),
    };
    let out = serde_json::to_string(&trace(&mut host, &reg, &req).unwrap()).unwrap();
    assert!(out.contains("\"label\":\"7\""), "{out}");
}
