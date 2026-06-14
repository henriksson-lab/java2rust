#!/usr/bin/env bash
# Compile-check harness: convert self-contained Java snippets to Rust and run
# `rustc --crate-type lib` on each. Reports pass/fail and the first errors.
#
# Goal metric for the "generate compiling Rust" effort.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
bin="$here/target/release/java2rust-rs"
work="$(mktemp -d)"
verbose="${VERBOSE:-0}"

cargo build -q --release --manifest-path "$here/Cargo.toml"

# name|java-source   (self-contained: no external types)
cases=(
'field_int|class A { int i; }'
'fields_sum|class A { int a; int b; int sum() { return a + b; } }'
'method_arith|class A { int add(int a, int b) { return a + b; } }'
'static_method|class A { static int twice(int x) { return x * 2; } }'
'local_for|class A { int compute() { int s = 0; for (int i = 0; i < 10; i++) { s += i; } return s; } }'
'if_else|class A { int sign(int x) { if (x > 0) { return 1; } else { return 0 - 1; } } }'
'while_loop|class A { int m() { int i = 0; while (i < 5) { i = i + 1; } return i; } }'
'float_div|class A { double half(double v) { return v / 2.0; } }'
'bool_field|class A { boolean flag; boolean get() { return flag; } }'
'nested|class A { int f(int n) { int r = 1; for (int i = 1; i <= n; i++) { r = r * i; } return r; } }'
'char_ret|class A { char first() { char c = 65; return c; } }'
'long_arith|class A { long big(long x) { return x + 1; } }'
'multi_method|class A { int a; int getA() { return a; } int dbl() { return a + a; } }'
'cast|class A { int trunc(double d) { return (int) d; } }'
'ternary|class A { int abs(int x) { int r = x > 0 ? x : 0 - x; return r; } }'
'field_mut|class A { int n; void set(int v) { n = v; } }'
'bool_ops|class A { boolean both(boolean a, boolean b) { return a && b; } }'
'mod_op|class A { int rem(int a, int b) { return a % b; } }'
'neg|class A { int neg(int x) { return -x; } }'
'shift|class A { int sh(int x) { return x << 2; } }'
'cmp_chain|class A { boolean inRange(int x) { return x >= 0 && x < 10; } }'
'do_while|class A { int m() { int i = 0; do { i += 1; } while (i < 3); return i; } }'
'call_own|class A { int twice(int x) { return x + x; } int quad(int x) { return twice(twice(x)); } }'
'embedded_inc|class A { int m() { int[] a = new int[3]; int i = 0; a[i++] = 5; return i; } }'
'println|class A { void p(int i) { System.out.println(i); } }'
'println_noarg|class A { void p() { System.out.println(); } }'
'assert_stmt|class A { void m(int x) { assert x > 0; } }'
'assert_msg|class A { void m(int x) { assert x > 0 : "neg"; } }'
'string_local|class A { String s() { String x = "hi"; return x; } }'
'pre_inc_embed|class A { int m() { int i = 0; int j = ++i; return j; } }'
'sync|class A { int x; void m() { synchronized (this) { x = 1; } } }'
'try_finally|class A { int m() { int r = 0; try { r = 1; } finally { r = 2; } return r; } }'
'null_ret|class A { String s() { return null; } }'
'throw_msg|class A { void m(int x) { if (x < 0) { throw new IllegalArgumentException("neg"); } } }'
'instanceof|class A { boolean m(String o) { return o instanceof String; } }'
'field_getter|class A { String name; String get() { return name; } }'
'field_setter|class A { String name; void set(String n) { name = n; } }'
'local_return|class A { String make() { String s = "x"; return s; } }'
'array_index|class A { int at(int[] a, int i) { return a[i]; } }'
'list_new|import java.util.List; import java.util.ArrayList; class A { List<Integer> make() { return new ArrayList<>(); } }'
'map_new|import java.util.Map; import java.util.HashMap; class A { Map<String, Integer> make() { return new HashMap<>(); } }'
'boxed|class A { Integer box(Integer x) { return x; } }'
'math_max|class A { int big(int a, int b) { return Math.max(a, b); } }'
'math_sqrt|class A { double r(double x) { return Math.sqrt(x); } }'
'math_abs|class A { int a(int x) { return Math.abs(x); } }'
'list_size|import java.util.List; class A { int n(List<Integer> xs) { return xs.size(); } }'
'list_empty|import java.util.List; class A { boolean e(List<Integer> xs) { return xs.isEmpty(); } }'
'str_equals|class A { boolean eq(String a, String b) { return a.equals(b); } }'
'list_add|import java.util.List; import java.util.ArrayList; class A { int build() { List<Integer> xs = new ArrayList<>(); xs.add(1); xs.add(2); return xs.size(); } }'
'list_get|import java.util.List; class A { int g(List<Integer> xs, int i) { return xs.get(i); } }'
'list_contains|import java.util.List; class A { boolean c(List<Integer> xs, int x) { return xs.contains(x); } }'
'map_put|import java.util.Map; import java.util.HashMap; class A { int m() { Map<String, Integer> mm = new HashMap<>(); mm.put("a", 1); return mm.size(); } }'
'str_lower|class A { String l(String s) { return s.toLowerCase(); } }'
'str_sub|class A { String sub(String s) { return s.substring(1); } }'
'str_char|class A { char at(String s, int i) { return s.charAt(i); } }'
'foreach|import java.util.List; class A { int sum(List<Integer> xs) { int s = 0; for (int x : xs) { s += x; } return s; } }'
'foreach_arr|class A { int sum(int[] xs) { int s = 0; for (int x : xs) { s += x; } return s; } }'
'str_format|class A { String f(int n, String name) { return String.format("%d items for %s", n, name); } }'
'stream_map|import java.util.List; import java.util.stream.Collectors; class A { List<Integer> dbl(List<Integer> xs) { return xs.stream().map(x -> x * 2).collect(Collectors.toList()); } }'
'stream_foreach|import java.util.List; class A { void p(List<Integer> xs) { xs.stream().forEach(x -> System.out.println(x)); } }'
'stream_count|import java.util.List; class A { int c(List<Integer> xs) { return (int) xs.stream().count(); } }'
'stream_filter|import java.util.List; import java.util.stream.Collectors; class A { List<Integer> pos(List<Integer> xs) { return xs.stream().filter(x -> x > 0).collect(Collectors.toList()); } }'
'stream_anymatch|import java.util.List; class A { boolean anyPos(List<Integer> xs) { return xs.stream().anyMatch(x -> x > 0); } }'
'stream_filtermap|import java.util.List; import java.util.stream.Collectors; class A { List<Integer> m(List<Integer> xs) { return xs.stream().filter(x -> x > 0).map(x -> x * 2).collect(Collectors.toList()); } }'
'stream_sum|import java.util.List; class A { int total(List<Integer> xs) { return xs.stream().mapToInt(x -> x).sum(); } }'
'str_split|class A { int parts(String s) { return s.split(",").size(); } }'
'str_contains|class A { boolean has(String s, String sub) { return s.contains(sub); } }'
'str_starts|class A { boolean p(String s) { return s.startsWith("a"); } }'
'str_replace|class A { String r(String s) { return s.replace("a", "b"); } }'
'int_range|import java.util.stream.IntStream; class A { void run() { IntStream.range(0, 3).forEach(i -> System.out.println(i)); } }'
'optional|import java.util.Optional; class A { Optional<Integer> find(boolean b) { if (b) { return Optional.of(1); } return Optional.empty(); } int g(boolean b) { return find(b).orElse(0); } }'
'opt_present|import java.util.Optional; class A { boolean has(Optional<String> o) { return o.isPresent(); } }'
'stream_sorted|import java.util.List; import java.util.stream.Collectors; class A { List<Integer> s(List<Integer> xs) { return xs.stream().sorted().collect(Collectors.toList()); } }'
'stream_reduce|import java.util.List; class A { int total(List<Integer> xs) { return xs.stream().reduce(0, (a, b) -> a + b); } }'
'stream_joining|import java.util.List; import java.util.stream.Collectors; class A { String j(List<String> xs) { return xs.stream().collect(Collectors.joining(", ")); } }'
)

pass=0; total=0
for entry in "${cases[@]}"; do
  name="${entry%%|*}"; src="${entry#*|}"
  total=$((total+1))
  printf '%s' "$src" > "$work/$name.java"
  "$bin" -d "$work/$name.java" -o "$work/out_$name" >/dev/null 2>&1
  rs="$work/out_$name/$name.rs"
  if [[ ! -f "$rs" ]]; then echo "FAIL $name (no output)"; continue; fi
  if err=$(rustc --edition 2021 --crate-type lib -A warnings --emit=metadata -o "$work/$name.rmeta" "$rs" 2>&1); then
    pass=$((pass+1)); echo "ok   $name"
  else
    echo "FAIL $name"
    if [[ "$verbose" == 1 ]]; then
      echo "----- $name.rs -----"; cat "$rs"
      echo "----- errors -----"; echo "$err" | grep -E "^error" | head -6; echo
    fi
  fi
done
echo "compile: $pass/$total"
echo "workdir: $work"
