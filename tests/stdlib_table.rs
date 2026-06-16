//! Coverage for the declarative JDK rewrite table (`src/stdlib.rs`).
//!
//! These assert the *shape* of the emitted Rust for representative table
//! entries; `tools/compilecheck.sh` additionally compiles a snippet per entry
//! with `rustc`. The final test cross-checks that the table's `mutates` flag
//! never drifts from the borrow analyzer's mutation list — the two together are
//! the single source of truth for `&mut` inference on stdlib calls.

use java2rust_rs::convert;

fn body(java: &str) -> String {
    convert(java)
}

#[test]
fn character_predicates_map_to_char_methods() {
    let out = body("class A { boolean d(char c) { return Character.isDigit(c); } }");
    assert!(out.contains("(c).is_ascii_digit()"), "{out}");
    let out = body("class A { boolean l(char c) { return Character.isLetter(c); } }");
    assert!(out.contains("(c).is_alphabetic()"), "{out}");
    let out = body("class A { char u(char c) { return Character.toUpperCase(c); } }");
    assert!(out.contains("(c).to_ascii_uppercase()"), "{out}");
}

#[test]
fn integer_radix_and_compare() {
    let out = body("class A { String h(int x) { return Integer.toHexString(x); } }");
    assert!(out.contains("format!(\"{:x}\", x)"), "{out}");
    let out = body("class A { int c(int a, int b) { return Integer.compare(a, b); } }");
    assert!(out.contains("(a > b) as i32 - (a < b) as i32"), "{out}");
}

#[test]
fn math_gap_functions() {
    // float-only Math fns coerce their args to f64 (Rust has no int sqrt/atan/…).
    let out = body("class A { double h(double a, double b) { return Math.hypot(a, b); } }");
    assert!(out.contains("(a as f64).hypot(b as f64)"), "{out}");
    let out = body("class A { double r(double d) { return Math.toRadians(d); } }");
    assert!(out.contains("(d as f64).to_radians()"), "{out}");
}

#[test]
fn string_gap_methods() {
    let out = body("class A { String r(String s, int n) { return s.repeat(n); } }");
    assert!(out.contains(".repeat((n) as usize)"), "{out}");
    let out = body("class A { boolean b(String s) { return s.isBlank(); } }");
    assert!(out.contains(".trim().is_empty()"), "{out}");
}

#[test]
fn collection_methods_route_by_category() {
    // List.indexOf is an element search (not the String `.find`).
    let out = body(
        "import java.util.List; class A { int i(List<Integer> xs, int x) { return xs.indexOf(x); } }",
    );
    assert!(out.contains(".iter().position("), "List.indexOf -> position:\n{out}");
    // Map.getOrDefault.
    let out = body(
        "import java.util.Map; class A { int g(Map<Integer, Integer> m, int k) { return m.getOrDefault(k, 0); } }",
    );
    assert!(out.contains(".cloned().unwrap_or(0)"), "{out}");
}

#[test]
fn user_typed_receiver_is_not_rewritten() {
    // A user class with its own `indexOf`/`clear` must keep the real call.
    let out = body(
        "class Buf { int indexOf(int x) { return x; } } class A { int f(Buf b) { return b.indexOf(3); } }",
    );
    assert!(out.contains("b.index_of(3)"), "user method preserved:\n{out}");
}

/// The table's `mutates` flag (documentation) must be a subset of the borrow
/// analyzer's mutation list (the operational `&mut` driver) — otherwise a
/// table-mutating method would not get a `&mut` receiver. This is the
/// single-source-of-truth guard the generalization plan calls for.
#[test]
fn mutating_table_entries_are_known_to_borrow_analysis() {
    for name in [
        "remove", "putIfAbsent", "clear", "addAll", "set", "computeIfAbsent", "merge", "retainAll",
    ] {
        if java2rust_rs::stdlib::name_mutates(name) {
            assert!(
                java2rust_rs::id_tracker::is_mutating_method(name),
                "`{name}` mutates per the stdlib table but the borrow analyzer \
                 does not treat it as mutating — `&mut` would not be inferred"
            );
        }
    }
}
