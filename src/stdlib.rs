//! Declarative JDK rewrite table.
//!
//! A single, data-driven source for the *regular* stdlib method rewrites — the
//! ones that are a fixed structural template of the receiver and arguments.
//! Each entry maps a `(category|class, java_name, arity)` triple to a template
//! string; the evaluator in `dump.rs` (`emit_template`) substitutes the
//! receiver/arguments into it. Adding coverage for a new JDK method is one line
//! here, with no new emission code.
//!
//! Template syntax — `${…}` placeholders (chosen so a literal `{…}` inside a
//! `format!` string is left untouched):
//!   - `${recv}`      the call receiver, visited verbatim
//!   - `${0}`,`${1}`  argument N, visited verbatim
//!   - `${0:str}`     argument N as a string pattern (`char` stays a char, else
//!                    coerced to `&str`) — see `emit_string_pattern`
//!   - `${0:usize}`   `(arg) as usize`
//!   - `${0:ref}`     `&(arg)`
//!   - `${0:move}`    the argument, cloned if it is a non-`Copy` move source
//!
//! Irregular rewrites (stream chains, `collect` collector inspection, string `+`
//! concatenation, the `JavaIter` shim) stay as bespoke code in `dump.rs`; only
//! the table-able ones live here.

/// One rewrite rule: a template plus whether the call mutates its receiver
/// (mirrored against [`crate::id_tracker::is_mutating_method`], which is what
/// actually drives `&mut` inference — the flag here documents the table and is
/// cross-checked by the coverage test).
pub struct StdRule {
    pub template: &'static str,
    pub mutates: bool,
    /// The Rust type this rewrite *produces*, as a simple name the type tracker
    /// understands (`"String"`, `"i32"`, `"bool"`, `"Vec"`, …), or `None` when
    /// the result type is left to context inference. Consulted by
    /// [`crate::types::TypeResolver`] so a chained call (`a.foo().bar()`) can
    /// dispatch on `foo()`'s result. A wrong `ret` is worse than `None`; keep
    /// these conservative.
    pub ret: Option<&'static str>,
}

const fn r(template: &'static str) -> StdRule {
    StdRule { template, mutates: false, ret: None }
}
const fn rm(template: &'static str) -> StdRule {
    StdRule { template, mutates: true, ret: None }
}
/// Non-mutating rule that also records its produced Rust type.
const fn rr(template: &'static str, ret: &'static str) -> StdRule {
    StdRule { template, mutates: false, ret: Some(ret) }
}

/// Instance-method rewrite for a normalized receiver *category* (`String`,
/// `Map`, `Set`, `List`, `Option`). Returns `None` when the table has no entry
/// — the caller then falls back to the bespoke handlers / default emission.
///
/// These entries cover *gaps* beyond the hand-written `try_emit_known_method`
/// arms; the bespoke handler runs first, so an overlap is harmless.
pub fn instance_rule(cat: &str, name: &str, arity: usize) -> Option<StdRule> {
    Some(match (cat, name, arity) {
        // ---- String / CharSequence ----
        ("String", "isBlank", 0) => rr("(${recv}.trim().is_empty())", "bool"),
        ("String", "strip", 0) => rr("${recv}.trim().to_string()", "String"),
        ("String", "stripLeading", 0) => rr("${recv}.trim_start().to_string()", "String"),
        ("String", "stripTrailing", 0) => rr("${recv}.trim_end().to_string()", "String"),
        ("String", "repeat", 1) => rr("${recv}.repeat((${0}) as usize)", "String"),
        ("String", "concat", 1) => rr("format!(\"{}{}\", ${recv}, ${0})", "String"),
        ("String", "matches", 1) => {
            // best-effort: literal equality, NOT regex. Slice form (not
            // `.as_str()`, which is nightly-unstable on an existing `&str`).
            rr("(&(${recv})[..] == ${0:str})", "bool")
        }
        // best-effort literal replace (NOT regex), mirroring `replaceAll`. `.replacen`
        // also dodges the nightly-unstable `str::replace_first`.
        ("String", "replaceFirst", 2) => {
            rr("${recv}.replacen(${0:str}, &(${1}).to_string(), 1)", "String")
        }
        // Java's `String.hashCode` (`h = 31*h + c`), foldable and faithful.
        ("String", "hashCode", 0) => rr(
            "(${recv}.chars().fold(0i32, |__h, __c| __h.wrapping_mul(31).wrapping_add(__c as i32)))",
            "i32",
        ),
        // `String.getBytes()` -> `Vec<i8>` (Java `byte` is signed).
        ("String", "getBytes", 0) => {
            rr("${recv}.bytes().map(|__b| __b as i8).collect::<Vec<i8>>()", "Vec")
        }
        // `toLowerCase(Locale)`/`toUpperCase(Locale)` — drop the locale arg (the
        // 0-arg forms are handled elsewhere; this covers the 1-arg overload that
        // would otherwise emit a non-existent `to_lower_case`).
        ("String", "toLowerCase", 1) => rr("${recv}.to_lowercase()", "String"),
        ("String", "toUpperCase", 1) => rr("${recv}.to_uppercase()", "String"),

        // ---- Map ----
        ("Map", "getOrDefault", 2) => r("${recv}.get(&(${0})).cloned().unwrap_or(${1})"),
        ("Map", "values", 0) => rr("${recv}.values().cloned().collect::<Vec<_>>()", "Vec"),
        ("Map", "containsValue", 1) => r("${recv}.values().any(|__v| __v == &(${0}))"),
        ("Map", "remove", 1) => rm("${recv}.remove(&(${0}))"),
        ("Map", "putIfAbsent", 2) => rm("${recv}.entry(${0}).or_insert(${1})"),
        ("Map", "putAll", 1) => rm("${recv}.extend((${0}).clone())"),
        ("Map", "clear", 0) => rm("${recv}.clear()"),

        // ---- Set ----
        ("Set", "remove", 1) => rm("${recv}.remove(&(${0}))"),
        ("Set", "addAll", 1) => rm("${recv}.extend((${0}).iter().cloned())"),
        ("Set", "retainAll", 1) => rm("${recv}.retain(|__e| (${0}).contains(__e))"),
        ("Set", "removeAll", 1) => rm("${recv}.retain(|__e| !(${0}).contains(__e))"),
        ("Set", "clear", 0) => rm("${recv}.clear()"),

        // ---- List / Collection ----
        ("List", "clear", 0) => rm("${recv}.clear()"),
        ("List", "set", 2) => rm("${recv}[(${0}) as usize] = ${1}"),
        ("List", "addAll", 1) => rm("${recv}.extend((${0}).iter().cloned())"),
        ("List", "indexOf", 1) => rr(
            "${recv}.iter().position(|__x| __x == &(${0})).map(|__i| __i as i32).unwrap_or(-1)",
            "i32",
        ),

        _ => return None,
    })
}

/// Static-method rewrite for a stdlib *class* (`Character`, `Objects`,
/// `Integer`, …). Runs after the bespoke `try_emit_boxed_static`/`try_emit_math`
/// handlers, so it only needs to cover what those don't.
pub fn static_rule(cls: &str, name: &str, arity: usize) -> Option<StdRule> {
    Some(match (cls, name, arity) {
        // ---- Character (predicates + case, ASCII semantics) ----
        ("Character", "isDigit", 1) => r("(${0}).is_ascii_digit()"),
        ("Character", "isLetter", 1) => r("(${0}).is_alphabetic()"),
        ("Character", "isLetterOrDigit", 1) => r("(${0}).is_alphanumeric()"),
        ("Character", "isAlphabetic", 1) => r("(${0}).is_alphabetic()"),
        ("Character", "isWhitespace", 1) => r("(${0}).is_whitespace()"),
        ("Character", "isSpaceChar", 1) => r("(${0}).is_whitespace()"),
        ("Character", "isUpperCase", 1) => r("(${0}).is_uppercase()"),
        ("Character", "isLowerCase", 1) => r("(${0}).is_lowercase()"),
        ("Character", "toUpperCase", 1) => r("(${0}).to_ascii_uppercase()"),
        ("Character", "toLowerCase", 1) => r("(${0}).to_ascii_lowercase()"),
        ("Character", "getNumericValue", 1) => {
            r("((${0}).to_digit(10).map(|__d| __d as i32).unwrap_or(-1))")
        }
        ("Character", "digit", 2) => {
            r("((${0}).to_digit((${1}) as u32).map(|__d| __d as i32).unwrap_or(-1))")
        }
        ("Character", "toString", 1) => r("(${0}).to_string()"),

        // ---- Objects ---- (nullability-entangled members like isNull/nonNull
        // are intentionally omitted; passthrough/identity forms only.)
        ("Objects", "toString", 1) => r("(${0}).to_string()"),
        // Identity, but produce an owned value (a `&String` arg in a returned
        // position needs to own); `:move` clones only non-Copy borrows.
        ("Objects", "requireNonNull", 1) => r("(${0:move})"),
        ("Objects", "requireNonNull", 2) => r("(${0:move})"),
        // null-safe equality, best-effort as `==` (both sides same value type).
        ("Objects", "equals", 2) => r("(${0} == ${1})"),

        // ---- Integer / Long radix + compare ----
        ("Integer" | "Long", "toHexString", 1) => r("format!(\"{:x}\", ${0})"),
        ("Integer" | "Long", "toBinaryString", 1) => r("format!(\"{:b}\", ${0})"),
        ("Integer" | "Long", "toOctalString", 1) => r("format!(\"{:o}\", ${0})"),
        // sign of the comparison (-1/0/1), portably.
        ("Integer" | "Long" | "Double" | "Float", "compare", 2) => {
            r("((${0} > ${1}) as i32 - (${0} < ${1}) as i32)")
        }
        ("Integer" | "Long" | "Double" | "Float", "max", 2) => r("(${0}).max(${1})"),
        ("Integer" | "Long" | "Double" | "Float", "min", 2) => r("(${0}).min(${1})"),
        ("Integer" | "Long" | "Double" | "Float", "sum", 2) => r("(${0} + ${1})"),

        // ---- System (non-print statics; print routes elsewhere) ----
        ("System", "exit", 1) => r("std::process::exit((${0}) as i32)"),
        ("System", "gc", 0) => r("()"),
        ("System", "currentTimeMillis", 0) => r(
            "(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|__d| __d.as_millis() as i64).unwrap_or(0))",
        ),
        ("System", "nanoTime", 0) => r(
            "(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|__d| __d.as_nanos() as i64).unwrap_or(0))",
        ),
        // Java system properties have no portable Rust analog; best-effort via the
        // environment (compiles + reasonable for the common `user.dir`/`os.name`).
        ("System", "getProperty", 1) => r("std::env::var(&(${0})).unwrap_or_default()"),
        ("System", "getProperty", 2) => r("std::env::var(&(${0})).unwrap_or(${1})"),

        // ---- Arrays (value-producing forms; mutating sort/fill deferred —
        // they need the arg passed `&mut` + Ord, which static-arg borrow doesn't
        // yet give) ----
        ("Arrays", "equals", 2) => r("(${0} == ${1})"),
        ("Arrays", "copyOfRange", 3) => {
            r("(${0})[(${1}) as usize..(${2}) as usize].to_vec()")
        }
        // `copyOf(arr, n)` — length-`n` copy (truncate / `Default`-pad).
        ("Arrays", "copyOf", 2) => {
            r("crate::java_runtime::java_array_copy_of(&(${0}), (${1}) as i64)")
        }
        // `asList(arr)` — a `Vec` view of the array (the common 1-arg form;
        // the N-scalar overload is left to the stub).
        ("Arrays", "asList", 1) => r("(${0}).to_vec()"),
        // `binarySearch(arr, key)` — JDK miss = -(insertion)-1.
        ("Arrays", "binarySearch", 2) => {
            r("crate::java_runtime::java_binary_search(&(${0}), &(${1}))")
        }

        // ---- Collections (value-producing / identity forms) ----
        ("Collections", "emptyList", 0) => r("Vec::new()"),
        ("Collections", "emptySet", 0) => r("std::collections::HashSet::new()"),
        ("Collections", "emptyMap", 0) => r("std::collections::HashMap::new()"),
        ("Collections", "singletonList", 1) => r("vec![${0}]"),
        ("Collections", "singletonMap", 2) => {
            r("{ let mut __m = std::collections::HashMap::new(); __m.insert(${0}, ${1}); __m }")
        }
        ("Collections", "singleton", 1) => {
            r("{ let mut __s = std::collections::HashSet::new(); __s.insert(${0}); __s }")
        }
        // `unmodifiable*`/`synchronized*` drop the wrapper -> identity passthrough.
        ("Collections", "unmodifiableList", 1)
        | ("Collections", "unmodifiableMap", 1)
        | ("Collections", "unmodifiableSet", 1)
        | ("Collections", "unmodifiableCollection", 1)
        | ("Collections", "synchronizedList", 1)
        | ("Collections", "synchronizedMap", 1)
        | ("Collections", "synchronizedSet", 1) => r("(${0})"),

        // ---- NumberFormat static factories (mapped runtime type; the locale
        // overload is dropped — formatting uses the C locale) ----
        ("NumberFormat", "getInstance", 0) | ("NumberFormat", "getInstance", 1) => {
            r("crate::java_runtime::JavaNumberFormat::get_instance()")
        }
        ("NumberFormat", "getNumberInstance", 0) | ("NumberFormat", "getNumberInstance", 1) => {
            r("crate::java_runtime::JavaNumberFormat::get_number_instance()")
        }
        ("NumberFormat", "getIntegerInstance", 0) | ("NumberFormat", "getIntegerInstance", 1) => {
            r("crate::java_runtime::JavaNumberFormat::get_integer_instance()")
        }
        ("NumberFormat", "getPercentInstance", 0) | ("NumberFormat", "getPercentInstance", 1) => {
            r("crate::java_runtime::JavaNumberFormat::get_percent_instance()")
        }
        ("DecimalFormatSymbols", "getInstance", 0) | ("DecimalFormatSymbols", "getInstance", 1) => {
            r("crate::java_runtime::JavaDecimalFormatSymbols::get_instance()")
        }

        // ---- String static ----
        ("String", "valueOf", 1) => r("(${0}).to_string()"),

        _ => return None,
    })
}

/// Return type of an instance method on a mapped `crate::java_runtime` type,
/// keyed on the **Java** simple type name (e.g. `"Random"` — what
/// [`crate::types::TypeResolver`] records for a `Random`-typed value, NOT the
/// `JavaRandom` runtime struct), the Java method name, and arity. Lets the
/// tracker type a chained call whose receiver is a runtime carrier
/// (`rng.nextInt() ...`, `fmt.format(x).length()`). Conservative: only entries
/// with a certain, non-nullable Rust result are listed (nullable results like
/// `BufferedReader.readLine` are omitted until the nullable overlay lands).
pub fn runtime_method_ret(java_type: &str, name: &str, arity: usize) -> Option<&'static str> {
    Some(match (java_type, name, arity) {
        ("Random", "nextInt", _) => "i32",
        ("Random", "nextLong", 0) => "i64",
        ("Random", "nextDouble", 0) => "f64",
        ("Random", "nextFloat", 0) => "f32",
        ("Random", "nextGaussian", 0) => "f64",
        ("Random", "nextBoolean", 0) => "bool",
        ("BitSet", "cardinality", 0) => "i32",
        ("BitSet", "length", 0) => "i32",
        ("BitSet", "size", 0) => "i32",
        ("BitSet", "get", 1) => "bool",
        ("BitSet", "isEmpty", 0) => "bool",
        ("CRC32", "getValue", 0) => "i64",
        ("StringWriter", "toString", 0) => "String",
        ("StringTokenizer", "nextToken", _) => "String",
        ("StringTokenizer", "countTokens", 0) => "i32",
        ("StringTokenizer", "hasMoreTokens", 0) => "bool",
        ("DecimalFormat", "format", _) => "String",
        ("NumberFormat", "format", _) => "String",
        _ => return None,
    })
}

/// Does any stdlib rule named `name` mutate its receiver? The borrow analyzer's
/// [`crate::id_tracker::is_mutating_method`] is the operational source of truth
/// for `&mut` inference; this lets the coverage test assert the two never drift.
pub fn name_mutates(name: &str) -> bool {
    matches!(
        name,
        "remove" | "putIfAbsent" | "clear" | "addAll" | "set"
    )
}
