use std::sync::OnceLock;

use whence::{
    lang::Registry,
    pos::Pos,
    syntax::{Doc, Role, Slot},
};

fn doc(lang: &str, path: &str, text: &str) -> Doc<'static> {
    static REG: OnceLock<Registry> = OnceLock::new();
    let reg = REG.get_or_init(|| Registry::embedded().unwrap());
    Doc::parse(reg.by_name(lang).unwrap(), path.into(), text.to_string())
}

const RUST: &str = include_str!("fixtures/rust/queries/sample.rs");

fn rust() -> (Doc<'static>, &'static str) {
    (doc("rust", "/s.rs", RUST), RUST)
}

const GO: &str = include_str!("fixtures/go/queries/sample.go");

fn go() -> (Doc<'static>, &'static str) {
    (doc("go", "/s.go", GO), GO)
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
fn destructure_crosses_pattern_and_value_kinds() {
    let (d, text) = rust();
    let x = d.ident_at(at(text, "x, y: yy", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(x) else {
        panic!("shorthand field of a struct pattern must be a binding")
    };
    assert_eq!(d.text_of(d.destructure(pattern, x, value).unwrap()), "10");

    let yy = d.ident_at(at(text, "yy }", 0)).unwrap();
    assert_eq!(d.text_of(d.destructure(pattern, yy, value).unwrap()), "20");
}

#[test]
fn an_update_construct_reports_the_value_it_starts_from() {
    let (d, text) = rust();
    let u = d.ident_at(at(text, "u = P { x: 3", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(u) else {
        panic!()
    };
    let fields = d.through(value).unwrap();
    let base = d.construct_base(fields).unwrap();
    assert_eq!(d.text_of(d.through(base).unwrap()), "base()");
}

#[test]
fn a_rest_pattern_is_not_positional() {
    let (d, text) = rust();
    let zz = d.ident_at(at(text, "zz) = (1, 2, 3)", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(zz) else {
        panic!()
    };
    assert!(d.pattern_index(pattern, zz).is_none());
    assert!(d.destructure(pattern, zz, value).is_none());
}

#[test]
fn a_tail_return_is_one_return_not_two() {
    let (d, text) = rust();
    let f = d
        .enclosing_function(d.ident_at(at_skip(text, "return a", 0, 7)).unwrap())
        .unwrap();
    let rs: Vec<&str> = d.returns_of(&f).iter().map(|n| d.text_of(*n)).collect();
    assert_eq!(rs, ["a"]);
}

#[test]
fn positional_pattern_against_a_call_yields_an_index() {
    let (d, text) = rust();
    let r = d.ident_at(at(text, "r) = split", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(r) else {
        panic!()
    };
    assert!(d.destructure(pattern, r, value).is_none());
    assert_eq!(d.pattern_index(pattern, r), Some(1));
}

#[test]
fn receiver_role_and_params_exclude_self() {
    let (d, text) = rust();
    let s = d.ident_at(at_skip(text, "&mut self", 0, 5)).unwrap();
    let Role::Receiver { func } = d.role_of(s) else {
        panic!("the receiver parameter is not a positional parameter")
    };
    assert_eq!(func.name, "bump");
    assert_eq!(func.params.len(), 1);
    assert!(d.has_mutable_receiver(&func));

    let first = d.ident_at(at(text, "d: i32", 0)).unwrap();
    assert!(matches!(d.role_of(first), Role::Param { index: 0, .. }));

    let by_ref = d.enclosing_function(d.ident_at(at(text, "self.y", 0)).unwrap());
    assert!(!d.has_mutable_receiver(&by_ref.unwrap()));
    let by_value = d.enclosing_function(d.ident_at(at(text, "self.x = 0", 0)).unwrap());
    assert!(!d.has_mutable_receiver(&by_value.unwrap()));
}

#[test]
fn a_destructuring_parameter_is_the_pattern_it_binds() {
    let (d, text) = rust();
    let b = d.ident_at(at(text, "b): (i32, i32)", 0)).unwrap();
    let Role::Param { func, index } = d.role_of(b) else {
        panic!()
    };
    assert_eq!(func.params.len(), 1);
    assert_eq!(index, 0);
    assert_eq!(d.text_of(func.params[0]), "(a, b)");
    assert_eq!(d.pattern_index(func.params[0], b), Some(1));
}

#[test]
fn go_names_sharing_one_declaration_are_separate_parameters() {
    let (d, text) = go();
    let b = d.ident_at(at(text, "b int, rest", 0)).unwrap();
    let Role::Param { func, index } = d.role_of(b) else {
        panic!("each name of `a, b int` is a parameter of its own")
    };
    assert_eq!(index, 1);
    assert_eq!(func.params.len(), 2);
    assert!(!d.param_is_mutable(&func, 1));

    let q = d.ident_at(at(text, "q *S", 0)).unwrap();
    let Role::Param { func, index } = d.role_of(q) else {
        panic!()
    };
    assert!(d.param_is_mutable(&func, index));
}

#[test]
fn assign_at_classifies_rebind_compound_and_deref() {
    let (d, text) = rust();
    let v = d.ident_at(at(text, "v = v + 1", 0)).unwrap();
    let a = d.assign_at(v).unwrap();
    assert_eq!(d.text_of(a.target), "v");
    assert!(!a.compound);
    assert_eq!(d.text_of(a.value.unwrap()), "v + 1");

    let this = d.ident_at(at(text, "self.x += d", 0)).unwrap();
    let a = d.assign_at(this).unwrap();
    assert_eq!(d.text_of(a.target), "self.x");
    assert!(a.compound);

    let b = d.ident_at(at_skip(text, "*b = w", 0, 1)).unwrap();
    let a = d.assign_at(b).unwrap();
    assert_eq!(d.text_of(a.target), "*b");
    let chain: Vec<&str> = d
        .place_chain(a.target)
        .iter()
        .map(|n| d.text_of(*n))
        .collect();
    assert_eq!(chain, ["*b", "b"]);
}

#[test]
fn go_short_var_decl_index_and_zero_value() {
    let (d, text) = go();
    let r = d.ident_at(at(text, "r := g(v)", 0)).unwrap();
    let Role::BoundBy { pattern, value } = d.role_of(r) else {
        panic!()
    };
    assert!(d.destructure(pattern, r, value).is_none());
    assert_eq!(d.pattern_index(pattern, r), Some(1));

    let z = d.ident_at(at(text, "z int", 0)).unwrap();
    assert!(matches!(d.role_of(z), Role::Declared { literal: true }));

    let w = d.ident_at(at(text, "w int = v", 0)).unwrap();
    let Role::BoundBy { value, .. } = d.role_of(w) else {
        panic!("a var declaration with a value binds")
    };
    assert_eq!(d.text_of(d.through(value).unwrap()), "v");
}

#[test]
fn go_receiver_outside_params() {
    let (d, text) = go();
    let s = d.ident_at(at(text, "s *S", 0)).unwrap();
    let Role::Receiver { func } = d.role_of(s) else {
        panic!("the receiver parameter is not a positional parameter")
    };
    assert_eq!(func.name, "Bump");
    assert_eq!(func.params.len(), 1);
    assert!(d.has_mutable_receiver(&func));

    let by_value = d.enclosing_function(d.ident_at(at(text, "s.Y", 0)).unwrap());
    assert!(!d.has_mutable_receiver(&by_value.unwrap()));
}

#[test]
fn go_call_using_finds_receiver_and_argument_slots() {
    let (d, text) = go();
    let p = d.ident_at(at(text, "p.Bump", 0)).unwrap();
    let (call, slot) = d.call_using(p).unwrap();
    assert_eq!(d.text_of(call.callee), "Bump");
    assert!(matches!(slot, Slot::Receiver));

    let v = d.ident_at(at_skip(text, "&v, p)", 0, 1)).unwrap();
    let (call, slot) = d.call_using(v).unwrap();
    assert_eq!(d.text_of(call.callee), "h");
    assert!(matches!(slot, Slot::Arg(0)));
    assert!(d.escaped(v).is_some());

    let arg = d.ident_at(at_skip(text, "&v, p)", 0, 4)).unwrap();
    assert!(matches!(d.call_using(arg), Some((_, Slot::Arg(1)))));
}

#[test]
fn function_group_separates_same_name_methods() {
    let (d, _) = rust();
    let gets: Vec<_> = d
        .functions()
        .into_iter()
        .filter(|f| f.name == "get")
        .collect();
    assert_eq!(gets.len(), 2);
    assert_ne!(d.function_group(&gets[0]), d.function_group(&gets[1]));
    assert_eq!(d.clauses_of(d.function_group(&gets[0]), "get", 0).len(), 1);
}
