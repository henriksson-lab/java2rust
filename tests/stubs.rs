//! Integration tests for stub generation (`--stubs`): unresolved external
//! symbols are recorded as best-effort signatures, while stdlib types and
//! types defined elsewhere in the same tree are not.

use std::collections::HashSet;

use java2rust_rs::convert_full;
use java2rust_rs::stubs::collect_defined_types;
use java2rust_rs::symbol_map::LinkIndex;

const SRC: &str = r#"
package org.app;
import com.ext.Widget;
public class M {
    public int run(Widget w, String s) {
        int n = helper(s, 3);
        w.doThing(s);
        Widget w2 = new Widget(7);
        return n + Gadget.compute(s);
    }
}
"#;

fn stubs_of(src: &str, known: &HashSet<String>) -> String {
    let (_, collector) = convert_full(src, &LinkIndex::default(), known, true);
    collector.render()
}

#[test]
fn records_external_type_with_ctor_and_method() {
    let out = stubs_of(SRC, &HashSet::new());
    assert!(out.contains("/// @java com.ext.Widget"), "{out}");
    assert!(out.contains("pub struct Widget"), "{out}");
    // Parameters are generic (any argument accepted); the ctor returns the stub.
    assert!(out.contains("pub fn new<A0>(a0: A0) -> Widget"), "ctor inferred:\n{out}");
    assert!(out.contains("pub fn do_thing<A0>(&self, a0: A0)"), "method inferred:\n{out}");
}

#[test]
fn records_static_call_as_associated_fn() {
    let out = stubs_of(SRC, &HashSet::new());
    assert!(out.contains("pub struct Gadget"), "{out}");
    // `compute` has no receiver (static) and takes a (generic) arg.
    assert!(out.contains("pub fn compute<A0>(a0: A0)"), "{out}");
}

#[test]
fn records_free_function_with_inferred_return() {
    let out = stubs_of(SRC, &HashSet::new());
    // `int n = helper(s, 3)` -> return type i32 (primitive, kept); params generic.
    assert!(out.contains("pub fn helper<A0, A1>(a0: A0, a1: A1) -> i32"), "{out}");
}

#[test]
fn all_caps_class_names_are_stubbed_but_short_generics_arent() {
    let java = r#"
package org.app;
import java.net.URL;
public class M {
    public URL u;
    public <T> T pick(T a) { return a; }
}
"#;
    let out = stubs_of(java, &HashSet::new());
    assert!(out.contains("struct URL"), "all-caps class URL is stubbed:\n{out}");
    assert!(!out.contains("struct T "), "short generic T is not stubbed:\n{out}");
}

#[test]
fn does_not_stub_stdlib_types() {
    let out = stubs_of(SRC, &HashSet::new());
    assert!(!out.contains("struct String"), "String must not be stubbed:\n{out}");
    assert!(!out.contains("@java org.app.String"), "{out}");
}

#[test]
fn generated_stub_file_is_valid_rust_shape() {
    let out = stubs_of(SRC, &HashSet::new());
    assert!(out.contains("pub struct Unknown;"), "{out}");
    assert!(out.contains("unimplemented!()"), "{out}");
}

#[test]
fn grouped_by_package_into_separate_files() {
    let (_, collector) = convert_full(SRC, &LinkIndex::default(), &HashSet::new(), true);
    let files = collector.render_grouped();
    // com.ext.Widget -> its own file; helper (free fn, no package) -> stubs.rs.
    let ext = files.get("stub_com_ext.rs").expect("per-package file for com.ext");
    assert!(ext.contains("pub struct Widget"), "Widget in its package file:\n{ext}");
    assert!(!ext.contains("pub fn helper"), "free fn not in package file");
    let base = files.get("stubs.rs").expect("base file for free fns");
    assert!(base.contains("pub fn helper"), "free fn in stubs.rs:\n{base}");
    // Each file is self-contained.
    assert!(ext.contains("pub struct Unknown;"), "self-contained header:\n{ext}");
}

#[test]
fn no_stubs_when_disabled() {
    let (_, collector) = convert_full(SRC, &LinkIndex::default(), &HashSet::new(), false);
    assert!(collector.is_empty(), "no collection when emit_stubs is false");
}

#[test]
fn known_tree_types_are_not_stubbed() {
    // Simulate B.java referencing A (defined in the same tree) plus an external.
    let a = "package p; public class A { public int get() { return 0; } }";
    let b = "package p; public class B { public int u(A a) { return a.get() + Ext.help(a); } }";
    let mut known = HashSet::new();
    collect_defined_types(a, &mut known);
    collect_defined_types(b, &mut known);
    assert!(known.contains("p.A"), "pre-pass found A: {known:?}");

    let out = stubs_of(b, &known);
    assert!(!out.contains("struct A "), "A is in-tree, must not be stubbed:\n{out}");
    assert!(out.contains("struct Ext"), "Ext is external, must be stubbed:\n{out}");
}
