use whence::{
    lang::Registry,
    pos::Pos,
    syntax::{Doc, Role},
};

fn doc() -> (Registry, String) {
    (
        Registry::embedded().unwrap(),
        include_str!("fixtures/erlang/queries/sample.erl").to_string(),
    )
}

/// Position of the `nth` (0-based) occurrence of `needle`, offset by `skip` bytes into it.
fn at_skip(text: &str, needle: &str, nth: usize, skip: usize) -> Pos {
    let mut from = 0;
    for _ in 0..nth {
        from += text[from..].find(needle).unwrap() + needle.len();
    }
    let idx = from + text[from..].find(needle).unwrap() + skip;
    whence::pos::pos_of(text, idx)
}

fn at(text: &str, needle: &str, nth: usize) -> Pos {
    at_skip(text, needle, nth, 0)
}

#[test]
fn role_of_binding_param_and_branch() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let body = d.ident_at(at(&text, "Body = ", 0)).unwrap();
    assert!(matches!(d.role_of(body), Role::BoundBy { .. }));
    let req0 = d.ident_at(at(&text, "Req0, Opts", 0)).unwrap();
    assert!(matches!(d.role_of(req0), Role::Param { index: 0, .. }));
    let v = d.ident_at(at_skip(&text, "{ok, V}", 0, 5)).unwrap(); // the V
    assert!(matches!(d.role_of(v), Role::BranchPattern { .. }));
}

#[test]
fn returns_of_handle_goes_through_case() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let f = d
        .enclosing_function(d.ident_at(at(&text, "Body = ", 0)).unwrap())
        .unwrap();
    let rs: Vec<&str> = d.returns_of(&f).iter().map(|n| d.text_of(*n)).collect();
    assert_eq!(rs, vec!["{V, R}", "{0, R}"]);
}

#[test]
fn call_site_args_and_callee() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let limit_ident = d.ident_at(at(&text, "Limit = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(limit_ident) else {
        panic!()
    };
    let call = d.call_at(value).unwrap();
    assert_eq!(d.callee_text(&call), "maps:get");
    assert_eq!(call.args.len(), 3);
    assert_eq!(d.arg_index(&call, call.args[1]), Some(1));
}

#[test]
fn destructure_tuple_and_record() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    // pattern {ok, V} against value {ok, N * 2} → N * 2
    let v = d.ident_at(at(&text, "V} ->", 0)).unwrap();
    let Role::BranchPattern { pattern, .. } = d.role_of(v) else {
        panic!()
    };
    let pick_ret = d.ident_at(at(&text, "N * 2", 0)).unwrap(); // ident N
    // N is (var) inside (binary_op_expr) inside the (tuple) {ok, N * 2}
    let value = pick_ret.0.parent().unwrap().parent().unwrap();
    assert_eq!(d.text_of(whence::syntax::N(value)), "{ok, N * 2}");
    assert_eq!(
        d.text_of(d.destructure(pattern, v, whence::syntax::N(value)).unwrap()),
        "N * 2"
    );
    // field access
    let peer = d.ident_at(at(&text, "Peer = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(peer) else {
        panic!()
    };
    let (cont, field) = d.field_access(value).unwrap();
    assert_eq!((d.text_of(cont), field.as_str()), ("Req0", "peer"));
    // construct_field on R = #req{...}
    let r = d.ident_at(at(&text, "R = #req", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(r) else {
        panic!()
    };
    assert_eq!(d.text_of(d.construct_field(value, "peer").unwrap()), "Peer");
}

#[test]
fn literal_and_opaque() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let limit = d.ident_at(at(&text, "Limit = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(limit) else {
        panic!()
    };
    let call = d.call_at(value).unwrap();
    assert!(d.is_literal(call.args[2])); // 10
    assert!(d.is_literal(call.args[0])); // limit (atom)
    assert!(!d.is_literal(call.args[1])); // Opts
}

#[test]
fn opaque_fun_param_and_calls_containing() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    // X in `fun(X) -> X end` sits in the fun's own parameter list: opaque, not Param.
    let x = d.ident_at(at(&text, "X) -> X end", 0)).unwrap();
    let Role::Opaque(fun) = d.role_of(x) else {
        panic!("expected Opaque")
    };
    assert_eq!(d.text_of(fun), "fun(X) -> X end");
    assert!(d.is_opaque(fun));

    let calls = d.calls_containing(at(&text, "Opts, 10", 0));
    assert_eq!(calls.len(), 1);
    assert_eq!(d.callee_text(&calls[0]), "maps:get");
    assert_eq!(d.callee_name_pos(&calls[0]), at(&text, "get(limit", 0));
    assert!(d.calls_containing(at(&text, "-module", 0)).is_empty());
}

#[test]
fn enclosing_function_and_snippet() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let n = d.ident_at(at(&text, "N * 2", 0)).unwrap();
    let f = d.enclosing_function(n).unwrap();
    assert_eq!(f.name, "pick");
    assert_eq!(f.params.len(), 1);
    assert_eq!(d.text_of(f.params[0]), "N");
    assert_eq!(d.text_of(f.body), "-> {ok, N * 2}"); // clause_body spans the arrow
    assert_eq!(d.line_of(n), "pick(N) when N > 5 -> {ok, N * 2};");
    assert_eq!(d.pos_of(n), at(&text, "N * 2", 0));
    // read_body/1's parameter is a record pattern: the whole pattern is one param.
    let b = d.ident_at(at(&text, "B}) -> B", 0)).unwrap();
    let rb = d.enclosing_function(b).unwrap();
    assert!(matches!(d.role_of(b), Role::Param { index: 0, .. }));
    assert_eq!(d.text_of(rb.params[0]), "#req{body = B}");
}

#[test]
fn plain_call_next_to_a_remote_one_keeps_its_bare_name() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    // `case pick(X) of _ -> tag(other(), maps:get(k, M)) end`
    let subject = d.calls_containing(at(&text, "X) of", 0));
    assert_eq!(subject.len(), 1);
    assert_eq!(d.callee_text(&subject[0]), "pick");
    let inner = d.calls_containing(at(&text, "other()", 0));
    assert_eq!(d.callee_text(&inner[0]), "other");
    assert_eq!(d.callee_text(&inner[1]), "tag");
    let remote = d.calls_containing(at(&text, "k, M)", 0));
    assert_eq!(d.callee_text(&remote[0]), "maps:get");
}

#[test]
fn branch_without_subject_and_compound_binding_value() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let w = d.ident_at(at(&text, "W} -> case", 0)).unwrap();
    let Role::Opaque(recv) = d.role_of(w) else {
        panic!("receive clause pattern must not borrow the nested case's subject")
    };
    assert!(d.text_of(recv).starts_with("receive"));

    let c = d.ident_at(at(&text, "C} = V", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(c) else {
        panic!()
    };
    assert_eq!(d.text_of(pattern), "{A = B, C}");
    assert_eq!(d.text_of(value), "V");
}

#[test]
fn nested_field_access_reports_the_outer_field() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let f = d
        .enclosing_function(d.ident_at(at(&text, "State#state", 0)).unwrap())
        .unwrap();
    let outer = d.returns_of(&f)[0];
    let (cont, field) = d.field_access(outer).unwrap();
    assert_eq!(
        (d.text_of(cont), field.as_str()),
        ("State#state.conn", "sock")
    );
    let (inner, field) = d.field_access(cont).unwrap();
    assert_eq!((d.text_of(inner), field.as_str()), ("State", "conn"));
}

#[test]
fn cons_pattern_does_not_destructure() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let t = d.ident_at(at(&text, "T] = [1", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(t) else {
        panic!()
    };
    assert_eq!(d.text_of(pattern), "[H | T]");
    assert_eq!(d.text_of(value), "[1, 2, 3]");
    assert!(d.destructure(pattern, t, value).is_none());

    let q = d.ident_at(at(&text, "Q] = [1", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(q) else {
        panic!()
    };
    assert_eq!(d.text_of(pattern), "[P, Q]");
    assert!(d.destructure(pattern, q, value).is_none());
}

#[test]
fn returns_do_not_leak_out_of_an_anonymous_fun() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let f = d
        .enclosing_function(d.ident_at(at(&text, "X of\n        _ -> fun", 0)).unwrap())
        .unwrap();
    let rs: Vec<&str> = d.returns_of(&f).iter().map(|n| d.text_of(*n)).collect();
    assert_eq!(rs, vec!["fun() -> 1 end"]);
}

#[test]
fn empty_construct_is_literal() {
    let (reg, text) = doc();
    let lang = reg.by_name("erlang").unwrap();
    let d = Doc::parse(lang, "/s.erl".into(), text.clone());
    let l = d.ident_at(at(&text, "Limit = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(l) else {
        panic!()
    };
    let call = d.call_at(value).unwrap();
    assert!(d.is_literal(call.args[0]));
    let e = d.ident_at(at(&text, "E = {[]", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(e) else {
        panic!()
    };
    assert!(d.is_literal(value));
}
