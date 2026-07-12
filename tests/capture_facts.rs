use new_java2rust::facts::{CaptureOwner, CaptureOwnerKind, CaptureSource};
use new_java2rust::parser::parse_java_source;
use new_java2rust::resolve::resolve_program;

fn capture_owners(path: &str) -> Vec<CaptureOwner> {
    let source = std::fs::read_to_string(std::path::Path::new("examples/java").join(path)).unwrap();
    let parsed = parse_java_source(&source);
    let resolved = resolve_program(&parsed.program, parsed.diagnostics);
    assert!(
        !resolved.diagnostics.has_errors(),
        "{:?}",
        resolved.diagnostics
    );
    resolved.facts.captures.owners
}

#[test]
fn streams_collect_lambdas_capture_only_stream_item() {
    let owners = capture_owners("06-stdlib/StreamsCollect.java");
    let lambdas = owners
        .iter()
        .filter(|owner| owner.kind == CaptureOwnerKind::Lambda)
        .collect::<Vec<_>>();

    assert_eq!(lambdas.len(), 2, "{owners:#?}");
    assert!(lambdas.iter().all(|owner| owner.captures.is_empty()));
}

#[test]
fn lambda_capture_records_enclosing_parameter() {
    let owners = capture_owners("06-stdlib/StreamCapturesLocal.java");
    let lambdas = owners
        .iter()
        .filter(|owner| owner.kind == CaptureOwnerKind::Lambda)
        .collect::<Vec<_>>();

    assert_eq!(lambdas.len(), 1, "{owners:#?}");
    let lambda = lambdas[0];
    assert_eq!(lambda.captures.len(), 1, "{lambda:#?}");
    assert_eq!(lambda.captures[0].name, "base");
    assert_eq!(lambda.captures[0].source, CaptureSource::Parameter);
    assert!(lambda.captures[0].effectively_final);
    assert!(!lambda.captures[0].mutable_after_capture);
}

#[test]
fn local_class_capture_records_enclosing_parameter_and_local() {
    let owners = capture_owners("04-objects/LocalClassCapture.java");
    let local = owners
        .iter()
        .find(|owner| {
            owner.kind == CaptureOwnerKind::LocalClass && owner.name.as_deref() == Some("Adder")
        })
        .unwrap_or_else(|| panic!("missing local class owner: {owners:#?}"));

    let captures = local
        .captures
        .iter()
        .map(|capture| (capture.name.as_str(), capture.source))
        .collect::<Vec<_>>();
    assert!(
        captures.contains(&("base", CaptureSource::Parameter)),
        "{local:#?}"
    );
    assert!(
        captures.contains(&("offset", CaptureSource::Local)),
        "{local:#?}"
    );
    assert!(
        !captures.iter().any(|(name, _)| *name == "value"),
        "{local:#?}"
    );
}

#[test]
fn inner_class_records_outer_this_capture() {
    let owners = capture_owners("04-objects/InnerCapture.java");
    let inner = owners
        .iter()
        .find(|owner| {
            owner.kind == CaptureOwnerKind::InnerClass && owner.name.as_deref() == Some("Adder")
        })
        .unwrap_or_else(|| panic!("missing inner class owner: {owners:#?}"));

    assert!(
        inner
            .captures
            .iter()
            .any(|capture| capture.source == CaptureSource::OuterThis
                && capture.name == "InnerCapture"),
        "{inner:#?}"
    );
}

#[test]
fn anonymous_class_owner_has_no_captures_for_current_fixture() {
    let owners = capture_owners("04-objects/AnonymousInterface.java");
    let anonymous = owners
        .iter()
        .find(|owner| owner.kind == CaptureOwnerKind::AnonymousClass)
        .unwrap_or_else(|| panic!("missing anonymous class owner: {owners:#?}"));

    assert!(anonymous.captures.is_empty(), "{anonymous:#?}");
}
