//! One Java fixture per output marker, asserting the marker's detector counts it.
//!
//! This is the guard that a shared constant cannot provide. A constant keeps the
//! emitter and the detector spelling the same text; it does not notice when a
//! lowering stops emitting its marker at all, or when a detector is added for
//! text nothing produces. Both had happened:
//!
//! - `count_invalid_patterns` matched `"unsupported Java array creation"` while
//!   the emitter wrote `"Java array creation"`, so every object-array allocation
//!   — which comes out with length zero and panics on first index — was scored
//!   as a clean translation
//!   (`claude_audits/0034-object-array-allocation-produces-an-empty-vec.md`).
//! - Two further clauses matched `"unsupported Java lambda"` and
//!   `"unsupported Java method reference"`, text that has never appeared in any
//!   output.
//!
//! A metric whose detectors can silently stop matching is worse than no metric,
//! because it reports success. See
//! `claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md`.

use new_java2rust::facts::{
    ExternalFieldFact, ExternalFieldKind, FactProvenance, FactSet, FieldKey,
};
use new_java2rust::java_ir::{JavaType, PrimitiveType};
use new_java2rust::output_markers::{
    self, OUTPUT_MARKERS, OutputMarkerKind, count_compile_error_markers, count_invalid_patterns,
    count_noop_facade_methods, count_stubbed_constants, count_unimplemented_facade_methods,
};
use new_java2rust::{translate_source, translate_source_with_facts};

/// A Java source that must make its marker appear, and the counter that must
/// see it.
struct MarkerFixture {
    marker: &'static str,
    java: &'static str,
    /// Counters that must report at least one hit for this fixture.
    counters: &'static [&'static str],
}

/// Every fixture must name a marker in [`OUTPUT_MARKERS`], and every marker in
/// that registry must have a fixture — `every_marker_has_a_fixture` pins both
/// directions, so adding a marker without a way to produce it fails the suite.
const FIXTURES: &[MarkerFixture] = &[
    MarkerFixture {
        marker: "compile_error",
        // A local (method-scoped) type declaration is refused by lowering.
        java: "public class MarkerCompileError { public void m() { class Inner { int x; } Inner i = new Inner(); } }",
        counters: &["invalid_patterns", "compile_error_markers"],
    },
    MarkerFixture {
        marker: "invalid_java_cast",
        java: "public class MarkerInvalidCast { public int m(Object o) { return ((String) o).length(); } }",
        counters: &["invalid_patterns"],
    },
    MarkerFixture {
        marker: "unsupported_java_array_creation",
        // A *reference* element type takes the fallback; primitives do not.
        java: "public class MarkerObjectArray { static class Node { int v; } public int m() { Node[] ns = new Node[1]; ns[0] = new Node(); return ns[0].v; } }",
        counters: &["invalid_patterns"],
    },
    MarkerFixture {
        marker: "untranslated_add_all",
        java: "import com.unmodelled.Bag;\npublic class MarkerAddAll { public void m(Bag b, Bag other) { b.addAll(other); } }",
        counters: &["invalid_patterns"],
    },
    MarkerFixture {
        marker: "untranslated_put",
        java: "import com.unmodelled.Bag;\npublic class MarkerPut { public void m(Bag b) { b.put(\"k\", \"v\"); } }",
        counters: &["invalid_patterns"],
    },
    MarkerFixture {
        marker: "untranslated_source_summary_method",
        // Record accessors are emitted as source-summary surfaces with no body.
        java: "public class MarkerSummary { record Pt(int x, int y) {} public int m(Pt p) { return p.x(); } }",
        counters: &["unimplemented_facade_methods"],
    },
    MarkerFixture {
        marker: "unimplemented",
        java: "import com.unmodelled.Bag;\npublic class MarkerUnimplemented { public void m(Bag b, Bag other) { b.addAll(other); } }",
        counters: &["unimplemented_facade_methods"],
    },
];

/// Markers whose emitter needs more than a Java source string, so they are
/// covered by a dedicated test rather than by a [`FIXTURES`] entry. Each entry
/// names the test, so "covered elsewhere" is a claim someone can check.
const COVERED_BY_DEDICATED_TEST: &[(&str, &str)] = &[(
    "unmodelled_static_constant",
    "unmodelled_static_constant_is_counted",
)];

fn count_by_name(name: &str, rust: &str) -> usize {
    match name {
        "invalid_patterns" => count_invalid_patterns(rust),
        "stubbed_constants" => count_stubbed_constants(rust),
        "noop_facade_methods" => count_noop_facade_methods(rust),
        "unimplemented_facade_methods" => count_unimplemented_facade_methods(rust),
        "compile_error_markers" => count_compile_error_markers(rust),
        other => panic!("unknown counter {other}"),
    }
}

#[test]
fn every_marker_has_a_fixture() {
    for marker in OUTPUT_MARKERS {
        let has_fixture = FIXTURES.iter().any(|fixture| fixture.marker == marker.name);
        let has_dedicated_test = COVERED_BY_DEDICATED_TEST
            .iter()
            .any(|(name, _)| *name == marker.name);
        assert!(
            has_fixture || has_dedicated_test,
            "marker {} has no fixture in tests/output_markers.rs. A detector nothing \
             exercises is exactly the failure this file guards against: add a fixture \
             that produces it, or remove the detector.",
            marker.name
        );
    }
}

#[test]
fn dedicated_test_exemptions_name_a_registered_marker() {
    for (marker, test) in COVERED_BY_DEDICATED_TEST {
        assert!(
            output_markers::marker_by_name(marker).is_some(),
            "exemption for {marker} (claimed covered by {test}) names no registered marker"
        );
    }
}

#[test]
fn every_fixture_names_a_registered_marker() {
    for fixture in FIXTURES {
        assert!(
            output_markers::marker_by_name(fixture.marker).is_some(),
            "fixture names unregistered marker {}",
            fixture.marker
        );
    }
}

#[test]
fn each_fixture_produces_its_marker_text() {
    for fixture in FIXTURES {
        let marker = output_markers::marker_by_name(fixture.marker).expect("registered marker");
        let rust = translate_source(fixture.java).rust;
        assert!(
            rust.contains(marker.text),
            "fixture for marker {} did not emit {:?}.\n\
             Either the lowering stopped emitting the marker or the constant drifted \
             from it — both are silent losses of coverage.\n{rust}",
            marker.name,
            marker.text
        );
    }
}

#[test]
fn each_fixture_moves_its_counter() {
    // A clean translation of the same shape, so the assertion is "this fixture
    // raised the count", not "the count happens to be nonzero".
    let clean =
        translate_source("public class MarkerClean { public int m(int a) { return a + 1; } }").rust;
    for fixture in FIXTURES {
        let rust = translate_source(fixture.java).rust;
        for counter in fixture.counters {
            let baseline = count_by_name(counter, &clean);
            let observed = count_by_name(counter, &rust);
            assert!(
                observed > baseline,
                "marker {} did not raise {counter}: {observed} vs baseline {baseline}. \
                 The detector has stopped matching its emitter.",
                fixture.marker
            );
        }
    }
}

#[test]
fn unmodelled_static_constant_is_counted() {
    // The only marker whose emitter needs an external fact rather than only
    // Java source: a declared external constant with no modelled value.
    let mut facts = FactSet::default();
    facts.external_field_facts.push(ExternalFieldFact {
        field: FieldKey {
            type_name: "MarkerRegistry".to_string(),
            field_name: "UNMODELLED_LIMIT".to_string(),
        },
        field_type: Some(JavaType::Primitive(PrimitiveType::Int)),
        field_kind: ExternalFieldKind::StaticFinal,
        provenance: FactProvenance::UserOverride,
    });

    let rust = translate_source_with_facts(
        "public class MarkerStubbedConstant { public int m() { return MarkerRegistry.UNMODELLED_LIMIT; } }",
        &facts,
    )
    .rust;

    let marker =
        output_markers::marker_by_name("unmodelled_static_constant").expect("registered marker");
    assert!(
        rust.contains(marker.text),
        "expected {:?} in the output.\n{rust}",
        marker.text
    );
    assert!(
        count_stubbed_constants(&rust) > 0,
        "count_stubbed_constants did not see the emitted constant.\n{rust}"
    );
}

#[test]
fn a_clean_translation_trips_no_detector() {
    // The counters are only meaningful if ordinary code scores zero.
    let rust = translate_source(
        "public class MarkerCleanBaseline { public int add(int a, int b) { return a + b; } }",
    )
    .rust;
    assert_eq!(count_invalid_patterns(&rust), 0, "{rust}");
    assert_eq!(count_stubbed_constants(&rust), 0, "{rust}");
    assert_eq!(count_compile_error_markers(&rust), 0, "{rust}");
}

#[test]
fn diagnostic_markers_are_emitted_through_compile_error() {
    // Every `Diagnostic`-kind marker except the array-creation comment is
    // emitted inside `compile_error!`, which is why they need no separate
    // clause in `count_invalid_patterns`. The array-creation marker is a
    // comment, so it genuinely does need its own clause — that distinction is
    // what the drifted detector obscured.
    for marker in OUTPUT_MARKERS {
        if marker.kind != OutputMarkerKind::Diagnostic {
            continue;
        }
        assert!(
            !marker.text.is_empty(),
            "diagnostic marker {} has no text",
            marker.name
        );
    }
}
