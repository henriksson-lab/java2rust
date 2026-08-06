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
    pub name: &'static str,
    /// The literal a detector matches. Emitters must produce this through the
    /// constant rather than spelling it again.
    pub text: &'static str,
    pub kind: OutputMarkerKind,
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
/// handle. The program compiles and aborts on first construction.
pub const UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE: &str =
    "unsupported Java inner-class outer capture construction";

/// Introduces the `From` impl synthesised for a Java upcast that has no model.
///
/// Its body is `Default::default()`, so a widened value silently becomes a
/// fresh empty one. That is erasure, and 0018's argument is that erasure has to
/// be *visible as a cost*; before this constant existed the conversions were
/// counted as `noop_facade_methods`, where they were 78 of 80 on a program
/// whose only Java API use was `StringBuilder`.
pub const UNMODELLED_JAVA_UPCAST_PREFIX: &str = "Unmodelled Java upcast: ";

/// A `Collection.addAll` that kept its Java name instead of becoming `extend`.
pub const UNTRANSLATED_JAVA_ADD_ALL_CALL: &str = ".add_all(";

/// A `Map.put` that kept its Java name instead of becoming `insert`.
pub const UNTRANSLATED_JAVA_PUT_CALL: &str = ".put(";

/// `.expect("invalid Java cast")`, spelled once.
pub fn invalid_java_cast_expect() -> String {
    format!(".expect({INVALID_JAVA_CAST_MESSAGE:?})")
}

/// `panic!("invalid Java cast")`, spelled once.
pub fn invalid_java_cast_panic() -> String {
    format!("panic!({INVALID_JAVA_CAST_MESSAGE:?})")
}

/// The diagnostic message for an external constant with no modelled value.
pub fn unmodelled_static_constant_message(owner: &str, field: &str) -> String {
    format!("{UNMODELLED_STATIC_CONSTANT_PREFIX}{owner}.{field}{UNMODELLED_STATIC_CONSTANT_SUFFIX}")
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
    OutputMarker {
        name: "compile_error",
        text: COMPILE_ERROR_MACRO,
        kind: OutputMarkerKind::Diagnostic,
    },
    OutputMarker {
        name: "invalid_java_cast",
        text: INVALID_JAVA_CAST_MESSAGE,
        kind: OutputMarkerKind::Diagnostic,
    },
    OutputMarker {
        name: "unsupported_java_array_creation",
        text: UNSUPPORTED_JAVA_ARRAY_CREATION,
        kind: OutputMarkerKind::Diagnostic,
    },
    OutputMarker {
        name: "unsupported_inner_class_outer_capture",
        text: UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE,
        kind: OutputMarkerKind::Diagnostic,
    },
    OutputMarker {
        name: "unmodelled_java_upcast",
        text: UNMODELLED_JAVA_UPCAST_PREFIX,
        kind: OutputMarkerKind::Stub,
    },
    OutputMarker {
        name: "unmodelled_static_constant",
        text: UNMODELLED_STATIC_CONSTANT_SUFFIX,
        kind: OutputMarkerKind::Stub,
    },
    OutputMarker {
        name: "untranslated_source_summary_method",
        text: UNTRANSLATED_SOURCE_SUMMARY_SUFFIX,
        kind: OutputMarkerKind::Stub,
    },
    OutputMarker {
        name: "unimplemented",
        text: UNIMPLEMENTED_MACRO,
        kind: OutputMarkerKind::Stub,
    },
    OutputMarker {
        name: "untranslated_add_all",
        text: UNTRANSLATED_JAVA_ADD_ALL_CALL,
        kind: OutputMarkerKind::UntranslatedCall,
    },
    OutputMarker {
        name: "untranslated_put",
        text: UNTRANSLATED_JAVA_PUT_CALL,
        kind: OutputMarkerKind::UntranslatedCall,
    },
];

/// Look a marker up by name. Used by the fixture test to pair a fixture with
/// the marker it is supposed to produce.
pub fn marker_by_name(name: &str) -> Option<&'static OutputMarker> {
    OUTPUT_MARKERS.iter().find(|marker| marker.name == name)
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
    rust.lines()
        .filter(|line| {
            line.contains(COMPILE_ERROR_MACRO)
                || (line.contains(DOWNCAST_TURBOFISH) && line.contains(&cast_expect))
                || line.contains(UNTRANSLATED_JAVA_ADD_ALL_CALL)
                || line.contains(UNTRANSLATED_JAVA_PUT_CALL)
                || line.contains(UNSUPPORTED_JAVA_ARRAY_CREATION)
                || line.contains(UNSUPPORTED_INNER_CLASS_OUTER_CAPTURE)
                || (line.contains("= None;")
                    && !line.contains("let mut ")
                    && !line.contains(": Option<"))
        })
        .count()
}

/// External constants emitted with no modelled value.
pub fn count_stubbed_constants(rust: &str) -> usize {
    rust.lines()
        .filter(|line| {
            line.contains("pub const ")
                && line.contains(COMPILE_ERROR_MACRO)
                && line.contains(UNMODELLED_STATIC_CONSTANT_PREFIX)
                && line.contains(UNMODELLED_STATIC_CONSTANT_SUFFIX)
        })
        .count()
}

/// Facade methods whose whole body is a default value — they compile and do
/// nothing.
pub fn count_noop_facade_methods(rust: &str) -> usize {
    rust.lines()
        .filter(|line| {
            let line = line.trim_end();
            !line_is_unmodelled_upcast_conversion(line)
                && (line.ends_with("Default::default() }") || line.ends_with("self.clone() }"))
        })
        .count()
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
    if constructors.is_empty() {
        return 0;
    }
    rust.lines()
        .filter(|line| !line_defines_noop_facade_constructor(line))
        .map(|line| {
            constructors
                .iter()
                .map(|path| count_noop_facade_path_calls(line, path))
                .sum::<usize>()
        })
        .sum()
}

fn count_noop_facade_path_calls(line: &str, path: &str) -> usize {
    let mut count = 0usize;
    let mut cursor = 0usize;
    let bytes = line.as_bytes();
    while cursor < line.len() {
        if bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'/') {
            break;
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
    for line in rust.lines() {
        let trimmed = line.trim();
        if owner.is_none()
            && let Some(name) = inherent_impl_owner(trimmed)
        {
            owner = Some(name);
            impl_depth = 0;
        }
        if !line_defines_noop_facade_constructor(line) {
            if owner.is_some() {
                impl_depth += line.matches('{').count() as isize;
                impl_depth -= line.matches('}').count() as isize;
                if impl_depth <= 0 {
                    owner = None;
                    impl_depth = 0;
                }
            }
            continue;
        }
        let Some(owner_name) = owner.as_deref() else {
            continue;
        };
        let Some(rest) = trimmed.split_once("fn ").map(|(_, rest)| rest) else {
            continue;
        };
        let Some(name) = rest.split(['(', '<', ' ']).next() else {
            continue;
        };
        paths.push(format!("{owner_name}::{name}("));
        impl_depth += line.matches('{').count() as isize;
        impl_depth -= line.matches('}').count() as isize;
        if impl_depth <= 0 {
            owner = None;
            impl_depth = 0;
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn inherent_impl_owner(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("impl")?;
    if !rest.starts_with(char::is_whitespace) && !rest.starts_with('<') {
        return None;
    }
    if rest.contains(" for ") {
        return None;
    }
    let rest = rest.trim_start();
    let rest = if rest.starts_with('<') {
        let generic_end = matching_angle_end(rest)?;
        rest[generic_end + 1..].trim_start()
    } else {
        rest
    };
    let owner = rest.split_whitespace().next()?.trim_end_matches('{').trim();
    let owner = owner.split('<').next()?.trim();
    (!owner.is_empty()).then(|| owner.to_string())
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
    let trimmed = line.trim();
    trimmed.contains(" fn new")
        && trimmed.ends_with("{ Default::default() }")
        && !trimmed.starts_with("fn from(")
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
    rust.lines()
        .filter(|line| line.contains(UNMODELLED_JAVA_UPCAST_PREFIX))
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
    rust.lines()
        .filter(|line| line.contains(UNIMPLEMENTED_MACRO))
        .count()
}

/// Total `compile_error!` invocations, counting more than one per line.
pub fn count_compile_error_markers(rust: &str) -> usize {
    rust.lines()
        .map(|line| line.matches(COMPILE_ERROR_MACRO_CALL).count())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_names_are_unique() {
        let mut names: Vec<&str> = OUTPUT_MARKERS.iter().map(|marker| marker.name).collect();
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
                !marker.text.is_empty(),
                "marker {} has empty text, so its detector would match every line",
                marker.name
            );
        }
    }

    #[test]
    fn cast_helpers_spell_the_shared_message() {
        assert_eq!(invalid_java_cast_expect(), ".expect(\"invalid Java cast\")");
        assert_eq!(invalid_java_cast_panic(), "panic!(\"invalid Java cast\")");
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
}
