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

/// One rewrite rule: a template, plus the Rust type it produces. `&mut`
/// inference is driven operationally by [`crate::id_tracker::is_mutating_method`]
/// (cross-checked against [`name_mutates`] by the coverage test); the table marks
/// mutating entries only via the [`rm`] constructor, for source readability.
pub struct StdRule {
    pub template: &'static str,
    /// The Rust type this rewrite *produces*, as a simple name the type tracker
    /// understands (`"String"`, `"i32"`, `"bool"`, `"Vec"`, …), or `None` when
    /// the result type is left to context inference. Consulted by
    /// [`crate::types::TypeResolver`] so a chained call (`a.foo().bar()`) can
    /// dispatch on `foo()`'s result. A wrong `ret` is worse than `None`; keep
    /// these conservative.
    pub ret: Option<&'static str>,
}

const fn r(template: &'static str) -> StdRule {
    StdRule { template, ret: None }
}
/// A rule whose call mutates its receiver. Structurally identical to [`r`] — the
/// distinct name documents, at the table's call site, which entries mutate (the
/// operational `&mut` signal is [`crate::id_tracker::is_mutating_method`]).
const fn rm(template: &'static str) -> StdRule {
    StdRule { template, ret: None }
}
/// Non-mutating rule that also records its produced Rust type.
const fn rr(template: &'static str, ret: &'static str) -> StdRule {
    StdRule { template, ret: Some(ret) }
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
        // String search family (migrated from the bespoke `try_emit_known_method`
        // arms). `${0:str}` is exactly the old `emit_string_pattern` coercion
        // (char stays a char, else `&(..)[..]`). The category gate (`recv_category
        // == "String"`) is what previously disambiguated `indexOf`/`lastIndexOf`
        // from the `("List", …)` element-search complement. `split`'s limit arg is
        // dropped (Rust `.split` already keeps trailing empties).
        ("String", "startsWith", 1) => rr("${recv}.starts_with(${0:str})", "bool"),
        ("String", "endsWith", 1) => rr("${recv}.ends_with(${0:str})", "bool"),
        ("String", "indexOf", 1) => {
            rr("${recv}.find(${0:str}).map(|i| i as i32).unwrap_or(-1)", "i32")
        }
        ("String", "lastIndexOf", 1) => {
            rr("${recv}.rfind(${0:str}).map(|i| i as i32).unwrap_or(-1)", "i32")
        }
        ("String", "split", 1) | ("String", "split", 2) => {
            rr("${recv}.split(${0:str}).map(|x| x.to_string()).collect::<Vec<_>>()", "Vec")
        }
        // NO-GO (measured 2026-06-21): migrating the String *value* ops
        // (`trim`/`charAt`/`substring`/`toCharArray`/`equalsIgnoreCase`) here
        // REGRESSES (+7: bjaaprop +3, vcf +1, jsoup +3). Unlike the search family
        // above (whose default emission `.starts_with` etc. are real `str`
        // methods), these default-emit to NON-existent methods (`.substring`,
        // `.char_at`, `.equals_ignore_case`) on the Unknown-category receivers the
        // bespoke arms (no category gate) used to catch — e.g.
        // `x.toString().substring(1)`. They stay bespoke in `try_emit_known_method`
        // until the receiver of such chains types as `String` (resolver work).

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
        ("Character", "isDigit", 1) => rr("(${0}).is_ascii_digit()", "bool"),
        ("Character", "isLetter", 1) => rr("(${0}).is_alphabetic()", "bool"),
        ("Character", "isLetterOrDigit", 1) => rr("(${0}).is_alphanumeric()", "bool"),
        ("Character", "isAlphabetic", 1) => rr("(${0}).is_alphabetic()", "bool"),
        ("Character", "isWhitespace", 1) => rr("(${0}).is_whitespace()", "bool"),
        ("Character", "isSpaceChar", 1) => rr("(${0}).is_whitespace()", "bool"),
        ("Character", "isUpperCase", 1) => rr("(${0}).is_uppercase()", "bool"),
        ("Character", "isLowerCase", 1) => rr("(${0}).is_lowercase()", "bool"),
        ("Character", "toUpperCase", 1) => rr("(${0}).to_ascii_uppercase()", "char"),
        ("Character", "toLowerCase", 1) => rr("(${0}).to_ascii_lowercase()", "char"),
        ("Character", "getNumericValue", 1) => {
            rr("((${0}).to_digit(10).map(|__d| __d as i32).unwrap_or(-1))", "i32")
        }
        ("Character", "digit", 2) => {
            rr("((${0}).to_digit((${1}) as u32).map(|__d| __d as i32).unwrap_or(-1))", "i32")
        }
        ("Character", "toString", 1) => rr("(${0}).to_string()", "String"),

        // ---- Objects ---- (nullability-entangled members like isNull/nonNull
        // are intentionally omitted; passthrough/identity forms only.)
        ("Objects", "toString", 1) => rr("(${0}).to_string()", "String"),
        // Identity, but produce an owned value (a `&String` arg in a returned
        // position needs to own); `:move` clones only non-Copy borrows.
        ("Objects", "requireNonNull", 1) => r("(${0:move})"),
        ("Objects", "requireNonNull", 2) => r("(${0:move})"),
        // null-safe equality, best-effort as `==` (both sides same value type).
        ("Objects", "equals", 2) => rr("(${0} == ${1})", "bool"),

        // ---- Integer / Long radix + compare ----
        ("Integer" | "Long", "toHexString", 1) => rr("format!(\"{:x}\", ${0})", "String"),
        ("Integer" | "Long", "toBinaryString", 1) => rr("format!(\"{:b}\", ${0})", "String"),
        ("Integer" | "Long", "toOctalString", 1) => rr("format!(\"{:o}\", ${0})", "String"),
        // sign of the comparison (-1/0/1), portably. `compare` always returns
        // `int` regardless of operand type, so `i32` is certain (unlike
        // `sum`/`max`/`min`, which are operand-typed -> left `None`).
        ("Integer" | "Long" | "Double" | "Float", "compare", 2) => {
            rr("((${0} > ${1}) as i32 - (${0} < ${1}) as i32)", "i32")
        }
        ("Integer" | "Long" | "Double" | "Float", "max", 2) => r("(${0}).max(${1})"),
        ("Integer" | "Long" | "Double" | "Float", "min", 2) => r("(${0}).min(${1})"),
        ("Integer" | "Long" | "Double" | "Float", "sum", 2) => r("(${0} + ${1})"),

        // ---- System (non-print statics; print routes elsewhere) ----
        ("System", "exit", 1) => r("std::process::exit((${0}) as i32)"),
        ("System", "gc", 0) => r("()"),
        ("System", "currentTimeMillis", 0) => rr(
            "(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|__d| __d.as_millis() as i64).unwrap_or(0))",
            "i64",
        ),
        ("System", "nanoTime", 0) => rr(
            "(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|__d| __d.as_nanos() as i64).unwrap_or(0))",
            "i64",
        ),
        // Java system properties have no portable Rust analog; best-effort via the
        // environment (compiles + reasonable for the common `user.dir`/`os.name`).
        ("System", "getProperty", 1) => rr("std::env::var(&(${0})).unwrap_or_default()", "String"),
        ("System", "getProperty", 2) => rr("std::env::var(&(${0})).unwrap_or(${1})", "String"),

        // ---- Arrays (value-producing forms; mutating sort/fill deferred —
        // they need the arg passed `&mut` + Ord, which static-arg borrow doesn't
        // yet give) ----
        ("Arrays", "equals", 2) => rr("(${0} == ${1})", "bool"),
        ("Arrays", "copyOfRange", 3) => {
            rr("(${0})[(${1}) as usize..(${2}) as usize].to_vec()", "Vec")
        }
        // `copyOf(arr, n)` — length-`n` copy (truncate / `Default`-pad).
        ("Arrays", "copyOf", 2) => {
            rr("crate::java_runtime::java_array_copy_of(&(${0}), (${1}) as i64)", "Vec")
        }
        // `asList(arr)` — a `Vec` view of the array (the common 1-arg form;
        // the N-scalar overload is left to the stub).
        ("Arrays", "asList", 1) => rr("(${0}).to_vec()", "Vec"),
        // `binarySearch(arr, key)` — JDK miss = -(insertion)-1.
        ("Arrays", "binarySearch", 2) => {
            rr("crate::java_runtime::java_binary_search(&(${0}), &(${1}))", "i32")
        }

        // ---- Collections (value-producing / identity forms) ----
        ("Collections", "emptyList", 0) => rr("Vec::new()", "Vec"),
        ("Collections", "emptySet", 0) => rr("std::collections::HashSet::new()", "HashSet"),
        ("Collections", "emptyMap", 0) => rr("std::collections::HashMap::new()", "HashMap"),
        ("Collections", "singletonList", 1) => rr("vec![${0}]", "Vec"),
        ("Collections", "singletonMap", 2) => {
            rr("{ let mut __m = std::collections::HashMap::new(); __m.insert(${0}, ${1}); __m }", "HashMap")
        }
        ("Collections", "singleton", 1) => {
            rr("{ let mut __s = std::collections::HashSet::new(); __s.insert(${0}); __s }", "HashSet")
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
        // `ret` is the *Java* type name (`"NumberFormat"`) so a chained
        // `.format(x)` resolves via `runtime_method_ret("NumberFormat",…)`.
        ("NumberFormat", "getInstance", 0) | ("NumberFormat", "getInstance", 1) => {
            rr("crate::java_runtime::JavaNumberFormat::get_instance()", "NumberFormat")
        }
        ("NumberFormat", "getNumberInstance", 0) | ("NumberFormat", "getNumberInstance", 1) => {
            rr("crate::java_runtime::JavaNumberFormat::get_number_instance()", "NumberFormat")
        }
        ("NumberFormat", "getIntegerInstance", 0) | ("NumberFormat", "getIntegerInstance", 1) => {
            rr("crate::java_runtime::JavaNumberFormat::get_integer_instance()", "NumberFormat")
        }
        ("NumberFormat", "getPercentInstance", 0) | ("NumberFormat", "getPercentInstance", 1) => {
            rr("crate::java_runtime::JavaNumberFormat::get_percent_instance()", "NumberFormat")
        }
        ("DecimalFormatSymbols", "getInstance", 0) | ("DecimalFormatSymbols", "getInstance", 1) => {
            rr("crate::java_runtime::JavaDecimalFormatSymbols::get_instance()", "DecimalFormatSymbols")
        }

        // ---- Optional / stream static factories (migrated from the bespoke
        // `try_emit_optional_static` / `try_emit_int_range`). `ret` left `None`:
        // the results (`Some(x)`/`None`/a `Range`) aren't simple named types, and
        // the bespoke handlers typed them the same (via context). ----
        ("Optional", "of", 1) | ("Optional", "ofNullable", 1) => r("Some(${0})"),
        ("Optional", "empty", 0) => r("None"),
        ("IntStream" | "LongStream", "range", 2) => r("((${0})..(${1}))"),
        ("IntStream" | "LongStream", "rangeClosed", 2) => r("((${0})..=(${1}))"),

        // ---- java.util.regex.Pattern static factories (regex crate). `ret` is the
        // Java type name so a chained `.matcher(x)`/`.split(x)` resolves. ----
        ("Pattern", "compile", 1) => rr("crate::java_runtime::JavaPattern::compile(${0})", "Pattern"),
        ("Pattern", "compile", 2) => {
            rr("crate::java_runtime::JavaPattern::compile_2(${0}, ${1})", "Pattern")
        }
        ("Pattern", "quote", 1) => rr("crate::java_runtime::JavaPattern::quote(${0})", "String"),
        ("Pattern", "matches", 2) => {
            rr("crate::java_runtime::JavaPattern::matches_static(${0}, ${1})", "bool")
        }

        // ---- String static ----
        ("String", "valueOf", 1) => rr("(${0}).to_string()", "String"),

        _ => return None,
    })
}

/// Route a `new <IoType>(..)` constructor to a family **factory free fn** in the
/// runtime (`src/runtime/io_read.rs` / `io_write.rs`), keyed on the Java simple
/// type name + ctor arity. The read/write families collapse to shared carriers
/// (`JavaInputStream`/`JavaReader`/`JavaOutputStream`/`JavaWriter`), so per-type
/// ctors collide by arity (`FileInputStream(path)` vs `BufferedInputStream(stream)`)
/// — the factory fns disambiguate by name and each return the carrier (their
/// generic `impl Read/Write` params let nested concrete carriers compose). Pure
/// data, consulted by the dumper's `visit_object_creation`. The `PrintWriter`/
/// `PrintStream` ctors are NOT here — they disambiguate by *argument type*, so
/// they stay bespoke in the dumper.
pub fn io_ctor_factory(simple: &str, arity: usize) -> Option<&'static str> {
    Some(match (simple, arity) {
        // read: byte streams -> JavaInputStream
        ("FileInputStream", 1) => "crate::java_runtime::java_file_input_stream",
        ("ByteArrayInputStream", 1) => "crate::java_runtime::java_byte_array_input_stream",
        ("ByteArrayInputStream", 3) => "crate::java_runtime::java_byte_array_input_stream_3",
        ("BufferedInputStream", 1) => "crate::java_runtime::java_buffered_input_stream",
        ("BufferedInputStream", 2) => "crate::java_runtime::java_buffered_input_stream_2",
        ("InputStream", 1) | ("DataInputStream", 1) => "crate::java_runtime::java_input_stream",
        // read: java.util.zip decompressing streams -> JavaInputStream
        // (flate2-backed; compose with BufferedInputStream/InputStreamReader).
        ("GZIPInputStream", 1) => "crate::java_runtime::java_gzip_input_stream",
        ("GZIPInputStream", 2) => "crate::java_runtime::java_gzip_input_stream_2",
        ("InflaterInputStream", 1) => "crate::java_runtime::java_inflater_input_stream",
        ("InflaterInputStream", 2) => "crate::java_runtime::java_inflater_input_stream_2",
        ("InflaterInputStream", 3) => "crate::java_runtime::java_inflater_input_stream_3",
        // read: char readers -> JavaReader
        ("BufferedReader", 1) => "crate::java_runtime::java_buffered_reader",
        ("BufferedReader", 2) => "crate::java_runtime::java_buffered_reader_2",
        ("InputStreamReader", 1) => "crate::java_runtime::java_input_stream_reader",
        ("InputStreamReader", 2) => "crate::java_runtime::java_input_stream_reader_2",
        ("FileReader", 1) => "crate::java_runtime::java_file_reader",
        ("StringReader", 1) => "crate::java_runtime::java_string_reader",
        ("LineNumberReader", 1) => "crate::java_runtime::java_line_number_reader",
        ("Reader", 1) => "crate::java_runtime::java_reader",
        // write: byte streams
        ("OutputStream", 1) => "crate::java_runtime::java_output_stream",
        ("FileOutputStream", 1) => "crate::java_runtime::java_file_output_stream",
        ("FileOutputStream", 2) => "crate::java_runtime::java_file_output_stream_append",
        ("BufferedOutputStream", 1) | ("DataOutputStream", 1) | ("FilterOutputStream", 1) => {
            "crate::java_runtime::java_buffered_output_stream"
        }
        ("BufferedOutputStream", 2) => "crate::java_runtime::java_buffered_output_stream_sized",
        ("ByteArrayOutputStream", 0) => "crate::java_runtime::JavaByteArrayOutputStream::new",
        ("ByteArrayOutputStream", 1) => "crate::java_runtime::JavaByteArrayOutputStream::new_1",
        // write: java.util.zip compressing streams -> JavaOutputStream.
        ("GZIPOutputStream", 1) => "crate::java_runtime::java_gzip_output_stream",
        ("GZIPOutputStream", 2) => "crate::java_runtime::java_gzip_output_stream_2",
        ("DeflaterOutputStream", 1) => "crate::java_runtime::java_deflater_output_stream",
        ("DeflaterOutputStream", 2) => "crate::java_runtime::java_deflater_output_stream_2",
        // write: char writers
        ("Writer", 1) => "crate::java_runtime::java_writer",
        ("OutputStreamWriter", 1) => "crate::java_runtime::java_output_stream_writer",
        ("OutputStreamWriter", 2) => "crate::java_runtime::java_output_stream_writer_charset",
        ("BufferedWriter", 1) => "crate::java_runtime::java_buffered_writer",
        ("BufferedWriter", 2) => "crate::java_runtime::java_buffered_writer_sized",
        ("FileWriter", 1) => "crate::java_runtime::java_file_writer",
        ("FileWriter", 2) => "crate::java_runtime::java_file_writer_append",
        ("StringWriter", 0) => "crate::java_runtime::JavaStringWriter::new",
        ("StringWriter", 1) => "crate::java_runtime::JavaStringWriter::new_1",
        _ => return None,
    })
}

/// The emission verdict for an arity/arg-type-overloaded method on a mapped
/// runtime carrier (see [`runtime_method_overload`]).
#[derive(Clone, Copy)]
pub enum Overload {
    /// Emit the bare snake-cased name (no suffix): the arity-0 base overload, or a
    /// carrier method emitted bare so the generic String/collection rewrites can't
    /// fire on it (the char-writer `write`/`append`/`flush`/…).
    Bare,
    /// Emit `snake_name_<arity>` — the higher-arity overload.
    Suffix,
    /// Emit a fixed Rust name (`Random.nextInt(bound)` -> `next_int_bound`).
    Rename(&'static str),
    /// `CRC32.update(x)` (arity 1) disambiguates by ARG type, not arity: a
    /// byte-array arg -> `update_1`, an `int` arg -> the base `update`. The dumper
    /// resolves the arg type and falls back to the base (default emit) for `int`.
    ByArgVec,
}

/// Arity/arg-type overload resolution for the mapped runtime carriers (`BitSet`,
/// the `java.util.zip` `CRC32`/`Deflater`/`Inflater`, `Random`, and the
/// char-writer family). Java overloads these methods but Rust needs distinct
/// names; this table holds the per-`(java_type, name, arity)` verdict, consumed by
/// the dumper's `try_emit_known_method` (which turns it into the emitted name and
/// applies the carrier's short-circuit/fall-through default for `None`). Keyed on
/// the Java simple type name (what `recv_type_name` reports).
pub fn runtime_method_overload(java_type: &str, name: &str, arity: usize) -> Option<Overload> {
    use Overload::*;
    Some(match (java_type, name, arity) {
        // `BitSet`: get/set/clear/flip overloaded by arity (base keeps the bare
        // name, handled by the dumper's short-circuit default).
        ("BitSet", "set", 2)
        | ("BitSet", "set", 3)
        | ("BitSet", "clear", 2)
        | ("BitSet", "flip", 2)
        | ("BitSet", "get", 2) => Suffix,
        // `java.util.zip` CRC32/Deflater/Inflater: higher-arity overloads.
        ("CRC32" | "Deflater" | "Inflater", "update", 3)
        | ("CRC32" | "Deflater" | "Inflater", "setInput", 3)
        | ("CRC32" | "Deflater" | "Inflater", "setDictionary", 3)
        | ("CRC32" | "Deflater" | "Inflater", "deflate", 3)
        | ("CRC32" | "Deflater" | "Inflater", "deflate", 4) => Suffix,
        ("CRC32" | "Deflater" | "Inflater", "update", 1) => ByArgVec,
        // `Random.nextInt(bound)`.
        ("Random", "nextInt", 1) => Rename("next_int_bound"),
        // java.util.regex: `Matcher.group/start/end(n)` and `Pattern.split(_,limit)`
        // are arity-overloaded; the 1/2-arg forms get the `_N` suffix (the 0/1-arg
        // base keeps the bare name via default snake-case emission / short-circuit).
        ("Matcher", "group", 1) | ("Matcher", "start", 1) | ("Matcher", "end", 1) => Suffix,
        ("Pattern", "split", 2) => Suffix,
        // Char-writer carriers: println/print + write/append overloaded by arity;
        // the rest emitted bare so the generic String rewrites don't fire.
        (
            "Writer" | "OutputStreamWriter" | "BufferedWriter" | "FileWriter" | "PrintWriter"
            | "PrintStream" | "StringWriter",
            n,
            a,
        ) => match (n, a) {
            ("println", 0) | ("print", 0) => Bare,
            ("println", 1) | ("print", 1) | ("write", 3) | ("append", 3) => Suffix,
            ("write", 1) | ("append", 1) | ("flush", 0) | ("close", 0) | ("checkError", 0)
            | ("printf", _) | ("format", _) => Bare,
            _ => return None,
        },
        _ => return None,
    })
}

/// Return type of an instance method on a mapped `crate::java_runtime` type,
/// keyed on the **Java** simple type name (e.g. `"Random"` — what
/// [`crate::types::TypeResolver`] records for a `Random`-typed value, NOT the
/// `JavaRandom` runtime struct), the Java method name, and arity. Lets the
/// tracker type a chained call whose receiver is a runtime carrier
/// (`rng.nextInt() ...`, `fmt.format(x).length()`). A **nullable** runtime return
/// is expressed directly as an `"Option<T>"` ret string — the resolver's
/// `parse_rust_type` lowers it to `Type::Opt`, so the §5 overlay sees the truth
/// (e.g. `BufferedReader.readLine` → `Option<String>`; `Matcher.group` likewise
/// when the regex runtime is wired). No flag needed: the type string *is* the
/// overlay carrier. Keep entries conservative — a wrong `ret` is worse than `None`.
pub fn runtime_method_ret(java_type: &str, name: &str, arity: usize) -> Option<&'static str> {
    Some(match (java_type, name, arity) {
        // Nullable: `readLine()` returns the next line or null at EOF → `Option`.
        ("BufferedReader" | "Reader" | "LineNumberReader", "readLine", 0) => "Option<String>",
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
        // java.util.concurrent.atomic.* (src/runtime/atomic.rs) — `.get()`/arithmetic
        // return the primitive (NOT the carrier), so chains/coercions type correctly.
        ("AtomicInteger", "get" | "getAndIncrement" | "incrementAndGet" | "getAndDecrement"
            | "decrementAndGet" | "getAndAdd" | "addAndGet" | "getAndSet" | "intValue", _) => "i32",
        ("AtomicInteger", "longValue", 0) => "i64",
        ("AtomicLong", "get" | "getAndIncrement" | "incrementAndGet" | "getAndDecrement"
            | "decrementAndGet" | "getAndAdd" | "addAndGet" | "getAndSet" | "longValue", _) => "i64",
        ("AtomicLong", "intValue", 0) => "i32",
        ("AtomicBoolean", "get" | "getAndSet", _) => "bool",
        ("AtomicInteger" | "AtomicLong" | "AtomicBoolean", "compareAndSet", 2) => "bool",
        // java.util.regex (regex crate). NOTE: `Matcher.group(n)` is nullable in
        // Java, but the runtime `group_1` returns empty-`String` for a
        // non-participating group (not `Option`) — so the resolver MUST say
        // `String` too (U1: render/resolver agreement). Returning `Option<String>`
        // here without making the runtime + chain-unwrap match is a U1 violation
        // (`.is_some()` on a `String`). Nullable-group is a future refinement
        // (needs `group_1-> Option` AND chained-`.group(n).m()` unwrapping).
        // `matcher`/`pattern` return the carrier Java type names so chains resolve.
        ("Pattern", "matcher", 1) => "Matcher",
        ("Pattern", "split", _) => "Vec",
        ("Pattern", "pattern", 0) => "String",
        ("Matcher", "find", 0) | ("Matcher", "matches", 0) | ("Matcher", "lookingAt", 0) => "bool",
        ("Matcher", "group", _) => "String",
        ("Matcher", "start", _) | ("Matcher", "end", _) | ("Matcher", "groupCount", 0) => "i32",
        ("Matcher", "replaceAll", 1) | ("Matcher", "replaceFirst", 1) => "String",
        ("Matcher", "pattern", 0) => "Pattern",
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
