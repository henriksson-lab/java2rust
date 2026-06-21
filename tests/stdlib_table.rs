//! Coverage for the declarative JDK rewrite table (`src/stdlib.rs`).
//!
//! These assert the *shape* of the emitted Rust for representative table
//! entries; `tools/compilecheck.sh` additionally compiles a snippet per entry
//! with `rustc`. The final test cross-checks that `name_mutates` never drifts
//! from the borrow analyzer's mutation list (`is_mutating_method`) — together the
//! single source of truth for `&mut` inference on stdlib calls.

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
fn string_search_family_routes_by_category() {
    // Migrated from bespoke arms to `instance_rule("String", …)`; the arg is
    // coerced to `&str` via `${0:str}` (`&(..)[..]`), or stays a `char`.
    let out = body("class A { boolean f(String s) { return s.startsWith(\">\"); } }");
    assert!(out.contains(".starts_with(&(\">\".to_string())[..])"), "startsWith:\n{out}");
    let out = body("class A { boolean f(String s) { return s.endsWith(\"x\"); } }");
    assert!(out.contains(".ends_with(&(\"x\".to_string())[..])"), "endsWith:\n{out}");
    let out = body("class A { int f(String s) { return s.indexOf(\"y\"); } }");
    assert!(out.contains(".find(&(\"y\".to_string())[..]).map(|i| i as i32).unwrap_or(-1)"), "indexOf:\n{out}");
    let out = body("class A { int f(String s) { return s.lastIndexOf(\"z\"); } }");
    assert!(out.contains(".rfind(&(\"z\".to_string())[..]).map(|i| i as i32).unwrap_or(-1)"), "lastIndexOf:\n{out}");
    // A `char` arg stays a char pattern (not slice-coerced).
    let out = body("class A { int f(String s, char c) { return s.indexOf(c); } }");
    assert!(out.contains(".find((c))"), "indexOf(char):\n{out}");
    // split drops its limit arg; both arities collapse to the same rewrite.
    let out = body("class A { String[] f(String s) { return s.split(\",\"); } }");
    assert!(out.contains(".split(&(\",\".to_string())[..]).map(|x| x.to_string()).collect::<Vec<_>>()"), "split:\n{out}");
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
fn optional_and_stream_static_factories() {
    // Migrated from bespoke `try_emit_optional_static` / `try_emit_int_range`.
    let out = body("import java.util.Optional; class A { Optional<String> f() { return Optional.of(\"x\"); } }");
    assert!(out.contains("Some(\"x\".to_string())"), "Optional.of:\n{out}");
    let out = body("import java.util.Optional; class A { Optional<String> f() { return Optional.empty(); } }");
    assert!(out.contains("None"), "Optional.empty:\n{out}");
    let out = body("import java.util.stream.IntStream; class A { void f() { IntStream.range(0, 5); } }");
    assert!(out.contains("((0)..(5))"), "IntStream.range:\n{out}");
    let out = body("import java.util.stream.IntStream; class A { void f() { IntStream.rangeClosed(0, 5); } }");
    assert!(out.contains("((0)..=(5))"), "IntStream.rangeClosed:\n{out}");
}

#[test]
fn runtime_carrier_overloads_route_by_arity() {
    // BitSet: base overload bare, higher arity suffixed, non-overloaded methods
    // short-circuit to default emission (NOT the collection `.get` -> indexing).
    let out = body("import java.util.BitSet; class A { void f(BitSet b) { b.set(1); b.set(1,true); boolean x=b.get(0); int c=b.cardinality(); } }");
    assert!(out.contains("b.set(1)") && out.contains("b.set_2(1, true)"), "BitSet set:\n{out}");
    assert!(out.contains("b.get(0)") && out.contains("b.cardinality()"), "BitSet base/non-overloaded:\n{out}");
    // Random.nextInt(bound) -> next_int_bound; nextInt() falls through to default.
    let out = body("import java.util.Random; class A { void f(Random r) { int a=r.nextInt(); int d=r.nextInt(10); } }");
    assert!(out.contains("r.next_int()") && out.contains("r.next_int_bound(10)"), "Random nextInt:\n{out}");
    // Char-writer: println/print arity-suffixed, write emitted bare (no push_str).
    let out = body("import java.io.PrintWriter; class A { void f(PrintWriter w) { w.println(); w.println(\"x\"); w.write(\"z\"); } }");
    assert!(out.contains("w.println()") && out.contains("w.println_1("), "Writer println:\n{out}");
    assert!(out.contains("w.write(") && !out.contains("push_str"), "Writer write stays bare:\n{out}");
}

#[test]
fn stringbuilder_routes_through_string_method_rewrites() {
    // The String-method gates use `recv_is_string` (recv_category == String), which
    // also covers StringBuilder/CharSequence (all mapped to a Rust String). So
    // `StringBuilder.equals/compareTo` get the String rewrites instead of falling
    // through to a non-existent `.equals`/`.compare_to`.
    let out = body("class A { boolean f(StringBuilder sb, String s) { return sb.equals(s); } }");
    assert!(out.contains("[..] == &(") && !out.contains(".equals("), "StringBuilder.equals:\n{out}");
    let out = body("class A { int g(StringBuilder sb, StringBuilder o) { return sb.compareTo(o); } }");
    assert!(out.contains("(__a > __b) as i32") && !out.contains(".compare_to("), "StringBuilder.compareTo:\n{out}");
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
