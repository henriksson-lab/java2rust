//! Marker text that the translator emits into generated Rust, owned in one
//! place so an emitter and its detector cannot drift apart.
//!
//! Every corpus counter in `src/main.rs` used to carry its own string literal,
//! with nothing checking the correspondence. One had already drifted:
//! `count_invalid_patterns` looked for `"unsupported Java array creation"`
//! while the emitter wrote `"Java array creation"`, so an object-array
//! allocation that comes out with length zero — every index into it panics —
//! was counted as a clean translation
//! ([0034](../claude_audits/0034-object-array-allocation-produces-an-empty-vec.md)).
//! Auditing the rest found two more detectors matching text that nothing in the
//! tree has ever produced: `"unsupported Java lambda"` and
//! `"unsupported Java method reference"`, where the emitted messages are
//! `"unsupported Java expression: lambda"` and
//! `"unsupported Java expression: method reference"`.
//!
//! A metric whose detectors can silently stop matching is worse than no metric,
//! because it reports success. Two things guard against that here:
//!
//! 1. the detector and the emitter read the **same constant**, so a change to
//!    the text moves both;
//! 2. [`OUTPUT_MARKERS`] enumerates them, and `tests/output_markers.rs`
//!    translates one Java fixture per marker and asserts the corresponding
//!    count rises — which catches the case a shared constant cannot, namely a
//!    lowering that stops emitting its marker at all.

/// What a marker says about the translation that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMarkerKind {
    /// The translator refused to translate a construct and said so in the
    /// output. Loud, and the desired behaviour for anything unsupported.
    Diagnostic,
    /// A Java API name survived into the output unmapped. Silent: the output
    /// may well compile against a facade that does nothing.
    UntranslatedCall,
    /// A modelled surface exists but has no behaviour behind it.
    Stub,
}

/// One marker, its text, and how to talk about it.
#[derive(Debug, Clone, Copy)]
pub struct OutputMarker {
    /// Stable identifier for tests and reports. Not emitted.
    name: &'static str,
    /// The literal a detector matches. Emitters must produce this through the
    /// constant rather than spelling it again.
    text: &'static str,
    kind: OutputMarkerKind,
}

impl OutputMarker {
    pub const fn new(name: &'static str, text: &'static str, kind: OutputMarkerKind) -> Self {
        Self { name, text, kind }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn text(&self) -> &'static str {
        self.text
    }

    pub fn kind(&self) -> OutputMarkerKind {
        self.kind
    }
}

/// The `compile_error!` macro itself — the general "refused to translate" mark.
pub const COMPILE_ERROR_MACRO: &str = "compile_error!";

/// `compile_error!(` — used where a detector counts *invocations* rather than
/// mentions, so a bare macro name in a comment does not inflate the count.
pub const COMPILE_ERROR_MACRO_CALL: &str = "compile_error!(";

/// `unimplemented!(` — a modelled surface with no behaviour behind it.
pub const UNIMPLEMENTED_MACRO: &str = "unimplemented!(";

/// The message inside the `expect` that guards an erased-object downcast.
pub const INVALID_JAVA_CAST_MESSAGE: &str = "invalid Java cast";

/// The turbofish that starts an erased-object downcast.
pub const DOWNCAST_TURBOFISH: &str = ".downcast::<";

/// Emitted for a lambda that reached the unsupported-expression fallback.
///
/// Not a registry entry, and deliberately has no clause of its own in
/// [`count_invalid_patterns`]: the message is emitted inside `compile_error!`,
/// so the general clause counts it. `count_invalid_patterns` used to carry a
/// separate clause spelled `"unsupported Java lambda"`, which nothing has ever
/// produced — redundant *and* misspelled.
///
/// The lowering arm is currently unreachable as well: `lower_expr` handles
/// `Expr::Lambda` before consulting the unsupported fallback. The constant
/// stays because the fallback is the right shape if that ever changes.
pub const UNSUPPORTED_JAVA_LAMBDA: &str = "unsupported Java expression: lambda";

/// Emitted for a method reference that reached the unsupported-expression
/// fallback. Same status as [`UNSUPPORTED_JAVA_LAMBDA`].
pub const UNSUPPORTED_JAVA_METHOD_REFERENCE: &str = "unsupported Java expression: method reference";

/// Emitted where an object-array allocation falls back to an empty `Vec`.
pub const UNSUPPORTED_JAVA_ARRAY_CREATION: &str = "unsupported Java array creation";

/// Start of the message for an external constant with no modelled value.
pub const UNMODELLED_STATIC_CONSTANT_PREFIX: &str = "external static constant ";

/// End of the message for an external constant with no modelled value.
pub const UNMODELLED_STATIC_CONSTANT_SUFFIX: &str = " has no modeled value";

/// Start of the message for a source-summary method that was not translated.
pub const UNTRANSLATED_SOURCE_SUMMARY_PREFIX: &str = "source-summary method ";

/// End of the message for a source-summary method that was not translated.
pub const UNTRANSLATED_SOURCE_SUMMARY_SUFFIX: &str = " is not translated";

/// Emitted where a non-static inner class's constructor cannot build its outer
/// handle. The generated file refuses to compile at the unsupported construction.
pub const UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE: &str =
    "unsupported Java inner-class outer capture construction";

/// Introduces the `From` impl synthesised for a Java upcast that has no model.
///
/// Its body panics with this marker so a widened value fails loudly instead of
/// becoming a fresh empty one. Before this constant existed the conversions
/// were counted as `noop_facade_methods`, where they were 78 of 80 on a program
/// whose only Java API use was `StringBuilder`.
pub const UNMODELLED_JAVA_UPCAST_PREFIX: &str = "Unmodelled Java upcast: ";

/// A `Collection.addAll` that kept its Java name instead of becoming `extend`.
pub const UNTRANSLATED_JAVA_ADD_ALL_CALL: &str = ".add_all(";

/// A `Map.put` that kept its Java name instead of becoming `insert`.
pub const UNTRANSLATED_JAVA_PUT_CALL: &str = ".put(";

/// Emitted at a call site whose facade target returns a default or cloned
/// receiver value instead of modelling Java behaviour.
pub const NOOP_FACADE_METHOD_CALL_SITE: &str = "noop Java facade method call";

/// `.expect("invalid Java cast")`, spelled once.
pub fn invalid_java_cast_expect() -> String {
    format!(".expect({INVALID_JAVA_CAST_MESSAGE:?})")
}

/// The diagnostic message for an external constant with no modelled value.
pub fn unmodelled_static_constant_message(owner: &str, field: &str) -> String {
    format!("{UNMODELLED_STATIC_CONSTANT_PREFIX}{owner}.{field}{UNMODELLED_STATIC_CONSTANT_SUFFIX}")
}

pub fn noop_facade_method_call_site_marker(
    receiver_type: &str,
    method_name: &str,
    arg_count: usize,
) -> String {
    format!(
        "{}{receiver_type}.{method_name}/{arg_count} */",
        noop_facade_method_call_site_marker_prefix()
    )
}

pub fn noop_facade_method_call_site_marker_prefix() -> String {
    format!("/* {NOOP_FACADE_METHOD_CALL_SITE}: ")
}

/// The diagnostic message for a source-summary method with no translated body.
pub fn untranslated_source_summary_message(owner: &str, method: &str) -> String {
    format!(
        "{UNTRANSLATED_SOURCE_SUMMARY_PREFIX}{owner}.{method}{UNTRANSLATED_SOURCE_SUMMARY_SUFFIX}"
    )
}

/// Every marker, in the order the corpus report lists them.
///
/// `tests/output_markers.rs` requires a fixture for each entry, so adding a
/// marker here without a fixture that produces it fails the suite. That is
/// deliberate: an unexercised detector is exactly the failure this module
/// exists to prevent.
pub const OUTPUT_MARKERS: &[OutputMarker] = &[
    OutputMarker::new(
        "compile_error",
        COMPILE_ERROR_MACRO,
        OutputMarkerKind::Diagnostic,
    ),
    OutputMarker::new(
        "invalid_java_cast",
        INVALID_JAVA_CAST_MESSAGE,
        OutputMarkerKind::Diagnostic,
    ),
    OutputMarker::new(
        "unsupported_java_array_creation",
        UNSUPPORTED_JAVA_ARRAY_CREATION,
        OutputMarkerKind::Diagnostic,
    ),
    OutputMarker::new(
        "unsupported_inner_class_outer_capture",
        UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE,
        OutputMarkerKind::Diagnostic,
    ),
    OutputMarker::new(
        "unmodelled_java_upcast",
        UNMODELLED_JAVA_UPCAST_PREFIX,
        OutputMarkerKind::Stub,
    ),
    OutputMarker::new(
        "unmodelled_static_constant",
        UNMODELLED_STATIC_CONSTANT_SUFFIX,
        OutputMarkerKind::Stub,
    ),
    OutputMarker::new(
        "untranslated_source_summary_method",
        UNTRANSLATED_SOURCE_SUMMARY_SUFFIX,
        OutputMarkerKind::Stub,
    ),
    OutputMarker::new("unimplemented", UNIMPLEMENTED_MACRO, OutputMarkerKind::Stub),
    OutputMarker::new(
        "untranslated_add_all",
        UNTRANSLATED_JAVA_ADD_ALL_CALL,
        OutputMarkerKind::UntranslatedCall,
    ),
    OutputMarker::new(
        "untranslated_put",
        UNTRANSLATED_JAVA_PUT_CALL,
        OutputMarkerKind::UntranslatedCall,
    ),
    OutputMarker::new(
        "noop_facade_call_site",
        NOOP_FACADE_METHOD_CALL_SITE,
        OutputMarkerKind::Stub,
    ),
];

/// Look a marker up by name. Used by the fixture test to pair a fixture with
/// the marker it is supposed to produce.
pub fn marker_by_name(name: &str) -> Option<&'static OutputMarker> {
    OUTPUT_MARKERS.iter().find(|marker| marker.name() == name)
}

/// Lines that show the translator produced something it could not stand behind.
///
/// Every clause matches a constant above rather than its own literal. Three of
/// these had drifted from their emitter and were silently matching nothing —
/// which is why this lives next to the constants and is exercised per marker by
/// `tests/output_markers.rs`.
///
/// `unsupported Java expression: …` messages are deliberately *not* given their
/// own clauses: they are emitted through `compile_error!`, so the first clause
/// already counts them. A second, differently-spelled clause for the same text
/// is exactly the drift this module exists to prevent.
pub fn count_invalid_patterns(rust: &str) -> usize {
    let cast_expect = invalid_java_cast_expect();
    let inner_capture_panic_prefix = format!("panic!(\"{UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE}");
    let mut open_raw_terminator = None;
    rust.lines()
        .map(|line| line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator))
        .filter(|line| {
            line_contains_outside_literals(line, COMPILE_ERROR_MACRO)
                || (line_contains_outside_literals(line, DOWNCAST_TURBOFISH)
                    && line_contains_outside_literals(line, &cast_expect))
                || line_contains_outside_literals(line, UNTRANSLATED_JAVA_ADD_ALL_CALL)
                || line_contains_outside_literals(line, UNTRANSLATED_JAVA_PUT_CALL)
                || line_contains_outside_literals(line, UNSUPPORTED_JAVA_ARRAY_CREATION)
                || line_contains_outside_literals(line, UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE)
                || line_contains_outside_literals(line, &inner_capture_panic_prefix)
                || (line_contains_outside_literals(line, "= None;")
                    && !line_contains_outside_literals(line, "let mut ")
                    && !line_contains_outside_literals(line, ": Option<"))
        })
        .count()
}

fn line_contains_outside_literals(line: &str, pattern: &str) -> bool {
    line_count_outside_literals(line, pattern) > 0
}

fn line_count_outside_literals(line: &str, pattern: &str) -> usize {
    let mut count = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut skip_until = 0usize;
    for (idx, ch) in line.char_indices() {
        if idx < skip_until {
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            continue;
        }
        if let Some(end) = raw_string_literal_end_at(line, idx) {
            skip_until = end;
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            _ if line[idx..].starts_with(pattern) => count += 1,
            _ => {}
        }
    }
    count
}

fn raw_string_literal_end_at(line: &str, start: usize) -> Option<usize> {
    let rest = line.get(start..)?;
    if !rest.starts_with('r') {
        return None;
    }
    let mut hashes = 0usize;
    for ch in rest[1..].chars() {
        if ch == '#' {
            hashes += 1;
            continue;
        }
        if ch != '"' {
            return None;
        }
        let content_start = start + 1 + hashes + 1;
        let terminator = format!("\"{}", "#".repeat(hashes));
        return Some(
            line[content_start..]
                .find(&terminator)
                .map(|relative_end| content_start + relative_end + terminator.len())
                .unwrap_or(line.len()),
        );
    }
    None
}

fn line_with_multiline_raw_strings_elided(
    line: &str,
    open_terminator: &mut Option<String>,
) -> String {
    let mut visible = String::new();
    let mut cursor = 0usize;
    while cursor < line.len() {
        if let Some(terminator) = open_terminator.as_deref() {
            let Some(relative_end) = line[cursor..].find(terminator) else {
                return visible;
            };
            cursor += relative_end + terminator.len();
            *open_terminator = None;
            continue;
        }
        let Some((start, terminator)) = unterminated_raw_string_on_line(line, cursor) else {
            visible.push_str(&line[cursor..]);
            return visible;
        };
        visible.push_str(&line[cursor..start]);
        *open_terminator = Some(terminator);
        cursor = line.len();
    }
    visible
}

/// External constants emitted with no modelled value.
pub fn count_stubbed_constants(rust: &str) -> usize {
    let unmodelled_constant_compile_error_prefix =
        format!("{COMPILE_ERROR_MACRO_CALL}\"{UNMODELLED_STATIC_CONSTANT_PREFIX}");
    let mut open_raw_terminator = None;
    rust.lines()
        .map(|line| line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator))
        .filter(|line| {
            line_contains_outside_literals(line, "pub const ")
                && line_contains_outside_literals(line, &unmodelled_constant_compile_error_prefix)
                && line.contains(UNMODELLED_STATIC_CONSTANT_SUFFIX)
        })
        .count()
}

/// Facade methods whose whole body is a default value — they compile and do
/// nothing.
pub fn count_noop_facade_methods(rust: &str) -> usize {
    let mut open_raw_terminator = None;
    let mut count = 0usize;
    for line in rust.lines() {
        let line = line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator);
        let line = line.trim_end();
        if !line_is_unmodelled_upcast_conversion(line) && line_defines_noop_facade_method(line) {
            count += 1;
        }
    }
    count
}

/// Call sites of facade constructors that do nothing.
///
/// [0018](../claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md)
/// asks for this counter and a previous pass recorded it as unbuildable from
/// generated text. That was half right. The obstacle named there — that the
/// surviving no-op methods are called `to_string`, `clone`, `into`, so counting
/// `.to_string(` would count mostly unrelated calls — is real for *methods* and
/// does **not** apply to constructors: a facade constructor is called through a
/// qualified path, `Foo::new_2(…)`, which cannot collide with anything.
///
/// So this counts what is soundly countable: every `Type::new…(` whose facade
/// definition in the same file has a `Default::default()` body. Both halves come
/// from one file, so there is no cross-file inference and no name guessing.
///
/// The measure that matters is *cost*, not surface: a facade constructor
/// counted once may be called from a hundred places, and this is what makes
/// erasure visible as a cost rather than invisible as a saving.
pub fn count_noop_facade_call_sites(rust: &str) -> usize {
    let constructors = noop_facade_constructors(rust);
    let mut open_raw_terminator = None;
    let constructor_calls = rust
        .lines()
        .map(|line| line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator))
        .filter(|line| !line_defines_noop_facade_constructor(line))
        .map(|line| {
            constructors
                .iter()
                .map(|path| count_noop_facade_path_calls(&line, path))
                .sum::<usize>()
        })
        .sum::<usize>();
    let mut open_raw_terminator = None;
    constructor_calls
        + rust
            .lines()
            .map(|line| line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator))
            .filter(|line| {
                line_contains_outside_literals(line, &noop_facade_method_call_site_marker_prefix())
            })
            .count()
}

fn count_noop_facade_path_calls(line: &str, path: &str) -> usize {
    let mut count = 0usize;
    let mut cursor = 0usize;
    let bytes = line.as_bytes();
    while cursor < line.len() {
        if bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'/') {
            break;
        }
        if let Some(end) = raw_string_literal_end_at(line, cursor) {
            cursor = end;
            continue;
        }
        if bytes.get(cursor) == Some(&b'"') {
            cursor += 1;
            let mut escaped = false;
            while cursor < line.len() {
                let byte = bytes[cursor];
                cursor += 1;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    break;
                }
            }
            continue;
        }
        if line[cursor..].starts_with(path) && starts_on_rust_path_boundary(line, cursor) {
            count += 1;
            cursor += path.len();
        } else {
            cursor += line[cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
        }
    }
    count
}

fn starts_on_rust_path_boundary(line: &str, start: usize) -> bool {
    line[..start]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_rust_ident_continue(ch))
}

fn is_rust_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

/// `Type::new…(` for every facade constructor in `rust` with a no-op body.
fn noop_facade_constructors(rust: &str) -> Vec<String> {
    let mut owner = None;
    let mut impl_depth = 0isize;
    let mut paths = Vec::new();
    let mut open_raw_terminator = None;
    for line in rust.lines() {
        let line = line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator);
        let trimmed = line.trim();
        if owner.is_none()
            && let Some(name) = inherent_impl_owner(trimmed)
        {
            owner = Some(name);
            impl_depth = 0;
        }
        let constructor_name =
            line_noop_facade_function_name(&line).filter(|name| name.starts_with("new"));
        let Some(constructor_name) = constructor_name else {
            if owner.is_some() {
                impl_depth += line_count_syntax_char(&line, '{') as isize;
                impl_depth -= line_count_syntax_char(&line, '}') as isize;
                if impl_depth <= 0 {
                    owner = None;
                    impl_depth = 0;
                }
            }
            continue;
        };
        let Some(owner_name) = owner.as_deref() else {
            continue;
        };
        paths.push(format!("{owner_name}::{constructor_name}("));
        impl_depth += line_count_syntax_char(&line, '{') as isize;
        impl_depth -= line_count_syntax_char(&line, '}') as isize;
        if impl_depth <= 0 {
            owner = None;
            impl_depth = 0;
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn unterminated_raw_string_on_line(line: &str, start: usize) -> Option<(usize, String)> {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut cursor = start;
    while cursor < line.len() {
        let ch = line[cursor..]
            .chars()
            .next()
            .expect("cursor is inside the line");
        if escaped {
            escaped = false;
            cursor += ch.len_utf8();
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            cursor += ch.len_utf8();
            continue;
        }
        if line[cursor..].starts_with("//") {
            return None;
        }
        if line[cursor..].starts_with("/*") {
            let Some(relative_end) = line[cursor + 2..].find("*/") else {
                return None;
            };
            cursor += 2 + relative_end + "*/".len();
            continue;
        }
        if let Some((content_start, terminator)) = raw_string_literal_parts_at(line, cursor) {
            if line[content_start..].find(&terminator).is_none() {
                return Some((cursor, terminator));
            }
            cursor = raw_string_literal_end_at(line, cursor).unwrap_or(line.len());
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    None
}

fn raw_string_literal_parts_at(line: &str, start: usize) -> Option<(usize, String)> {
    let rest = line.get(start..)?;
    if !rest.starts_with('r') {
        return None;
    }
    let mut hashes = 0usize;
    for ch in rest[1..].chars() {
        if ch == '#' {
            hashes += 1;
            continue;
        }
        if ch != '"' {
            return None;
        }
        let content_start = start + 1 + hashes + 1;
        return Some((content_start, format!("\"{}", "#".repeat(hashes))));
    }
    None
}

fn line_count_syntax_char(line: &str, target: char) -> usize {
    let mut count = 0usize;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut cursor = 0usize;
    while cursor < line.len() {
        if let Some(end) = raw_string_literal_end_at(line, cursor) {
            cursor = end;
            continue;
        }
        let ch = line[cursor..]
            .chars()
            .next()
            .expect("cursor is inside the line");
        if escaped {
            escaped = false;
            cursor += ch.len_utf8();
            continue;
        }
        if in_string || in_char {
            if ch == '\\' {
                escaped = true;
            } else if in_string && ch == '"' {
                in_string = false;
            } else if in_char && ch == '\'' {
                in_char = false;
            }
            cursor += ch.len_utf8();
            continue;
        }
        if line[cursor..].starts_with("//") {
            break;
        }
        if line[cursor..].starts_with("/*") {
            let Some(relative_end) = line[cursor + 2..].find("*/") else {
                break;
            };
            cursor += 2 + relative_end + "*/".len();
            continue;
        }
        match ch {
            '"' => in_string = true,
            '\'' => in_char = true,
            _ if ch == target => count += 1,
            _ => {}
        }
        cursor += ch.len_utf8();
    }
    count
}

fn inherent_impl_owner(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("impl")?;
    if !rest.starts_with(char::is_whitespace) && !rest.starts_with('<') {
        return None;
    }
    let rest = rest.trim_start();
    let rest = if rest.starts_with('<') {
        let generic_end = matching_angle_end(rest)?;
        rest[generic_end + 1..].trim_start()
    } else {
        rest
    };
    let owner = rest
        .split(|ch: char| ch == '<' || ch == '{' || ch.is_whitespace())
        .next()
        .unwrap_or_default();
    if owner.is_empty() {
        return None;
    }
    let mut after_owner = rest[owner.len()..].trim_start();
    if after_owner.starts_with('<') {
        let generic_end = matching_angle_end(after_owner)?;
        after_owner = after_owner[generic_end + 1..].trim_start();
    }
    if after_owner.starts_with("for ") {
        return None;
    }
    Some(owner.to_string())
}

fn matching_angle_end(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// A facade constructor whose whole body is `Default::default()`.
fn line_defines_noop_facade_constructor(line: &str) -> bool {
    line_noop_facade_function_name(line).is_some_and(|name| name.starts_with("new"))
}

fn line_defines_noop_facade_method(line: &str) -> bool {
    line_noop_facade_function_name(line).is_some()
}

fn line_noop_facade_function_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let rest = trimmed
        .strip_prefix("pub fn ")
        .or_else(|| trimmed.strip_prefix("fn "))?;
    if !(trimmed.ends_with("{ Default::default() }") || trimmed.ends_with("{ self.clone() }")) {
        return None;
    }
    let name = rest.split(['(', '<', ' ']).next()?;
    (!name.is_empty()).then_some(name)
}

/// `From` impls synthesised for Java upcasts with no model.
///
/// Counted apart from [`count_noop_facade_methods`] because they are a
/// different phenomenon with a different fix: a no-op *method* means a modelled
/// API does nothing, while this means a widening conversion throws the value
/// away. Lumping them together made the facade counter 97% noise on a trivial
/// program and hid the signal it exists to show
/// ([0018](../claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md)).
pub fn count_unmodelled_upcast_conversions(rust: &str) -> usize {
    let mut open_raw_terminator = None;
    rust.lines()
        .map(|line| line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator))
        .filter(|line| line_contains_outside_literals(line, UNMODELLED_JAVA_UPCAST_PREFIX))
        .count()
}

/// The conversion body a synthesised upcast `From` impl has.
///
/// Matched on the body rather than the doc comment above it, so a counter that
/// walks lines does not have to remember the previous one.
fn line_is_unmodelled_upcast_conversion(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("fn from(") && line.ends_with("Default::default() }")
}

/// Facade methods that panic when reached.
pub fn count_unimplemented_facade_methods(rust: &str) -> usize {
    let mut open_raw_terminator = None;
    rust.lines()
        .map(|line| line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator))
        .filter(|line| line_contains_outside_literals(line, UNIMPLEMENTED_MACRO))
        .count()
}

/// Total `compile_error!` invocations, counting more than one per line.
pub fn count_compile_error_markers(rust: &str) -> usize {
    let mut open_raw_terminator = None;
    rust.lines()
        .map(|line| line_with_multiline_raw_strings_elided(line, &mut open_raw_terminator))
        .map(|line| line_count_outside_literals(&line, COMPILE_ERROR_MACRO_CALL))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_names_are_unique() {
        let mut names: Vec<&str> = OUTPUT_MARKERS.iter().map(OutputMarker::name).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(
            names.len(),
            unique,
            "duplicate marker name in OUTPUT_MARKERS"
        );
    }

    #[test]
    fn marker_text_is_never_empty() {
        for marker in OUTPUT_MARKERS {
            assert!(
                !marker.text().is_empty(),
                "marker {} has empty text, so its detector would match every line",
                marker.name()
            );
        }
    }

    #[test]
    fn cast_helpers_spell_the_shared_message() {
        assert_eq!(invalid_java_cast_expect(), ".expect(\"invalid Java cast\")");
    }

    #[test]
    fn constant_and_summary_messages_bracket_their_subject() {
        let constant = unmodelled_static_constant_message("java.awt.Font", "BOLD");
        assert!(constant.starts_with(UNMODELLED_STATIC_CONSTANT_PREFIX));
        assert!(constant.ends_with(UNMODELLED_STATIC_CONSTANT_SUFFIX));
        assert!(constant.contains("java.awt.Font.BOLD"));

        let summary = untranslated_source_summary_message("Pt", "x");
        assert!(summary.starts_with(UNTRANSLATED_SOURCE_SUMMARY_PREFIX));
        assert!(summary.ends_with(UNTRANSLATED_SOURCE_SUMMARY_SUFFIX));
        assert!(summary.contains("Pt.x"));
    }

    #[test]
    fn invalid_pattern_count_ignores_literal_marker_text() {
        let cast_expect = invalid_java_cast_expect();
        let rust = format!(
            "let text = \"{COMPILE_ERROR_MACRO} {DOWNCAST_TURBOFISH} {cast_expect} {UNTRANSLATED_JAVA_ADD_ALL_CALL} = None;\";\n\
             let raw = r#\"{UNTRANSLATED_JAVA_PUT_CALL}\"#;\n\
             let array = \"{UNSUPPORTED_JAVA_ARRAY_CREATION}\";\n\
             let inner = r#\"{UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE}\"#;\n\
             let ch = '=';\n"
        );
        assert_eq!(count_invalid_patterns(&rust), 0);

        let real = format!(
            "compile_error!(\"unsupported Java expression\");\n\
             value.downcast::<Target>(){cast_expect};\n\
             /* {UNTRANSLATED_JAVA_PUT_CALL} */\n\
             /* {UNSUPPORTED_JAVA_ARRAY_CREATION} */\n\
             /* {UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE} */\n\
             panic!(\"{UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE} for Inner\");\n\
             field = None;\n"
        );
        assert_eq!(count_invalid_patterns(&real), 7);

        assert_eq!(
            count_invalid_patterns("let mut field = None;\nlet opt: Option<i32> = None;\n"),
            0
        );
    }

    #[test]
    fn invalid_pattern_count_ignores_multiline_raw_string_opening_line() {
        let rust = format!(
            "let text = r#\"{COMPILE_ERROR_MACRO} {UNTRANSLATED_JAVA_ADD_ALL_CALL}\n\
             {UNTRANSLATED_JAVA_PUT_CALL} {COMPILE_ERROR_MACRO_CALL} still literal\"#;\n"
        );

        assert_eq!(count_invalid_patterns(&rust), 0);
    }

    #[test]
    fn shared_marker_counters_skip_multiline_raw_string_bodies() {
        let rust = format!(
            "let text = r#\"start\n\
             {COMPILE_ERROR_MACRO_CALL} {UNIMPLEMENTED_MACRO}\n\
             {UNMODELLED_JAVA_UPCAST_PREFIX}A -> B\n\
             pub const VALUE: i32 = {COMPILE_ERROR_MACRO}(\"{UNMODELLED_STATIC_CONSTANT_PREFIX}Owner.VALUE{UNMODELLED_STATIC_CONSTANT_SUFFIX}\");\n\
             \"#; {COMPILE_ERROR_MACRO_CALL}\"real\");\n"
        );

        assert_eq!(count_compile_error_markers(&rust), 1);
        assert_eq!(count_unimplemented_facade_methods(&rust), 0);
        assert_eq!(count_unmodelled_upcast_conversions(&rust), 0);
        assert_eq!(count_stubbed_constants(&rust), 0);
        assert_eq!(count_invalid_patterns(&rust), 1);
    }

    #[test]
    fn noop_facade_method_count_requires_function_line_shape() {
        let rust = r#"
// pub fn fake(&self) -> Font { Default::default() }
let text = "pub fn fake(&self) -> Font { Default::default() }";
pub fn real(&self) -> Font { Default::default() }
pub fn cloned(&self) -> Font { self.clone() }
"#;

        assert_eq!(count_noop_facade_methods(rust), 2);
    }

    #[test]
    fn noop_facade_method_count_skips_multiline_raw_string_contents() {
        let rust = r##"
let doc = r#"
pub fn fake(&self) -> Font { Default::default() }
impl Font {
    pub fn new() -> Font { Default::default() }
}
Font::new();
"#;
pub fn real(&self) -> Font { Default::default() }
"##;

        assert_eq!(count_noop_facade_methods(rust), 1);
        assert_eq!(count_noop_facade_call_sites(rust), 0);
    }

    #[test]
    fn noop_facade_constructor_owner_parses_hrtbs_without_trait_impls() {
        assert_eq!(
            inherent_impl_owner("impl<T> Widget<T> where T: for <'a> Parser<'a> {"),
            Some("Widget".to_string())
        );
        assert_eq!(
            inherent_impl_owner("impl<T> Display<T> for Widget<T> {"),
            None
        );

        let rust = r#"
impl<T> Widget<T> where T: for <'a> Parser<'a> {
    pub fn new() -> Widget<T> { Default::default() }
}
let built = Widget::new();
impl<T> Display<T> for Gadget<T> {
    pub fn new() -> Gadget<T> { Default::default() }
}
let ignored = Gadget::new();
"#;

        assert_eq!(count_noop_facade_call_sites(rust), 1);
    }

    #[test]
    fn stubbed_constant_count_requires_syntactic_const_and_compile_error() {
        let literal = format!(
            "let text = \"pub const VALUE: i32 = {COMPILE_ERROR_MACRO}({UNMODELLED_STATIC_CONSTANT_PREFIX}Owner.VALUE{UNMODELLED_STATIC_CONSTANT_SUFFIX});\";\n"
        );
        assert_eq!(count_stubbed_constants(&literal), 0);

        let unrelated_compile_error = format!(
            "pub const VALUE: i32 = {COMPILE_ERROR_MACRO}(\"other\"); \
             let text = \"{UNMODELLED_STATIC_CONSTANT_PREFIX}Owner.VALUE{UNMODELLED_STATIC_CONSTANT_SUFFIX}\";\n"
        );
        assert_eq!(count_stubbed_constants(&unrelated_compile_error), 0);

        let real = format!(
            "pub const VALUE: i32 = {COMPILE_ERROR_MACRO}(\"{UNMODELLED_STATIC_CONSTANT_PREFIX}Owner.VALUE{UNMODELLED_STATIC_CONSTANT_SUFFIX}\");\n"
        );
        assert_eq!(count_stubbed_constants(&real), 1);
    }

    #[test]
    fn unimplemented_facade_count_ignores_literal_marker_text() {
        let literal = format!(
            "let text = \"{UNIMPLEMENTED_MACRO}\";\n\
             let raw = r#\"{UNIMPLEMENTED_MACRO}\"#;\n"
        );
        assert_eq!(count_unimplemented_facade_methods(&literal), 0);

        let real = format!(
            "pub fn missing(&self) -> i32 {{ {UNIMPLEMENTED_MACRO}\"missing\") }}\n\
             /* {UNIMPLEMENTED_MACRO} still visible in comments */\n"
        );
        assert_eq!(count_unimplemented_facade_methods(&real), 2);
    }

    #[test]
    fn compile_error_marker_count_ignores_literal_marker_text() {
        let literal = format!(
            "let text = \"{COMPILE_ERROR_MACRO_CALL}\";\n\
             let raw = r#\"{COMPILE_ERROR_MACRO_CALL}\"#;\n"
        );
        assert_eq!(count_compile_error_markers(&literal), 0);

        let real =
            format!("{COMPILE_ERROR_MACRO_CALL}\"first\"); {COMPILE_ERROR_MACRO_CALL}\"second\");");
        assert_eq!(count_compile_error_markers(&real), 2);
    }
}
