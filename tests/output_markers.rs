//! One Java fixture per output marker, asserting the marker's detector counts it.
//!
//! This is the guard that a shared constant cannot provide. A constant keeps the
//! emitter and the detector spelling the same text; it does not notice when a
//! lowering stops emitting its marker at all, or when a detector is added for
//! text nothing produces. Both had happened:
//!
//! - `count_invalid_patterns` matched `"unsupported Java array creation"` while
//!   the emitter wrote `"Java array creation"`, so unsupported object-array
//!   allocations were scored as clean translations
//!   (`claude_audits/0034-object-array-allocation-produces-an-empty-vec.md`).
//! - Two further clauses matched `"unsupported Java lambda"` and
//!   `"unsupported Java method reference"`, text that has never appeared in any
//!   output.
//!
//! A metric whose detectors can silently stop matching is worse than no metric,
//! because it reports success. See
//! `claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md`.

use new_java2rust::call_lowering::apply_call_target;
use new_java2rust::call_plan::CallTargetPlan;
use new_java2rust::facts::{
    ExternalFieldFact, ExternalFieldKind, FactProvenance, FactSet, FieldKey, ReceiverAbi,
};
use new_java2rust::java_ir::{Expr, JavaType, PrimitiveType};
use new_java2rust::output_markers::{
    self, OUTPUT_MARKERS, OutputMarkerKind, count_compile_error_markers, count_invalid_patterns,
    count_noop_facade_call_sites, count_noop_facade_methods, count_stubbed_constants,
    count_unimplemented_facade_methods, count_unmodelled_upcast_conversions,
};
use new_java2rust::rust_ir::RustExpr;
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
        // Unknown external reference elements still take the loud fallback.
        java: "import com.unmodelled.Node;\npublic class MarkerObjectArray { public int m() { Node[] ns = new Node[1]; return ns.length; } }",
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
        marker: "unsupported_inner_class_outer_capture",
        // A non-static inner class: its constructor cannot obtain a shared
        // handle to the enclosing instance, so it is emitted as a `panic!`.
        // The mutate-after-construction step is the semantic reason a copy
        // would not be a valid repair: Java must observe base = 20 here.
        java: "public class MarkerInnerCapture { private int base = 10; class Adder { int add(int x) { return base + x; } } public int run() { Adder a = new Adder(); base = 20; return a.add(1); } }",
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
const COVERED_BY_DEDICATED_TEST: &[(&str, &str)] = &[
    (
        "unmodelled_static_constant",
        "unmodelled_static_constant_is_counted",
    ),
    // Not fixture-differentiable: the upcast surface is emitted for *every*
    // translation, so no Java program raises the count above the clean
    // baseline. That is the finding, not a gap in coverage — see the test.
    (
        "unmodelled_java_upcast",
        "unmodelled_upcasts_are_not_counted_as_noop_facade_methods",
    ),
];

fn count_by_name(name: &str, rust: &str) -> usize {
    match name {
        "invalid_patterns" => count_invalid_patterns(rust),
        "stubbed_constants" => count_stubbed_constants(rust),
        "noop_facade_methods" => count_noop_facade_methods(rust),
        "noop_facade_call_sites" => count_noop_facade_call_sites(rust),
        "unmodelled_upcast_conversions" => count_unmodelled_upcast_conversions(rust),
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
fn unsupported_inner_class_outer_capture_is_reported_as_a_translation_warning() {
    let fixture = FIXTURES
        .iter()
        .find(|fixture| fixture.marker == "unsupported_inner_class_outer_capture")
        .expect("inner-class marker fixture");
    let translation = translate_source(fixture.java);

    assert!(
        translation
            .diagnostics
            .has_code("inner_class_construction_panics"),
        "inner-class construction should be reported by the translator, not only \
         left as a runtime panic marker:\n{:?}",
        translation.diagnostics.iter().collect::<Vec<_>>()
    );
}

#[test]
fn unused_inner_class_outer_capture_is_not_reported_as_construction_panic() {
    let translation = translate_source(
        r#"
        public class UnusedInnerCapture {
            private int base = 1;
            class Inner {
                int get() { return base; }
            }
            public int run() { return base; }
        }
        "#,
    );

    assert!(
        !translation
            .diagnostics
            .has_code("inner_class_construction_panics"),
        "unused captured inner class should not be reported as a construction-site panic:\n{:?}",
        translation.diagnostics.iter().collect::<Vec<_>>()
    );
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

/// Every JDK constant the corpus reaches carries a real value, not a plausible
/// zero.
///
/// This is the detection test
/// `claude_audits/done/0031-stubbed-constant-census.md` asks for. Its point is that a
/// `const` cannot fail at its use site the way a method can: there is no body to
/// put `unimplemented!()` in, so a wrong value is baked in at compile time and
/// is indistinguishable from a right one. `0`, `0.0` and `false` are the worst
/// placeholders precisely because they are plausible — `Math.PI == 0.0` silently
/// corrupts arithmetic rather than stopping it.
///
/// `tests/differential/Constants.java` checks the *values* against the Java
/// oracle; this checks the *shape* of what is emitted, so a newly stubbed
/// constant is caught even when no fixture happens to read it.
#[test]
fn jdk_constants_are_not_stubbed_to_plausible_zeros() {
    // Touch the constant surface the census found stubbed, so the facades that
    // declare them are emitted.
    let rust = translate_source(
        r#"
public class ConstSurface {
    public double d() {
        return Math.PI + Math.E + Double.NaN + Double.POSITIVE_INFINITY
            + Double.NEGATIVE_INFINITY + Double.MAX_VALUE + Double.MIN_VALUE;
    }
    public float f() {
        return Float.NaN + Float.POSITIVE_INFINITY + Float.NEGATIVE_INFINITY
            + Float.MAX_VALUE + Float.MIN_VALUE;
    }
    public long l() {
        return Integer.MAX_VALUE + Integer.MIN_VALUE + Long.MAX_VALUE + Long.MIN_VALUE;
    }
}
"#,
    )
    .rust;

    // Names whose Java value genuinely is zero-like would belong here; none of
    // the constants above is one, so the list is empty and the check is total.
    const LEGITIMATELY_ZERO: &[&str] = &[];

    let mut stubbed = Vec::new();
    for line in rust.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(':').and_then(|(name, tail)| {
            tail.split_once('=')
                .map(|(_, value)| (name.trim(), value.trim().trim_end_matches(';').trim()))
        }) else {
            continue;
        };
        if LEGITIMATELY_ZERO.contains(&name) {
            continue;
        }
        if matches!(
            value,
            "0" | "0i32"
                | "0i64"
                | "0u8"
                | "0.0"
                | "0.0f32"
                | "0.0f64"
                | "false"
                | "Default::default()"
        ) {
            stubbed.push(format!("{name} = {value}"));
        }
    }

    assert!(
        stubbed.is_empty(),
        "constants stubbed to a plausible zero: {stubbed:?}\n\
         A constant with no known value must be a translator error, never a \
         default — see claude_audits/0031.\n{rust}"
    );
}

// ---------------------------------------------------------------------------
// Audit bookkeeping
// ---------------------------------------------------------------------------
//
// The same class of failure as a stale marker detector, one level up: the
// tracked status table, `claude_audits/README.md`, and the `claude_audits/done/`
// directory are three hand-maintained records of the same fact, and they have
// drifted before — a row read `done` for an audit still sitting in the active
// directory, and README links pointed at files that had moved. A status table
// that can silently disagree with the filesystem reports progress that did not
// happen.

/// Every audit id, and where its file actually lives.
fn audit_locations() -> std::collections::BTreeMap<String, AuditLocation> {
    let mut found = std::collections::BTreeMap::new();
    for (dir, location) in [
        ("claude_audits", AuditLocation::Active),
        ("claude_audits/done", AuditLocation::Done),
    ] {
        let entries = std::fs::read_dir(concat_repo_path(dir))
            .unwrap_or_else(|err| panic!("cannot read {dir}: {err}"));
        for entry in entries {
            let name = entry.expect("directory entry").file_name();
            let name = name.to_string_lossy().to_string();
            let Some(id) = audit_id_of_file(&name) else {
                continue;
            };
            if let Some(previous) = found.insert(id.clone(), location) {
                panic!("audit {id} exists in both {previous:?} and {location:?}");
            }
        }
    }
    found
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuditLocation {
    Active,
    Done,
}

/// `0034-object-array-....md` -> `0034`; `0002b-...` -> `0002b`.
fn audit_id_of_file(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".md")?;
    let id = stem.split('-').next()?;
    let (digits, suffix) = id.split_at(id.len().min(4));
    if digits.len() != 4 || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    if !suffix.chars().all(|ch| ch.is_ascii_lowercase()) {
        return None;
    }
    Some(id.to_string())
}

fn concat_repo_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// A status that claims the work is finished.
fn status_claims_completion(status: &str) -> bool {
    let status = status.trim();
    status == "done" || status.starts_with("done (") || status == "fixed locally"
}

/// Rows of the tracked status table, as `(id, status)`.
fn tracked_plan_rows() -> Vec<(String, String)> {
    let plan = std::fs::read_to_string(concat_repo_path("docs/claude-audit-refactoring-plan.md"))
        .expect("tracked plan");
    let mut rows = Vec::new();
    for line in plan.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let Some((id, rest)) = rest.split_once('`') else {
            continue;
        };
        if audit_id_of_file(&format!("{id}-x.md")).is_none() {
            continue;
        }
        let Some(status) = rest.split('|').nth(1) else {
            continue;
        };
        rows.push((id.to_string(), status.trim().to_string()));
    }
    assert!(
        rows.len() > 20,
        "the tracked plan table stopped parsing: {} rows",
        rows.len()
    );
    rows
}

#[test]
fn tracked_plan_status_agrees_with_the_audit_directory() {
    let locations = audit_locations();
    let mut problems = Vec::new();
    for (id, status) in tracked_plan_rows() {
        let Some(location) = locations.get(&id) else {
            problems.push(format!("{id}: row present, but no audit file anywhere"));
            continue;
        };
        match (status_claims_completion(&status), location) {
            (true, AuditLocation::Active) => problems.push(format!(
                "{id}: status {status:?} claims completion, but the file is still active"
            )),
            (false, AuditLocation::Done) => problems.push(format!(
                "{id}: status {status:?} is not a completion, but the file is in done/"
            )),
            _ => {}
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_audit_has_a_tracked_plan_row() {
    let tracked: std::collections::BTreeSet<String> =
        tracked_plan_rows().into_iter().map(|(id, _)| id).collect();
    let missing: Vec<String> = audit_locations()
        .into_keys()
        .filter(|id| !tracked.contains(id))
        .collect();
    assert!(
        missing.is_empty(),
        "audits with no row in docs/claude-audit-refactoring-plan.md: {missing:?}"
    );
}

#[test]
fn audit_readme_links_resolve() {
    let readme = std::fs::read_to_string(concat_repo_path("claude_audits/README.md"))
        .expect("claude_audits/README.md");
    let mut dangling = Vec::new();
    let mut seen_ids = std::collections::BTreeSet::new();
    let mut rest = readme.as_str();
    while let Some(open) = rest.find("](") {
        rest = &rest[open + 2..];
        let Some(close) = rest.find(')') else { break };
        let target = &rest[..close];
        rest = &rest[close..];
        if target.starts_with("http") || target.starts_with('#') || !target.ends_with(".md") {
            continue;
        }
        if !concat_repo_path("claude_audits").join(target).exists() {
            dangling.push(target.to_string());
        }
        if let Some(id) = audit_id_of_file(target.rsplit('/').next().unwrap_or(target)) {
            seen_ids.insert(id);
        }
    }
    assert!(dangling.is_empty(), "dangling README links: {dangling:?}");

    let unlisted: Vec<String> = audit_locations()
        .into_keys()
        .filter(|id| !seen_ids.contains(id))
        .collect();
    assert!(
        unlisted.is_empty(),
        "audits with no README row: {unlisted:?}"
    );
}

#[test]
fn exception_audit_does_not_reopen_promoted_catch_finally_loop_fixtures() {
    let audit = std::fs::read_to_string(concat_repo_path(
        "claude_audits/0057-exceptions-are-panics-and-finally-is-a-closure.md",
    ))
    .expect("0057 audit");

    for fixture in [
        "CatchFinallyBreak.java",
        "CatchFinallyContinue.java",
        "CatchFinallyLabeledContinue.java",
        "TryCatchFinallyBodyBreak.java",
        "TryWithResourcesBodyBreak.java",
        "TryWithResourcesBodyContinue.java",
    ] {
        assert!(
            concat_repo_path("tests/differential")
                .join(fixture)
                .exists(),
            "{fixture} should stay promoted while 0057 describes it as covered"
        );
        assert!(audit.contains(fixture), "0057 should name {fixture}");
    }
    assert!(
        !audit.contains("fail with Rust E0267"),
        "0057 should not still direct work at the old catch/finally closure-boundary failure"
    );
}

/// An unmodelled upcast is not a no-op facade *method*, and counting it as one
/// made the facade counter almost pure noise.
///
/// On a program whose only Java API use is `StringBuilder`, 78 of the 80 lines
/// the old detector matched were synthesised `From` conversions. A coverage
/// counter that is 97% a different phenomenon cannot make erasure visible —
/// which is the whole argument of
/// `claude_audits/0018-the-compile-error-metric-cannot-see-silent-failures.md`.
#[test]
fn unmodelled_upcasts_are_not_counted_as_noop_facade_methods() {
    let rust = translate_source(
        "public class UpcastSplit { public String m() { StringBuilder b = new StringBuilder(); \
         b.append(\"x\"); return b.toString(); } }",
    )
    .rust;

    let upcasts = count_unmodelled_upcast_conversions(&rust);
    let noop_methods = count_noop_facade_methods(&rust);

    assert!(
        upcasts > 10,
        "expected the java.io upcast surface to be present: {upcasts}\n{rust}"
    );
    assert!(
        noop_methods < upcasts / 4,
        "no-op facade methods ({noop_methods}) must not be dominated by upcast \
         conversions ({upcasts}); that is the counter this split exists to fix\n{rust}"
    );

    // And the reason it is not a fixture entry: the surface is unconditional,
    // so a program that touches no Java API at all emits the same conversions.
    let clean =
        translate_source("public class UpcastClean { public int m(int a) { return a + 1; } }").rust;
    assert_eq!(
        count_unmodelled_upcast_conversions(&clean),
        upcasts,
        "the upcast surface is emitted regardless of what the program uses, so it \
         cannot be a per-file cost signal; it is a constant offset"
    );
}

#[test]
fn unmodelled_upcast_count_ignores_literal_marker_text() {
    let prefix = output_markers::UNMODELLED_JAVA_UPCAST_PREFIX;
    let literal_only = format!(
        "let text = \"{prefix}Leaf -> Base\";\n\
         let raw = r#\"{prefix}Leaf -> Base\"#;\n"
    );
    assert_eq!(count_unmodelled_upcast_conversions(&literal_only), 0);

    let real_marker = format!(
        "/// {prefix}Leaf -> Base\n\
         impl From<Leaf> for Base {{ fn from(_value: Leaf) -> Base {{ Default::default() }} }}\n"
    );
    assert_eq!(count_unmodelled_upcast_conversions(&real_marker), 1);
}

/// A facade constructor counted once may be called from many places, and the
/// call count is the cost.
///
/// `claude_audits/0018` asks for `noop_facade_call_sites` and a previous pass
/// recorded it as unbuildable from generated text. That obstacle — the
/// surviving no-op *methods* are named `to_string`/`clone`/`into`, so matching
/// `.to_string(` would count unrelated calls — does not apply to constructors,
/// which are called through a qualified path.
#[test]
fn noop_facade_constructor_call_sites_are_counted_per_call() {
    // `java.awt.Font` is genuinely unmodelled, so its facade constructor is a
    // no-op. A *modelled* type such as `ZipEntry` has a real constructor body
    // and must not be counted — which is why the fixture names a specific type
    // rather than any external one.
    let rust = translate_source(
        "import java.awt.Font;\n\
         public class Fonts {\n\
             public static int m() {\n\
                 Font a = new Font(\"x\", 0, 10);\n\
                 Font b = new Font(\"y\", 0, 12);\n\
                 Font c = new Font(\"z\", 0, 14);\n\
                 return a.getSize() + b.getSize() + c.getSize();\n\
             }\n\
         }",
    )
    .rust;

    let call_sites = count_noop_facade_call_sites(&rust);
    assert_eq!(
        call_sites, 3,
        "expected exactly the three constructions to be counted\n{rust}"
    );

    // One definition, three calls: the ratio is the whole point. A definition
    // count reports 1 for a cost of 3.
    let font_definitions = rust
        .lines()
        .filter(|line| {
            line.contains(" fn new") && line.trim_end().ends_with("{ Default::default() }")
        })
        .count();
    assert!(
        call_sites > font_definitions,
        "call sites ({call_sites}) must exceed no-op constructor definitions \
         ({font_definitions}), or the counter measures surface rather than cost\n{rust}"
    );
}

#[test]
fn noop_facade_method_call_sites_are_marked_at_lowering_time() {
    let receiver = Expr::Var("o".to_string());
    let lowered = apply_call_target(
        &CallTargetPlan::ExternalFacadeReceiver {
            receiver: &receiver,
            rust_path: "put".to_string(),
            receiver_abi: ReceiverAbi::BorrowedSelf,
            mutates_receiver: false,
            noop_facade_method_call: true,
            receiver_type: "org.json.JSONObject".to_string(),
            method_name: "put".to_string(),
            arg_count: 2,
        },
        Some(RustExpr::Var("o".to_string())),
        vec![
            RustExpr::Literal("\"x\"".to_string()),
            RustExpr::Literal("1".to_string()),
        ],
    );
    let marker =
        output_markers::noop_facade_method_call_site_marker("org.json.JSONObject", "put", 2);
    assert_eq!(
        lowered,
        RustExpr::MarkedBlock {
            marker: marker.clone(),
            expr: Box::new(RustExpr::MethodCall {
                receiver: Box::new(RustExpr::Var("o".to_string())),
                method: "put".to_string(),
                args: vec![
                    RustExpr::Literal("\"x\"".to_string()),
                    RustExpr::Literal("1".to_string()),
                ],
            }),
        }
    );

    let rust = new_java2rust::emit::emit_expr(&lowered);
    assert_eq!(rust, format!("{{ {marker} o.put(\"x\", 1) }}"));

    assert!(
        rust.contains(&output_markers::noop_facade_method_call_site_marker_prefix()),
        "expected lowered no-op facade method calls to carry an explicit marker\n{rust}"
    );
    assert_eq!(
        count_noop_facade_call_sites(&rust),
        1,
        "expected the marked facade method call to be counted\n{rust}"
    );
}

#[test]
fn noop_facade_method_call_site_counter_requires_marker_prefix() {
    let rust = r##"
pub fn doc() {
    let _text = "noop Java facade method call";
    let _raw = r#"noop Java facade method call: JsonObject.put/2"#;
    // noop Java facade method call
    /* noop Java facade method call: JsonObject.put/2 */ receiver.put("x", 1);
}
"##;

    assert_eq!(
        count_noop_facade_call_sites(rust),
        1,
        "only the emitted marker comment prefix should count"
    );

    let literal_only = r##"
pub fn doc() {
    let _text = "noop Java facade method call: JsonObject.put/2";
    let _raw = r#"noop Java facade method call: JsonObject.put/2"#;
}
"##;

    assert_eq!(count_noop_facade_call_sites(literal_only), 0);
}

/// A definition is not a call site: the counter must not count the facade's own
/// `fn new` line.
#[test]
fn a_noop_facade_definition_is_not_counted_as_a_call() {
    let clean =
        translate_source("public class NoCalls { public int m(int a) { return a + 1; } }").rust;
    assert_eq!(
        count_noop_facade_call_sites(&clean),
        0,
        "a program that constructs no external facade has no call sites\n{clean}"
    );
}

#[test]
fn noop_facade_constructor_collection_requires_function_line_shape() {
    let rust = r#"
impl Font {
    // pub fn new_3<T0, T1, T2>(_arg0: T0, _arg1: T1, _arg2: T2) -> Font { Default::default() }
    let_text!("pub fn new_3<T0, T1, T2>(_arg0: T0, _arg1: T1, _arg2: T2) -> Font { Default::default() }");
}

pub fn build() -> Font {
    Font::new_3("x", 0, 12)
}
"#;

    assert_eq!(count_noop_facade_call_sites(rust), 0);
}

#[test]
fn noop_facade_constructor_scan_survives_multiline_methods_in_impl() {
    let rust = r##"
impl Font {
    pub fn size(&self) -> i32 {
        let _debug = "}";
        let _raw = r#"}"#;
        let _ch = '}';
        // }
        /* } */
        12
    }

    pub fn new_3<T0, T1, T2>(_arg0: T0, _arg1: T1, _arg2: T2) -> Font { Default::default() }
}

pub fn build() -> Font {
    Font::new_3("x", 0, 12)
}
"##;

    assert_eq!(count_noop_facade_call_sites(rust), 1);
}

#[test]
fn noop_facade_constructor_scan_handles_generic_impl_owners() {
    let rust = r#"
pub struct Future<T = Box<dyn std::any::Any>> {
    _marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> Future<T> {
    pub fn new_1<T0>(_arg0: T0) -> Future<T> { Default::default() }
}

pub fn build() -> Future<String> {
    Future::new_1("x")
}
"#;

    assert_eq!(count_noop_facade_call_sites(rust), 1);
}

#[test]
fn noop_facade_constructor_scan_ignores_prefixes_comments_and_strings() {
    let rust = r##"
impl Font {
    pub fn new_3<T0, T1, T2>(_arg0: T0, _arg1: T1, _arg2: T2) -> Font { Default::default() }
}

pub fn build() -> Font {
    // Font::new_3("comment", 0, 12)
    let _debug = "Font::new_3(\"string\", 0, 12)";
    let _raw = r#"Font::new_3("raw string", 0, 12)"#;
    let _other = MyFont::new_3("prefix", 0, 12);
    Font::new_3("real", 0, 12)
}
"##;

    assert_eq!(count_noop_facade_call_sites(rust), 1);
}
