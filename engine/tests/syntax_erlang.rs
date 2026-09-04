use whence::syntax::{Doc, N, Role, Slot};

mod common;
use common::{at, at_skip};

fn parse(text: &str) -> Doc<'static> {
    common::parse("erlang", "/s.erl", text)
}

fn doc() -> (Doc<'static>, &'static str) {
    let text = include_str!("fixtures/erlang/queries/sample.erl");
    (parse(text), text)
}

/// The smallest node spanning the first occurrence of `needle`.
fn node_at_text<'d>(d: &'d Doc<'_>, text: &str, needle: &str) -> N<'d> {
    let start = text.find(needle).unwrap();
    let mut n = d
        .tree
        .root_node()
        .descendant_for_byte_range(start, start + needle.len())
        .unwrap();
    while let Some(p) = n.parent() {
        if (p.start_byte(), p.end_byte()) != (n.start_byte(), n.end_byte()) {
            break;
        }
        n = p;
    }
    N(n)
}

#[test]
fn role_of_binding_param_and_branch() {
    let (d, text) = doc();
    let body = d.ident_at(at(text, "Body = ", 0)).unwrap();
    assert!(matches!(d.role_of(body), Role::BoundBy { .. }));
    let req0 = d.ident_at(at(text, "Req0, Opts", 0)).unwrap();
    assert!(matches!(
        d.role_of(req0),
        Role::Param {
            slot: Slot::Arg(0),
            ..
        }
    ));
    let v = d.ident_at(at_skip(text, "{ok, V}", 0, 5)).unwrap(); // the V
    assert!(matches!(d.role_of(v), Role::BranchPattern { .. }));
}

#[test]
fn call_with_callee_at_needs_the_position_on_the_callee() {
    let (d, text) = doc();
    let call = d.call_with_callee_at(at(text, "get(limit", 0)).unwrap();
    assert_eq!(d.callee_text(&call), "maps:get");
    assert!(d.call_with_callee_at(at(text, "limit, Opts", 0)).is_none());
    let call = d.call_with_callee_at(at(text, "pick(Limit", 0)).unwrap();
    assert_eq!(d.callee_text(&call), "pick");
}

#[test]
fn returns_of_handle_goes_through_case() {
    let (d, text) = doc();
    let f = d
        .enclosing_function(d.ident_at(at(text, "Body = ", 0)).unwrap())
        .unwrap();
    let rs: Vec<&str> = d.returns_of(&f).iter().map(|n| d.text_of(*n)).collect();
    assert_eq!(rs, vec!["{V, R}", "{0, R}"]);
}

#[test]
fn call_site_args_and_callee() {
    let (d, text) = doc();
    let limit_ident = d.ident_at(at(text, "Limit = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(limit_ident) else {
        panic!()
    };
    let call = d.call_at(value).unwrap();
    assert_eq!(d.callee_text(&call), "maps:get");
    assert_eq!(call.args.len(), 3);
    assert_eq!(d.text_of(call.args[1]), "Opts");
}

#[test]
fn destructure_tuple_and_record() {
    let (d, text) = doc();
    // pattern {ok, V} against value {ok, N * 2} → N * 2
    let v = d.ident_at(at(text, "V} ->", 0)).unwrap();
    let Role::BranchPattern { pattern, .. } = d.role_of(v) else {
        panic!()
    };
    let pick_ret = d.ident_at(at(text, "N * 2", 0)).unwrap(); // ident N
    // N is (var) inside (binary_op_expr) inside the (tuple) {ok, N * 2}
    let value = pick_ret.0.parent().unwrap().parent().unwrap();
    assert_eq!(d.text_of(whence::syntax::N(value)), "{ok, N * 2}");
    assert_eq!(
        d.text_of(d.destructure(pattern, v, whence::syntax::N(value)).unwrap()),
        "N * 2"
    );
    // field access
    let peer = d.ident_at(at(text, "Peer = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(peer) else {
        panic!()
    };
    let (cont, field) = d.field_access(value).unwrap();
    assert_eq!((d.text_of(cont), field.as_str()), ("Req0", "peer"));
    // construct_field on R = #req{...}
    let r = d.ident_at(at(text, "R = #req", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(r) else {
        panic!()
    };
    assert_eq!(d.text_of(d.construct_field(value, "peer").unwrap()), "Peer");
}

#[test]
fn literal_and_opaque() {
    let (d, text) = doc();
    let limit = d.ident_at(at(text, "Limit = ", 0)).unwrap();
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
    let (d, text) = doc();
    // X in `fun(X) -> X end` sits in the fun's own parameter list: opaque, not Param.
    let x = d.ident_at(at(text, "X) -> X end", 0)).unwrap();
    let Role::Opaque(fun) = d.role_of(x) else {
        panic!("expected Opaque")
    };
    assert_eq!(d.text_of(fun), "fun(X) -> X end");
    assert!(d.is_opaque(fun));

    let calls = d.calls_containing(at(text, "Opts, 10", 0));
    assert_eq!(calls.len(), 1);
    assert_eq!(d.callee_text(&calls[0]), "maps:get");
    assert_eq!(d.pos_of(calls[0].callee), at(text, "get(limit", 0));
    assert!(d.calls_containing(at(text, "-module", 0)).is_empty());
}

#[test]
fn enclosing_function_and_snippet() {
    let (d, text) = doc();
    let n = d.ident_at(at(text, "N * 2", 0)).unwrap();
    let f = d.enclosing_function(n).unwrap();
    assert_eq!(f.name, "pick");
    assert_eq!(f.params.len(), 1);
    assert_eq!(d.text_of(f.params[0]), "N");
    assert_eq!(d.text_of(f.body.unwrap()), "-> {ok, N * 2}"); // clause_body spans the arrow
    assert_eq!(d.pos_of(n), at(text, "N * 2", 0));
    // read_body/1's parameter is a record pattern: the whole pattern is one param.
    let b = d.ident_at(at(text, "B}) -> B", 0)).unwrap();
    let rb = d.enclosing_function(b).unwrap();
    assert!(matches!(
        d.role_of(b),
        Role::Param {
            slot: Slot::Arg(0),
            ..
        }
    ));
    assert_eq!(d.text_of(rb.params[0]), "#req{body = B}");
}

#[test]
fn plain_call_next_to_a_remote_one_keeps_its_bare_name() {
    let (d, text) = doc();
    // `case pick(X) of _ -> tag(other(), maps:get(k, M)) end`
    let subject = d.calls_containing(at(text, "X) of", 0));
    assert_eq!(subject.len(), 1);
    assert_eq!(d.callee_text(&subject[0]), "pick");
    let inner = d.calls_containing(at(text, "other()", 0));
    assert_eq!(d.callee_text(&inner[0]), "other");
    assert_eq!(d.callee_text(&inner[1]), "tag");
    let remote = d.calls_containing(at(text, "k, M)", 0));
    assert_eq!(d.callee_text(&remote[0]), "maps:get");
}

#[test]
fn branch_without_subject_and_compound_binding_value() {
    let (d, text) = doc();
    let w = d.ident_at(at(text, "W} -> case", 0)).unwrap();
    let Role::Opaque(recv) = d.role_of(w) else {
        panic!("receive clause pattern must not borrow the nested case's subject")
    };
    assert!(d.text_of(recv).starts_with("receive"));

    let text2 = "g(X) -> case X of A -> try f() of B -> B catch _:_ -> 0 end end.\nf() -> 1.\n";
    let d2 = parse(text2);
    let b = d2.ident_at(at(text2, "B -> B", 0)).unwrap();
    assert!(matches!(d2.role_of(b), Role::Use));

    let c = d.ident_at(at(text, "C} = V", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(c) else {
        panic!()
    };
    assert_eq!(d.text_of(pattern), "{A = B, C}");
    assert_eq!(d.text_of(value), "V");
}

#[test]
fn nested_field_access_reports_the_outer_field() {
    let (d, text) = doc();
    let f = d
        .enclosing_function(d.ident_at(at(text, "State#state", 0)).unwrap())
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
    let (d, text) = doc();
    let t = d.ident_at(at(text, "T] = [1", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(t) else {
        panic!()
    };
    assert_eq!(d.text_of(pattern), "[H | T]");
    assert_eq!(d.text_of(value), "[1, 2, 3]");
    assert!(d.destructure(pattern, t, value).is_none());

    let q = d.ident_at(at(text, "Q] = [1", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(q) else {
        panic!()
    };
    assert_eq!(d.text_of(pattern), "[P, Q]");
    assert!(d.destructure(pattern, q, value).is_none());
}

#[test]
fn returns_do_not_leak_out_of_an_anonymous_fun() {
    let (d, text) = doc();
    let f = d
        .enclosing_function(d.ident_at(at(text, "X of\n        _ -> fun", 0)).unwrap())
        .unwrap();
    let rs: Vec<&str> = d.returns_of(&f).iter().map(|n| d.text_of(*n)).collect();
    assert_eq!(rs, vec!["fun() -> 1 end"]);
}

#[test]
fn empty_construct_is_literal() {
    let (d, text) = doc();
    let l = d.ident_at(at(text, "Limit = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(l) else {
        panic!()
    };
    let call = d.call_at(value).unwrap();
    assert!(d.is_literal(call.args[0]));
    let e = d.ident_at(at(text, "E = {[]", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(e) else {
        panic!()
    };
    assert!(d.is_literal(value));
}

#[test]
fn a_comment_inside_an_argument_list_is_not_an_argument() {
    let text = "f(X, Y, Z) -> Z.\ng() -> f(1, %% note\n 2, 3).\nh(A, %% c\n B) -> B.\n";
    let d = parse(text);
    let call = d.call_with_callee_at(at(text, "f(1", 0)).unwrap();
    let args: Vec<&str> = call.args.iter().map(|a| d.text_of(*a)).collect();
    assert_eq!(args, ["1", "2", "3"]);

    let b = d.ident_at(at(text, "B) ->", 0)).unwrap();
    let Role::Param { func, slot } = d.role_of(b) else {
        panic!()
    };
    assert_eq!(func.params.len(), 2);
    assert!(matches!(slot, Slot::Arg(1)));
}

#[test]
fn function_group_joins_clauses_and_separates_functions() {
    let (d, _) = doc();
    let fns = d.functions();
    let picks: Vec<_> = fns.iter().filter(|f| f.name == "pick").collect();
    assert_eq!(picks.len(), 2);
    assert_eq!(d.function_group(picks[0]), d.function_group(picks[1]));
    let handle = fns.iter().find(|f| f.name == "handle").unwrap();
    assert_eq!(d.clauses_of(d.function_group(picks[0]), "pick", 1).len(), 2);
    assert_eq!(d.clauses_of(d.function_group(handle), "handle", 2).len(), 1);
}

#[test]
fn positional_helpers_see_tuples_not_records() {
    let (d, text) = doc();
    let tuple = node_at_text(&d, text, "{ok, V}");
    assert_eq!(d.positional(tuple).map(|v| v.len()), Some(2));
    let v = d.ident_at(at_skip(text, "{ok, V}", 0, 5)).unwrap();
    assert_eq!(d.pattern_index(tuple, v), Some(1));

    let record = node_at_text(&d, text, "#req{body = B}");
    assert!(d.positional(record).is_none());
}

#[test]
fn every_catch_clause_tail_is_a_return() {
    let text = "g() -> X = try f() catch _:_ -> default; error:_ -> other end, X.\nf() -> 1.\n";
    let d = parse(text);
    let x = d.ident_at(at(text, "X = ", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(x) else {
        panic!()
    };
    let tails: Vec<&str> = d.tails_of(value).iter().map(|n| d.text_of(*n)).collect();
    assert_eq!(tails, ["f()", "default", "other"]);
}
