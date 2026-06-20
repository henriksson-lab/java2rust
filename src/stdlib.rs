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
}

const fn r(template: &'static str) -> StdRule {
    StdRule { template, mutates: false }
}
const fn rm(template: &'static str) -> StdRule {
    StdRule { template, mutates: true }
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
        ("String", "isBlank", 0) => r("(${recv}.trim().is_empty())"),
        ("String", "strip", 0) => r("${recv}.trim().to_string()"),
        ("String", "stripLeading", 0) => r("${recv}.trim_start().to_string()"),
        ("String", "stripTrailing", 0) => r("${recv}.trim_end().to_string()"),
        ("String", "repeat", 1) => r("${recv}.repeat((${0}) as usize)"),
        ("String", "concat", 1) => r("format!(\"{}{}\", ${recv}, ${0})"),
        ("String", "matches", 1) => {
            // best-effort: literal equality, NOT regex. Slice form (not
            // `.as_str()`, which is nightly-unstable on an existing `&str`).
            r("(&(${recv})[..] == ${0:str})")
        }
        // best-effort literal replace (NOT regex), mirroring `replaceAll`. `.replacen`
        // also dodges the nightly-unstable `str::replace_first`.
        ("String", "replaceFirst", 2) => r("${recv}.replacen(${0:str}, &(${1}).to_string(), 1)"),
        // Java's `String.hashCode` (`h = 31*h + c`), foldable and faithful.
        ("String", "hashCode", 0) => {
            r("(${recv}.chars().fold(0i32, |__h, __c| __h.wrapping_mul(31).wrapping_add(__c as i32)))")
        }
        // `String.getBytes()` -> `Vec<i8>` (Java `byte` is signed).
        ("String", "getBytes", 0) => r("${recv}.bytes().map(|__b| __b as i8).collect::<Vec<i8>>()"),

        // ---- Map ----
        ("Map", "getOrDefault", 2) => r("${recv}.get(&(${0})).cloned().unwrap_or(${1})"),
        ("Map", "values", 0) => r("${recv}.values().cloned().collect::<Vec<_>>()"),
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
        ("List", "indexOf", 1) => {
            r("${recv}.iter().position(|__x| __x == &(${0})).map(|__i| __i as i32).unwrap_or(-1)")
        }

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

        // ---- Collections (value-producing / identity forms) ----
        ("Collections", "emptyList", 0) => r("Vec::new()"),
        ("Collections", "emptySet", 0) => r("std::collections::HashSet::new()"),
        ("Collections", "singletonList", 1) => r("vec![${0}]"),
        // `unmodifiable*` drop the immutability wrapper -> identity passthrough.
        ("Collections", "unmodifiableList", 1)
        | ("Collections", "unmodifiableMap", 1)
        | ("Collections", "unmodifiableSet", 1)
        | ("Collections", "unmodifiableCollection", 1) => r("(${0})"),

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

/// Does any stdlib rule named `name` mutate its receiver? The borrow analyzer's
/// [`crate::id_tracker::is_mutating_method`] is the operational source of truth
/// for `&mut` inference; this lets the coverage test assert the two never drift.
pub fn name_mutates(name: &str) -> bool {
    matches!(
        name,
        "remove" | "putIfAbsent" | "clear" | "addAll" | "set"
    )
}
