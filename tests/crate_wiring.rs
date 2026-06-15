//! Integration test for crate/module wiring (`--crate`).
//!
//! Builds a small multi-package project on disk, then checks:
//! 1. the project self-map resolves cross-file references to `crate::…` paths;
//! 2. `finish_crate` emits a `lib.rs` + `mod.rs` tree and a `Cargo.toml`.

use std::fs;
use std::path::PathBuf;

use java2rust_rs::crate_layout::{build_project_map, finish_crate};
use java2rust_rs::stubs::StubCollector;
use java2rust_rs::symbol_map::LinkIndex;
use java2rust_rs::convert_with_links;

fn tmp(sub: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(sub);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn project_map_resolves_cross_file_refs_to_crate_paths() {
    let src = tmp("cw_src/com/ex/model");
    fs::write(
        src.join("Point.java"),
        "package com.ex.model;\npublic class Point { public int getX() { return 0; } }\n",
    )
    .unwrap();

    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_src");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = r#"
package com.ex.util;
import com.ex.model.Point;
public class Maker {
    public Point make(Point p) { return p; }
}
"#;
    let out = convert_with_links(consumer, &link);
    // Cross-file type resolves to its crate module path (input basename is the
    // first segment: `cw_src`).
    assert!(
        out.contains("crate::cw_src::com::ex::model::point::Point"),
        "cross-file ref resolved to crate path:\n{out}"
    );
}

#[test]
fn static_calls_are_crate_qualified() {
    let src = tmp("cw_static/com/ex/util");
    fs::write(
        src.join("Helper.java"),
        "package com.ex.util;\npublic class Helper { public static int twice(int x) { return x; } }\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_static");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = r#"
package com.ex;
import com.ex.util.Helper;
public class App {
    public int run() { return Helper.twice(3); }
}
"#;
    let out = convert_with_links(consumer, &link);
    assert!(
        out.contains("crate::cw_static::com::ex::util::helper::Helper::twice(3)"),
        "static call crate-qualified:\n{out}"
    );
}

#[test]
fn switch_on_enum_qualifies_case_labels() {
    // A `switch` on an enum: bare case labels are qualified to `Enum::Label`
    // unit-variant patterns (a bare label would be a binding -> E0408).
    let src = tmp("cw_enum/com/z");
    fs::write(
        src.join("Color.java"),
        "package com.z;\npublic enum Color { RED, GREEN, BLUE }\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_enum");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = "package com.z;\npublic class A { public int f(Color c) { switch (c) { case RED: case GREEN: return 1; default: return 0; } } }\n";
    let out = convert_with_links(consumer, &link);
    assert!(
        out.contains("crate::cw_enum::com::z::color::Color::RED")
            && out.contains("crate::cw_enum::com::z::color::Color::GREEN"),
        "case labels qualified as Enum::Label:\n{out}"
    );
}

#[test]
fn inherited_static_constant_resolves_to_parent_path() {
    // A bare reference to a `static final` declared in a superclass resolves to
    // the parent's associated const (`Parent::MAX`), not `self.base.…` (statics
    // aren't reached through Deref).
    let src = tmp("cw_istatic/com/z");
    fs::write(
        src.join("Base.java"),
        "package com.z;\npublic class Base { public static final int MAX = 9; }\n",
    )
    .unwrap();
    fs::write(
        src.join("Sub.java"),
        "package com.z;\npublic class Sub extends Base { public int f() { return MAX; } }\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_istatic");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let sub = "package com.z;\npublic class Sub extends Base { public int f() { return MAX; } }\n";
    let out = convert_with_links(sub, &link);
    assert!(
        out.contains("crate::cw_istatic::com::z::base::Base::MAX"),
        "inherited static const via parent path:\n{out}"
    );
}

#[test]
fn fully_qualified_static_call_resolves_to_crate_path() {
    // A static call written with a fully-qualified name and no import
    // (`com.z.Util.help()`) resolves to the type's crate path + `::`, instead of
    // leaking the package chain as a value (`com.z.Util.help`).
    let src = tmp("cw_fqn/com/z");
    fs::write(
        src.join("Util.java"),
        "package com.z;\npublic class Util { public static int help() { return 1; } }\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_fqn");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = "package com.z;\npublic class A { public int f() { return com.z.Util.help(); } }\n";
    let out = convert_with_links(consumer, &link);
    assert!(
        out.contains("crate::cw_fqn::com::z::util::Util::help()"),
        "fully-qualified static call resolved to crate path:\n{out}"
    );
}

#[test]
fn statically_imported_constant_is_qualified() {
    // A bare reference to a statically-imported constant resolves to the owning
    // type's crate path, for both explicit and wildcard static imports.
    let src = tmp("cw_static_imp/com/z");
    fs::write(
        src.join("Limits.java"),
        "package com.z;\npublic class Limits { public static final int MAX_LEN = 9; }\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_static_imp");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let explicit = "package com.z;\nimport static com.z.Limits.MAX_LEN;\npublic class A { public int f() { return MAX_LEN; } }\n";
    let out = convert_with_links(explicit, &link);
    assert!(
        out.contains("crate::cw_static_imp::com::z::limits::Limits::MAX_LEN"),
        "explicit static import qualified:\n{out}"
    );

    let wildcard = "package com.z;\nimport static com.z.Limits.*;\npublic class B { public int f() { return MAX_LEN; } }\n";
    let out = convert_with_links(wildcard, &link);
    assert!(
        out.contains("crate::cw_static_imp::com::z::limits::Limits::MAX_LEN"),
        "wildcard static import qualified (constant-shaped name):\n{out}"
    );
}

#[test]
fn interface_typed_fields_box_dyn_and_params_ref_dyn() {
    // An interface resolves to a Rust trait, which isn't a value type: owned
    // positions (fields) get `Box<dyn Trait>`; parameters get `&dyn Trait`.
    let src = tmp("cw_dyn/com/z");
    fs::write(
        src.join("Animal.java"),
        "package com.z;\npublic interface Animal { int legs(); }\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_dyn");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = r#"
package com.z;
public class Zoo {
    private Animal star;
    public int count(Animal a) { return a.legs(); }
}
"#;
    let out = convert_with_links(consumer, &link);
    assert!(
        out.contains("star: Box<dyn crate::cw_dyn::com::z::animal::Animal>"),
        "interface field is Box<dyn Trait>:\n{out}"
    );
    assert!(
        out.contains("&dyn crate::cw_dyn::com::z::animal::Animal"),
        "interface param is &dyn Trait:\n{out}"
    );
}

#[test]
fn non_trait_bounds_dropped_and_non_generic_type_args_dropped() {
    // `Klass` is a (non-generic) struct in the link map. As a type-parameter
    // bound it's invalid in Rust (`T: Klass`) -> dropped; and referenced with a
    // type argument (`Klass<X>`) the spurious arg is dropped.
    let json = r#"{ "types": { "com.x.Klass": { "rust_path": "crate::x::klass::Klass", "kind": "struct", "generic": false } } }"#;
    let mut link = LinkIndex::default();
    link.merge_json(json).unwrap();

    let consumer = "package p;\nimport com.x.Klass;\npublic class A<T extends Klass> { public Klass<String> k; }\n";
    let out = convert_with_links(consumer, &link);
    assert!(out.contains("struct A<T>"), "class bound dropped (no `T: Klass`):\n{out}");
    assert!(!out.contains("T: crate::x"), "no struct bound:\n{out}");
    assert!(
        out.contains("crate::x::klass::Klass>") || out.contains("crate::x::klass::Klass,"),
        "non-generic type args dropped (`Klass`, not `Klass<String>`):\n{out}"
    );
    assert!(!out.contains("Klass<String>"), "spurious type arg dropped:\n{out}");
}

#[test]
fn generic_interface_field_is_boxed_and_supertrait_is_bare() {
    // A generic interface as a field type keeps its args inside `Box<dyn …>`;
    // as a supertrait it's emitted bare (`trait Sub : Iter<String>`), not boxed.
    let src = tmp("cw_giface/p");
    fs::write(src.join("Iter.java"), "package p;\npublic interface Iter<T> { T next(); }\n").unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_giface");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let field = "package p;\npublic class Holder { private Iter<String> it; }\n";
    let out = convert_with_links(field, &link);
    assert!(
        out.contains("it: Box<dyn crate::cw_giface::p::iter::Iter<String>>"),
        "generic interface field boxed with args:\n{out}"
    );

    let sub = "package p;\npublic interface Sub extends Iter<String> { }\n";
    let out = convert_with_links(sub, &link);
    assert!(
        out.contains("trait Sub : crate::cw_giface::p::iter::Iter<String>"),
        "generic supertrait emitted bare:\n{out}"
    );
    assert!(!out.contains("Sub : Box<dyn"), "supertrait not boxed:\n{out}");
}

#[test]
fn dep_modules_sanitize_keyword_segments_and_dollar_names() {
    // A dependency package named `impl` (a Rust keyword) and a synthetic `$`
    // type/return are sanitized: `impl/mod.rs`, `struct My_Type`, and a sibling
    // return qualified as `crate::org::r#impl::Helper_X`.
    let json = r#"{ "types": {
        "org.impl.MyType": { "rust_path": "org::impl::My$Type", "kind": "struct",
            "methods": { "make": { "rust": "make", "rust_path": "org::impl::My$Type::make",
                "receiver": "none", "ret": "Helper$X", "ret_nullable": false, "params": [] } } },
        "org.impl.HelperX": { "rust_path": "org::impl::Helper$X", "kind": "struct" }
    } }"#;
    let mut link = LinkIndex::default();
    link.merge_json(json).unwrap();
    let out = tmp("cw_kw");
    java2rust_rs::crate_layout::generate_dep_modules(&out, &link);

    let file = fs::read_to_string(out.join("org").join("impl").join("mod.rs"))
        .expect("org/impl/mod.rs written");
    assert!(file.contains("pub struct My_Type"), "$ name sanitized:\n{file}");
    assert!(file.contains("pub struct Helper_X"), "$ type sanitized:\n{file}");
    assert!(
        file.contains("-> crate::org::r#impl::Helper_X"),
        "return path keyword-escaped + sanitized:\n{file}"
    );
}

#[test]
fn scoped_type_drops_qualifier_when_name_resolves() {
    // `Outer.Inner` whose `Inner` resolves to a full crate path emits that path
    // alone — the `Outer::` qualifier is subsumed (the nested type is hoisted).
    let json = r#"{ "types": { "com.x.Entry": { "rust_path": "crate::x::entry::Entry", "kind": "struct" } } }"#;
    let mut link = LinkIndex::default();
    link.merge_json(json).unwrap();

    let consumer = "package p;\nimport com.x.Entry;\npublic class A { public Map.Entry e; }\n";
    let out = convert_with_links(consumer, &link);
    assert!(out.contains("crate::x::entry::Entry"), "resolved path emitted:\n{out}");
    assert!(!out.contains("Map::crate"), "stale `Map::` qualifier dropped:\n{out}");
}

#[test]
fn non_static_inner_class_captures_enclosing_instance() {
    // A non-static inner class reaches the outer instance's members. It's hoisted
    // with an `__outer: Rc<RefCell<Outer>>` capture field, an invented
    // constructor taking the parent, and outer-member refs via `__outer.borrow()`.
    let src = tmp("cw_inner/p");
    fs::write(
        src.join("Outer.java"),
        "package p;\npublic class Outer {\n  int codec;\n  class Inner { int read() { return codec; } }\n  Inner make() { return new Inner(); }\n}\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_inner");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = "package p;\npublic class Outer {\n  int codec;\n  class Inner { int read() { return codec; } }\n  Inner make() { return new Inner(); }\n}\n";
    let out = convert_with_links(consumer, &link);
    assert!(
        out.contains("__outer: std::rc::Rc<std::cell::RefCell<crate::cw_inner::p::outer::Outer>>"),
        "capture field:\n{out}"
    );
    assert!(out.contains("self.__outer.borrow().codec"), "outer field via __outer.borrow():\n{out}");
    assert!(
        out.contains("pub fn new(__outer: std::rc::Rc<std::cell::RefCell<"),
        "invented ctor takes parent:\n{out}"
    );
    assert!(
        out.contains("Inner::new(std::rc::Rc::new(std::cell::RefCell::new(self.clone())))"),
        "call site threads the enclosing instance:\n{out}"
    );
}

#[test]
fn dependency_types_are_emitted_as_crate_modules() {
    // A jar-recovered dependency type (`rust_path` not crate-/std-relative)
    // becomes a unit struct under its package path, with generic-parameter
    // methods that return the concrete sibling type (so builder chains compile).
    let json = r#"{ "types": {
        "org.json.JSONObject": {
            "rust_path": "org::json::JSONObject", "kind": "struct",
            "methods": {
                "put": { "rust": "put", "rust_path": "org::json::JSONObject::put",
                         "receiver": "ref", "ret": "JSONObject", "ret_nullable": false,
                         "params": [ {"type": "&String", "by_ref": true, "mutable": false, "nullable": false},
                                     {"type": "&Object", "by_ref": true, "mutable": false, "nullable": false} ] },
                "length": { "rust": "length", "rust_path": "org::json::JSONObject::length",
                            "receiver": "ref", "ret": "i32", "ret_nullable": false, "params": [] }
            }
        }
    } }"#;
    let mut link = LinkIndex::default();
    link.merge_json(json).unwrap();

    let out = tmp("cw_deps");
    java2rust_rs::crate_layout::generate_dep_modules(&out, &link);

    let file = fs::read_to_string(out.join("org").join("json").join("mod.rs"))
        .expect("org/json/mod.rs written");
    assert!(file.contains("pub struct JSONObject;"), "unit struct:\n{file}");
    // Two params -> two generic params, any argument accepted.
    assert!(
        file.contains("pub fn put<A0, A1>(&self, a0: A0, a1: A1) -> crate::org::json::JSONObject"),
        "generic-param builder method with sibling return:\n{file}"
    );
    // Primitive return kept verbatim; no-param method has no generics.
    assert!(file.contains("pub fn length(&self) -> i32"), "primitive return kept:\n{file}");
}

#[test]
fn stub_types_map_to_crate_paths() {
    // The two-pass crate flow resolves stubbed externals to `crate::stub_*::Name`.
    let mut s = StubCollector::default();
    s.note_type("org.xerial.snappy.SnappyInputStream", "SnappyInputStream");
    s.note_type("java.io.File", "File");
    let map = s.crate_symbol_map();
    assert_eq!(
        map.types["org.xerial.snappy.SnappyInputStream"].rust_path,
        "crate::stub_org_xerial_snappy::SnappyInputStream"
    );
    assert_eq!(map.types["java.io.File"].rust_path, "crate::stub_java_io::File");
}

#[test]
fn inheritance_emits_base_and_resolves_inherited_members() {
    let src = tmp("cw_inh/com/z");
    fs::write(
        src.join("Animal.java"),
        "package com.z;\npublic class Animal { protected String name; public int legs() { return 4; } }\n",
    )
    .unwrap();
    fs::write(
        src.join("Dog.java"),
        "package com.z;\npublic class Dog extends Animal {\n  public int total() { return legs() + name.length(); }\n  public String d() { return super.toString(); }\n}\n",
    )
    .unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_inh");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let dog = fs::read_to_string(src.join("Dog.java")).unwrap();
    let out = convert_with_links(&dog, &link);
    // Base composition + Deref.
    assert!(out.contains("pub base: crate::cw_inh::com::z::animal::Animal"), "base field:\n{out}");
    assert!(out.contains("impl std::ops::Deref for Dog"), "Deref:\n{out}");
    // Inherited field via base, inherited method via Deref (self.), super via base.
    assert!(out.contains("self.base.name"), "inherited field:\n{out}");
    assert!(out.contains("self.legs()"), "inherited method call:\n{out}");
    assert!(out.contains("self.base."), "super -> self.base:\n{out}");
}

#[test]
fn interface_param_becomes_dyn_trait() {
    // A non-generic interface used as a parameter type -> `&dyn Trait` (so any
    // implementor coerces at the call site).
    let src = tmp("cw_poly/com/s");
    fs::write(src.join("Shape.java"), "package com.s;\npublic interface Shape { int area(); }\n").unwrap();
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_poly");
    let mut link = LinkIndex::default();
    link.merge(build_project_map(&root));

    let consumer = "package com.s;\npublic class Calc { public int m(Shape s) { return s.area(); } }\n";
    let out = convert_with_links(consumer, &link);
    assert!(
        out.contains("&dyn crate::cw_poly::com::s::shape::Shape"),
        "interface param is &dyn:\n{out}"
    );
}

#[test]
fn finish_crate_emits_module_tree_and_cargo() {
    let root = tmp("cw_out/pkg/sub");
    fs::write(root.join("thing.rs"), "pub struct Thing {}\n").unwrap();
    let out_root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cw_out");

    finish_crate(&out_root).unwrap();

    let lib = fs::read_to_string(out_root.join("lib.rs")).unwrap();
    assert!(lib.contains("pub mod pkg;"), "lib.rs declares top module:\n{lib}");
    let cargo = fs::read_to_string(out_root.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("[lib]") && cargo.contains("path = \"lib.rs\""), "Cargo.toml:\n{cargo}");
    let modrs = fs::read_to_string(out_root.join("pkg/mod.rs")).unwrap();
    assert!(modrs.contains("pub mod sub;"), "pkg/mod.rs declares child:\n{modrs}");
    let subrs = fs::read_to_string(out_root.join("pkg/sub/mod.rs")).unwrap();
    assert!(subrs.contains("pub mod thing;"), "sub/mod.rs declares file module:\n{subrs}");
}
