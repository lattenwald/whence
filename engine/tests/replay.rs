use std::path::{Path, PathBuf};

use whence::{
    host_replay::ReplayHost,
    lang::Registry,
    pos::Pos,
    trace::{TraceRequest, trace},
    tree::{Limits, Tree},
};

struct Case {
    dir: &'static str,
    file: &'static str,
    pos: (u32, u32),
    limits: Limits,
    expected: &'static str,
}

impl Default for Case {
    fn default() -> Self {
        Case {
            dir: "",
            file: "",
            pos: (0, 0),
            limits: Limits::default(),
            expected: "expected.json",
        }
    }
}

fn fixture(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
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
    serde_json::from_value::<Tree>(exp).expect("golden is a well-formed Tree");
    got
}

#[test]
fn local_chain() {
    let v = check(Case {
        dir: "erlang/local_chain",
        file: "a.erl",
        pos: (6, 4),
        ..Default::default()
    });
    let z = &v["root"];
    assert_eq!(z["kind"], "binding");
    assert_eq!(z["label"], "Z");
    assert_eq!(z["via"], "match");
    let y = &z["children"][0];
    assert_eq!((&y["kind"], &y["label"]), (&"binding".into(), &"Y".into()));
    let x = &y["children"][0];
    assert_eq!((&x["kind"], &x["label"]), (&"param".into(), &"X".into()));
    // The parameter is reached through Y's pattern, not through a call site.
    assert_eq!(x["via"], "match");
    assert_eq!(x["children"][0]["stop"]["reason"], "entry_point");
}

#[test]
fn param_callers() {
    let v = check(Case {
        dir: "erlang/param_callers",
        file: "b.erl",
        pos: (3, 8),
        ..Default::default()
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
        dir: "erlang/call_result",
        file: "d.erl",
        pos: (5, 4),
        ..Default::default()
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
        dir: "erlang/call_result",
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
        dir: "erlang/external_and_entry",
        file: "e.erl",
        pos: (5, 4),
        ..Default::default()
    });
    let stop = &v["root"]["children"][0]["stop"];
    assert_eq!(stop["reason"], "external");
    assert_eq!(stop["detail"], "os:getenv");
}

#[test]
fn entry_point_param() {
    let v = check(Case {
        dir: "erlang/external_and_entry",
        file: "e.erl",
        pos: (4, 18),
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
        dir: "erlang/limits",
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
        dir: "erlang/limits",
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
        dir: "erlang/local_chain",
        file: "a.erl",
        pos: (6, 4),
        limits: Limits {
            depth: 1,
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(v["root"]["children"][0]["stop"]["reason"], "limit");
    assert_eq!(v["root"]["children"][0]["stop"]["detail"], "depth");
}

#[test]
fn node_limit_stops() {
    let v = run(&Case {
        dir: "erlang/local_chain",
        file: "a.erl",
        pos: (6, 4),
        limits: Limits {
            nodes: 2,
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(
        v["root"]["children"][0]["children"][0]["stop"]["detail"],
        "nodes"
    );
}

#[test]
fn a_diamond_is_not_recursion() {
    let v = check(Case {
        dir: "erlang/diamond",
        file: "n.erl",
        pos: (5, 4),
        ..Default::default()
    });
    // Both clauses of pick/1 return their parameter, so both reach the same argument.
    let kids = v["root"]["children"][0]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    for k in kids {
        assert_eq!(k["children"][0]["stop"]["reason"], "literal");
        assert_eq!(k["children"][0]["label"], "3");
    }
    assert!(!serde_json::to_string(&v).unwrap().contains("recursion"));
    assert_ne!(kids[0]["children"][0]["id"], kids[1]["children"][0]["id"]);
    let ids = collect_ids(&v["root"]);
    let distinct: std::collections::HashSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), distinct.len());
}

fn collect_ids(n: &serde_json::Value) -> Vec<&str> {
    let mut out = vec![n["id"].as_str().unwrap()];
    for c in n["children"].as_array().unwrap() {
        out.extend(collect_ids(c));
    }
    out
}

#[test]
fn self_recursive_call_is_cut() {
    let v = check(Case {
        dir: "erlang/recursion",
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
        dir: "erlang/recursion",
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
        dir: "erlang/field_access",
        file: "g.erl",
        pos: (8, 4),
        ..Default::default()
    });
    let of = &v["root"]["children"][0];
    assert_eq!(of["kind"], "field");
    assert_eq!(of["label"], "peer of R");
    let r = &of["children"][0];
    assert_eq!(
        (&r["kind"], &r["via"]),
        (&"binding".into(), &"field".into())
    );
    let set = &r["children"][0];
    assert_eq!(set["via"], "field_set");
    assert_eq!(set["label"], "peer of Req0");
    let container = &set["children"][0];
    assert_eq!(container["kind"], "param");
    assert_eq!(container["via"], "field");
    assert_eq!(container["children"][0]["stop"]["reason"], "entry_point");
}

#[test]
fn a_shadowing_container_is_not_matched_by_name() {
    let v = check(Case {
        dir: "erlang/field_projection",
        file: "p.erl",
        pos: (7, 18),
        expected: "expected_shadow.json",
        ..Default::default()
    });
    // The fun's own R, not the outer `R = #r{a = X}` two lines up.
    let stop = &v["root"]["children"][0]["children"][0];
    assert_eq!(stop["stop"]["reason"], "unresolved");
    assert_eq!(stop["stop"]["detail"], "bound inside anonymous_fun");
    assert!(!serde_json::to_string(&v).unwrap().contains("\"X\""));
}

#[test]
fn a_field_is_taken_from_a_callee_construction() {
    let v = check(Case {
        dir: "erlang/field_projection",
        file: "p.erl",
        pos: (16, 4),
        expected: "expected_call.json",
        ..Default::default()
    });
    let r = &v["root"]["children"][0]["children"][0];
    assert_eq!(r["label"], "R");
    let call = &r["children"][0];
    assert_eq!(
        (&call["kind"], &call["label"]),
        (&"call_result".into(), &"make".into())
    );
    let set = &call["children"][0];
    assert_eq!(
        (&set["via"], &set["label"]),
        (&"field_set".into(), &"V".into())
    );
    assert_eq!(set["children"][0]["label"], "1");
}

#[test]
fn an_update_that_sets_the_field_is_its_source() {
    let v = check(Case {
        dir: "erlang/field_projection",
        file: "p.erl",
        pos: (17, 4),
        expected: "expected_update.json",
        ..Default::default()
    });
    let set = &v["root"]["children"][0]["children"][0]["children"][0];
    assert_eq!(
        (&set["via"], &set["label"]),
        (&"field_set".into(), &"2".into())
    );
}

#[test]
fn an_update_without_the_field_passes_to_its_base() {
    let v = check(Case {
        dir: "erlang/field_projection",
        file: "p.erl",
        pos: (18, 4),
        expected: "expected_base.json",
        ..Default::default()
    });
    let r2 = &v["root"]["children"][0]["children"][0];
    assert_eq!(r2["label"], "R2");
    let r = &r2["children"][0];
    assert_eq!((&r["label"], &r["via"]), (&"R".into(), &"field".into()));
    assert_eq!(r["children"][0]["label"], "make");
}

#[test]
fn every_construction_of_the_container_is_a_sibling() {
    let v = check(Case {
        dir: "erlang/honesty_field_branches",
        file: "f.erl",
        pos: (11, 4),
        ..Default::default()
    });
    let of = &v["root"]["children"][0];
    assert_eq!(of["kind"], "field");
    // Both case branches bind R: the server reports two definitions.
    let r = &of["children"][0];
    assert_eq!((&r["kind"], &r["label"]), (&"branch".into(), &"R".into()));
    let kids = r["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    for (k, want) in kids.iter().zip(["one", "two"]) {
        assert_eq!(k["kind"], "binding");
        assert_eq!(k["children"][0]["via"], "field_set");
        assert_eq!(k["children"][0]["label"], want);
    }
}

#[test]
fn split_off_collapses_every_fork_to_one_stop() {
    let no_split = Limits {
        split: false,
        ..Default::default()
    };
    let v = run(&Case {
        dir: "erlang/honesty_field_branches",
        file: "f.erl",
        pos: (11, 4),
        limits: no_split,
        ..Default::default()
    });
    let r = &v["root"]["children"][0]["children"][0];
    assert_eq!(r["kind"], "branch");
    assert_eq!(r["truncated"], 2);
    assert_eq!(r["children"].as_array().unwrap().len(), 1);
    assert_eq!(
        r["children"][0]["stop"]["detail"],
        "2 candidates: definitions"
    );

    let v = run(&Case {
        dir: "erlang/diamond",
        file: "n.erl",
        pos: (5, 4),
        limits: no_split,
        ..Default::default()
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["kind"], "call_result");
    assert_eq!(
        call["children"][0]["stop"]["detail"],
        "2 candidates: return expressions"
    );
    assert_eq!(v["stats"]["truncated"], 2);
}

#[test]
fn references_that_are_not_call_sites_are_not_an_entry_point() {
    let v = check(Case {
        dir: "erlang/honesty_callback_ref",
        file: "c.erl",
        pos: (5, 9),
        ..Default::default()
    });
    let stop = &v["root"]["children"][0];
    assert_eq!(stop["stop"]["reason"], "unresolved");
    assert_eq!(
        stop["stop"]["detail"],
        "1 reference(s) to cb/1 are not call sites"
    );
    // `fun cb/1` is where the user has to look, so it is a node to jump to.
    let stray = &stop["children"][0];
    assert_eq!(stray["stop"]["detail"], "reference is not a call site");
    assert_eq!(
        (&stray["loc"]["line"], &stray["loc"]["col"]),
        (&3.into(), &22.into())
    );
}

#[test]
fn several_definitions_are_all_shown() {
    let v = check(Case {
        dir: "erlang/honesty_multi_def",
        file: "m.erl",
        pos: (9, 4),
        ..Default::default()
    });
    let x = &v["root"]["children"][0];
    assert_eq!((&x["kind"], &x["label"]), (&"branch".into(), &"X".into()));
    let kids = x["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    let leaves: Vec<&str> = kids
        .iter()
        .map(|k| k["children"][0]["label"].as_str().unwrap())
        .collect();
    assert_eq!(leaves, ["one", "two"]);
}

#[test]
fn a_branching_right_hand_side_expands_to_its_tails() {
    let v = check(Case {
        dir: "erlang/honesty_case_rhs",
        file: "z.erl",
        pos: (8, 4),
        ..Default::default()
    });
    let branch = &v["root"]["children"][0];
    assert_eq!(branch["kind"], "branch");
    assert_eq!(branch["label"], "case K of");
    assert_eq!(branch["via"], "match");
    let kids = branch["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    for (k, label) in kids.iter().zip(["one", "two"]) {
        assert_eq!(k["label"], label);
        assert_eq!(k["via"], "match");
        assert_eq!(k["stop"]["reason"], "literal");
    }
}

#[test]
fn a_destructuring_parameter_narrows_the_argument() {
    let v = check(Case {
        dir: "erlang/honesty_param_destructure",
        file: "p.erl",
        pos: (5, 29),
        ..Default::default()
    });
    assert_eq!(v["root"]["kind"], "param");
    // The pattern binds B to the record's `body` field, not to the whole record.
    let arg = &v["root"]["children"][0];
    assert_eq!(arg["label"], "hello");
    assert_eq!(arg["stop"]["reason"], "literal");
}

#[test]
fn a_zero_time_budget_stops_at_once() {
    let v = run(&Case {
        dir: "erlang/local_chain",
        file: "a.erl",
        pos: (6, 4),
        limits: Limits {
            time_ms: 0,
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(v["root"]["stop"]["reason"], "limit");
    assert_eq!(v["root"]["stop"]["detail"], "time");
}

#[test]
fn ids_do_not_depend_on_the_checkout_path() {
    let case = Case {
        dir: "erlang/call_result",
        file: "d.erl",
        pos: (5, 4),
        ..Default::default()
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
    let dir = fixture("erlang/local_chain");
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

// live_*: host answers recorded from a real elp session (whence-record.json in each fixture).

#[test]
fn live_limit_recorded_from_elp() {
    let v = check(Case {
        dir: "erlang/live_limit",
        file: "src/handler.erl",
        pos: (8, 24),
        ..Default::default()
    });
    assert_eq!(v["root"]["label"], "Limit");
    let s = serde_json::to_string(&v).unwrap();
    assert!(s.contains(r#""reason":"literal""#));
    assert!(s.contains("maps:get"));
    assert!(!s.contains("recursion"));
}

#[test]
fn live_callers_recorded_from_elp() {
    let v = check(Case {
        dir: "erlang/live_callers",
        file: "src/handler.erl",
        pos: (12, 25),
        ..Default::default()
    });
    assert_eq!(v["root"]["kind"], "param");
    assert_eq!(v["root"]["children"].as_array().unwrap().len(), 3);
    let s = serde_json::to_string(&v).unwrap();
    assert!(s.contains("no call sites of handle/2"));
}

#[test]
fn rust_rebind() {
    let v = check(Case {
        dir: "rust/rebind",
        file: "src/main.rs",
        pos: (9, 9),
        ..Default::default()
    });
    assert_eq!(v["root"]["kind"], "branch");
    let kids = v["root"]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 3);
    // Newest write first, then the binding the definition points at.
    assert_eq!(kids[0]["via"], "mutation");
    assert_eq!(kids[1]["via"], "rebind");
    assert_eq!(kids[1]["children"][0]["label"], "v + 1");
    assert_eq!(kids[2]["via"], "match");
    assert_eq!(kids[2]["children"][0]["label"], "a");
}

#[test]
fn rust_escape() {
    let v = check(Case {
        dir: "rust/escape",
        file: "src/main.rs",
        pos: (29, 10),
        ..Default::default()
    });
    let kids = v["root"]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 4);
    let details: Vec<&str> = kids[..3]
        .iter()
        .map(|k| k["stop"]["detail"].as_str().unwrap())
        .collect();
    assert_eq!(
        details,
        [
            "may be written by external method len",
            "may be written by bump(…)",
            "may be written by mutate(&mut s)",
        ]
    );
    // `s.peek()` takes `&self`: reading through a method is not a write.
    assert!(!serde_json::to_string(&v).unwrap().contains("peek"));
    assert_eq!(kids[3]["via"], "match");
}

#[test]
fn rust_mut_param() {
    let v = check(Case {
        dir: "rust/mut_param",
        file: "src/main.rs",
        pos: (11, 4),
        ..Default::default()
    });
    let kids = v["root"]["children"].as_array().unwrap();
    // `fill` takes `&mut Vec<i32>`; `count` takes `&Vec<i32>` and is not a write.
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0]["stop"]["detail"], "may be written by fill(…)");
    assert_eq!(kids[1]["kind"], "param");
}

#[test]
fn rust_abstract_method() {
    let v = check(Case {
        dir: "rust/abstract",
        file: "src/main.rs",
        pos: (34, 4),
        ..Default::default()
    });
    let call = &v["root"]["children"][0];
    assert_eq!(
        (&call["kind"], &call["label"]),
        (&"call_result".into(), &"abs".into())
    );
    let kids = call["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    for k in kids {
        assert_eq!(k["stop"]["reason"], "literal");
    }
}

#[test]
fn rust_trait_default_is_a_callee_beside_its_overrides() {
    let v = check(Case {
        dir: "rust/abstract",
        file: "src/main.rs",
        pos: (34, 8),
        expected: "expected_default.json",
        ..Default::default()
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["label"], "dflt");
    // Both implementations and the trait's own body.
    assert_eq!(call["children"].as_array().unwrap().len(), 3);
}

#[test]
fn rust_multi_value() {
    let v = check(Case {
        dir: "rust/multi_value",
        file: "src/main.rs",
        pos: (8, 16),
        ..Default::default()
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["kind"], "call_result");
    // `r` is element 1 of the pattern, so element 1 of the returned tuple.
    let set = &call["children"][0];
    assert_eq!(
        (&set["via"], &set["label"]),
        (&"field_set".into(), &"b".into())
    );
    assert_eq!(call["children"].as_array().unwrap().len(), 1);
}

#[test]
fn rust_loop_var() {
    let v = check(Case {
        dir: "rust/loop_var",
        file: "src/main.rs",
        pos: (7, 13),
        ..Default::default()
    });
    assert_eq!(v["root"]["kind"], "binding");
    let it = &v["root"]["children"][0];
    assert_eq!(
        (&it["via"], &it["label"]),
        (&"element".into(), &"items".into())
    );
}

#[test]
fn rust_same_name_methods_do_not_merge() {
    let v = check(Case {
        dir: "rust/same_name",
        file: "src/main.rs",
        pos: (22, 9),
        ..Default::default()
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["label"], "get");
    let kids = call["children"].as_array().unwrap();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0]["label"], "1");
}

#[test]
fn rust_field_write() {
    let v = check(Case {
        dir: "rust/field_write",
        file: "src/main.rs",
        pos: (14, 9),
        ..Default::default()
    });
    let p = &v["root"]["children"][0]["children"][0];
    assert_eq!(p["kind"], "branch");
    let kids = p["children"].as_array().unwrap();
    // `p.y = 3` writes another field and is dropped; the construction stays.
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0]["via"], "mutation");
    assert_eq!(kids[0]["children"][0]["via"], "field_set");
    assert_eq!(kids[0]["children"][0]["label"], "9");
    assert_eq!(kids[1]["children"][0]["label"], "1");
    assert!(!serde_json::to_string(&v).unwrap().contains("\"3\""));
}

#[test]
fn go_rebind() {
    let v = check(Case {
        dir: "go/rebind",
        file: "main.go",
        pos: (11, 6),
        ..Default::default()
    });
    let kids = v["root"]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 3);
    assert_eq!(kids[0]["via"], "mutation");
    // `v++` reads the old value and names no new one.
    assert_eq!(kids[0]["children"][0]["stop"]["reason"], "literal");
    assert_eq!(kids[1]["via"], "rebind");
    assert_eq!(kids[2]["children"][0]["label"], "a");
}

#[test]
fn go_multi_value() {
    let v = check(Case {
        dir: "go/multi_value",
        file: "main.go",
        pos: (14, 10),
        ..Default::default()
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["kind"], "call_result");
    let set = &call["children"][0];
    assert_eq!(
        (&set["via"], &set["label"]),
        (&"field_set".into(), &"b".into())
    );
    assert_eq!(call["children"].as_array().unwrap().len(), 1);
}

#[test]
fn go_receiver() {
    let v = check(Case {
        dir: "go/receiver",
        file: "main.go",
        pos: (27, 6),
        ..Default::default()
    });
    let kids = v["root"]["children"].as_array().unwrap();
    assert_eq!(kids.len(), 3);
    assert_eq!(kids[0]["stop"]["detail"], "may be written by h(&s)");
    assert_eq!(kids[1]["stop"]["detail"], "may be written by Bump(…)");
    // `Get` takes a value receiver, so its writes stay inside it.
    assert!(!serde_json::to_string(&v).unwrap().contains("Get"));
    assert_eq!(kids[2]["kind"], "binding");
}

#[test]
fn go_interface() {
    let v = check(Case {
        dir: "go/interface",
        file: "main.go",
        pos: (20, 8),
        ..Default::default()
    });
    let call = &v["root"]["children"][0];
    assert_eq!(call["label"], "Abs");
    let kids = call["children"].as_array().unwrap();
    assert_eq!(kids.len(), 2);
    for k in kids {
        assert_eq!(k["stop"]["reason"], "literal");
    }
}

#[test]
fn go_zero_value() {
    let v = check(Case {
        dir: "go/zero_value",
        file: "main.go",
        pos: (8, 6),
        ..Default::default()
    });
    assert_eq!(v["root"]["stop"]["reason"], "literal");
    assert_eq!(v["root"]["stop"]["detail"], "zero value");
}

#[test]
fn go_range() {
    let v = check(Case {
        dir: "go/range",
        file: "main.go",
        pos: (9, 7),
        ..Default::default()
    });
    assert_eq!(v["root"]["kind"], "binding");
    let it = &v["root"]["children"][0];
    assert_eq!(
        (&it["via"], &it["label"]),
        (&"element".into(), &"xs".into())
    );
}

#[test]
fn a_call_through_a_variable_is_not_entered() {
    let v = check(Case {
        dir: "erlang/dynamic_call",
        file: "d.erl",
        pos: (4, 4),
        ..Default::default()
    });
    let stop = &v["root"]["children"][0];
    assert_eq!(stop["stop"]["reason"], "unresolved");
    assert_eq!(stop["stop"]["detail"], "definition of Cb is not a function");
    assert!(!serde_json::to_string(&v).unwrap().contains("recursive"));
}
