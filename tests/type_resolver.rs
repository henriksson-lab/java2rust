//! Phase 0 tests for the unified type resolver (`src/types.rs`).
//!
//! Pure `Type` query-method tests plus a few `type_of` integration cases on
//! parsed snippets. The resolver is not yet wired into codegen.

use java2rust_rs::ast::{Arena, Node, NodeId};
use java2rust_rs::id_tracker::{self, IdTracker};
use java2rust_rs::symbol_map::LinkIndex;
use java2rust_rs::type_tracker;
use java2rust_rs::types::{Category, Prim, Type, TypeResolver};
use java2rust_rs::parse;

// ---- pure Type query methods ----

#[test]
fn numeric_rust_strings() {
    assert_eq!(Type::Prim(Prim::I32).numeric_rust(), Some("i32"));
    assert_eq!(Type::Prim(Prim::F64).numeric_rust(), Some("f64"));
    assert_eq!(Type::Prim(Prim::Char).numeric_rust(), None);
    assert_eq!(Type::Prim(Prim::Bool).numeric_rust(), None);
    assert_eq!(Type::Str.numeric_rust(), None);
}

#[test]
fn category_mapping() {
    assert_eq!(Type::Str.category(), Some(Category::String));
    assert_eq!(Type::Vec(Box::new(Type::Str)).category(), Some(Category::List));
    assert_eq!(
        Type::Map(Box::new(Type::Str), Box::new(Type::Prim(Prim::F64))).category(),
        Some(Category::Map)
    );
    assert_eq!(Type::Set(Box::new(Type::Str)).category(), Some(Category::Set));
    assert_eq!(Type::Opt(Box::new(Type::Str)).category(), Some(Category::Option));
    assert_eq!(Type::Prim(Prim::I32).category(), None);
}

#[test]
fn char_and_charvec_and_boxdyn() {
    assert!(Type::Prim(Prim::Char).is_char());
    assert!(Type::Vec(Box::new(Type::Prim(Prim::Char))).is_char_vec());
    assert!(!Type::Vec(Box::new(Type::Str)).is_char_vec());
    assert!(Type::TraitObj("Foo".into()).is_box_dyn());
}

#[test]
fn map_value_and_elem() {
    let m = Type::Map(Box::new(Type::Str), Box::new(Type::Prim(Prim::F64)));
    assert_eq!(m.map_value(), Some(&Type::Prim(Prim::F64)));
    let v = Type::Vec(Box::new(Type::Prim(Prim::I32)));
    assert_eq!(v.elem(), Some(&Type::Prim(Prim::I32)));
}

// ---- type_of integration ----

fn with<R>(java: &str, f: impl FnOnce(&Arena, &TypeResolver) -> R) -> R {
    let (arena, root) = parse::create_compilation_unit(java).expect("parse");
    let mut id = IdTracker::new();
    id_tracker::run(&arena, root, &mut id);
    type_tracker::run(&arena, root, &mut id);
    let link = LinkIndex::default();
    let r = TypeResolver::new(&arena, &id, &link, None);
    f(&arena, &r)
}

fn find(arena: &Arena, pred: impl Fn(&Node) -> bool) -> NodeId {
    (0..arena.nodes.len() as u32)
        .map(NodeId)
        .find(|&n| pred(arena.kind(n)))
        .expect("matching node")
}

#[test]
fn literals_resolve() {
    with("class A { int m() { return 5; } }", |arena, r| {
        let lit = find(arena, |n| matches!(n, Node::IntegerLiteralExpr { .. }));
        assert_eq!(r.type_of(lit), Type::Prim(Prim::I32));
    });
    with("class A { double m() { return 1.5; } }", |arena, r| {
        let lit = find(arena, |n| matches!(n, Node::DoubleLiteralExpr { .. }));
        assert_eq!(r.type_of(lit), Type::Prim(Prim::F64));
    });
    with("class A { String m() { return \"x\"; } }", |arena, r| {
        let lit = find(arena, |n| matches!(n, Node::StringLiteralExpr { .. }));
        assert_eq!(r.type_of(lit), Type::Str);
    });
}

#[test]
fn local_name_resolves_to_declared_type() {
    // `x` in `return x` resolves to the int local.
    with("class A { int m() { int x = 5; return x; } }", |arena, r| {
        let name = find(arena, |n| matches!(n, Node::NameExpr { name } if name == "x"));
        assert_eq!(r.type_of(name), Type::Prim(Prim::I32));
    });
}

#[test]
fn collection_field_resolves_structurally() {
    let src = "import java.util.Map; class A { Map<String, Integer> mm; \
               Map<String, Integer> g() { return mm; } }";
    with(src, |arena, r| {
        let name = find(arena, |n| matches!(n, Node::NameExpr { name } if name == "mm"));
        let t = r.type_of(name);
        assert_eq!(t.category(), Some(Category::Map));
        assert_eq!(t.map_value(), Some(&Type::Prim(Prim::I32)));
    });
}

#[test]
fn string_param_resolves() {
    with("class A { int m(String s) { return s.length(); } }", |arena, r| {
        let name = find(arena, |n| matches!(n, Node::NameExpr { name } if name == "s"));
        assert_eq!(r.type_of(name), Type::Str);
    });
}

// ---- Phase 1: expression reach ----

#[test]
fn binary_promotes_int_times_double() {
    with("class A { double m(int a, double b) { return a * b; } }", |arena, r| {
        let bin = find(arena, |n| matches!(n, Node::BinaryExpr { .. }));
        assert_eq!(r.type_of(bin), Type::Prim(Prim::F64));
    });
}

#[test]
fn array_index_gives_element() {
    with("class A { int m(int[] a) { return a[0]; } }", |arena, r| {
        let idx = find(arena, |n| matches!(n, Node::ArrayAccessExpr { .. }));
        assert_eq!(r.type_of(idx), Type::Prim(Prim::I32));
    });
}

#[test]
fn map_get_gives_value_type() {
    let src = "import java.util.Map; class A { int m(Map<String, Integer> mm, String k) \
               { return mm.get(k); } }";
    with(src, |arena, r| {
        let call = find(arena, |n| matches!(n, Node::MethodCallExpr { name, .. } if name == "get"));
        assert_eq!(r.type_of(call), Type::Prim(Prim::I32));
    });
}

#[test]
fn optional_get_unwraps() {
    let src = "import java.util.Optional; class A { int m(Optional<Integer> o) { return o.get(); } }";
    with(src, |arena, r| {
        let call = find(arena, |n| matches!(n, Node::MethodCallExpr { name, .. } if name == "get"));
        assert_eq!(r.type_of(call), Type::Prim(Prim::I32));
    });
}

#[test]
fn boxed_unboxing_is_numeric() {
    with("class A { double m(Integer x) { return x.doubleValue(); } }", |arena, r| {
        let call =
            find(arena, |n| matches!(n, Node::MethodCallExpr { name, .. } if name == "doubleValue"));
        assert_eq!(r.type_of(call), Type::Prim(Prim::F64));
    });
}

#[test]
fn parse_java_type_shapes() {
    use java2rust_rs::types::parse_java_type;
    // A Java field type string recovers element/value types.
    assert_eq!(
        parse_java_type("Map<AminoAcidCompound, Double>").map_value(),
        Some(&Type::Prim(Prim::F64))
    );
    assert_eq!(parse_java_type("List<Integer>").elem(), Some(&Type::Prim(Prim::I32)));
    assert!(parse_java_type("char[]").is_char_vec());
    assert_eq!(parse_java_type("double"), Type::Prim(Prim::F64));
}

#[test]
fn parse_rust_type_shapes() {
    use java2rust_rs::types::parse_rust_type;
    assert_eq!(parse_rust_type("i64"), Type::Prim(Prim::I64));
    assert!(parse_rust_type("Vec<char>").is_char_vec());
    assert_eq!(
        parse_rust_type("std::collections::HashMap<String, f64>").map_value(),
        Some(&Type::Prim(Prim::F64))
    );
    assert!(parse_rust_type("Box<dyn Foo>").is_box_dyn());
    assert_eq!(parse_rust_type("crate::a::b::Widget"), Type::Named { path: "Widget".into(), args: vec![] });
}
