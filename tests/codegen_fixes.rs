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
