use std::path::{Path, PathBuf};

use whence::{
    host_replay::ReplayHost,
    lang::Registry,
    pos::Pos,
    trace::{TraceRequest, trace},
    tree::Limits,
};

struct Case {
    dir: &'static str,
    file: &'static str,
    pos: (u32, u32),
    limits: Limits,
    expected: &'static str,
}

fn fixture(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/erlang")
        .join(dir)
}

fn run(c: &Case) -> serde_json::Value {
    run_in(c, &fixture(c.dir))
}

fn run_in(c: &Case, dir: &Path) -> serde_json::Value {
    let dir = dir.to_path_buf();
    let mut host = ReplayHost::load(&dir).unwrap();
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest {
        root: dir.clone(),
        file: dir.join(c.file),
        pos: Pos {
            line: c.pos.0,
            col: c.pos.1,
        },
        limits: c.limits,
    };
    let tree = trace(&mut host, &reg, &req).unwrap();
    let mut v = serde_json::to_value(&tree).unwrap();
    relativise(&mut v, &dir);
    v["stats"]["ms"] = 0.into();
    v
}

fn relativise(v: &mut serde_json::Value, dir: &Path) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map.iter_mut() {
                if k == "file"
                    && let Some(s) = val.as_str()
                    && let Ok(rel) = Path::new(s).strip_prefix(dir)
                {
                    *val = rel.display().to_string().into();
                    continue;
                }
                relativise(val, dir);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                relativise(item, dir);
            }
        }
        _ => {}
    }
}

fn check(c: Case) -> serde_json::Value {
    let got = run(&c);
    let exp_path = fixture(c.dir).join(c.expected);
    if std::env::var("UPDATE_EXPECTED").is_ok() {
        std::fs::write(&exp_path, serde_json::to_string_pretty(&got).unwrap()).unwrap();
    }
    let raw = std::fs::read_to_string(&exp_path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} — run with UPDATE_EXPECTED=1 after inspecting the output",
            exp_path.display()
        )
    });
    let exp: serde_json::Value = serde_json::from_str(&raw).unwrap();
    pretty_assertions::assert_eq!(got, exp);
    got
}

#[test]
fn local_chain() {
    let v = check(Case {
        dir: "local_chain",
        file: "a.erl",
        pos: (6, 4),
        limits: Limits::default(),
        expected: "expected.json",
    });
    let z = &v["root"];
    assert_eq!(z["kind"], "binding");
    assert_eq!(z["label"], "Z");
    assert_eq!(z["via"], "match");
    let y = &z["children"][0];
    assert_eq!((&y["kind"], &y["label"]), (&"binding".into(), &"Y".into()));
    let x = &y["children"][0];
    assert_eq!((&x["kind"], &x["label"]), (&"param".into(), &"X".into()));
    assert_eq!(x["children"][0]["stop"]["reason"], "entry_point");
}

#[test]
fn param_callers() {
    let v = check(Case {
        dir: "param_callers",
        file: "b.erl",
        pos: (3, 8),
        limits: Limits::default(),
        expected: "expected.json",
    });
    assert_eq!(v["root"]["kind"], "param");
    let kids = v["root"]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0]["stop"]["reason"], "literal");
    assert_eq!(kids[1]["kind"], "binding");
    assert_eq!(kids[1]["children"][0]["stop"]["reason"], "external");
    assert_eq!(kids[1]["children"][0]["stop"]["detail"], "os:getenv");
}

#[test]
fn call_result() {
    let v = check(Case {
        dir: "call_result",
        file: "d.erl",
        pos: (5, 4),
        limits: Limits::default(),
        expected: "expected.json",
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["kind"], "call_result");
    assert_eq!(call["label"], "pick");
    assert_eq!(call["via"], "return");
    let kids = call["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0]["stop"]["detail"], "constructed value tuple");
    assert_eq!(kids[1]["stop"]["reason"], "literal");
}

#[test]
fn call_result_frame() {
    let v = check(Case {
        dir: "call_result",
        file: "d.erl",
        pos: (9, 4),
        limits: Limits::default(),
        expected: "expected_frame.json",
    });
    assert_eq!(v["root"]["children"][0]["children"][0]["via"], "arg");
    assert_eq!(
        v["root"]["children"][0]["children"][0]["children"][0]["stop"]["reason"],
        "literal"
    );
    // Resolving the parameter through the frame costs no host/references call.
    assert_eq!(v["stats"]["host_requests"], 4);
}

#[test]
fn external_call() {
    let v = check(Case {
        dir: "external_and_entry",
        file: "e.erl",
        pos: (5, 4),
        limits: Limits::default(),
        expected: "expected.json",
    });
    let stop = &v["root"]["children"][0]["stop"];
    assert_eq!(stop["reason"], "external");
    assert_eq!(stop["detail"], "cowboy_req:body");
}

#[test]
fn entry_point_param() {
    let v = check(Case {
        dir: "external_and_entry",
        file: "e.erl",
        pos: (4, 24),
        limits: Limits::default(),
        expected: "expected_entry.json",
    });
    assert_eq!(v["root"]["kind"], "param");
    let stop = &v["root"]["children"][0]["stop"];
    assert_eq!(stop["reason"], "entry_point");
    assert_eq!(stop["detail"], "no call sites of handle/1");
}

#[test]
fn fanout_truncates() {
    let v = check(Case {
        dir: "limits",
        file: "l.erl",
        pos: (3, 8),
        limits: Limits {
            fanout: 3,
            ..Default::default()
        },
        expected: "expected_fanout.json",
    });
    assert_eq!(v["root"]["children"].as_array().unwrap().len(), 3);
    assert_eq!(v["root"]["truncated"], 9);
    assert_eq!(v["stats"]["truncated"], 9);
}

#[test]
fn recursion_stops() {
    let v = check(Case {
        dir: "limits",
        file: "l.erl",
        pos: (5, 16),
        limits: Limits::default(),
        expected: "expected_recursion.json",
    });
    assert_eq!(v["root"]["kind"], "param");
    let stop = &v["root"]["children"][0]["stop"];
    assert_eq!(stop["reason"], "unresolved");
    assert_eq!(stop["detail"], "recursion");
}

#[test]
fn depth_limit_stops() {
    let v = run(&Case {
        dir: "local_chain",
        file: "a.erl",
        pos: (6, 4),
        limits: Limits {
            depth: 1,
            ..Default::default()
        },
        expected: "",
    });
    assert_eq!(v["root"]["children"][0]["stop"]["reason"], "limit");
    assert_eq!(v["root"]["children"][0]["stop"]["detail"], "depth");
}

#[test]
fn node_limit_stops() {
    let v = run(&Case {
        dir: "local_chain",
        file: "a.erl",
        pos: (6, 4),
        limits: Limits {
            nodes: 2,
            ..Default::default()
        },
        expected: "",
    });
    assert_eq!(
        v["root"]["children"][0]["children"][0]["stop"]["detail"],
        "nodes"
    );
}

#[test]
fn a_diamond_is_not_recursion() {
    let v = check(Case {
        dir: "diamond",
        file: "n.erl",
        pos: (5, 4),
        limits: Limits::default(),
        expected: "expected.json",
    });
    // Both clauses of pick/1 return their parameter, so both reach the same argument.
    let kids = v["root"]["children"][0]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    for k in kids {
        assert_eq!(k["children"][0]["stop"]["reason"], "literal");
        assert_eq!(k["children"][0]["label"], "3");
    }
    assert!(!serde_json::to_string(&v).unwrap().contains("recursion"));
}

#[test]
fn self_recursive_call_is_cut() {
    let v = check(Case {
        dir: "recursion",
        file: "r.erl",
        pos: (5, 4),
        limits: Limits::default(),
        expected: "expected_loop.json",
    });
    let s = serde_json::to_string(&v).unwrap();
    assert!(s.contains("recursive call to loop/1"), "{s}");
    assert!(!s.contains("\"limit\""), "{s}");
    assert!(v["stats"]["nodes"].as_u64().unwrap() < 10);
}

#[test]
fn accumulator_recursion_keeps_the_base_case() {
    let v = check(Case {
        dir: "recursion",
        file: "r.erl",
        pos: (11, 4),
        limits: Limits::default(),
        expected: "expected_sum.json",
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["label"], "sum");
    let kids = call["children"].as_array().unwrap();
    // The recursive clause is cut; the base clause resolves Acc to the call's argument.
    assert_eq!(
        kids[0]["children"][0]["stop"]["detail"],
        "recursive call to sum/2"
    );
    assert_eq!(kids[1]["kind"], "param");
    assert_eq!(kids[1]["children"][0]["stop"]["reason"], "literal");
    assert!(!serde_json::to_string(&v).unwrap().contains("\"limit\""));
}

#[test]
fn field_set_is_followed_to_the_construction() {
    let v = check(Case {
        dir: "field_access",
        file: "g.erl",
        pos: (8, 4),
        limits: Limits::default(),
        expected: "expected.json",
    });
    let field = &v["root"]["children"][0];
    assert_eq!(field["kind"], "field");
    assert_eq!(field["via"], "field_set");
    let stop = &field["children"][0]["stop"];
    assert_eq!(stop["reason"], "unresolved");
    assert_eq!(stop["detail"], "field peer of Req0");
}

#[test]
fn ids_do_not_depend_on_the_checkout_path() {
    let case = Case {
        dir: "call_result",
        file: "d.erl",
        pos: (5, 4),
        limits: Limits::default(),
        expected: "expected.json",
    };
    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("elsewhere");
    std::fs::create_dir(&copy).unwrap();
    for f in std::fs::read_dir(fixture(case.dir)).unwrap() {
        let f = f.unwrap().path();
        std::fs::copy(&f, copy.join(f.file_name().unwrap())).unwrap();
    }
    pretty_assertions::assert_eq!(run_in(&case, &copy), run(&case));
}

#[test]
fn cursor_off_an_identifier_is_an_error() {
    let dir = fixture("local_chain");
    let mut host = ReplayHost::load(&dir).unwrap();
    let reg = Registry::embedded().unwrap();
    let req = TraceRequest {
        root: dir.clone(),
        file: dir.join("a.erl"),
        pos: Pos { line: 0, col: 0 },
        limits: Limits::default(),
    };
    assert!(matches!(
        trace(&mut host, &reg, &req),
        Err(whence::trace::TraceError::NotIdentifier)
    ));
}
