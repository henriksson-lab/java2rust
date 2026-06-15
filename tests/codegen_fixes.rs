//! Tests for codegen-quality fixes (Tier 2): overload disambiguation and
//! receiver-type-gated stdlib method rewrites.

use java2rust_rs::convert;

#[test]
fn overloaded_methods_get_distinct_names() {
    // Two `g` overloads (arity 1 and 2) must not produce duplicate `fn g`.
    let java = r#"
class C {
    int f() { return g(1) + g(1, 2); }
    int g(int a) { return a; }
    int g(int a, int b) { return a + b; }
}
"#;
    let out = convert(java);
    assert!(out.contains("fn g("), "first overload keeps base name:\n{out}");
    assert!(out.contains("fn g_2("), "second (arity 2) overload suffixed:\n{out}");
    // Self-calls resolve to the arity-matching overload.
    assert!(out.contains("self.g(1)"), "arity-1 self-call -> g:\n{out}");
    assert!(out.contains("self.g_2(1, 2)"), "arity-2 self-call -> g_2:\n{out}");
}

#[test]
fn overloaded_constructors_get_distinct_names() {
    let java = r#"
class C {
    int x;
    public C() { this.x = 0; }
    public C(int x) { this.x = x; }
}
"#;
    let out = convert(java);
    assert!(out.contains("fn new("), "first ctor keeps `new`:\n{out}");
    assert!(out.contains("fn new_1("), "second ctor (arity 1) suffixed:\n{out}");
}

#[test]
fn stdlib_rewrite_skipped_on_user_typed_receiver() {
    // `h.size()` where `h` is a user type must call the user `size`, not `.len()`.
    let java = r#"
class C {
    int f(Helper h) { return h.size(); }
}
"#;
    let out = convert(java);
    assert!(out.contains("h.size()"), "user method preserved:\n{out}");
    assert!(!out.contains(".len()"), "no collection rewrite on user type:\n{out}");
}

#[test]
fn inner_class_carries_outer_type_params() {
    // A hoisted non-static inner class that uses the outer `<T>` re-declares it
    // (with PhantomData so it's used in a field).
    let java = "class Outer<T> { T value; class Inner { T get() { return null; } } }";
    let out = convert(java);
    assert!(out.contains("struct Inner<T>"), "inner gains <T>:\n{out}");
    assert!(out.contains("PhantomData<T>"), "phantom field:\n{out}");
    assert!(out.contains("impl<T> Inner<T>"), "impl carries <T>:\n{out}");
}

#[test]
fn static_nested_class_does_not_carry_outer_params() {
    // A `static` nested class can't use the outer's type params in Java, so it
    // stays non-generic.
    let java = "class Outer<T> { static class Inner { int x() { return 0; } } }";
    let out = convert(java);
    assert!(out.contains("struct Inner {") || out.contains("struct Inner{"), "static nested stays plain:\n{out}");
    assert!(!out.contains("struct Inner<"), "no spurious generics:\n{out}");
}

#[test]
fn local_class_emitted_inline() {
    // A class declared inside a method body becomes a local item (Rust allows
    // `struct`/`impl` inside fn bodies), not dropped to an empty statement.
    let java = "class Outer { int run() { class Helper { int twice(int x) { return x*2; } } Helper h = new Helper(); return h.twice(5); } }";
    let out = convert(java);
    assert!(out.contains("struct Helper"), "local struct emitted:\n{out}");
    assert!(out.contains("impl Helper"), "local impl emitted:\n{out}");
    assert!(out.contains("fn twice"), "local method preserved:\n{out}");
    // It's inside run(), before the `let h`.
    let run = &out[out.find("fn run").unwrap()..];
    assert!(run.find("struct Helper").unwrap() < run.find("let h").unwrap(), "inline in body:\n{out}");
}

#[test]
fn anonymous_class_lowered_to_inline_struct() {
    // `new Object() { ... }` becomes an inline struct + impl, instantiated — the
    // body is preserved (not "omitted").
    let java = "class Outer { int run() { var h = new Object() { int twice(int x) { return x*2; } }; return h.twice(7); } }";
    let out = convert(java);
    assert!(out.contains("struct __Anon0"), "anon struct generated:\n{out}");
    assert!(out.contains("fn twice"), "anon body method preserved:\n{out}");
    assert!(out.contains("__Anon0::default()"), "anon instantiated:\n{out}");
    assert!(!out.contains("omitted"), "body not dropped:\n{out}");
}

#[test]
fn java_var_uses_let_inference() {
    let out = convert("class C { void m() { var x = 3; } }");
    assert!(out.contains("let x = 3") || out.contains("let mut x = 3"), "var -> inferred let:\n{out}");
    assert!(!out.contains(": var"), "no `var` type annotation:\n{out}");
}

#[test]
fn getclass_comparison_folds_to_constant() {
    // The Java `equals` idiom `getClass() != o.getClass()` is constant in Rust
    // (operands are statically typed): `==` -> true, `!=` -> false.
    let java = "class P { public boolean equals(Object o) { if (o == null || getClass() != o.getClass()) return false; return true; } }";
    let out = convert(java);
    assert!(out.contains("|| false"), "getClass() != folds to false:\n{out}");
    assert!(!out.contains("get_class"), "no get_class call:\n{out}");
}

#[test]
fn inherited_field_from_external_super_goes_through_base() {
    // A bare name unresolved locally, in a class extending an external (stub)
    // superclass, is treated as that parent's field: `self.base.<field>`.
    let out = convert("class C extends Ext { int m() { return val; } }");
    assert!(out.contains("pub base: Ext"), "external base embedded:\n{out}");
    assert!(out.contains("self.base.val"), "inherited field via base:\n{out}");
}

#[test]
fn uppercase_local_is_let_not_const() {
    // A local variable that merely starts uppercase is a `let` binding (mutable
    // if reassigned), not a `const` — only class fields become associated consts.
    let java = "class C { void m(int i) { int X = i; X >>= 7; } }";
    let out = convert(java);
    assert!(out.contains("let mut X"), "uppercase local is a mutable let:\n{out}");
    assert!(!out.contains("const X"), "not emitted as const:\n{out}");
}

#[test]
fn uppercase_static_field_is_const() {
    // A class-level (static) uppercase field stays an associated const.
    let java = "class C { static final int MAX = 9; }";
    let out = convert(java);
    assert!(out.contains("const MAX"), "static field is const:\n{out}");
}

#[test]
fn inherited_field_in_constructor_uses_self_placeholder() {
    // Inside a constructor body `self` doesn't exist yet (`__self` is used); an
    // inherited field read must also go through `__self.base`, not `self.base`.
    let out = convert("class C extends Ext { C() { val = 1; } }");
    assert!(out.contains("__self.base.val"), "ctor inherited field via __self.base:\n{out}");
    // No *bare* `self.base.val` (i.e. not the `__self` form).
    assert!(
        !out.replace("__self.base.val", "").contains("self.base.val"),
        "no bare self.base in ctor:\n{out}"
    );
}

#[test]
fn stdlib_rewrite_still_applies_to_collections() {
    // A genuine List receiver must still get `.size()` -> `.len()`.
    let java = r#"
import java.util.List;
class C {
    int f(List<String> xs) { return xs.size(); }
}
"#;
    let out = convert(java);
    assert!(out.contains(".len()"), "collection rewrite preserved:\n{out}");
}
