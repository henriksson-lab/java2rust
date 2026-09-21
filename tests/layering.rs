fn production_source_without_cfg_test_items(source: &str) -> String {
    let mut output = String::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.trim_start().starts_with("#[cfg(test)]") {
            output.push_str(line);
            output.push('\n');
            continue;
        }

        while let Some(next) = lines.peek() {
            if next.trim_start().starts_with("#[") {
                lines.next();
            } else {
                break;
            }
        }

        if lines
            .peek()
            .is_some_and(|next| next.trim_start().starts_with("mod tests"))
        {
            break;
        }

        let mut depth = 0isize;
        let mut saw_body = false;
        for item_line in lines.by_ref() {
            for ch in item_line.chars() {
                match ch {
                    '{' => {
                        depth += 1;
                        saw_body = true;
                    }
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if !saw_body || depth <= 0 {
                break;
            }
        }
    }
    output
}

#[test]
fn batch_translation_options_is_not_a_public_field_bag() {
    let source = include_str!("../src/lib.rs");
    let options_source = source
        .split("#[derive(Clone)]\npub struct BatchTranslationOptions")
        .nth(1)
        .and_then(|tail| {
            tail.split("impl fmt::Debug for BatchTranslationOptions")
                .next()
        })
        .expect("BatchTranslationOptions should remain discoverable");
    for field in [
        "pub emission_jobs:",
        "pub event_sink:",
        "pub(crate) emission_jobs:",
        "pub(crate) event_sink:",
    ] {
        assert!(
            !options_source.contains(field),
            "BatchTranslationOptions should expose constructor/accessors, not raw fields: {field}"
        );
    }
    for accessor in [
        "pub fn new(emission_jobs: usize, event_sink: Option<Arc<BatchTranslationEventSink>>) -> Self",
        "pub fn emission_jobs(&self) -> usize",
        "pub fn event_sink(&self) -> Option<&BatchTranslationEventSink>",
    ] {
        assert!(
            source.contains(accessor),
            "BatchTranslationOptions should keep accessor API: {accessor}"
        );
    }
}

#[test]
fn batch_source_is_not_a_public_field_bag() {
    let source = include_str!("../src/lib.rs");
    let source_record = source
        .split("#[derive(Debug, Clone)]\npub struct BatchSource")
        .nth(1)
        .and_then(|tail| tail.split("impl BatchSource").next())
        .expect("BatchSource should remain discoverable");
    for field in ["pub path:", "pub source:"] {
        assert!(
            !source_record.contains(field),
            "BatchSource should expose constructor/accessors, not raw fields: {field}"
        );
    }
    for accessor in [
        "pub fn new(path: impl Into<String>, source: impl Into<String>) -> Self",
        "pub fn path(&self) -> &str",
        "pub fn source(&self) -> &str",
    ] {
        assert!(
            source.contains(accessor),
            "BatchSource should keep accessor API: {accessor}"
        );
    }
}

#[test]
fn batch_translation_is_not_a_public_field_bag() {
    let source = include_str!("../src/lib.rs");
    let translation_source = source
        .split("#[derive(Debug, Clone)]\npub struct BatchTranslation")
        .nth(1)
        .and_then(|tail| tail.split("impl BatchTranslation").next())
        .expect("BatchTranslation should remain discoverable");
    for field in ["pub path:", "pub rust:", "pub diagnostics:"] {
        assert!(
            !translation_source.contains(field),
            "BatchTranslation should expose constructor/accessors, not raw fields: {field}"
        );
    }
    for accessor in [
        "pub fn new(path: impl Into<String>, rust: String, diagnostics: DiagnosticBag) -> Self",
        "pub fn path(&self) -> &str",
        "pub fn rust(&self) -> &str",
        "pub fn diagnostics(&self) -> &DiagnosticBag",
    ] {
        assert!(
            source.contains(accessor),
            "BatchTranslation should keep accessor API: {accessor}"
        );
    }
}

#[test]
fn batch_translation_event_is_not_a_public_field_bag() {
    let source = include_str!("../src/lib.rs");
    let event_source = source
        .split("#[derive(Debug, Clone)]\npub struct BatchTranslationEvent")
        .nth(1)
        .and_then(|tail| tail.split("impl BatchTranslationEvent").next())
        .expect("BatchTranslationEvent should remain discoverable");
    for field in [
        "pub phase:",
        "pub index:",
        "pub path:",
        "pub elapsed_ms:",
        "pub status:",
    ] {
        assert!(
            !event_source.contains(field),
            "BatchTranslationEvent should expose constructor/accessors, not raw fields: {field}"
        );
    }
    for accessor in [
        "pub fn new(",
        "pub fn phase(&self) -> &'static str",
        "pub fn index(&self) -> usize",
        "pub fn path(&self) -> &str",
        "pub fn elapsed_ms(&self) -> u128",
        "pub fn status(&self) -> &'static str",
    ] {
        assert!(
            source.contains(accessor),
            "BatchTranslationEvent should keep accessor API: {accessor}"
        );
    }
}

#[test]
fn batch_translation_timings_is_not_a_public_field_bag() {
    let source = include_str!("../src/lib.rs");
    let timings_source = source
        .split("#[derive(Debug, Clone, Default)]\npub struct BatchTranslationTimings")
        .nth(1)
        .and_then(|tail| tail.split("impl BatchTranslationTimings").next())
        .expect("BatchTranslationTimings should remain discoverable");
    for field in [
        "pub primary_parse_ms:",
        "pub context_parse_ms:",
        "pub summary_parse_ms:",
        "pub global_analysis_ms:",
        "pub global_resolve_ms:",
        "pub global_semantic_ms:",
        "pub global_representation_ms:",
        "pub global_signature_ms:",
        "pub emission_ms:",
    ] {
        assert!(
            !timings_source.contains(field),
            "BatchTranslationTimings should expose read-only accessors, not raw fields: {field}"
        );
    }
    for accessor in [
        "pub fn primary_parse_ms(&self) -> u128",
        "pub fn context_parse_ms(&self) -> u128",
        "pub fn summary_parse_ms(&self) -> u128",
        "pub fn global_analysis_ms(&self) -> u128",
        "pub fn global_resolve_ms(&self) -> u128",
        "pub fn global_semantic_ms(&self) -> u128",
        "pub fn global_representation_ms(&self) -> u128",
        "pub fn global_signature_ms(&self) -> u128",
        "pub fn emission_ms(&self) -> u128",
    ] {
        assert!(
            source.contains(accessor),
            "BatchTranslationTimings should keep accessor API: {accessor}"
        );
    }
}

#[test]
fn batch_analysis_profile_is_not_a_public_metrics_field_bag() {
    let source = include_str!("../src/lib.rs");
    let profile_source = source
        .split("#[derive(Debug, Clone, Default)]\npub struct BatchAnalysisProfile")
        .nth(1)
        .and_then(|tail| tail.split("impl BatchAnalysisProfile").next())
        .expect("BatchAnalysisProfile should remain discoverable");
    for field in [
        "pub primary_files:",
        "pub context_files:",
        "pub summary_files:",
        "pub program_types:",
        "pub program_methods:",
        "pub program_constructors:",
        "pub program_fields:",
        "pub external_fact_count:",
    ] {
        assert!(
            !profile_source.contains(field),
            "BatchAnalysisProfile should expose read-only accessors, not raw metric fields: {field}"
        );
    }
    for accessor in [
        "pub fn primary_files(&self) -> usize",
        "pub fn context_files(&self) -> usize",
        "pub fn summary_files(&self) -> usize",
        "pub fn program_types(&self) -> usize",
        "pub fn program_methods(&self) -> usize",
        "pub fn program_constructors(&self) -> usize",
        "pub fn program_fields(&self) -> usize",
        "pub fn external_fact_count(&self) -> usize",
    ] {
        assert!(
            source.contains(accessor),
            "BatchAnalysisProfile should keep accessor API: {accessor}"
        );
    }
}

#[test]
fn batch_translation_report_is_not_a_public_field_bag() {
    let source = include_str!("../src/lib.rs");
    let report_source = source
        .split("#[derive(Debug, Clone)]\npub struct BatchTranslationReport")
        .nth(1)
        .and_then(|tail| tail.split("impl BatchTranslationReport").next())
        .expect("BatchTranslationReport should remain discoverable");
    for field in [
        "pub translations:",
        "pub timings:",
        "pub analysis_profile:",
        "pub events:",
    ] {
        assert!(
            !report_source.contains(field),
            "BatchTranslationReport should expose read-only accessors, not raw fields: {field}"
        );
    }
    for accessor in [
        "pub fn translations(&self) -> &[BatchTranslation]",
        "pub fn timings(&self) -> &BatchTranslationTimings",
        "pub fn analysis_profile(&self) -> &BatchAnalysisProfile",
        "pub fn events(&self) -> &[BatchTranslationEvent]",
    ] {
        assert!(
            source.contains(accessor),
            "BatchTranslationReport should keep accessor API: {accessor}"
        );
    }
}

#[test]
fn plan_layers_keep_rust_rendering_out_of_source_analysis() {
    let signature_source = include_str!("../src/signature.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("signature production source should be present");
    let cases = [
        (
            "facts.rs",
            include_str!("../src/facts.rs"),
            &[
                "rust_ir",
                "RustExpr",
                "RustType",
                "emit::",
                "rust_type_for_slot",
            ][..],
        ),
        (
            "shape_projection.rs",
            include_str!("../src/shape_projection.rs"),
            &["rust_ir", "RustType", "slot_render", "rust_type_for_slot"][..],
        ),
        (
            "slot_shape.rs",
            include_str!("../src/slot_shape.rs"),
            &[
                "indexed_read_shape",
                "indexed_assignment_shape",
                "LoweredType",
                "OptionalElementBorrow",
                "VecElementBorrow",
            ][..],
        ),
        (
            "signature.rs",
            signature_source,
            &["rust_ir", "RustType", "slot_render", "rust_type_for_slot"][..],
        ),
        (
            "call_plan.rs",
            include_str!("../src/call_plan.rs"),
            &[
                "rust_ir",
                "RustExpr",
                "RustType",
                "emit::",
                "emit_type",
                "apply_call_target",
                "apply_new_target",
            ][..],
        ),
        (
            "exception_plan.rs",
            include_str!("../src/exception_plan.rs"),
            &[
                "rust_ir",
                "RustExpr",
                "RustStmt",
                "RustType",
                "emit::",
                "JavaException::new",
            ][..],
        ),
        (
            "stream.rs",
            include_str!("../src/stream.rs"),
            &[
                "rust_ir",
                "RustExpr",
                "emit::",
                "emit_expr",
                "render_stream_plan",
                "emit_stream_lambda_body",
                ".collect::<Vec<_>>()",
                ".next()",
                "stream_lambdas_are_capture_free",
            ][..],
        ),
        (
            "stream_shape.rs",
            include_str!("../src/stream_shape.rs"),
            &[
                "LoweredType",
                "stream_terminal_actual",
                "stream_collect_actual",
            ][..],
        ),
    ];

    for (module, source, forbidden) in cases {
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "{module} should not contain `{needle}`"
            );
        }
    }
}

#[test]
fn output_markers_are_not_public_field_bags() {
    let source = include_str!("../src/output_markers.rs");
    assert!(source.contains("pub struct OutputMarker"));
    let body = source
        .split("pub struct OutputMarker")
        .nth(1)
        .and_then(|tail| tail.split("impl OutputMarker").next())
        .expect("OutputMarker body should be present");
    for field in ["pub name:", "pub text:", "pub kind:"] {
        assert!(
            !body.contains(field),
            "OutputMarker should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(source.contains("pub const fn new("));
    assert!(source.contains("pub fn name(&self)"));
    assert!(source.contains("pub fn text(&self)"));
    assert!(source.contains("pub fn kind(&self)"));
}

#[test]
fn representation_boundary_policy_carriers_are_not_public_field_bags() {
    let source = include_str!("../src/representation.rs");
    let type_representation_source = source
        .split("pub struct TypeRepresentation")
        .nth(1)
        .and_then(|tail| tail.split("impl TypeRepresentation").next())
        .expect("TypeRepresentation should remain discoverable");
    for field in [
        "pub java_name: String",
        "pub representation: ObjectRepresentation",
        "pub provenance: RepresentationProvenance",
        "pub direct_subtypes: Vec<String>",
    ] {
        assert!(
            !type_representation_source.contains(field),
            "TypeRepresentation should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(source.contains("pub fn java_name(&self) -> &str"));
    assert!(source.contains("pub fn representation(&self) -> &ObjectRepresentation"));
    assert!(source.contains("pub fn provenance(&self) -> RepresentationProvenance"));
    assert!(source.contains("pub fn direct_subtypes(&self) -> &[String]"));

    let enum_path_source = source
        .split("pub struct JavaEnumVariantPath")
        .nth(1)
        .and_then(|tail| tail.split("impl JavaEnumVariantPath").next())
        .expect("JavaEnumVariantPath should remain discoverable");
    assert!(!enum_path_source.contains("pub enum_name: String"));
    assert!(!enum_path_source.contains("pub variant_name: String"));
    assert!(source.contains("pub fn enum_name(&self) -> &str"));
    assert!(source.contains("pub fn variant_name(&self) -> &str"));

    let boundary_policy_source = source
        .split("pub struct SourceSummarySuperclassBoundaryPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split("impl SourceSummarySuperclassBoundaryPolicy")
                .next()
        })
        .expect("SourceSummarySuperclassBoundaryPolicy should remain discoverable");
    assert!(!boundary_policy_source.contains("pub boundary_types: Vec<String>"));
    assert!(!boundary_policy_source.contains("pub facts: Vec<HierarchyOverride>"));
    assert!(source.contains("pub fn boundary_types(&self) -> &[String]"));
    assert!(source.contains("pub fn facts(&self) -> &[HierarchyOverride]"));
}

#[test]
fn functional_interface_shape_is_not_a_public_field_bag() {
    let source = include_str!("../src/stdlib.rs");
    let shape_source = source
        .split("pub struct FunctionalInterfaceShape")
        .nth(1)
        .and_then(|tail| tail.split("impl FunctionalInterfaceShape").next())
        .expect("FunctionalInterfaceShape should remain discoverable");
    assert!(!shape_source.contains("pub method: &'static str"));
    assert!(!shape_source.contains("pub inputs: Vec<FunctionalTypeSource>"));
    assert!(!shape_source.contains("pub returns: Option<FunctionalTypeSource>"));
    assert!(source.contains("pub fn method(&self) -> &'static str"));
    assert!(source.contains("pub fn inputs(&self) -> &[FunctionalTypeSource]"));
    assert!(source.contains("pub fn returns(&self) -> Option<FunctionalTypeSource>"));
}

#[test]
fn stdlib_external_api_method_policy_is_not_a_public_field_bag() {
    let source = include_str!("../src/stdlib.rs");
    let policy_source = source
        .split("pub struct ExternalApiMethodPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl ExternalApiMethodPolicy").next())
        .expect("ExternalApiMethodPolicy should remain discoverable");
    for field in [
        "pub role: ExternalApiRole",
        "pub receiver_mutability: ReceiverMutabilityFact",
        "pub return_nullability: NullabilityState",
        "pub provenance: FactProvenance",
    ] {
        assert!(
            !policy_source.contains(field),
            "ExternalApiMethodPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(source.contains("pub fn role(&self) -> ExternalApiRole"));
    assert!(source.contains("pub fn receiver_mutability(&self) -> ReceiverMutabilityFact"));
    assert!(source.contains("pub fn return_nullability(&self) -> NullabilityState"));
    assert!(source.contains("pub fn provenance(&self) -> FactProvenance"));
}

#[test]
fn type_helpers_live_outside_slot_shape() {
    let source = include_str!("../src/types.rs");
    let slot_shape_source = include_str!("../src/slot_shape.rs");

    assert!(source.contains("pub fn sequence_element_type"));
    assert!(source.contains("pub fn optional_element_type"));
    assert!(source.contains("pub fn list_java_type"));
    assert!(source.contains("pub fn optional_java_type"));
    assert!(!slot_shape_source.contains("pub fn list_java_type"));
    assert!(!slot_shape_source.contains("pub fn optional_java_type"));
}

#[test]
fn member_access_plan_stays_fact_and_shape_based() {
    let source = include_str!("../src/member_access_plan.rs");

    assert!(source.contains("infer_member_access_type"));
    assert!(source.contains("pub struct JavaMemberAccessPolicy"));
    assert!(source.contains("pub enum JavaMemberAccessKind"));
    let java_member_access_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct JavaMemberAccessPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl JavaMemberAccessPolicy").next())
        .expect("JavaMemberAccessPolicy source should remain discoverable");
    for field in [
        "pub member: String",
        "pub java_ty: JavaType",
        "pub kind: JavaMemberAccessKind",
    ] {
        assert!(
            !java_member_access_policy_source.contains(field),
            "JavaMemberAccessPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    let member_access_plan_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct MemberAccessPlan")
        .nth(1)
        .and_then(|tail| tail.split("impl MemberAccessPlan").next())
        .expect("MemberAccessPlan source should remain discoverable");
    for field in [
        "pub member: String",
        "pub java_ty: JavaType",
        "pub shape: RustSlotShape",
        "pub op: MemberAccessOp",
    ] {
        assert!(
            !member_access_plan_source.contains(field),
            "MemberAccessPlan should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(source.contains("pub fn member(&self) -> &str"));
    assert!(source.contains("pub fn java_ty(&self) -> &JavaType"));
    assert!(source.contains("pub fn kind(&self) -> JavaMemberAccessKind"));
    assert!(source.contains("pub fn shape(&self) -> &RustSlotShape"));
    assert!(source.contains("pub fn op(&self) -> &MemberAccessOp"));
    assert!(source.contains(
        "pub fn new(member: impl Into<String>, java_ty: JavaType, kind: JavaMemberAccessKind) -> Self"
    ));
    assert!(source.contains("Some(JavaMemberAccessPolicy::new("));
    assert!(source.contains("Some(MemberAccessPlan::new("));
    assert!(!source.contains("Some(JavaMemberAccessPolicy {"));
    assert!(!source.contains("Some(MemberAccessPlan {"));
    assert!(!source.contains("pub fn java_member_access_policy("));
    assert!(source.contains("pub fn java_member_access_policy_with_facts"));
    assert!(!source.contains("pub fn plan_member_access("));
    assert!(source.contains("pub fn plan_member_access_with_facts"));
    assert!(!source.contains("pub fn external_field_fact_for_owner"));
    assert!(!source.contains("pub fn external_field_type_for_owner"));
    assert!(source.contains("pub(crate) fn external_field_type_for_owner"));
    assert!(source.contains("let policy = java_member_access_policy_with_facts"));
    assert!(source.contains("project_java_value_shape"));
    assert!(!source.contains("builtin_external_library_packs"));
    assert!(!source.contains("PrimitiveType"));
    assert!(!source.contains("JavaType::Primitive"));
    assert!(!source.contains("RustSlotShape::conservative"));
    assert!(!source.contains("is_length_receiver"));
}

#[test]
fn unsupported_lowering_policy_stays_planned_before_lowering() {
    let source = include_str!("../src/unsupported.rs");
    let lower_source = include_str!("../src/lower.rs");
    let member_lowering_source = lower_source
        .split("let mut functions = Vec::new();")
        .nth(1)
        .expect("function collection setup should remain discoverable")
        .split("for member in &ty.members")
        .nth(1)
        .and_then(|tail| {
            tail.split("for inherited in crate::inherited_members")
                .next()
        })
        .expect("member lowering loop should remain discoverable");

    assert!(source.contains("pub struct UnsupportedDiagnosticPolicy"));
    assert!(source.contains("pub struct UnsupportedLoweringMessagePolicy"));
    let unsupported_message_policy_source = source
        .split(
            "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct UnsupportedLoweringMessagePolicy",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl UnsupportedLoweringMessagePolicy").next())
        .expect("unsupported lowering message policy source should remain discoverable");
    assert!(!unsupported_message_policy_source.contains("pub message: String"));
    assert!(
        source.contains("impl UnsupportedLoweringMessagePolicy")
            && source.contains("pub fn new(message: impl Into<String>)")
            && source.contains("pub fn message(&self) -> &str"),
        "unsupported lowering message policy should expose constructor/accessors, not raw fields"
    );
    let unsupported_diagnostic_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct UnsupportedDiagnosticPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl UnsupportedDiagnosticPolicy").next())
        .expect("unsupported diagnostic policy source should remain discoverable");
    assert!(!unsupported_diagnostic_policy_source.contains("pub code: &'static str"));
    assert!(!unsupported_diagnostic_policy_source.contains("pub message: String"));
    assert!(
        source.contains("impl UnsupportedDiagnosticPolicy")
            && source.contains("pub fn new(code: &'static str, message: impl Into<String>)")
            && source.contains("pub fn code(&self) -> &'static str")
            && source.contains("pub fn message(&self) -> &str"),
        "unsupported diagnostic policy should expose constructor/accessors, not raw fields"
    );
    assert!(source.contains("pub(crate) fn plan_unsupported_expression"));
    assert!(source.contains("pub(crate) fn plan_unsupported_member"));
    assert!(source.contains("pub(crate) fn plan_unsupported_statement"));
    assert!(!source.contains("pub fn plan_unsupported_expression"));
    assert!(!source.contains("pub fn plan_unsupported_member"));
    assert!(!source.contains("pub fn plan_unsupported_statement"));
    assert!(source.contains("pub(crate) fn unsupported_member_diagnostic_policy"));
    assert!(source.contains("pub(crate) fn unsupported_constructor_message_policy"));
    assert!(source.contains("pub(crate) fn unsupported_statement_message_policy"));
    assert!(source.contains("pub(crate) fn unsupported_expression_message_policy"));
    assert!(!source.contains("pub fn unsupported_member_diagnostic_policy"));
    assert!(!source.contains("pub fn unsupported_constructor_message_policy"));
    assert!(!source.contains("pub fn unsupported_statement_message_policy"));
    assert!(!source.contains("pub fn unsupported_expression_message_policy"));
    assert!(lower_source.contains("plan_unsupported_expression"));
    assert!(lower_source.contains("plan_unsupported_member"));
    assert!(lower_source.contains("plan_unsupported_statement"));
    assert!(lower_source.contains("unsupported_member_diagnostic_policy"));
    assert!(!lower_source.contains("UnsupportedExpressionLowering::Lambda"));
    assert!(!lower_source.contains("UnsupportedExpressionLowering::MethodReference"));
    assert!(!member_lowering_source.contains("UnsupportedMemberLowering::ParserUnsupported"));
    assert!(!member_lowering_source.contains("Member::Unsupported"));
    assert!(!lower_source.contains("\"unsupported_member\""));
    assert!(!lower_source.contains("UnsupportedStatementLowering::TryCatchFinally"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::LocalTypeDeclaration"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::ConstructorDelegation"));
    assert!(!lower_source.contains("UnsupportedStatementLowering::ParserUnsupported"));
}

#[test]
fn unsupported_compile_error_literals_are_centralized() {
    // `main.rs` reads marker text from `output_markers.rs`, and `stdlib.rs`
    // stubbed-constant emitters route through unsupported helper text.
    let allowed_files = ["src/unsupported.rs", "src/output_markers.rs"];
    for entry in std::fs::read_dir("src").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let rendered_path = path.to_string_lossy().replace('\\', "/");
        if allowed_files
            .iter()
            .any(|allowed| rendered_path.ends_with(allowed))
        {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("compile_error!("),
            "direct compile_error! producer should route through src/unsupported.rs: {rendered_path}"
        );
    }

    let unsupported_source = std::fs::read_to_string("src/unsupported.rs").unwrap();
    let compile_error_expr = unsupported_source
        .split("pub(crate) fn compile_error_expr")
        .nth(1)
        .unwrap()
        .split("/// A `compile_error!` at item position")
        .next()
        .unwrap();
    assert!(!unsupported_source.contains("pub fn compile_error_expr(message"));
    assert!(!unsupported_source.contains("pub fn compile_error_stmt(message"));
    assert!(!unsupported_source.contains("pub fn compile_error_expr_consuming(expr"));
    assert!(
        compile_error_expr.contains("RustExpr::MacroCall"),
        "expression-position compile_error! should use structured macro-call IR"
    );
    assert!(
        !compile_error_expr.contains("RustExpr::Raw(format!(\"compile_error!"),
        "expression-position compile_error! should not be a raw formatted expression"
    );
    assert!(
        !unsupported_source.contains("RustStmt::Raw(format!(\"compile_error!"),
        "statement-position compile_error! should use structured macro-call IR"
    );
    assert!(
        !unsupported_source
            .contains("RustExpr::Raw(format!(\n        \"{{ let _ = {}; compile_error!"),
        "compile_error expressions that consume a value should use structured block IR"
    );
    for helper in [
        "compile_error_item_text",
        "compile_error_value_text",
        "compile_error_diverging_text",
    ] {
        assert!(
            unsupported_source.contains(&format!("pub(crate) fn {helper}(")),
            "{helper} should stay available to internal rendered-text support"
        );
        assert!(
            !unsupported_source.contains(&format!("pub fn {helper}(")),
            "{helper} should not be public API"
        );
    }
    assert!(
        !unsupported_source.contains("RustExpr::Raw(format!(\"panic!"),
        "unsupported constructor diverging placeholders should use structured panic macro IR"
    );
}

#[test]
fn lowered_program_is_not_a_public_field_bag() {
    let lower_source = include_str!("../src/lower.rs");
    let lowered_program_source = lower_source
        .split("#[derive(Debug, Clone)]\npub struct LoweredProgram")
        .nth(1)
        .and_then(|tail| tail.split("impl LoweredProgram").next())
        .expect("LoweredProgram source should remain discoverable");

    assert!(!lowered_program_source.contains("pub file: RustFile"));
    assert!(!lowered_program_source.contains("pub diagnostics: DiagnosticBag"));
    assert!(lower_source.contains("pub fn file(&self) -> &RustFile"));
    assert!(lower_source.contains("pub fn diagnostics(&self) -> &DiagnosticBag"));
    assert!(lower_source.contains("pub fn diagnostics_mut(&mut self) -> &mut DiagnosticBag"));
    assert!(lower_source.contains("pub fn into_parts(self) -> (RustFile, DiagnosticBag)"));
}

#[test]
fn production_formatted_raw_constructors_stay_in_named_boundaries() {
    let stdlib_source = include_str!("../src/stdlib.rs");
    for helper in [
        "java_class_support_item",
        "java_class_support_item_with_classloader_shim",
        "java_floating_point_display_support_item",
        "java_comparable_support_item",
        "java_platform_facade_support_item",
        "java_string_split_support_item",
        "java_string_format_support_item",
    ] {
        assert!(
            stdlib_source.contains(&format!("pub(crate) fn {helper}(")),
            "{helper} should remain crate-local support-item implementation detail"
        );
        assert!(
            !stdlib_source.contains(&format!("pub fn {helper}(")),
            "{helper} should not expose raw support-item construction as public API"
        );
    }

    let allowed_files = [
        "src/javalibs.rs",                // handwritten support item text
        "src/representation_lowering.rs", // whole-item compatibility emission
        "src/stdlib.rs",                  // erased-object runtime support item text
    ];
    let mut direct_raw_format_sites = Vec::new();
    for entry in std::fs::read_dir("src").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let rendered_path = path.to_string_lossy().replace('\\', "/");
        if allowed_files
            .iter()
            .any(|allowed| rendered_path.ends_with(allowed))
        {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let production_source = production_source_without_cfg_test_items(&source);
        for (index, line) in production_source.lines().enumerate() {
            if line.contains("RustExpr::Raw(format!(")
                || line.contains("RustStmt::Raw(format!(")
                || line.contains("RustItem::Raw(format!(")
            {
                direct_raw_format_sites.push(format!(
                    "{rendered_path}:{}:{}",
                    index + 1,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        direct_raw_format_sites.is_empty(),
        "production formatted Raw constructors should route through named structured/raw-leaf boundaries:\n{}",
        direct_raw_format_sites.join("\n")
    );
}

#[test]
fn string_concat_layers_keep_semantics_and_rendering_separate() {
    let semantic_source = include_str!("../src/string_concat.rs");
    let plan_source = include_str!("../src/string_concat_plan.rs");
    let render_source = include_str!("../src/string_concat_render.rs");

    assert!(!semantic_source.contains("rust_ir"));
    assert!(!semantic_source.contains("emit::"));
    assert!(!semantic_source.contains("format!("));
    let concat_parts_source = semantic_source
        .split("pub(crate) struct StringConcatParts")
        .nth(1)
        .and_then(|tail| tail.split("impl<'a> StringConcatParts").next())
        .expect("StringConcatParts should remain discoverable");
    assert!(!concat_parts_source.contains("pub(crate) parts:"));
    assert!(semantic_source.contains("pub(crate) fn parts(&self) -> &[&'a Expr]"));
    assert!(semantic_source.contains("pub(crate) fn into_parts(self) -> Vec<&'a Expr>"));
    assert!(!plan_source.contains("rust_ir"));
    assert!(!plan_source.contains("emit::"));
    assert!(!plan_source.contains("format!("));
    assert!(!render_source.contains("analyze_string_concat"));
    assert!(!render_source.contains("build_string_concat_plan"));
    assert!(render_source.contains("macro_call_projection(\"format\""));
    assert!(render_source.contains("java_display_operand_projection"));
    assert!(!render_source.contains("pub fn render_string_concat"));
    assert!(!render_source.contains("render_java_display_operand"));
    assert!(!render_source.contains("strip_arithmetic_display_conversion"));
}

#[test]
fn conversion_plan_owns_stream_collect_actual_shape_selection_layering() {
    let stream_source = include_str!("../src/stream.rs");
    let coercion_source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");

    assert!(conversion_source.contains("collected_sequence_from_element_shape"));
    assert!(!coercion_source.contains("collected_sequence_from_element_shape"));
    assert!(!stream_source.contains("OwnershipShape"));
    assert!(!stream_source.contains("list_java_type"));
    assert!(stream_source.contains("pub struct StreamElement"));
    assert!(stream_source.contains("    shape: RustSlotShape"));
    assert!(!stream_source.contains("pub shape: RustSlotShape"));
    assert!(stream_source.contains("pub fn shape(&self) -> &RustSlotShape"));
    for field in [
        "pub source: StreamSource",
        "pub ops: Vec<StreamOp",
        "pub terminal: StreamTerminal",
        "pub source_element: StreamElement",
        "pub actual: LoweredType",
        "pub expr: &'a Expr",
        "pub optional_sequence: bool",
        "pub iteration: StreamSourceIteration",
        "pub range: Option<StreamSourceRange",
        "pub param_name: String",
        "pub rust_param_name: String",
        "pub body: StreamLambdaBody",
        "pub output_shape: RustSlotShape",
    ] {
        assert!(!stream_source.contains(field));
    }
    assert!(stream_source.contains("pub fn source(&self) -> &StreamSource"));
    assert!(stream_source.contains("pub fn ops(&self) -> &[StreamOp"));
    assert!(stream_source.contains("pub fn terminal(&self) -> &StreamTerminal"));
    assert!(stream_source.contains("use crate::conversion_plan::LoweredType"));
    assert!(stream_source.contains("LoweredType::collected_sequence_from_element_shape"));
}

#[test]
fn dependency_mode_config_is_not_a_public_field_bag() {
    let source = include_str!("../src/dependency_mode.rs");

    for (struct_name, forbidden_fields, required_accessors) in [
        (
            "DependencySourceRoot",
            &[
                "pub path: PathBuf",
                "pub mode: DependencyMode",
                "pub package_prefixes: Vec<String>",
            ][..],
            &[
                "pub fn path(&self) -> &Path",
                "pub fn mode(&self) -> DependencyMode",
                "pub fn package_prefixes(&self) -> &[String]",
            ][..],
        ),
        (
            "DependencyFactProvider",
            &[
                "pub name: String",
                "pub mode: DependencyMode",
                "pub facts: FactSet",
            ][..],
            &[
                "pub fn name(&self) -> &str",
                "pub fn mode(&self) -> DependencyMode",
                "pub fn facts(&self) -> &FactSet",
            ][..],
        ),
        (
            "DependencyModeConfig",
            &[
                "pub source_roots: Vec<DependencySourceRoot>",
                "pub fact_providers: Vec<DependencyFactProvider>",
                "pub enable_builtin_external_packs: bool",
            ][..],
            &[
                "pub fn source_roots(&self) -> &[DependencySourceRoot]",
                "pub fn fact_providers(&self) -> &[DependencyFactProvider]",
                "pub fn enable_builtin_external_packs(&self) -> bool",
            ][..],
        ),
    ] {
        let struct_source = source
            .split(&format!("pub struct {struct_name}"))
            .nth(1)
            .and_then(|tail| tail.split("impl").next())
            .unwrap_or_else(|| panic!("{struct_name} source should remain discoverable"));
        for field in forbidden_fields {
            assert!(
                !struct_source.contains(field),
                "{struct_name} should not expose `{field}`"
            );
        }
        for accessor in required_accessors {
            assert!(
                source.contains(accessor),
                "{struct_name} should expose `{accessor}`"
            );
        }
    }
}

#[test]
fn aliasing_uses_binding_fact_view_instead_of_legacy_name_map() {
    let full_source = include_str!("../src/aliasing.rs");
    let source = full_source
        .split("#[cfg(test)]")
        .next()
        .expect("production aliasing source should be present");

    assert!(
        !source.contains(".bindings.get("),
        "alias analysis should consume all_binding_facts() or binding ids instead of the legacy name map"
    );
    assert!(
        !source.contains("fn observable_aliases(program: &Program, usage: &UsageFacts)"),
        "the no-facts observable-alias convenience wrapper should stay test-only"
    );
    assert!(
        full_source.contains("fn observable_aliases_with_facts"),
        "production alias diagnostics should use the fact-aware collector"
    );
    let observable_alias_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub(crate) struct ObservableAlias")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(Debug, Clone, PartialEq, Eq)]").next())
        .expect("ObservableAlias should remain discoverable");
    for field in [
        "pub(crate) callable: String",
        "pub(crate) original: String",
        "pub(crate) alias: String",
        "pub(crate) mutated_through: String",
        "pub(crate) observed_through: String",
        "pub(crate) mutation: MutationKind",
    ] {
        assert!(!observable_alias_source.contains(field));
    }
    for accessor in [
        "pub(crate) fn callable(&self) -> &str",
        "pub(crate) fn original(&self) -> &str",
        "pub(crate) fn alias(&self) -> &str",
        "pub(crate) fn mutated_through(&self) -> &str",
        "pub(crate) fn observed_through(&self) -> &str",
        "pub(crate) fn mutation(&self) -> &MutationKind",
    ] {
        assert!(source.contains(accessor));
    }
}

#[test]
fn legacy_usage_binding_name_map_is_owned_by_usage_module() {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden_usage_fact_accesses = [
        "method_usage.bindings.",
        "constructor_usage.bindings.",
        "usage.bindings.",
        "facts.bindings.",
        "method_usage.bindings_by_id.",
        "constructor_usage.bindings_by_id.",
        "usage.bindings_by_id.",
        "facts.bindings_by_id.",
    ];
    for entry in std::fs::read_dir(&src_dir).expect("src directory should be readable") {
        let path = entry.expect("src entry should be readable").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs")
            || path.file_name().and_then(|name| name.to_str()) == Some("usage.rs")
        {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("source file should be readable");
        let production_source = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source should be present");
        if !production_source.contains("MethodUsageFacts") {
            continue;
        }
        for needle in forbidden_usage_fact_accesses {
            assert!(
                !production_source.contains(needle),
                "{} must use MethodUsageFacts accessors or all_binding_facts() instead of touching {needle}",
                path.display()
            );
        }
    }

    for (path, source, allowed_direct_mutations) in [
        (
            "src/aliasing.rs",
            include_str!("../src/aliasing.rs"),
            0usize,
        ),
        (
            "src/callable_analysis.rs",
            include_str!("../src/callable_analysis.rs"),
            0,
        ),
        (
            "src/object_model.rs",
            include_str!("../src/object_model.rs"),
            0,
        ),
        ("src/signature.rs", include_str!("../src/signature.rs"), 0),
        (
            "src/slot_policy.rs",
            include_str!("../src/slot_policy.rs"),
            0,
        ),
    ] {
        let direct_binding_map_mutations = [
            "method_usage.bindings.insert(",
            "constructor_usage.bindings.insert(",
            "usage.bindings.insert(",
            "facts.bindings.insert(",
            "method_usage.bindings_by_id.insert(",
            "constructor_usage.bindings_by_id.insert(",
            "usage.bindings_by_id.insert(",
            "facts.bindings_by_id.insert(",
        ]
        .iter()
        .map(|needle| source.matches(needle).count())
        .sum::<usize>();
        assert_eq!(
            direct_binding_map_mutations, allowed_direct_mutations,
            "{path} should build test MethodUsageFacts through MethodUsageFacts helpers instead of mutating legacy maps directly"
        );
    }

    let slot_shape_source = include_str!("slot_shape_design.rs");
    let usage_source = include_str!("../src/usage.rs");
    assert!(
        !slot_shape_source.contains("binding_facts_for_name")
            && !slot_shape_source.contains("unique_binding_facts_for_name")
            && !slot_shape_source.contains("contains_binding_id")
            && !slot_shape_source.contains("contains_binding_name"),
        "slot-shape test helpers should name id-less source-name compatibility explicitly and use exact-id accessors directly"
    );
    assert!(
        !usage_source.contains("from_binding_facts"),
        "MethodUsageFacts constructors should name exact-id, id-less compatibility, or visible-binding fixture intent explicitly"
    );
}

#[test]
fn usage_receiver_type_fallback_does_not_scan_all_binding_history_by_name() {
    let source = include_str!("../src/usage.rs")
        .split("fn usage_java_type_name")
        .next()
        .expect("usage receiver type helper should be present");
    let helper = source
        .split("fn agreed_binding_java_type")
        .nth(1)
        .expect("agreed binding type helper should be present");

    assert!(
        !helper.contains(".all_binding_facts()"),
        "receiver type recovery must not infer a local from non-visible same-name binding history"
    );
    assert!(
        helper.contains("idless_compatibility_binding_java_type_for_name(facts, name)"),
        "receiver type recovery should name the id-less compatibility fallback explicitly"
    );
    assert!(
        source.contains("fn idless_compatibility_binding_java_type_for_name"),
        "usage should keep expression-name compatibility fallback behind a named helper"
    );
}

#[test]
fn representation_plan_keeps_rust_item_rendering_out_of_plan_layer() {
    let source = include_str!("../src/representation.rs");

    assert!(!source.contains("RepresentationTypeEmission"));
    assert!(!source.contains("RawRustItem"));
    assert!(!source.contains("pub trait"));
    assert!(!source.contains("Box<dyn std::any::Any>"));
    assert!(source.contains("ObjectRepresentation::ErasedObject"));
}

#[test]
fn object_representation_policy_source_boundaries_stay_in_object_model() {
    let representation_source = include_str!("../src/representation.rs");
    let object_model_source = include_str!("../src/object_model.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");
    let facts_source = include_str!("../src/facts.rs");
    let shape_projection_source = include_str!("../src/shape_projection.rs");

    assert!(effective_facts_source.contains("pub struct EffectiveFacts"));
    assert!(effective_facts_source.contains("pub fn object_safe_trait_fact_for_name"));
    assert!(effective_facts_source.contains("pub fn closed_hierarchy_variants_for"));
    assert!(effective_facts_source.contains("pub fn dispatch_requirements_for"));
    assert!(object_model_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(object_model_source.contains("pub struct ObjectModelInput"));
    let object_model_input_source = object_model_source
        .split("#[derive(Debug, Clone, Copy)]\npub struct ObjectModelInput")
        .nth(1)
        .and_then(|tail| tail.split("impl<'a> ObjectModelInput").next())
        .expect("ObjectModelInput source should remain discoverable");
    for field in [
        "pub program: &'a Program",
        "pub facts: &'a FactSet",
        "pub effective_facts: &'a EffectiveFacts<'a>",
        "pub usage: Option<&'a UsageFacts>",
    ] {
        assert!(
            !object_model_input_source.contains(field),
            "ObjectModelInput should expose constructor/accessors, not raw public fields: {field}"
        );
    }
    assert!(object_model_source.contains("pub fn new(\n        program: &'a Program"));
    assert!(object_model_source.contains("pub fn program(&self) -> &'a Program"));
    assert!(object_model_source.contains("pub fn facts(&self) -> &'a FactSet"));
    assert!(
        object_model_source.contains("pub fn effective_facts(&self) -> &'a EffectiveFacts<'a>")
    );
    assert!(object_model_source.contains("pub fn usage(&self) -> Option<&'a UsageFacts>"));
    assert!(object_model_source.contains("pub fn plan_object_model_from_input"));
    assert!(object_model_source.contains("plan_object_model_with_effective_facts"));
    assert!(representation_source.contains("ObjectModelInput"));
    assert!(representation_source.contains("plan_object_model_from_input"));
    assert!(representation_source.contains("ObjectModelInput::new("));
    let object_model_plan_source = object_model_source
        .split("pub struct ObjectModelPlan")
        .nth(1)
        .and_then(|tail| tail.split("impl ObjectModelPlan").next())
        .expect("ObjectModelPlan source should remain discoverable");
    assert!(!object_model_plan_source.contains("pub strategies:"));
    assert!(!object_model_plan_source.contains("pub hierarchy_edges:"));
    assert!(!object_model_plan_source.contains("pub diagnostics:"));
    assert!(object_model_source.contains("pub fn strategies(&self)"));
    assert!(object_model_source.contains("pub fn hierarchy_edges(&self)"));
    assert!(object_model_source.contains("pub fn diagnostics(&self)"));
    assert!(object_model_source.contains("pub fn into_parts("));
    assert!(object_model_source.contains("pub fn strategy_for(&self, root: &str)"));
    let object_strategy_plan_source = object_model_source
        .split("pub struct ObjectStrategyPlan")
        .nth(1)
        .and_then(|tail| tail.split("impl ObjectStrategyPlan").next())
        .expect("ObjectStrategyPlan source should remain discoverable");
    for field in [
        "pub root:",
        "pub strategy:",
        "pub requirements:",
        "pub direct_subtypes:",
        "pub source_methods_access_instance_fields:",
        "pub rejected:",
        "pub provenance:",
        "pub diagnostics:",
    ] {
        assert!(
            !object_strategy_plan_source.contains(field),
            "ObjectStrategyPlan should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(object_model_source.contains("pub fn provenance(&self) -> &[FactProvenance]"));
    assert!(object_model_source.contains("pub fn primary_provenance(&self)"));
    assert!(object_model_source.contains("fn add_provenance(&mut self"));
    assert!(object_model_source.contains("pub enum ObjectRepresentationRequest"));
    assert!(facts_source.contains("pub enum DispatchRequirementKind"));
    assert!(facts_source.contains("pub struct DispatchRequirementFact"));
    assert!(object_model_source.contains("pub type DispatchRequirement"));
    assert!(object_model_source.contains("dispatch_requirements_for"));
    assert!(object_model_source.contains("pub struct RepresentationLookupPolicy"));
    assert!(object_model_source.contains("pub enum RepresentationLookupFallback"));
    assert!(object_model_source.contains("pub fn object_representation_lookup_policy"));
    let representation_lookup_policy_source = object_model_source
        .split("pub struct RepresentationLookupPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl RepresentationLookupPolicy").next())
        .expect("RepresentationLookupPolicy source should remain discoverable");
    for field in [
        "pub java_ty: JavaType",
        "pub fallback: RepresentationLookupFallback",
        "pub representation: ObjectRepresentation",
    ] {
        assert!(
            !representation_lookup_policy_source.contains(field),
            "RepresentationLookupPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(object_model_source.contains("pub fn fallback(&self) -> RepresentationLookupFallback"));
    assert!(object_model_source.contains("pub fn representation(&self) -> &ObjectRepresentation"));
    assert!(!representation_source.contains("pub struct RepresentationLookupPolicy"));
    assert!(!representation_source.contains("pub enum RepresentationLookupFallback"));
    assert!(representation_source.contains("object_representation_lookup_policy"));
    assert!(representation_source.contains("pub struct RepresentationDispatchPolicy"));
    assert!(representation_source.contains("pub fn representation_dispatch_policy"));
    let representation_dispatch_policy_source = representation_source
        .split("pub struct RepresentationDispatchPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl RepresentationDispatchPolicy").next())
        .expect("RepresentationDispatchPolicy source should remain discoverable");
    assert!(
        !representation_dispatch_policy_source.contains("pub representation: ObjectRepresentation")
    );
    assert!(!representation_dispatch_policy_source.contains("pub dispatch: DispatchShape"));
    assert!(
        representation_source.contains("pub fn representation(&self) -> &ObjectRepresentation")
    );
    assert!(representation_source.contains("pub fn dispatch(&self) -> &DispatchShape"));
    assert!(representation_source.contains("pub fn into_dispatch(self) -> DispatchShape"));
    assert!(representation_source.contains("representation_lookup_policy"));
    assert!(object_model_source.contains("ConservativeConcrete"));
    assert!(object_model_source.contains("ConservativeErasedObject"));
    assert!(object_model_source.contains("pub enum ObjectSafetyPolicy"));
    assert!(object_model_source.contains("pub struct ObjectSafetyFactPolicy"));
    let object_safety_fact_policy_source = object_model_source
        .split("pub struct ObjectSafetyFactPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl ObjectSafetyFactPolicy").next())
        .expect("ObjectSafetyFactPolicy source should remain discoverable");
    for field in [
        "pub type_name: String",
        "pub object_safe_trait: Option<bool>",
        "pub violation: Option<ObjectSafetyViolation>",
    ] {
        assert!(
            !object_safety_fact_policy_source.contains(field),
            "ObjectSafetyFactPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(!object_model_source.contains("pub fn object_safety_fact_policy("));
    assert!(object_model_source.contains("pub fn object_safety_fact_policy_with_effective_facts"));
    assert!(object_model_source.contains("object_safety_policy_for_request"));
    assert_eq!(
        object_model_source
            .matches("object_safe_trait_fact_for_name")
            .count(),
        1
    );
    assert_eq!(
        object_model_source
            .matches("object_safety_violation_for_name")
            .count(),
        1
    );
    assert!(object_model_source.contains("ErasedObjectOverride"));
    assert!(object_model_source.contains("object_representation_request_requires_object_safety"));
    assert!(!object_model_source.contains("has_explicit_hierarchy_override"));
    assert!(object_model_source.contains("pub struct HierarchyOverrideValidationPolicy"));
    assert!(object_model_source.contains("pub fn hierarchy_override_validation_policy"));
    assert!(object_model_source.contains("pub struct HierarchyOverrideApplicationPolicy"));
    assert!(!object_model_source.contains("pub fn hierarchy_override_application_policy("));
    assert!(
        object_model_source
            .contains("pub fn hierarchy_override_application_policy_with_effective_facts")
    );
    assert!(object_model_source.contains("pub struct HierarchyOverrideRepresentationPolicy"));
    assert!(!object_model_source.contains("pub fn hierarchy_override_representation_policy("));
    assert!(
        object_model_source
            .contains("pub fn hierarchy_override_representation_policy_with_effective_facts")
    );
    let override_validation_policy_source = object_model_source
        .split("pub struct HierarchyOverrideValidationPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl HierarchyOverrideValidationPolicy").next())
        .expect("HierarchyOverrideValidationPolicy source should remain discoverable");
    assert!(!override_validation_policy_source.contains("pub type_name: String"));
    assert!(
        !override_validation_policy_source
            .contains("pub diagnostics: Vec<RepresentationDiagnostic>")
    );
    let override_application_policy_source = object_model_source
        .split("pub struct HierarchyOverrideApplicationPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl HierarchyOverrideApplicationPolicy").next())
        .expect("HierarchyOverrideApplicationPolicy source should remain discoverable");
    for field in [
        "pub type_name: String",
        "pub representation: Option<ObjectRepresentation>",
        "pub provenance: FactProvenance",
        "pub direct_subtypes: Vec<String>",
        "pub diagnostics: Vec<RepresentationDiagnostic>",
    ] {
        assert!(
            !override_application_policy_source.contains(field),
            "HierarchyOverrideApplicationPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    let override_representation_policy_source = object_model_source
        .split("pub struct HierarchyOverrideRepresentationPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split("impl HierarchyOverrideRepresentationPolicy")
                .next()
        })
        .expect("HierarchyOverrideRepresentationPolicy source should remain discoverable");
    for field in [
        "pub representation: ObjectRepresentation",
        "pub object_safety: ObjectSafetyPolicy",
        "pub diagnostics: Vec<RepresentationDiagnostic>",
    ] {
        assert!(
            !override_representation_policy_source.contains(field),
            "HierarchyOverrideRepresentationPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(!representation_source.contains("pub struct RepresentationHierarchyFactPolicy"));
    assert!(!representation_source.contains("representation_hierarchy_fact_policy"));
    assert!(object_model_source.contains("pub struct ObjectHierarchyEdge"));
    let object_hierarchy_edge_source = object_model_source
        .split("pub struct ObjectHierarchyEdge")
        .nth(1)
        .and_then(|tail| tail.split("impl ObjectHierarchyEdge").next())
        .expect("ObjectHierarchyEdge source should remain discoverable");
    for field in [
        "pub super_name: String",
        "pub sub_name: String",
        "pub kind: ObjectHierarchyEdgeKind",
    ] {
        assert!(
            !object_hierarchy_edge_source.contains(field),
            "ObjectHierarchyEdge should expose constructor/accessors, not raw public fields: {field}"
        );
    }
    assert!(object_model_source.contains("pub fn new(\n        super_name:"));
    assert!(object_model_source.contains("pub fn super_name(&self) -> &str"));
    assert!(object_model_source.contains("pub fn sub_name(&self) -> &str"));
    assert!(object_model_source.contains("pub fn kind(&self) -> ObjectHierarchyEdgeKind"));
    assert!(object_model_source.contains("pub enum ObjectHierarchyEdgeKind"));
    assert!(object_model_source.contains("pub struct ObjectHierarchyEdgeRepresentationPolicy"));
    assert!(object_model_source.contains("pub fn object_hierarchy_edge_representation_policy"));
    let edge_representation_policy_source = object_model_source
        .split("pub struct ObjectHierarchyEdgeRepresentationPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split("impl ObjectHierarchyEdgeRepresentationPolicy")
                .next()
        })
        .expect("ObjectHierarchyEdgeRepresentationPolicy source should remain discoverable");
    assert!(
        !edge_representation_policy_source.contains("pub representation: ObjectRepresentation")
    );
    assert!(!edge_representation_policy_source.contains("pub represented: bool"));
    assert!(object_model_source.contains("pub fn represented(&self) -> bool"));
    assert!(object_model_source.contains("hierarchy_edges: Vec<ObjectHierarchyEdge>"));
    assert!(representation_source.contains("let (strategies, hierarchy_edges, diagnostics)"));
    assert!(representation_source.contains("for edge in hierarchy_edges"));
    assert!(representation_source.contains("object_hierarchy_edge_representation_policy"));
    assert!(!representation_source.contains("fn collect_hierarchy_edges("));
    assert!(!representation_source.contains("ObjectRepresentation::ClosedEnum { .. }"));
    assert!(effective_facts_source.contains("pub fn extends_type_for"));
    assert!(effective_facts_source.contains("pub fn implemented_types_for"));
    assert!(effective_facts_source.contains("pub fn hierarchy_edges_for_type"));
    assert_eq!(
        representation_source
            .matches("hierarchy_edges_for_type")
            .count(),
        0
    );
    assert!(!object_model_source.contains("pub struct ClosedEnumFactPolicy"));
    assert!(!object_model_source.contains("pub fn closed_enum_fact_policy"));
    assert!(object_model_source.contains("pub struct ClosedEnumInferencePolicy"));
    assert!(!object_model_source.contains("pub fn closed_enum_inference_policy("));
    assert!(
        object_model_source.contains("pub fn closed_enum_inference_policy_with_effective_facts")
    );
    assert!(object_model_source.contains("pub enum ClosedEnumProof"));
    assert!(!object_model_source.contains("pub fn closed_enum_proof_policy("));
    assert!(object_model_source.contains("pub fn closed_enum_proof_policy_with_effective_facts"));
    assert!(object_model_source.contains("MissingPermittedSubtypes"));
    let closed_enum_inference_policy_source = object_model_source
        .split("pub struct ClosedEnumInferencePolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl ClosedEnumInferencePolicy").next())
        .expect("ClosedEnumInferencePolicy source should remain discoverable");
    for field in [
        "pub type_name: String",
        "pub representation: Option<ObjectRepresentation>",
        "pub provenance: Option<FactProvenance>",
    ] {
        assert!(
            !closed_enum_inference_policy_source.contains(field),
            "ClosedEnumInferencePolicy should expose accessors, not raw public fields: {field}"
        );
    }
    let closed_enum_proof_policy_source = object_model_source
        .split("pub struct ClosedEnumProofPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl ClosedEnumProofPolicy").next())
        .expect("ClosedEnumProofPolicy source should remain discoverable");
    assert!(!closed_enum_proof_policy_source.contains("pub type_name: String"));
    assert!(!closed_enum_proof_policy_source.contains("pub proof: ClosedEnumProof"));
    assert!(facts_source.contains("pub struct SealedPermitsFact"));
    assert!(effective_facts_source.contains("sealed_permitted_subtypes_for"));
    assert!(!representation_source.contains("fn validate_hierarchy_override"));
    assert!(object_model_source.contains("pub struct ObjectHierarchyEdgeDiagnosticInput"));
    assert!(object_model_source.contains("pub struct HierarchyEdgeDiagnosticPolicy"));
    assert!(object_model_source.contains("pub enum HierarchyEdgeDiagnosticOutcome"));
    assert!(object_model_source.contains("pub fn object_hierarchy_edge_diagnostic_policy"));
    let edge_diagnostic_input_source = object_model_source
        .split("pub struct ObjectHierarchyEdgeDiagnosticInput")
        .nth(1)
        .and_then(|tail| tail.split("impl ObjectHierarchyEdgeDiagnosticInput").next())
        .expect("ObjectHierarchyEdgeDiagnosticInput source should remain discoverable");
    for field in [
        "pub type_name: String",
        "pub declared_extends: Option<String>",
        "pub declared_implements: Vec<String>",
        "pub edges: Vec<ObjectHierarchyEdge>",
        "pub represented_super_names: BTreeSet<String>",
    ] {
        assert!(
            !edge_diagnostic_input_source.contains(field),
            "ObjectHierarchyEdgeDiagnosticInput should use a constructor, not raw public fields: {field}"
        );
    }
    let edge_diagnostic_policy_source = object_model_source
        .split("pub struct HierarchyEdgeDiagnosticPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl HierarchyEdgeDiagnosticPolicy").next())
        .expect("HierarchyEdgeDiagnosticPolicy source should remain discoverable");
    for field in [
        "pub type_name: String",
        "pub outcomes: Vec<HierarchyEdgeDiagnosticOutcome>",
        "pub diagnostics: Vec<RepresentationDiagnostic>",
    ] {
        assert!(
            !edge_diagnostic_policy_source.contains(field),
            "HierarchyEdgeDiagnosticPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    let representation_diagnostic_source = object_model_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct RepresentationDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl RepresentationDiagnostic").next())
        .expect("object_model::RepresentationDiagnostic struct source should be present");
    assert!(!representation_diagnostic_source.contains("pub code: &'static str"));
    assert!(!representation_diagnostic_source.contains("pub message: String"));
    assert!(
        object_model_source.contains("impl RepresentationDiagnostic")
            && object_model_source
                .contains("pub fn new(code: &'static str, message: impl Into<String>)")
            && object_model_source.contains("pub fn code(&self) -> &'static str")
            && object_model_source.contains("pub fn message(&self) -> &str"),
        "representation diagnostics should expose constructor/accessors, not raw fields"
    );
    assert!(representation_source.contains("ObjectHierarchyEdgeDiagnosticInput"));
    assert!(representation_source.contains("pub fn hierarchy_edge_diagnostic_policy"));
    assert!(representation_source.contains("object_hierarchy_edge_diagnostic_policy"));
    assert!(!representation_source.contains("pub struct HierarchyEdgeDiagnosticPolicy"));
    assert!(!representation_source.contains("pub enum HierarchyEdgeDiagnosticOutcome"));
    assert!(!representation_source.contains("render_hierarchy_edge_diagnostic_outcome"));
    assert!(!representation_source.contains("pub struct AnonymousClassDiagnosticPolicy"));
    assert!(!representation_source.contains("pub fn anonymous_class_diagnostic_policy"));
    assert!(!object_model_source.contains("pub fn object_representation_policy("));
    assert!(
        object_model_source.contains("pub fn object_representation_policy_with_effective_facts")
    );
    let object_representation_policy_source = object_model_source
        .split("pub struct ObjectRepresentationPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl ObjectRepresentationPolicy").next())
        .expect("ObjectRepresentationPolicy source should remain discoverable");
    for field in [
        "pub object_safety: ObjectSafetyPolicy",
        "pub representation: ObjectRepresentation",
        "pub diagnostics: Vec<RepresentationDiagnostic>",
    ] {
        assert!(
            !object_representation_policy_source.contains(field),
            "ObjectRepresentationPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(!facts_source.contains("HierarchyStrategy::BoxedDynTrait"));
    assert!(!facts_source.contains("BoxedDynTrait {"));
    assert!(!object_model_source.contains("ObjectRepresentation::BoxedDynTrait"));
    assert!(!object_model_source.contains("ObjectRepresentationRequest::BoxedDynTraitOverride"));
    assert!(shape_projection_source.contains("DispatchShape::BoxedDynTrait"));
    assert!(!representation_source.contains("interface_representation_from_facts"));
    assert!(!representation_source.contains("dyn_trait_representation_from_facts"));
    assert!(!representation_source.contains("boxed_dyn_trait_representation_from_facts"));
}

#[test]
fn switch_plan_keeps_rust_rendering_out_of_plan_layer() {
    let source = include_str!("../src/switch_plan.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("RustStmt"));
    assert!(!source.contains("emit::"));
    assert!(!source.contains("emit_expr"));
    assert!(!source.contains("emit_statements"));
    assert!(!source.contains("render_switch"));
    assert!(!source.contains("switch_case_condition"));
    assert!(!include_str!("../src/switch_lowering.rs").contains("pub label_condition:"));
    assert!(!include_str!("../src/switch_lowering.rs").contains("pub statements: Vec<RustStmt>"));
    assert!(!source.contains("pub selector:"));
    assert!(!source.contains("pub cases:"));
    assert!(!source.contains("pub break_label:"));
    assert!(!source.contains("pub value_binding:"));
    assert!(!source.contains("pub matched_binding:"));
    assert!(!source.contains("pub(crate) labels:"));
    assert!(!source.contains("pub return_expr:"));
    assert!(source.contains("pub fn plan_simple_return_switch"));
    assert!(source.contains("pub(crate) fn plan_simple_return_switch_case"));
    assert!(!source.contains("pub fn plan_simple_return_switch_case"));
}

#[test]
fn shape_axis_free_lowering_layers_do_not_choose_shape_axes() {
    let lowering_sources = [
        include_str!("../src/access_lowering.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production access_lowering source should be present"),
        include_str!("../src/call_lowering.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production call_lowering source should be present"),
        include_str!("../src/conversion_lowering.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production conversion_lowering source should be present"),
        include_str!("../src/exception_lowering.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production exception_lowering source should be present"),
        include_str!("../src/literal_lowering.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production literal_lowering source should be present"),
        include_str!("../src/loop_lowering.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production loop_lowering source should be present"),
        include_str!("../src/string_concat_render.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production string_concat_render source should be present"),
        include_str!("../src/switch_lowering.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production switch_lowering source should be present"),
    ];

    for source in lowering_sources {
        assert!(!source.contains("OwnershipShape"));
        assert!(!source.contains("ViewShape"));
        assert!(!source.contains("DispatchShape"));
        assert!(!source.contains("MutabilityShape"));
        assert!(!source.contains("RustSlotShape::conservative"));
        assert!(!source.contains("RustSlotShape::for_storage"));
        assert!(!source.contains("ObjectRepresentation"));
        assert!(!source.contains("dispatch_for"));
        assert!(!source.contains("representation_for"));
    }

    let lower_source = include_str!("../src/lower.rs");
    assert!(lower_source.contains("OwnershipShape"));
    assert!(lower_source.contains("ViewShape"));
    assert!(
        lower_source.contains("DispatchShape"),
        "lower.rs remains the tracked orchestrator cleanup for shape-axis decisions"
    );
    assert!(lower_source.contains("plan_slot_conversion"));
    assert!(lower_source.contains("plan_lowered_slot_conversion"));
    assert!(lower_source.contains("apply_slot_expr_conversion"));
    assert!(lower_source.contains("apply_lowered_slot_expr_conversion"));
    assert!(!lower_source.contains("coercion::apply_expr_to_slot_with_facts"));
    assert!(!lower_source.contains("coercion::apply_lowered_expr_to_slot_with_facts"));
}

#[test]
fn conversion_plan_keeps_rust_expression_rendering_out_of_plan_layer() {
    let source = include_str!("../src/conversion_plan.rs");
    let lowering_source = include_str!("../src/conversion_lowering.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("conversion lowering production source should be present");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("apply_expr_to_shape"));
    assert!(!source.contains("apply_instanceof_check"));
    assert!(source.contains("pub struct ShapeCoercionPlan"));
    assert!(source.contains("pub struct SyntaxCoercionPlan"));
    assert!(source.contains("pub enum InstanceOfCheck"));
    assert!(source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
    assert!(source.contains("pub struct CastConversionPlan"));
    let conversion_diagnostic_source = source
        .split("pub struct ConversionDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(Debug, Clone, PartialEq, Eq)]").next())
        .unwrap();
    assert!(conversion_diagnostic_source.contains("pub fn new("));
    assert!(conversion_diagnostic_source.contains("pub fn code(&self) -> &'static str"));
    assert!(conversion_diagnostic_source.contains("pub fn message(&self) -> &str"));
    for field in ["pub code: &'static str", "pub message: String"] {
        assert!(
            !conversion_diagnostic_source.contains(field),
            "ConversionDiagnostic should expose accessors, not raw public fields: {field}"
        );
    }
    let shape_coercion_plan_source = source
        .split("pub struct ShapeCoercionPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub struct SyntaxCoercionPlan").next())
        .unwrap();
    assert!(shape_coercion_plan_source.contains("pub fn new("));
    assert!(shape_coercion_plan_source.contains("pub fn actual(&self) -> &RustSlotShape"));
    assert!(shape_coercion_plan_source.contains("pub fn target(&self) -> &RustSlotShape"));
    assert!(
        shape_coercion_plan_source.contains("pub fn dispatch(&self) -> &DispatchCoercionPolicy")
    );
    assert!(shape_coercion_plan_source.contains("pub fn view(&self) -> &ViewCoercionPolicy"));
    assert!(
        shape_coercion_plan_source
            .contains("pub fn optional(&self) -> &OptionalShapeCoercionPolicy")
    );
    assert!(
        shape_coercion_plan_source.contains("pub fn ownership(&self) -> &OwnershipCoercionPolicy")
    );
    assert!(
        shape_coercion_plan_source
            .contains("pub fn primitive_widening(&self) -> Option<PrimitiveWideningPolicy>")
    );
    for field in [
        "pub actual: RustSlotShape",
        "pub target: RustSlotShape",
        "pub dispatch: DispatchCoercionPolicy",
        "pub view: ViewCoercionPolicy",
        "pub optional: OptionalShapeCoercionPolicy",
        "pub ownership: OwnershipCoercionPolicy",
        "pub primitive_widening: Option<PrimitiveWideningPolicy>",
    ] {
        assert!(
            !shape_coercion_plan_source.contains(field),
            "ShapeCoercionPlan should expose accessors, not raw public fields: {field}"
        );
    }
    let cast_conversion_plan_source = source
        .split("pub struct CastConversionPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub struct InstanceOfPlan").next())
        .unwrap();
    assert!(source.contains(
        "pub(crate) fn new(\n        actual: RustSlotShape,\n        target: RustSlotShape,\n        semantics: JavaCastSemanticsPolicy,\n        coercion: ShapeCoercionPlan,"
    ));
    assert!(source.contains("pub fn actual(&self) -> &RustSlotShape"));
    assert!(source.contains("pub fn target(&self) -> &RustSlotShape"));
    assert!(source.contains("pub fn semantics(&self) -> &JavaCastSemanticsPolicy"));
    assert!(source.contains("pub fn coercion(&self) -> &ShapeCoercionPlan"));
    assert!(!cast_conversion_plan_source.contains("pub actual: RustSlotShape"));
    assert!(!cast_conversion_plan_source.contains("pub target: RustSlotShape"));
    assert!(!cast_conversion_plan_source.contains("pub semantics: JavaCastSemanticsPolicy"));
    assert!(!cast_conversion_plan_source.contains("pub coercion: ShapeCoercionPlan"));
    assert!(source.contains("pub struct InstanceOfPlan"));
    let instanceof_plan_source = source
        .split("pub struct InstanceOfPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub struct SlotExprConversionPlan").next())
        .unwrap();
    assert!(instanceof_plan_source.contains("pub(crate) fn new("));
    assert!(instanceof_plan_source.contains("actual: RustSlotShape"));
    assert!(instanceof_plan_source.contains("target: RustSlotShape"));
    assert!(instanceof_plan_source.contains("check: InstanceOfCheck"));
    assert!(source.contains("pub fn check(&self) -> &InstanceOfCheck"));
    assert!(!instanceof_plan_source.contains("pub actual: RustSlotShape"));
    assert!(!instanceof_plan_source.contains("pub target: RustSlotShape"));
    assert!(!instanceof_plan_source.contains("pub check: InstanceOfCheck"));
    assert!(source.contains("pub enum LoweredType"));
    assert!(source.contains("pub enum SlotConversionPlan"));
    assert!(source.contains("pub struct SlotExprConversionPlan"));
    assert!(source.contains("pub struct LoweredSlotExprConversionPlan"));
    let slot_expr_conversion_plan_source = source
        .split("pub struct SlotExprConversionPlan")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub struct LoweredSlotExprConversionPlan")
                .next()
        })
        .unwrap();
    assert!(slot_expr_conversion_plan_source.contains("pub fn new("));
    assert!(slot_expr_conversion_plan_source.contains("pub fn actual(&self) -> &RustSlotShape"));
    assert!(slot_expr_conversion_plan_source.contains("pub fn slot(&self) -> &Slot"));
    assert!(
        slot_expr_conversion_plan_source
            .contains("pub fn conversion(&self) -> &SlotConversionPlan")
    );
    for field in [
        "pub actual: RustSlotShape",
        "pub slot: Slot",
        "pub conversion: SlotConversionPlan",
    ] {
        assert!(
            !slot_expr_conversion_plan_source.contains(field),
            "SlotExprConversionPlan should expose accessors, not raw public fields: {field}"
        );
    }
    let lowered_slot_expr_conversion_plan_source = source
        .split("pub struct LoweredSlotExprConversionPlan")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
                .next()
        })
        .unwrap();
    assert!(lowered_slot_expr_conversion_plan_source.contains("pub fn new("));
    assert!(
        lowered_slot_expr_conversion_plan_source.contains("pub fn actual(&self) -> &LoweredType")
    );
    assert!(lowered_slot_expr_conversion_plan_source.contains("pub fn slot(&self) -> &Slot"));
    assert!(
        lowered_slot_expr_conversion_plan_source
            .contains("pub fn conversion(&self) -> &SlotConversionPlan")
    );
    for field in [
        "pub actual: LoweredType",
        "pub slot: Slot",
        "pub conversion: SlotConversionPlan",
    ] {
        assert!(
            !lowered_slot_expr_conversion_plan_source.contains(field),
            "LoweredSlotExprConversionPlan should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(source.contains("pub fn actual_shape_for_slot_expr"));
    assert!(source.contains("pub fn plan_slot_conversion"));
    assert!(source.contains("pub fn plan_lowered_slot_conversion"));
    assert!(source.contains("shape_coercion_plan_with_facts"));
    assert!(source.contains("plan_instanceof_check"));
    assert!(source.contains("slot_conversion_plan_with_facts"));
    assert!(source.contains("lowered_slot_conversion_plan_with_facts"));
    assert!(lowering_source.contains("apply_shape_coercion_plan(expr, plan.coercion())"));
    assert!(lowering_source.contains("instanceof_check_projection(expr, plan.check())"));
    assert!(lowering_source.contains("pub(crate) fn apply_slot_expr_conversion"));
    assert!(lowering_source.contains("pub(crate) fn apply_lowered_slot_expr_conversion"));
    assert!(lowering_source.contains("pub(crate) fn apply_lowered_slot_conversion_to_expr"));
    assert!(!lowering_source.contains("pub fn apply_slot_expr_conversion"));
    assert!(!lowering_source.contains("pub fn apply_lowered_slot_expr_conversion"));
    assert!(!lowering_source.contains("pub fn apply_lowered_slot_conversion_to_expr"));
    assert!(!lowering_source.contains("FactSet"));
    assert!(!lowering_source.contains("plan_slot_conversion"));
    assert!(!lowering_source.contains("plan_lowered_slot_conversion"));
    assert!(!lowering_source.contains("slot_conversion_plan_with_facts"));
    assert!(!lowering_source.contains("lowered_slot_conversion_plan_with_facts"));
    assert!(!lowering_source.contains("apply_expr_to_shape_with_facts"));
    assert!(!lowering_source.contains("plan_instanceof_check"));

    let coercion_source = include_str!("../src/coercion.rs");
    assert!(!coercion_source.contains("pub enum LoweredType"));
    assert!(!coercion_source.contains("pub fn actual_shape_for_slot_expr"));
}

#[test]
fn item_assembly_return_type_wrappers_delegate_to_fact_aware_rendering() {
    let source = include_str!("../src/item_assembly.rs");

    assert!(source.contains("fn method_return_type_with_facts"));
    assert!(!source.contains("pub fn method_return_type_with_facts"));
    assert!(!source.contains("pub fn method_return_type("));
    assert!(!source.contains("rust_type_for_slot(&shape)"));
    assert!(!source.contains("fn raw_rust_expr_mentions_identifier"));
    assert!(!source.contains("fn valid_raw_move_identifier"));
    assert!(source.contains("recover_parseable_raw_operand_projection"));
    assert!(source.contains("fn method_function_shell_with_facts_and_exception_plan"));
    assert!(!source.contains("pub fn method_function_shell_with_facts_and_exception_plan"));
}

#[test]
fn lower_orchestrator_delegates_static_storage_defaults_to_item_assembly() {
    let source = include_str!("../src/lower.rs");
    let item_assembly_source = include_str!("../src/item_assembly.rs");
    let static_initializer = source
        .split("fn static_storage_initial_value_expr")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn static_storage_array_initial_value_expr")
                .next()
        })
        .expect("static storage initializer should be present");

    assert!(!static_initializer.contains("default_expr_for_slot"));
    assert!(!static_initializer.contains("slot_render"));
    assert!(static_initializer.contains("static_storage_initial_fallback_value"));
    assert!(item_assembly_source.contains("pub(crate) fn static_storage_initial_fallback_value"));
}

#[test]
fn typed_external_argument_effects_preserve_concrete_source_parameter_shape() {
    let signature_source = include_str!("../src/signature.rs");
    let helper = signature_source
        .split("fn external_argument_effect_constraints_for_binding")
        .nth(1)
        .and_then(|tail| tail.split("fn external_ffi_handover_for_call_arg").next())
        .expect("external argument effect binding helper should be present");

    assert!(helper.contains("java_types_have_same_signature_boundary_identity"));
    assert!(helper.contains("SignatureConstraint::RequiredDispatch(DispatchShape::Concrete)"));
    assert!(helper.contains("SignatureConstraint::RequiredView(ViewShape::Native)"));
    assert!(helper.contains("!is_declared_java_object_type"));
}

#[test]
fn resolved_unknown_source_calls_can_propagate_signature_constraints() {
    let signature_source = include_str!("../src/signature.rs");
    let propagation = signature_source
        .split("fn propagate_call_site_constraints")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn propagate_constructor_call_site_constraints")
                .next()
        })
        .expect("method call-site propagation should be present");

    assert!(propagation.contains("let Some(resolved) = call_site.resolved() else"));
    assert!(propagation.contains("constraints.methods.get(&callee_key)"));
    assert!(propagation.contains("resolved_source_call_propagates_signature_constraints"));
    assert!(propagation.contains("CallArgumentBoundary::Unknown"));
}

#[test]
fn signature_planning_keeps_declared_iterator_locals_native() {
    let signature_source = include_str!("../src/signature.rs");
    assert!(signature_source.contains("declared_local_jdk_iterator_constraints"));
    assert!(signature_source.contains("\"Iterator\" | \"java.util.Iterator\" | \"JavaIterator\""));
}

#[test]
fn signature_capture_boundaries_come_from_resolved_capture_facts() {
    let signature_source = include_str!("../src/signature.rs");
    let usage_source = include_str!("../src/usage.rs");

    assert!(signature_source.contains("captured_binding_names_for_method"));
    assert!(signature_source.contains("captured_binding_names_for_constructor"));
    assert!(signature_source.contains("CaptureOrigin::Method"));
    assert!(signature_source.contains("CaptureOrigin::Constructor"));
    assert!(signature_source.contains("apply_capture_constraints"));
    assert!(!usage_source.contains("Captured"));
    assert!(!signature_source.contains("BindingUse::Captured"));
}

#[test]
fn inner_class_construction_diagnostics_stay_internal() {
    let source = include_str!("../src/inner_class_capture.rs");

    assert!(source.contains("pub(crate) struct UnconstructableInnerClass"));
    assert!(!source.contains("pub struct UnconstructableInnerClass"));
    let unconstructable_source = source
        .split(
            "#[derive(Debug, Clone, PartialEq, Eq)]\npub(crate) struct UnconstructableInnerClass",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl UnconstructableInnerClass").next())
        .expect("unconstructable inner class carrier should remain discoverable");
    assert!(!unconstructable_source.contains("pub(crate) inner_type: String"));
    assert!(!unconstructable_source.contains("pub(crate) outer_type: String"));
    assert!(source.contains("pub(crate) fn inner_type(&self) -> &str"));
    assert!(source.contains("pub(crate) fn outer_type(&self) -> &str"));
    assert!(source.contains("pub(crate) fn unconstructable_inner_classes"));
    assert!(!source.contains("pub fn unconstructable_inner_classes"));
    assert!(source.contains("pub(crate) fn report_unconstructable_inner_classes"));
    assert!(!source.contains("pub fn report_unconstructable_inner_classes"));
}

#[test]
fn idless_local_binding_compatibility_surface_stays_explicit() {
    let binding_id_source = include_str!("../src/binding_id.rs");
    let lower_source = include_str!("../src/lower.rs");
    let expr_shape_source = include_str!("../src/expr_shape.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production expr_shape source should be present");
    let signature_source = include_str!("../src/signature.rs")
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("production signature source should be present");

    assert!(binding_id_source.contains("pub(crate) fn compatibility_local"));
    assert!(!binding_id_source.contains("pub fn compatibility_local"));
    let binding_id_struct_source = binding_id_source
        .split("#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub struct BindingId")
        .nth(1)
        .and_then(|tail| {
            tail.split(
                "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub enum BindingKind",
            )
            .next()
        })
        .expect("binding id source should remain discoverable");
    assert!(binding_id_struct_source.contains("kind: BindingKind"));
    assert!(binding_id_struct_source.contains("name: String"));
    assert!(binding_id_struct_source.contains("occurrence: usize"));
    assert!(!binding_id_struct_source.contains("pub kind: BindingKind"));
    assert!(!binding_id_struct_source.contains("pub name: String"));
    assert!(!binding_id_struct_source.contains("pub occurrence: usize"));
    assert!(binding_id_source.contains("pub fn name(&self) -> &str"));
    assert!(binding_id_source.contains("pub fn kind(&self) -> BindingKind"));
    assert!(binding_id_source.contains("pub fn occurrence(&self) -> usize"));
    let usage_source = include_str!("../src/usage.rs");
    assert!(
        usage_source.contains("#[cfg(test)]\n    pub(crate) fn from_idless_compatibility_facts")
    );
    assert!(usage_source.contains("#[cfg(test)]\n    pub(crate) fn from_exact_binding_facts"));
    assert!(usage_source.contains("pub(crate) fn from_exact_binding_facts"));
    assert!(!usage_source.contains("pub fn from_exact_binding_facts"));
    assert!(usage_source.contains("pub(crate) fn from_idless_compatibility_facts"));
    assert!(!usage_source.contains("pub fn from_idless_compatibility_facts"));
    assert!(usage_source.contains("#[cfg(test)]\n    fn from_visible_binding_fixture_facts"));
    assert!(!usage_source.contains("pub(crate) fn from_visible_binding_fixture_facts"));
    assert!(!usage_source.contains("pub fn from_visible_binding_fixture_facts"));
    assert!(usage_source.contains("pub fn source_method_usage("));
    assert!(usage_source.contains("pub fn source_constructor_usage("));
    assert!(usage_source.contains("fn source_method_usage_by_java_key"));
    assert!(!usage_source.contains("pub(crate) fn source_method_usage_by_java_key"));
    assert!(usage_source.contains("fn source_method_usage_for_owner"));
    assert!(!usage_source.contains("pub(crate) fn source_method_usage_for_owner"));
    assert!(usage_source.contains("pub(crate) fn resolved_source_method_usage_for_owner"));
    assert!(usage_source.contains("pub(crate) fn resolved_source_method_usage"));
    assert!(!usage_source.contains("pub fn source_method_usage_by_java_key"));
    assert!(!usage_source.contains("pub fn source_method_usage_for_owner"));
    assert!(!usage_source.contains("pub fn resolved_source_method_usage"));
    assert!(lower_source.contains("fn idless_compatibility_local_plan_for_declaration"));
    assert!(
        lower_source.contains(
            "idless_compatibility_local_plan_for_declaration(name, ty, initializer, ctx)"
        )
    );
    assert!(
        expr_shape_source
            .contains("pub(crate) fn idless_compatibility_local_plan_for_declared_type")
    );
    assert!(
        !expr_shape_source
            .contains("fn idless_compatibility_local_plan_for_declared_type<'a>(\n        &self"),
        "id-less compatibility local-plan lookup should not hang off ShapeEnv"
    );
    assert!(expr_shape_source.contains("pub(crate) fn fallback_local_shape_from_declared_type"));
    for helper in [
        "insert_map_key_set_view_receiver",
        "insert_map_values_view_receiver",
        "insert_map_entry_set_view_receiver",
        "insert_map_entry_value_receiver",
        "insert_simple_map_entry_value",
        "forget_simple_map_entry_value",
        "insert_simple_immutable_map_entry_local",
        "simple_map_entry_local_is_immutable",
        "forget_map_entry_value_receiver",
        "map_key_set_view_receiver",
        "map_values_view_receiver",
        "map_entry_set_view_receiver",
        "map_entry_value_receiver",
        "simple_map_entry_value",
        "insert_nested_array2_local",
        "local_uses_nested_array2_slice",
        "insert_local_shape_with_access_name_and_java_type",
        "insert_local_shape_with_access_name_java_type_and_binding",
        "insert_param_shape_with_access_name_and_java_type",
        "insert_param_shape_with_access_name_java_type_and_binding",
        "insert_field_shape_with_access_name_and_java_type",
        "insert_static_storage_field_shape_owned_by_with_access_name",
        "insert_type_static_storage_field_shape_with_access_name",
        "insert_static_field_shape_for_owner_with_access_name",
        "insert_loop_variable_shape_with_access_name_and_binding",
        "insert_catch_param_with_access_name_and_binding",
        "insert_binding_access_name",
    ] {
        assert!(
            expr_shape_source.contains(&format!("pub(crate) fn {helper}(")),
            "{helper} should stay crate-internal"
        );
        assert!(
            !expr_shape_source.contains(&format!("pub fn {helper}(")),
            "{helper} should not be public API"
        );
    }
    for removed_helper in ["insert_loop_variable_shape", "insert_catch_param_shape"] {
        assert!(
            !expr_shape_source.contains(&format!("fn {removed_helper}(")),
            "{removed_helper} should stay deleted; use binding-aware insertion instead"
        );
    }
    assert!(expr_shape_source.contains("fn insert_catch_param_with_binding("));
    assert!(
        !expr_shape_source.contains("pub fn insert_catch_param_with_binding(")
            && !expr_shape_source.contains("pub(crate) fn insert_catch_param_with_binding("),
        "catch-param insertion without an access-name boundary should stay private"
    );
    for private_helper in [
        "insert_field_shape_with_access_name",
        "insert_type_static_storage_field_shape_with_access_name_and_java_type",
        "insert_static_field_shape_with_access_name",
        "insert_static_field_shape_with_access_name_and_java_type",
        "insert_static_storage_field_shape_with_access_name",
        "insert_static_storage_field_shape_with_access_name_and_java_type",
    ] {
        assert!(
            expr_shape_source.contains(&format!("fn {private_helper}(")),
            "{private_helper} should stay available inside expr_shape"
        );
        assert!(
            !expr_shape_source.contains(&format!("pub fn {private_helper}("))
                && !expr_shape_source.contains(&format!("pub(crate) fn {private_helper}(")),
            "{private_helper} should stay private to expr_shape"
        );
    }
    assert_eq!(
        expr_shape_source
            .matches("idless_compatibility_local_plan_for_declared_type")
            .count(),
        1,
        "expr_shape should expose the compatibility primitive without adding production callers"
    );
    assert!(
        !expr_shape_source.contains("local_plan_occurrences")
            && !expr_shape_source.contains("consume_local_plan_occurrence"),
        "id-less local-plan compatibility must fail closed by unique exact match, not advance a shared occurrence cursor"
    );
    let expr_shape_context_source = expr_shape_source
        .split("#[derive(Debug, Clone, Copy, Default)]\npub struct ExprShapeContext")
        .nth(1)
        .and_then(|tail| tail.split("impl<'a> ExprShapeContext<'a>").next())
        .expect("ExprShapeContext source should remain discoverable");
    for field in ["pub signatures:", "pub representations:", "pub facts:"] {
        assert!(
            !expr_shape_context_source.contains(field),
            "ExprShapeContext should expose constructor/accessors, not raw public fields: {field}"
        );
    }
    assert!(expr_shape_source.contains("pub fn new(\n        signatures:"));
    assert!(expr_shape_source.contains("pub fn signatures(self)"));
    assert!(expr_shape_source.contains("pub fn representations(self)"));
    assert!(expr_shape_source.contains("pub fn facts(self)"));
    assert!(signature_source.contains("fn idless_compatibility_local(&self, name: &str)"));
    assert!(signature_source.contains("fn idless_compatibility_local_mut(&mut self, name: &str)"));
    assert!(
        !signature_source.contains("fn idless_compatibility_local_index(&self, name: &str)"),
        "id-less index lookup should stay in the shared helper, not per constraint-set wrappers"
    );
    assert!(
        signature_source.contains("fn param_index_for_constraints"),
        "parameter-name lookup should live in one helper"
    );
    assert_eq!(
        signature_source
            .matches("param_names.iter().position(|param| param == name)")
            .count(),
        1,
        "parameter-name lookup should not be duplicated across method and constructor constraints"
    );
    assert!(
        signature_source.contains("fn local_index_for_binding_constraints"),
        "binding-keyed local lookup should live in one helper"
    );
    assert!(
        signature_source.contains("fn local_for_binding_constraints"),
        "binding-keyed local reads should live in one helper"
    );
    assert!(
        signature_source.contains("fn local_mut_for_binding_constraints"),
        "binding-keyed local mutation should live in one helper"
    );
    assert_eq!(
        signature_source
            .matches("locals.iter().position(|local| local.binding == *binding)")
            .count(),
        1,
        "binding-keyed local lookup should not be duplicated across method and constructor constraints"
    );
    assert!(
        signature_source.contains("fn idless_compatibility_local_index_for_constraints"),
        "the id-less local-name uniqueness policy should live in one helper"
    );
    assert!(
        signature_source.contains("fn idless_compatibility_local_for_constraints"),
        "id-less local reads should live in one helper"
    );
    assert!(
        signature_source.contains("fn idless_compatibility_local_mut_for_constraints"),
        "id-less local mutation should live in one helper"
    );
    assert_eq!(
        signature_source
            .matches("idless_compatibility_local_index_for_constraints(locals, local_index_by_name, name)\n        .and_then(|index| locals.get(index))")
            .count(),
        1,
        "id-less local read lookup should not be duplicated across method and constructor constraints"
    );
    assert_eq!(
        signature_source
            .matches("local_index_for_binding_constraints(locals, local_index_by_binding, binding)\n        .and_then(|index| locals.get(index))")
            .count(),
        1,
        "binding-keyed local read lookup should not be duplicated across method and constructor constraints"
    );
    let idless_mut_helper = signature_source
        .split("fn idless_compatibility_local_mut_for_constraints")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn idless_compatibility_local_index_for_constraints")
                .next()
        })
        .expect("id-less mutable local helper should be present");
    assert_eq!(
        idless_mut_helper
            .matches("idless_compatibility_local_index_for_constraints")
            .count(),
        1,
        "id-less mutable local helper should use the shared index policy once"
    );
    assert!(idless_mut_helper.contains("locals.get_mut(index)"));
    let binding_mut_helper = signature_source
        .split("fn local_mut_for_binding_constraints")
        .nth(1)
        .and_then(|tail| tail.split("fn param_index_for_constraints").next())
        .expect("binding-keyed mutable local helper should be present");
    assert_eq!(
        binding_mut_helper
            .matches("local_index_for_binding_constraints")
            .count(),
        1,
        "binding-keyed mutable local helper should use the shared index policy once"
    );
    assert!(binding_mut_helper.contains("locals.get_mut(index)"));
    assert_eq!(
        signature_source
            .matches("locals.iter().filter(|local| local.name == name).count() > 1")
            .count(),
        1,
        "duplicate-name id-less compatibility must fail closed in one shared policy"
    );
    assert!(!signature_source.contains("fn local(&self, name: &str)"));
    assert!(!signature_source.contains("fn local_mut(&mut self, name: &str)"));
    assert!(!signature_source.contains("fn local_index(&self, name: &str)"));
    assert!(signature_source.contains("pub(crate) fn source_name(&self) -> &str"));
    assert!(!signature_source.contains("pub fn source_name(&self) -> &str"));
    assert!(!signature_source.contains("fn access_name(&self) -> &str"));

    assert_eq!(
        signature_source
            .matches("collect_duplicate_declared_local_names(block)")
            .count(),
        1,
        "duplicate-name compatibility should remain limited to legacy nullability fail-closed"
    );
    assert!(
        signature_source.contains("for duplicate in collect_duplicate_declared_local_names(block)")
    );
    assert!(signature_source.contains("#[cfg(test)]\nfn source_binding_nullability_constraints"));
    assert!(!signature_source.contains("pub(crate) fn source_binding_nullability_constraints("));
    assert!(!signature_source.contains("pub fn source_binding_nullability_constraints("));
    assert!(
        signature_source.contains("pub(crate) fn source_method_return_nullability_constraints")
    );
    assert!(!signature_source.contains("pub fn source_method_return_nullability_constraints"));
    assert!(signature_source.contains("fn constraints_for_declaration"));
    assert!(
        signature_source.contains("source_nullability.constraints_for_declaration(name, binding)")
    );
    assert!(
        signature_source.contains("constraints_for_declaration(name, declared_binding.as_ref())")
    );
    assert!(
        signature_source
            .contains(".constraints_for_declaration(var_name, declared_binding.as_ref())")
    );
    assert_eq!(
        signature_source
            .matches(".map(|binding| source_nullability.constraints_for_binding(binding))")
            .count(),
        0,
        "declaration nullability callers should not re-open-code binding/name fallback"
    );
    assert!(
        !signature_source
            .contains("let duplicate_names = collect_duplicate_declared_local_names(block);")
    );
    assert!(
        !signature_source.contains("pub fn local_shape<")
            && !signature_source.contains("pub fn required_local_plan<")
            && !signature_source.contains("pub fn required_local_shape(")
            && !signature_source.contains("pub fn local_binding_mutability("),
        "SignaturePlan local lookup should expose binding-keyed APIs, not reusable name-only accessors"
    );
    assert!(signature_source.contains("pub fn local_shape_by_binding"));
    assert!(signature_source.contains("pub fn required_local_shape_by_binding"));
    assert!(signature_source.contains("pub fn local_binding_mutability_by_binding"));
}

#[test]
fn constructor_delegation_name_substitution_stays_fenced() {
    let lower_source = include_str!("../src/lower.rs");
    let constructor_delegation_start = lower_source
        .find("fn source_super_constructor_with_trailing_fields")
        .expect("constructor delegation section should be present");
    let constructor_delegation_end = lower_source
        .find("fn delegated_constructor_with_trailing_body")
        .expect("constructor delegation section should end before lowering body emission");

    for forbidden in [
        "fn substitute_expr_vars",
        "fn substitute_stmt_vars",
        "fn substitute_block_vars",
    ] {
        assert!(
            !lower_source.contains(forbidden),
            "name-based substitution helpers must advertise their constructor-delegation compatibility scope"
        );
    }

    for (idx, _) in lower_source.match_indices("substitute_source_constructor_param_") {
        assert!(
            idx >= constructor_delegation_start && idx < constructor_delegation_end,
            "constructor parameter name-substitution compatibility must not grow callers outside constructor delegation"
        );
    }
}

#[test]
fn signature_uses_usage_binding_policy_for_legacy_name_fallback() {
    let signature_source = include_str!("../src/signature.rs");
    let usage_source = include_str!("../src/usage.rs")
        .split("mod tests")
        .next()
        .expect("usage production source should be present");
    let slot_policy_source = include_str!("../src/slot_policy.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production slot_policy source should be present");

    for forbidden in [
        "method_usage.bindings.get(",
        "constructor_usage.bindings.get(",
        "usage.bindings.get(",
        "facts.bindings.get(",
    ] {
        assert!(
            !signature_source.contains(forbidden),
            "signature should ask MethodUsageFacts for binding lookup instead of reading the legacy name map directly"
        );
    }
    assert!(
        signature_source.contains("binding_facts_for_param")
            && signature_source.contains("binding_facts_for_declaration_id")
            && !signature_source.contains("idless_compatibility_binding_usage_for_local"),
        "signature should use exact-id local lookup and delegate parameter compatibility to MethodUsageFacts"
    );
    assert!(
        !signature_source.contains("binding_for_declaration(param.binding.as_ref(), &param.name)"),
        "signature must make parameter name-fallback compatibility explicit at each call site"
    );
    assert!(
        !signature_source.contains("fn binding_usage_for_param")
            && signature_source.contains("fn binding_usage_for_bound_local")
            && !signature_source.contains("fn idless_compatibility_binding_usage_for_local"),
        "signature should keep bound local lookup explicit without owning an id-less name fallback wrapper"
    );
    assert!(
        !signature_source.contains("fn binding_usage_for_local"),
        "signature should not hide the bound-vs-idless fallback decision behind a boolean local lookup helper"
    );
    assert!(
        !slot_policy_source
            .contains("binding_for_declaration(param.binding.as_ref(), &param.name)"),
        "slot policy must make parameter name-fallback compatibility explicit at each call site"
    );
    assert!(
        !slot_policy_source.contains("fn binding_usage_for_param")
            && slot_policy_source.contains("binding_facts_for_param"),
        "slot policy should use MethodUsageFacts for parameter binding fallback instead of owning a duplicate helper"
    );
    assert!(
        !usage_source.contains("binding_for_declaration_with_name_fallback"),
        "MethodUsageFacts should not expose a boolean-controlled name fallback API"
    );
    assert!(
        usage_source.contains("pub(crate) fn binding_facts_for_declaration_id")
            && usage_source.contains("pub(crate) fn binding_facts_for_id")
            && !usage_source.contains("pub fn binding_facts_for_id")
            && usage_source.contains("fn idless_compatibility_binding_facts_for_source_name")
            && !usage_source
                .contains("pub(crate) fn idless_compatibility_binding_facts_for_source_name"),
        "MethodUsageFacts should expose exact-id lookups crate-locally while keeping id-less compatibility lookup private"
    );
    assert!(
        usage_source.contains("pub(crate) fn binding_facts_for_param")
            && usage_source.contains("&param.source_name")
            && usage_source.contains("if let Some(binding) = &param.binding"),
        "MethodUsageFacts should own the parameter binding-id/source-name fallback policy"
    );
    assert!(
        !usage_source.contains("fn binding_facts_for_name"),
        "direct legacy-name binding lookup should not be exposed as a MethodUsageFacts API"
    );
    assert!(
        !usage_source.contains("fn unique_binding_facts_for_name"),
        "unique legacy-name binding lookup should not be exposed as a MethodUsageFacts API"
    );
    assert!(
        usage_source.contains("fn snapshot_visible_binding_for_scope")
            && usage_source.contains("fn hide_visible_binding_for_scope")
            && usage_source.contains("fn restore_visible_binding_for_scope")
            && usage_source.contains("fn insert_visible_binding_for_scope")
            && usage_source.contains("fn insert_visible_binding_facts_for_scope")
            && usage_source.contains("fn current_visible_binding_facts_for_name")
            && usage_source.contains("fn current_visible_name_binding_mut_for_scope")
            && usage_source.contains("fn record_visible_binding_use")
            && usage_source.contains("fn record_declared_binding_use")
            && !usage_source.contains("pub(crate) fn snapshot_visible_binding_for_scope")
            && !usage_source.contains("pub(crate) fn hide_visible_binding_for_scope")
            && !usage_source.contains("pub(crate) fn restore_visible_binding_for_scope")
            && !usage_source.contains("pub(crate) fn insert_visible_binding_for_scope")
            && !usage_source.contains("pub(crate) fn insert_visible_binding_facts_for_scope")
            && !usage_source.contains("pub(crate) fn current_visible_binding_facts_for_name")
            && !usage_source.contains("pub(crate) fn current_visible_name_binding_mut_for_scope")
            && !usage_source.contains("pub(crate) fn record_visible_binding_use")
            && !usage_source.contains("pub(crate) fn record_declared_binding_use")
            && !usage_source.contains("pub fn snapshot_visible_binding_for_scope")
            && !usage_source.contains("pub fn hide_visible_binding_for_scope")
            && !usage_source.contains("pub fn restore_visible_binding_for_scope")
            && !usage_source.contains("pub fn insert_visible_binding_for_scope")
            && !usage_source.contains("pub fn insert_visible_binding_facts_for_scope")
            && !usage_source.contains("pub fn current_visible_binding_facts_for_name")
            && !usage_source.contains("pub fn current_visible_name_binding_mut_for_scope")
            && !usage_source.contains("pub fn record_visible_binding_use")
            && !usage_source.contains("pub fn record_declared_binding_use"),
        "MethodUsageFacts scope/name-map maintenance helpers should stay private to usage.rs"
    );
    let idless_binding_lookup = usage_source
        .split("fn idless_compatibility_binding_facts_for_source_name")
        .nth(1)
        .and_then(|tail| tail.split("pub(crate) fn binding_facts_for_param").next())
        .expect("id-less compatibility binding lookup should remain discoverable");
    assert!(idless_binding_lookup.contains("self.current_visible_binding_facts_for_name(name)"));
    assert!(
        usage_source.matches("current_visible_name_binding_mut_for_scope(name)").count() >= 2
            && !usage_source
                .contains("self.name_binding_is_current(name)\n            && let Some(binding) = self.bindings.get_mut(name)"),
        "visible name-map mutation should route through the private mutable helper"
    );
    let visible_fixture_constructor = usage_source
        .split("fn from_visible_binding_fixture_facts")
        .nth(1)
        .and_then(|tail| tail.split("#[cfg(test)]").next())
        .expect("visible-binding fixture constructor should remain discoverable");
    assert!(
        visible_fixture_constructor
            .contains("facts.insert_visible_binding_facts_for_scope(binding)")
    );
    assert!(
        !visible_fixture_constructor.contains(".bindings.insert(")
            && !visible_fixture_constructor.contains(".bindings_by_id.insert("),
        "visible-binding fixture construction should use MethodUsageFacts' private insertion policy"
    );
    let insert_helper = usage_source
        .split("fn insert_binding_use_facts")
        .nth(1)
        .and_then(|tail| tail.split("fn restore_binding_use_facts").next())
        .expect("insert binding helper should remain discoverable");
    assert!(
        insert_helper.contains("facts.insert_visible_binding_for_scope(name, binding, java_ty)")
    );
    assert!(
        !insert_helper.contains(".bindings.insert(")
            && !insert_helper.contains(".bindings_by_id.insert("),
        "free insertion helper should delegate to MethodUsageFacts instead of mutating maps directly"
    );
    assert!(
        usage_source.contains("fn visible_binding_id_for_name")
            && usage_source.contains("fn visible_binding_java_type_for_name")
            && usage_source.contains("pub(crate) fn receiver_handle_java_type")
            && usage_source.contains("fn name_binding_is_current")
            && !usage_source.contains("pub(crate) fn visible_binding_id_for_name")
            && !usage_source.contains("pub(crate) fn visible_binding_java_type_for_name")
            && !usage_source.contains("pub(crate) fn name_binding_is_current")
            && !usage_source.contains("pub fn visible_binding_id_for_name")
            && !usage_source.contains("pub fn visible_binding_java_type_for_name")
            && !usage_source.contains("pub fn receiver_handle_java_type")
            && !usage_source.contains("pub fn name_binding_is_current"),
        "remaining visible-name binding compatibility helpers should expose only semantic readers needed outside usage.rs"
    );
    assert!(
        !usage_source.contains("fn contains_binding_name"),
        "name-presence compatibility checks should not be exposed as a MethodUsageFacts API"
    );
    assert!(
        !usage_source.contains("pub fn binding_for_declaration("),
        "the broad implicit-fallback binding lookup wrapper should not exist"
    );
    assert!(
        !slot_policy_source.contains("pub fn method_assigns_to_binding"),
        "name-only assignment scans must not be exposed as a general slot-policy API"
    );
    assert!(
        slot_policy_source
            .contains("pub(crate) fn idless_compatibility_exact_returned_param_is_assignment_safe"),
        "the remaining exported id-less compatibility question should be scoped to the exact-return safety decision"
    );
    assert!(!slot_policy_source.contains("pub(crate) fn method_assigns_to_binding"));
    assert_eq!(
        slot_policy_source
            .matches("idless_compatibility_exact_returned_param_is_assignment_safe")
            .count(),
        2,
        "slot-policy exact-return safety predicate should have one id-less caller and one helper definition"
    );
    assert!(
        slot_policy_source
            .contains("if param.binding.is_some() {\n        if !binding\n            .uses")
            && slot_policy_source.contains(
                "} else {\n        if !idless_compatibility_exact_returned_param_is_assignment_safe"
            ),
        "slot-policy exact-return safety predicate must stay behind the id-less parameter branch"
    );
    assert_eq!(
        signature_source
            .matches("idless_compatibility_exact_returned_param_is_assignment_safe")
            .count(),
        2,
        "signature's exact-return safety predicate use should stay explicit and localized"
    );
    assert!(
        signature_source.contains(
            "let compatibility_name_returned = param.binding.is_none()\n            && idless_compatibility_exact_returned_param_is_assignment_safe("
        ),
        "signature's compatibility return predicate must stay behind the id-less parameter predicate"
    );
}

#[test]
fn slot_policy_uses_java_keyed_source_method_usage_lookup() {
    let source = include_str!("../src/slot_policy.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production slot_policy source should be present");

    assert!(
        source.contains("usage.source_method_usage(ty, method)")
            && !source.contains("usage.methods.get(&UsageMethodKey"),
        "slot policy has TypeDecl/MethodDecl context and should not re-key source method usage by plain strings"
    );
}

#[test]
fn declaration_context_usage_lookups_use_java_callable_keys() {
    let facts_source = include_str!("../src/facts.rs");
    let java_signature_source = include_str!("../src/java_signature.rs");
    let lib_source = include_str!("../src/lib.rs");
    let resolve_source = include_str!("../src/resolve.rs");
    let main_source = include_str!("../src/main.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");
    let source_constant_values_source = include_str!("../src/source_constant_values.rs");
    let method_reference_plan_source = include_str!("../src/method_reference_plan.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production method-reference plan source should be present");
    let constant_declaration_source = resolve_source
        .split("pub(crate) struct ConstantDeclaration")
        .nth(1)
        .and_then(|tail| tail.split("/// Folds `declarations`").next())
        .expect("ConstantDeclaration source should remain discoverable");
    assert!(java_signature_source.contains("pub struct JavaOwnerKey"));
    assert!(java_signature_source.contains("pub(crate) fn source_method_for_type"));
    assert!(!java_signature_source.contains("pub fn source_method_for_type"));
    assert!(java_signature_source.contains("pub(crate) fn source_method_for_owner"));
    assert!(java_signature_source.contains("pub(crate) fn resolved_source_method"));
    assert!(
        java_signature_source.contains("pub(crate) fn source_method(method: &MethodDecl) -> Self")
    );
    assert!(facts_source.contains("pub(crate) fn external_type_name_matches"));
    assert!(!facts_source.contains("pub fn external_type_name_matches"));
    let java_owner_key_source = java_signature_source
        .split("pub struct JavaOwnerKey")
        .nth(1)
        .and_then(|tail| tail.split("impl JavaOwnerKey").next())
        .expect("JavaOwnerKey source should remain discoverable");
    assert!(java_owner_key_source.contains("(String)"));
    assert!(!java_owner_key_source.contains("pub String"));
    assert!(!java_signature_source.contains("pub fn resolved_source"));
    assert!(!java_signature_source.contains("pub fn source_method(method: &MethodDecl) -> Self"));
    assert!(method_reference_plan_source.contains("owner: JavaOwnerKey"));
    assert!(method_reference_plan_source.contains("type_name: JavaOwnerKey"));
    assert!(
        !method_reference_plan_source.contains("owner: String")
            && !method_reference_plan_source.contains("type_name: String"),
        "method-reference plans must retain source owner identity until their explicit rendering boundary"
    );
    assert!(!constant_declaration_source.contains("pub owner: String"));
    assert!(!constant_declaration_source.contains("pub package: Option<String>"));
    assert!(!constant_declaration_source.contains("pub field: crate::java_ir::FieldDecl"));
    let overload_argument_source = resolve_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct OverloadArgumentApplicability")
        .nth(1)
        .and_then(|tail| tail.split("impl OverloadArgumentApplicability").next())
        .expect("OverloadArgumentApplicability source should remain discoverable");
    for field in [
        "pub arg_index: usize",
        "pub actual: JavaType",
        "pub target: JavaType",
        "pub conversion: OverloadArgumentConversion",
        "pub score: u32",
    ] {
        assert!(
            !overload_argument_source.contains(field),
            "OverloadArgumentApplicability should expose accessors, not raw public fields: {field}"
        );
    }
    let overload_report_source = resolve_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct OverloadApplicabilityReport")
        .nth(1)
        .and_then(|tail| tail.split("impl OverloadApplicabilityReport").next())
        .expect("OverloadApplicabilityReport source should remain discoverable");
    for field in [
        "pub mode: Option<OverloadApplicabilityMode>",
        "pub score: Option<u32>",
        "pub arguments: Vec<OverloadArgumentApplicability>",
        "pub rejections: Vec<OverloadRejectionReason>",
    ] {
        assert!(
            !overload_report_source.contains(field),
            "OverloadApplicabilityReport should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(resolve_source.contains("pub fn arguments(&self) -> &[OverloadArgumentApplicability]"));
    assert!(resolve_source.contains("pub fn rejections(&self) -> &[OverloadRejectionReason]"));
    assert!(
        resolve_source
            .contains("pub(crate) fn static_final_constant_declarations(program: &Program)")
    );
    assert!(resolve_source.contains("pub(crate) fn constant_value_facts_from_declarations("));
    assert!(resolve_source.contains("pub(crate) fn fold_constant_declarations("));
    assert!(!resolve_source.contains("pub fn static_final_constant_declarations("));
    assert!(!resolve_source.contains("pub fn constant_value_facts_from_declarations("));
    assert!(!resolve_source.contains("pub fn fold_constant_declarations("));
    assert!(lib_source.contains("mod source_constant_values;"));
    assert!(!lib_source.contains("pub mod source_constant_values;"));
    assert!(lib_source.contains("mod array_default_obligations;"));
    assert!(!lib_source.contains("pub mod array_default_obligations;"));
    for module in [
        "access_lowering",
        "aliasing",
        "array_lowering",
        "array_plan",
        "callable_analysis",
        "control_flow_lowering",
        "conversion_lowering",
        "fallibility_invariant",
        "fact_saturation",
        "graph",
        "inherited_members",
        "inner_class_capture",
        "item_assembly",
        "java_naming",
        "java_signature",
        "javalibs",
        "lattice",
        "literal_lowering",
        "liveness",
        "loop_plan",
        "method_reference_lowering",
        "method_reference_plan",
        "method_reference_shape",
        "nullability_plan",
        "representation_lowering",
        "statement_render",
        "string_concat",
        "string_concat_plan",
        "string_concat_render",
    ] {
        assert!(
            lib_source.contains(&format!("mod {module};")),
            "{module} should remain an internal module"
        );
        assert!(
            !lib_source.contains(&format!("pub mod {module};")),
            "{module} should not be exported as public crate API"
        );
    }
    assert!(source_constant_values_source.contains("pub(crate) enum ConstantValue"));
    assert!(source_constant_values_source.contains("pub(crate) struct ConstantScope"));
    assert!(source_constant_values_source.contains("pub(crate) fn fold_constant_expr"));
    assert!(!source_constant_values_source.contains("pub enum ConstantValue"));
    assert!(!source_constant_values_source.contains("pub struct ConstantScope"));
    assert!(!source_constant_values_source.contains("pub fn fold_constant_expr"));
    let array_default_obligations_source = include_str!("../src/array_default_obligations.rs");
    assert!(
        array_default_obligations_source
            .contains("pub(crate) fn program_allocates_object_array_of")
    );
    assert!(!array_default_obligations_source.contains("pub fn object_array_element_type_names"));
    assert!(!array_default_obligations_source.contains("pub fn program_allocates_object_array_of"));
    assert!(
        slot_render_source.contains("use crate::java_naming::rust_name_for_java_named_type;")
            && !slot_render_source.contains("pub(crate) fn rust_name_for_java_named_type"),
        "slot_render should use the canonical Java type-name projection instead of carrying a compatibility wrapper"
    );
    assert!(
        main_source.contains("let owner = java_simple_name.to_string();")
            && !main_source.contains("new_java2rust::slot_render::rust_name_for_java_named_type"),
        "the CLI facade-owner path receives a simple Java name and should not keep the generic Java type-name projection public"
    );

    let usage_source = include_str!("../src/usage.rs");
    assert!(usage_source.contains("pub(crate) fn resolved_source_method_usage_for_owner"));
    assert!(!usage_source.contains("pub(crate) fn source_method_usage_for_owner"));

    for (module, source, required) in [
        (
            "aliasing",
            include_str!("../src/aliasing.rs"),
            ".resolved_source_method_usage(",
        ),
        (
            "storage_strategy",
            include_str!("../src/storage_strategy.rs"),
            ".source_method_usage(implementor, method)",
        ),
        (
            "semantic_analysis",
            include_str!("../src/semantic_analysis.rs"),
            ".source_method_usage(ty, method)",
        ),
        (
            "signature",
            include_str!("../src/signature.rs"),
            ".source_method_usage(ty, method)",
        ),
    ] {
        assert!(
            source.contains(required),
            "{module} should route declaration-scoped usage lookups through Java callable keys"
        );
    }

    let storage_source = include_str!("../src/storage_strategy.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production storage_strategy source should be present");
    assert!(
        !storage_source.contains(".methods\n                .get(&crate::usage::UsageMethodKey"),
        "storage strategy has TypeDecl/MethodDecl context and should not re-key usage by plain strings"
    );
    let type_storage_facts_source = storage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct TypeStorageFacts")
        .nth(1)
        .and_then(|tail| tail.split("impl Default for TypeStorageFacts").next())
        .expect("TypeStorageFacts source should remain discoverable");
    for field in [
        "pub escape:",
        "pub locally_aliased:",
        "pub mutated:",
        "pub threaded:",
        "pub surface_forwardable:",
        "pub mutation_provably_absent:",
        "pub static_reachable:",
    ] {
        assert!(
            !type_storage_facts_source.contains(field),
            "TypeStorageFacts should expose evidence accessors/builders, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn escape(&self) -> Escape",
        "pub fn locally_aliased(&self) -> bool",
        "pub fn mutated(&self) -> bool",
        "pub fn threaded(&self) -> bool",
        "pub fn surface_forwardable(&self) -> bool",
        "pub fn mutation_provably_absent(&self) -> bool",
        "pub fn static_reachable(&self) -> bool",
    ] {
        assert!(
            storage_source.contains(accessor),
            "TypeStorageFacts should keep named evidence accessor: {accessor}"
        );
    }
    for builder in [
        "pub fn with_escape(mut self, escape: Escape) -> Self",
        "pub fn with_locally_aliased(mut self, locally_aliased: bool) -> Self",
        "pub fn with_mutated(mut self, mutated: bool) -> Self",
        "pub fn with_threaded(mut self, threaded: bool) -> Self",
        "pub fn with_surface_forwardable(mut self, surface_forwardable: bool) -> Self",
        "pub fn with_mutation_provably_absent(mut self, mutation_provably_absent: bool) -> Self",
        "pub fn with_static_reachable(mut self, static_reachable: bool) -> Self",
    ] {
        assert!(
            storage_source.contains(builder),
            "TypeStorageFacts should keep named evidence builder: {builder}"
        );
    }
    let storage_placement_source = storage_source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]\npub struct StoragePlacement")
        .nth(1)
        .and_then(|tail| tail.split("impl StoragePlacement").next())
        .expect("StoragePlacement source should remain discoverable");
    for field in ["pub shared:", "pub interior_mutable:", "pub sync:"] {
        assert!(
            !storage_placement_source.contains(field),
            "StoragePlacement should expose axis accessors, not raw public fields: {field}"
        );
    }
    assert!(
        storage_source
            .contains("pub fn new(shared: bool, interior_mutable: bool, sync: bool) -> Self"),
        "StoragePlacement should keep a named constructor for its decision axes"
    );
    for accessor in [
        "pub fn shared(self) -> bool",
        "pub fn interior_mutable(self) -> bool",
        "pub fn sync(self) -> bool",
    ] {
        assert!(
            storage_source.contains(accessor),
            "StoragePlacement should keep named axis accessor: {accessor}"
        );
    }

    let aliasing_source = include_str!("../src/aliasing.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production aliasing source should be present");
    assert!(
        !aliasing_source.contains(".methods\n            .get(&UsageMethodKey"),
        "aliasing has ResolvedCall identity and should not re-key receiver mutation by plain strings"
    );
    assert!(
        !aliasing_source.contains(".methods_by_java_key"),
        "aliasing should ask UsageFacts for resolved source usage instead of reading the Java-keyed table directly"
    );

    let facts_source = include_str!("../src/facts.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production facts source should be present");
    let call_plan_source = include_str!("../src/call_plan.rs");
    let method_reference_lowering_source = include_str!("../src/method_reference_lowering.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production method_reference_lowering source should be present");
    assert!(facts_source.contains("pub fn resolved_method("));
    assert!(facts_source.contains("pub fn resolved_constructor("));
    assert!(facts_source.contains("pub fn facade_abi_for_external_method("));
    assert!(facts_source.contains("pub fn facade_abi_for_external_call("));
    assert!(
        call_plan_source.contains("CallableKey::resolved_method(call,"),
        "resolved source call planning should not rebuild fact callable keys from owner/name fields"
    );
    assert!(
        call_plan_source.contains(".facade_abi_for_external_method(")
            && call_plan_source.contains(".facade_abi_for_external_call("),
        "external facade ABI planning should route owner/name/arity key construction through FactSet helpers"
    );
    assert!(call_plan_source.contains("pub(crate) fn facade_abi_argument_plan_for_rust_path"));
    assert!(!call_plan_source.contains("pub fn facade_abi_argument_plan_for_rust_path"));
    assert!(
        method_reference_lowering_source.contains("fn method_reference_facade_abi_candidates")
            && method_reference_lowering_source.contains(".facade_abi_for_external_method(")
            && !method_reference_lowering_source
                .contains("fn method_reference_callable_key_candidates"),
        "method-reference facade ABI planning should not expose temporary callable-key candidate lists"
    );
}

#[test]
fn receiver_fallback_owner_candidates_stay_wrapped_until_lookup() {
    let lower_source = include_str!("../src/lower.rs");
    assert!(
        !lower_source.contains("fn source_receiver_type_name"),
        "source receiver owner recovery should keep SourceContextOwnerKey wrapped until lookup"
    );
    assert!(
        !lower_source.contains("fn into_string(self) -> String"),
        "SourceContextOwnerKey should not expose an eager raw-string downgrade helper"
    );
    let owner_key_users = lower_source
        .split("struct SourceContextOwnerKey(String);")
        .nth(1)
        .expect("SourceContextOwnerKey declaration should remain discoverable");
    assert!(
        !owner_key_users.contains("SourceContextOwnerKey("),
        "SourceContextOwnerKey construction should go through SourceContextOwnerKey::new"
    );
    let helper = lower_source
        .split("fn fallback_receiver_type_name_candidates")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn nested_type_owner_candidates_for_receiver")
                .next()
        })
        .expect("fallback receiver owner candidate helper should be present");
    assert!(
        helper.contains(") -> Vec<SourceContextOwnerKey>"),
        "fallback receiver owner candidates should not be downgraded to raw strings at the helper boundary"
    );
    assert!(
        !helper.contains(".map(SourceContextOwnerKey::into_string)"),
        "string conversion should happen only at old string-returning edges"
    );

    let signature_fallback = lower_source
        .split("fn fallback_receiver_method_signature")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn fallback_receiver_is_static_owner_expr")
                .next()
        })
        .expect("receiver signature fallback should be present");
    assert!(
        signature_fallback.contains(
            "let resolved_receiver_name = receiver_owner.resolved_context_owner_key(ctx)"
        )
    );
    assert!(signature_fallback.contains("receiver_owner.resolved_context_owner_key(ctx)"));
    assert!(signature_fallback.contains("&resolved_receiver_name"));
    assert!(!signature_fallback.contains("resolved_context_owner_key(ctx, receiver_name)"));

    let static_receiver_check = lower_source
        .split("fn explicit_static_receiver_matches_resolved_owner")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn non_emitted_static_context_fallback_signature_for_method")
                .next()
        })
        .expect("explicit static receiver match helper should be present");
    assert!(static_receiver_check.contains("let owner_key = SourceContextOwnerKey::new(owner)"));
    assert!(static_receiver_check.contains("candidate.resolves_to_owner(ctx, &owner_key)"));
    assert!(
        static_receiver_check
            .contains("SourceContextOwnerKey::new(alias).resolves_to_owner(ctx, &owner_key)")
    );
    assert!(
        !static_receiver_check.contains("context_type_key_resolves_to_owner"),
        "resolved static receiver matching should keep candidate owner lookup on SourceContextOwnerKey"
    );
    assert!(
        !lower_source.contains("fn context_type_key_resolves_to_owner"),
        "raw owner-key comparison helper should stay collapsed into SourceContextOwnerKey"
    );
    let owner_key_impl = lower_source
        .split("impl SourceContextOwnerKey")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
                .next()
        })
        .expect("SourceContextOwnerKey impl should be present");
    let resolves_to_owner = owner_key_impl
        .split("fn resolves_to_owner")
        .nth(1)
        .and_then(|tail| tail.split("fn is_static_context_emitted").next())
        .expect("SourceContextOwnerKey::resolves_to_owner should be present");
    assert!(resolves_to_owner.contains("owner: &Self"));
    assert!(resolves_to_owner.contains("self.find_context_type_and_program(ctx)"));
    assert!(resolves_to_owner.contains("owner.find_context_type_and_program(ctx)"));
    assert!(
        resolves_to_owner
            .contains("std::ptr::eq(ty, owner_ty) && std::ptr::eq(program, owner_program)")
    );
    assert!(
        !resolves_to_owner.contains("find_context_type_by_key_in_lower_ctx(ctx, owner"),
        "SourceContextOwnerKey owner comparison should keep both sides wrapped"
    );
    assert!(owner_key_impl.contains("fn find_emitted_context_type_and_program"));
    for emitted_predicate in [
        "fn is_static_context_emitted",
        "fn is_non_program_emitted_context",
        "fn is_emitted_program_context",
    ] {
        let predicate = owner_key_impl
            .split(emitted_predicate)
            .nth(1)
            .and_then(|tail| tail.split("\n    fn ").next())
            .expect("SourceContextOwnerKey emitted-context predicate should be present");
        assert!(
            predicate.contains("find_emitted_context_type_and_program(ctx)"),
            "{emitted_predicate} should share SourceContextOwnerKey emitted lookup"
        );
        assert!(
            !predicate.contains("find_emitted_context_type_and_program_by_key_in_lower_ctx"),
            "{emitted_predicate} should not reopen raw emitted owner lookup"
        );
    }

    let emitted_context_receiver_check = lower_source
        .split("fn static_owner_receiver_is_emitted_context")
        .nth(1)
        .and_then(|tail| tail.split("fn static_owner_expr_candidates").next())
        .expect("static owner emitted-context receiver helpers should be present");
    assert!(emitted_context_receiver_check.contains("owner.is_static_context_emitted(ctx)"));
    assert!(emitted_context_receiver_check.contains("owner.is_non_program_emitted_context(ctx)"));
    assert!(
        !emitted_context_receiver_check.contains("static_owner_key_is_non_program_emitted_context")
    );
    assert!(
        !lower_source.contains("fn static_owner_key_is_non_program_emitted_context"),
        "non-program emitted-context checks should stay on SourceContextOwnerKey"
    );
    let static_context_owner_check = lower_source
        .split("fn static_context_owner_is_emitted")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn source_context_receiver_method_fallback_signature")
                .next()
        })
        .expect("static context emitted-owner helpers should remain discoverable");
    assert!(
        static_context_owner_check
            .contains("SourceContextOwnerKey::new(owner).is_static_context_emitted(ctx)")
    );
    assert!(
        static_context_owner_check
            .contains("SourceContextOwnerKey::new(owner).is_emitted_program_context(ctx)")
    );
    assert!(
        !static_context_owner_check
            .contains("find_emitted_context_type_and_program_by_key_in_lower_ctx(ctx, owner)"),
        "static emitted-owner helpers should keep raw emitted lookup behind SourceContextOwnerKey"
    );

    let current_static_fallback = lower_source
        .split("fn source_context_current_static_method_fallback_signature")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn source_context_static_signature_by_name_and_args")
                .next()
        })
        .expect("current static source-context fallback should be present");
    assert!(
        current_static_fallback
            .contains("let current_owner = SourceContextOwnerKey::new(current_type)")
    );
    assert!(current_static_fallback.contains("let ty = current_owner.find_context_type(ctx)?"));
    assert!(
        !current_static_fallback
            .contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );

    let source_context_fallback = lower_source
        .split("fn source_context_receiver_method_fallback_signature")
        .nth(1)
        .and_then(|tail| tail.split("fn receiver_expr_uses_dyn_trait_slots").next())
        .expect("source-context receiver fallback should be present");
    assert!(source_context_fallback.contains("owner.find_context_type(ctx)"));
    assert!(
        !source_context_fallback
            .contains("find_context_type_by_key_in_lower_ctx(ctx, owner.as_str())")
    );
    let source_call_owner_candidates = lower_source
        .split("fn source_call_target_owner_candidates")
        .nth(1)
        .and_then(|tail| tail.split("fn lowered_source_receiver_type_name").next())
        .expect("source call target owner candidates should remain discoverable");
    assert!(source_call_owner_candidates.contains("let owner_key = SourceContextOwnerKey::new"));
    assert!(source_call_owner_candidates.contains("owner_key.find_context_type(ctx)"));
    assert!(source_call_owner_candidates.contains(".find_emitted_context_type_and_program(ctx)"));
    assert!(
        !source_call_owner_candidates
            .contains("find_emitted_context_type_by_key_in_lower_ctx(ctx, owner)")
    );
    let lowered_receiver_path_owner = lower_source
        .split("fn lowered_source_receiver_path_type_name")
        .nth(1)
        .and_then(|tail| tail.split("fn receiver_shape_java_type").next())
        .expect("lowered source receiver path owner helper should remain discoverable");
    assert!(
        lowered_receiver_path_owner.contains("SourceContextOwnerKey::new(owner.as_str())")
            && lowered_receiver_path_owner.contains(".find_emitted_context_type_and_program(ctx)")
    );
    assert!(
        !lowered_receiver_path_owner
            .contains("find_emitted_context_type_by_key_in_lower_ctx(ctx, owner)")
    );
    let same_outer_owner = lower_source
        .split("fn same_outer_inner_constructor_owner")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn find_emitted_context_type_by_rust_use_name")
                .next()
        })
        .expect("same-outer inner-constructor owner helper should remain discoverable");
    assert!(
        same_outer_owner.contains("SourceContextOwnerKey::new(owner)")
            && same_outer_owner.contains(".find_emitted_context_type_and_program(ctx)")
    );
    assert!(
        !same_outer_owner.contains("find_emitted_context_type_by_key_in_lower_ctx(ctx, owner)")
    );

    let nested_owner_candidates = lower_source
        .split("fn nested_type_owner_candidates_for_receiver")
        .nth(1)
        .and_then(|tail| tail.split("fn collect_nested_type_owner_candidates").next())
        .expect("nested owner candidate helper should be present");
    assert!(
        nested_owner_candidates
            .contains("let current_owner = SourceContextOwnerKey::new(current_type)")
    );
    assert!(nested_owner_candidates.contains("current_owner.find_context_type(ctx)"));
    assert!(
        !nested_owner_candidates
            .contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );

    let receiverless_summary_return_shape = lower_source
        .split("fn source_summary_declared_receiverless_method_return_shape")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn source_summary_declared_inherited_method")
                .next()
        })
        .expect("receiverless summary return shape helper should be present");
    assert!(
        receiverless_summary_return_shape.contains("current_owner: &SourceContextOwnerKey"),
        "receiverless summary return-shape lookup should keep current owner identity wrapped"
    );
    assert!(
        receiverless_summary_return_shape
            .contains("current_owner.find_context_type_and_program(ctx)?")
    );
    assert!(
        !receiverless_summary_return_shape
            .contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );
    let inherited_summary_method = lower_source
        .split("fn source_summary_declared_inherited_method")
        .nth(1)
        .and_then(|tail| tail.split("fn lower_switch").next())
        .expect("inherited summary method helper should be present");
    assert!(
        inherited_summary_method.contains("source_context_super_type_for_decl("),
        "inherited summary lookup should keep superclass lookup on the wrapped context-super resolver"
    );
    assert!(
        !inherited_summary_method.contains("find_context_type_by_key_in_lower_ctx(ctx, &type_key)")
    );

    let returned_field_lookup = lower_source
        .split("fn source_method_returned_field_name")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn source_null_assertion_call_expr_stmt_is_noop")
                .next()
        })
        .expect("source returned-field lookup helper should be present");
    assert!(
        returned_field_lookup.contains("receiver_owner: &SourceContextOwnerKey"),
        "source returned-field lookup should keep owner identity wrapped at the helper boundary"
    );
    assert!(returned_field_lookup.contains("receiver_owner.find_context_type(ctx)"));
    assert!(!returned_field_lookup.contains("find_context_type_by_key_in_lower_ctx(ctx, receiver"));

    for (helper_name, next_helper) in [
        (
            "fn static_storage_reference_expr",
            "fn primitive_rust_cast_type",
        ),
        (
            "fn static_string_storage_reference_expr",
            "fn static_field_decl_by_name",
        ),
        (
            "fn source_instance_field_rust_name(",
            "fn lower_constructor_field_assignment_value",
        ),
        (
            "fn unsupported_source_constructor_call_expr",
            "fn substitute_source_constructor_param_expr_vars_by_name_compat",
        ),
    ] {
        let helper = lower_source
            .split(helper_name)
            .nth(1)
            .and_then(|tail| tail.split(next_helper).next())
            .expect("source owner lookup helper should be present");
        assert!(
            helper.contains("SourceContextOwnerKey::new")
                && (helper.contains(".find_context_type(ctx)")
                    || helper.contains(".find_context_type_and_program(ctx)")),
            "{helper_name} should keep owner lookup on SourceContextOwnerKey"
        );
        assert!(
            !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, owner")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, type_name"),
            "{helper_name} should not reopen raw source-owner context lookup"
        );
    }

    for (helper_name, next_helper) in [
        (
            "fn seed_outer_this_capture_fields",
            "fn imported_static_owner_type_name",
        ),
        (
            "fn source_enum_directly_implements_rust_trait_name",
            "fn source_enum_directly_implements_target_interface",
        ),
        (
            "fn source_enum_directly_implements_target_interface",
            "fn java_enum_static_field_owner_candidates",
        ),
        (
            "fn retarget_erased_source_static_call_to_free_function",
            "fn static_call_plan_rust_name",
        ),
        (
            "fn static_field_access_is_associated_const",
            "fn static_storage_access_name_for_owner",
        ),
        (
            "fn static_storage_access_name_for_owner",
            "fn static_storage_field_shape_for_owner",
        ),
        (
            "fn find_static_storage_owner_field",
            "fn lower_static_storage_assignment_value_expr",
        ),
        (
            "fn matches_source_java_enum_type",
            "fn static_storage_field_shape",
        ),
        (
            "fn static_storage_owner_type_name",
            "fn source_type_name_from_expr",
        ),
        (
            "fn source_type_name_from_expr",
            "fn nested_type_decl_by_receiver_alias",
        ),
        (
            "fn context_method_effective_rust_name",
            "fn context_fallback_signature_for_signature",
        ),
        (
            "fn source_or_jdk_subtype_reaches_in_lower_ctx",
            "#[cfg(test)]\nfn context_type_simple_name_is_ambiguous",
        ),
        (
            "fn source_supertype_conversion_impls_for_type",
            "fn source_supertype_conversion_impls_for_pair",
        ),
        (
            "fn context_type_simple_name_is_ambiguous_in_lower_ctx",
            "fn is_known_exception_supertype_name",
        ),
        (
            "fn interface_trait_argument_shapes_for_call_owner",
            "fn method_signature_for_owner_aliases",
        ),
        (
            "fn native_collections_sort_method_reference_returns_result",
            "fn render_native_system_print_stream_stmt",
        ),
        (
            "fn lower_source_associated_const_field_access",
            "fn lower_source_nested_type_access",
        ),
        (
            "fn lower_source_nested_type_access",
            "fn lower_java_array_length_access",
        ),
        (
            "fn source_java_enum_static_owner_from_expr",
            "fn context_source_enum_type_name_by_key",
        ),
        (
            "fn context_source_enum_type_name_by_key",
            "fn current_type_nested_enum_name_by_alias",
        ),
        (
            "fn retarget_source_declared_static_overload_name",
            "fn retarget_source_declared_overload_name",
        ),
        (
            "fn source_declared_instance_method_rust_name",
            "fn receiver_external_type_key",
        ),
        (
            "fn lower_source_constructor_expr_for_concrete_slot",
            "fn lower_external_crate_constructor_expr_for_slot",
        ),
        (
            "fn source_class_upcast_hierarchy_names",
            "fn type_param_source_class_upcast_bound_requiring_hierarchy_lowering",
        ),
        (
            "fn erased_object_source_supertype_instanceof_requires_hierarchy_lowering",
            "fn erased_object_source_supertype_cast_requires_hierarchy_lowering",
        ),
        (
            "fn erased_object_source_supertype_cast_requires_hierarchy_lowering",
            "fn source_context_has_class_subtype_reaching",
        ),
        (
            "fn optional_current_method_display_actual_shape",
            "fn lower_optional_array_expr_for_slice_slot",
        ),
        (
            "fn source_static_field_receiver_method_candidates",
            "fn source_context_resolved_static_method_fallback_signature",
        ),
        (
            "fn source_context_resolved_static_method_fallback_signature",
            "fn explicit_static_receiver_matches_resolved_owner",
        ),
    ] {
        let helper = lower_source
            .split(helper_name)
            .nth(1)
            .and_then(|tail| tail.split(next_helper).next())
            .expect("source owner utility helper should be present");
        assert!(
            helper.contains("SourceContextOwnerKey::new")
                && (helper.contains(".find_context_type(ctx)")
                    || helper.contains(".find_context_type_and_program(ctx)")),
            "{helper_name} should keep owner lookup on SourceContextOwnerKey"
        );
        assert!(
            !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, owner")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, type_name")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, target_name")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, receiver_name")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, actual_name")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, owner_key")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, &owner")
                && !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, &owner_key"),
            "{helper_name} should not reopen raw source-owner context lookup"
        );
    }
}

#[test]
fn enum_current_owner_lookup_stays_wrapped_until_context_lookup() {
    let lower_source = include_str!("../src/lower.rs");
    let static_field_candidates = lower_source
        .split("fn java_enum_static_field_owner_candidates")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn upper_camel_type_name_from_static_receiver")
                .next()
        })
        .expect("enum static-field owner candidate helper should be present");
    assert!(
        static_field_candidates
            .contains("SourceContextOwnerKey::new(current_type).find_context_type(ctx)")
    );
    assert!(
        !static_field_candidates
            .contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );

    let nested_enum_owner = lower_source
        .split("fn current_type_nested_enum_name_by_alias")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn lower_static_value_of_string_name_call")
                .next()
        })
        .expect("current-type nested enum owner helper should be present");
    assert!(
        nested_enum_owner
            .contains("SourceContextOwnerKey::new(current_type).find_context_type(ctx)?")
    );
    assert!(
        !nested_enum_owner.contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );

    let source_enum_current_type = lower_source
        .split("fn find_source_enum_type_decl_for_current_type")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn find_source_enum_type_decl_for_java_type")
                .next()
        })
        .expect("source enum current-type lookup helper should be present");
    assert!(source_enum_current_type.contains("SourceContextOwnerKey::new(current_type)"));
    assert!(source_enum_current_type.contains(".find_context_type(ctx)"));
    assert!(
        !source_enum_current_type
            .contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );
}

#[test]
fn current_field_owner_lookup_stays_wrapped_until_context_lookup() {
    let lower_source = include_str!("../src/lower.rs");
    let current_field_shape = lower_source
        .split("fn current_type_field_shape")
        .nth(1)
        .and_then(|tail| tail.split("fn lower_unary_expr_with_refinements").next())
        .expect("current field shape helper should be present");
    assert!(current_field_shape.contains("SourceContextOwnerKey::new(current_type)"));
    assert!(current_field_shape.contains(".find_context_type(ctx)"));
    assert!(
        !current_field_shape.contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );

    let current_source_field_name = lower_source
        .split("fn current_type_source_instance_field_rust_name")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn source_java_enum_instance_field_accessor_name")
                .next()
        })
        .expect("current source field-name helper should be present");
    assert!(current_source_field_name.contains("SourceContextOwnerKey::new(current_type)"));
    assert!(current_source_field_name.contains(".find_context_type(ctx)"));
    assert!(
        !current_source_field_name
            .contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );

    let static_final_array_field_type = lower_source
        .split("fn static_final_const_array_field_type")
        .nth(1)
        .and_then(|tail| tail.split("fn static_slice_const_array_expr_type").next())
        .expect("static-final const-array field type helper should be present");
    assert!(static_final_array_field_type.contains("SourceContextOwnerKey::new(current_type)"));
    assert!(static_final_array_field_type.contains(".find_context_type(ctx)"));
    assert!(
        !static_final_array_field_type
            .contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)")
    );
}

#[test]
fn current_owner_utility_lookups_stay_wrapped_until_context_lookup() {
    let lower_source = include_str!("../src/lower.rs");
    for (helper_name, next_helper) in [
        (
            "fn unresolved_current_source_call_is_ambiguous_null_overload",
            "fn expr_is_null_literal",
        ),
        (
            "fn java_type_is_in_scope_type_parameter",
            "fn simple_type_parameter_candidate_name",
        ),
        (
            "fn lower_interface_super_default_call",
            "fn interface_super_implemented_name",
        ),
        (
            "fn current_method_exception_plan",
            "fn external_facade_target_returns_result",
        ),
        (
            "fn lower_static_initialized_singleton_field_for_slot",
            "/// Lowers a lambda written directly at a functional-interface parameter",
        ),
    ] {
        let helper = lower_source
            .split(helper_name)
            .nth(1)
            .and_then(|tail| tail.split(next_helper).next())
            .expect("current-owner utility helper should be present");
        assert!(
            helper.contains("SourceContextOwnerKey::new(current_type)")
                && helper.contains(".find_context_type(ctx)"),
            "{helper_name} should keep current-owner lookup on SourceContextOwnerKey"
        );
        assert!(
            !helper.contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)"),
            "{helper_name} should not reopen raw current_type context lookup"
        );
    }
    let production_source = lower_source
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("production lower.rs source should precede tests");
    assert!(
        !production_source.contains("find_context_type_by_key_in_lower_ctx(ctx, current_type)"),
        "production current-owner lookups should stay behind SourceContextOwnerKey"
    );
    assert!(
        !production_source
            .contains("find_context_type_by_key_in_lower_ctx(ctx, ctx.current_type?)"),
        "production direct ctx.current_type lookups should stay behind SourceContextOwnerKey"
    );
    assert!(
        !production_source
            .contains("find_context_type_by_key_in_lower_ctx(ctx, &resolved.declaring_type)"),
        "production resolved-owner context lookups should stay behind SourceContextOwnerKey"
    );
    assert_eq!(
        production_source
            .matches("find_context_type_by_key_in_lower_ctx(ctx,")
            .count(),
        1,
        "production context type lookup should be reopened only by SourceContextOwnerKey::find_context_type"
    );
}

#[test]
fn signature_diagnostics_route_fields_through_capability_audit() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("fn push_field_signature_diagnostics"));
    assert!(source.contains(
        "push_capability_diagnostics_for_signature_constraints(\n        diagnostics,\n        slot,\n        &field.ty,"
    ));
    assert!(source.contains("push_field_signature_diagnostics("));
}

#[test]
fn usage_owns_call_site_boundary_facts() {
    let usage_source = include_str!("../src/usage.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production slot policy source should be present");
    let method_usage_source = usage_source
        .split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct MethodUsageFacts")
        .nth(1)
        .and_then(|tail| tail.split("impl UsageFacts").next())
        .expect("MethodUsageFacts source should remain discoverable");

    assert!(usage_source.contains("pub struct CallSiteUse"));
    let call_site_use_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct CallSiteUse")
        .nth(1)
        .and_then(|tail| tail.split("impl CallSiteUse").next())
        .expect("CallSiteUse source should remain discoverable");
    let constructor_call_site_use_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ConstructorCallSiteUse")
        .nth(1)
        .and_then(|tail| tail.split("impl ConstructorCallSiteUse").next())
        .expect("ConstructorCallSiteUse source should remain discoverable");
    let returned_call_result_use_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ReturnedCallResultUse")
        .nth(1)
        .and_then(|tail| tail.split("impl ReturnedCallResultUse").next())
        .expect("ReturnedCallResultUse source should remain discoverable");
    for field in [
        "pub name: String",
        "pub arg_count: usize",
        "pub result_returned: bool",
        "pub receiver_is_self: bool",
        "pub receiver_targets_self_field: bool",
        "pub receiver_type: Option<JavaType>",
        "pub null_arg_indices: Vec<usize>",
        "pub nullable_source_call_arg_indices: Vec<usize>",
        "pub boundary: CallArgumentBoundary",
        "pub resolved: Option<ResolvedCall>",
    ] {
        assert!(
            !call_site_use_source.contains(field),
            "CallSiteUse should expose accessors, not raw public fields: {field}"
        );
    }
    for field in [
        "pub name: String",
        "pub arg_count: usize",
        "pub null_arg_indices: Vec<usize>",
        "pub nullable_source_call_arg_indices: Vec<usize>",
        "pub boundary: CallArgumentBoundary",
        "pub resolved: Option<ResolvedConstructor>",
    ] {
        assert!(
            !constructor_call_site_use_source.contains(field),
            "ConstructorCallSiteUse should expose accessors, not raw public fields: {field}"
        );
    }
    for field in [
        "pub name: String",
        "pub arg_count: usize",
        "pub receiver_type: Option<JavaType>",
        "pub boundary: CallArgumentBoundary",
        "pub resolved: Option<ResolvedCall>",
    ] {
        assert!(
            !returned_call_result_use_source.contains(field),
            "ReturnedCallResultUse should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(!method_usage_source.contains("pub call_sites: Vec<CallSiteUse>"));
    assert!(
        !method_usage_source.contains("pub constructor_call_sites: Vec<ConstructorCallSiteUse>")
    );
    assert!(!method_usage_source.contains("pub returned_call_results: Vec<ReturnedCallResultUse>"));
    assert!(!method_usage_source.contains("pub field_uses: Vec<FieldUse>"));
    assert!(!method_usage_source.contains("pub mutates_self: bool"));
    assert!(usage_source.contains("pub fn call_sites(&self) -> &[CallSiteUse]"));
    assert!(
        usage_source.contains("pub fn constructor_call_sites(&self) -> &[ConstructorCallSiteUse]")
    );
    assert!(
        usage_source.contains("pub fn returned_call_results(&self) -> &[ReturnedCallResultUse]")
    );
    assert!(usage_source.contains("pub fn field_uses(&self) -> &[FieldUse]"));
    assert!(usage_source.contains("pub fn mutates_self(&self) -> bool"));
    assert!(!usage_source.contains("fn from_field_uses("));
    assert!(usage_source.contains("pub struct MethodApiBoundaryPolicy"));
    let call_result_policy_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct CallResultBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl CallResultBoundaryPolicy").next())
        .expect("CallResultBoundaryPolicy source should remain discoverable");
    let constructor_invocation_policy_source = usage_source
        .split(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct ConstructorInvocationBoundaryPolicy",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl ConstructorInvocationBoundaryPolicy").next())
        .expect("ConstructorInvocationBoundaryPolicy source should remain discoverable");
    let call_argument_policy_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct CallArgumentBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl CallArgumentBoundaryPolicy").next())
        .expect("CallArgumentBoundaryPolicy source should remain discoverable");
    let method_api_policy_source = usage_source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct MethodApiBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl MethodApiBoundaryPolicy").next())
        .expect("MethodApiBoundaryPolicy source should remain discoverable");
    for field in [
        "pub name: String",
        "pub arg_count: usize",
        "pub receiver_type: Option<JavaType>",
        "pub boundary: CallArgumentBoundary",
        "pub resolved: Option<ResolvedCall>",
    ] {
        assert!(
            !call_result_policy_source.contains(field),
            "CallResultBoundaryPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    for field in [
        "pub kind: ConstructorInvocationKind",
        "pub boundary: CallArgumentBoundary",
    ] {
        assert!(
            !constructor_invocation_policy_source.contains(field),
            "ConstructorInvocationBoundaryPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    for field in ["pub name: String", "pub boundary: CallArgumentBoundary"] {
        assert!(
            !call_argument_policy_source.contains(field),
            "CallArgumentBoundaryPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    for field in ["pub public_api: bool"] {
        assert!(
            !method_api_policy_source.contains(field),
            "MethodApiBoundaryPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(usage_source.contains("pub fn name(&self) -> &str"));
    assert!(usage_source.contains("pub fn arg_count(&self) -> usize"));
    assert!(usage_source.contains("pub fn result_returned(&self) -> bool"));
    assert!(usage_source.contains("pub fn receiver_is_self(&self) -> bool"));
    assert!(usage_source.contains("pub fn receiver_targets_self_field(&self) -> bool"));
    assert!(usage_source.contains("pub fn receiver_type(&self) -> Option<&JavaType>"));
    assert!(usage_source.contains("pub fn null_arg_indices(&self) -> &[usize]"));
    assert!(usage_source.contains("pub fn nullable_source_call_arg_indices(&self) -> &[usize]"));
    assert!(usage_source.contains("pub fn boundary(&self) -> CallArgumentBoundary"));
    assert!(usage_source.contains("pub fn resolved(&self) -> Option<&ResolvedCall>"));
    assert!(usage_source.contains("pub fn resolved(&self) -> Option<&ResolvedConstructor>"));
    assert!(usage_source.contains("pub fn kind(&self) -> ConstructorInvocationKind"));
    assert!(usage_source.contains("pub fn public_api(&self) -> bool"));
    assert!(usage_source.contains("pub fn method_api_boundary_policy"));
    assert!(usage_source.contains("pub fn call_argument_boundary_policy"));
    assert!(!usage_source.contains("fn method_is_api_boundary"));
    assert!(!slot_policy_source.contains("ResolvedCall"));
    assert!(!slot_policy_source.contains("resolved"));
}

#[test]
fn semantic_diagnostics_are_plumbed_through_translation_boundary() {
    let lib_source = include_str!("../src/lib.rs");
    let semantic_source = include_str!("../src/semantic_analysis.rs");
    let rewrite_source = include_str!("../src/rewrite.rs");
    let facts_source = include_str!("../src/facts.rs");
    let diagnostics_source = include_str!("../src/diagnostics.rs");

    assert!(semantic_source.contains("SemanticDiagnostic"));
    assert!(semantic_source.contains("semantic_external_api_effect_budget_exhausted"));
    assert!(lib_source.contains("for diagnostic in semantic.diagnostics"));
    assert!(lib_source.contains(".warning(diagnostic.code(), diagnostic.message().to_string())"));
    let diagnostic_source = diagnostics_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct Diagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl Diagnostic").next())
        .expect("diagnostics::Diagnostic source should remain discoverable");
    for field in [
        "pub severity: Severity",
        "pub code: &'static str",
        "pub message: String",
        "pub span: Option<Span>",
    ] {
        assert!(
            !diagnostic_source.contains(field),
            "diagnostics::Diagnostic should expose accessors, not raw public fields: {field}"
        );
    }
    let span_source = diagnostics_source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]\npub struct Span")
        .nth(1)
        .and_then(|tail| tail.split("impl Span").next())
        .expect("diagnostics::Span source should remain discoverable");
    for field in ["pub start: usize", "pub end: usize"] {
        assert!(
            !span_source.contains(field),
            "diagnostics::Span should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(diagnostics_source.contains("pub fn severity(&self) -> Severity"));
    assert!(diagnostics_source.contains("pub fn code(&self) -> &'static str"));
    assert!(diagnostics_source.contains("pub fn message(&self) -> &str"));
    assert!(diagnostics_source.contains("pub fn span(&self) -> Option<Span>"));
    assert!(diagnostics_source.contains("pub fn start(&self) -> usize"));
    assert!(diagnostics_source.contains("pub fn end(&self) -> usize"));
    let semantic_diagnostic_source = semantic_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SemanticDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl SemanticDiagnostic").next())
        .expect("SemanticDiagnostic source should remain discoverable");
    assert!(!semantic_diagnostic_source.contains("pub code: &'static str"));
    assert!(!semantic_diagnostic_source.contains("pub message: String"));
    assert!(semantic_source.contains("pub fn code(&self) -> &'static str"));
    assert!(semantic_source.contains("pub fn message(&self) -> &str"));
    let semantic_iterations_source = semantic_source
        .split(
            "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SemanticAnalysisIterations",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl SemanticAnalysisIterations").next())
        .expect("SemanticAnalysisIterations source should remain discoverable");
    assert!(!semantic_iterations_source.contains("pub field_nullability: usize"));
    assert!(semantic_source.contains("pub fn new(field_nullability: usize) -> Self"));
    assert!(semantic_source.contains("pub fn field_nullability(self) -> usize"));
    let semantic_config_source = semantic_source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SemanticAnalysisConfig")
        .nth(1)
        .and_then(|tail| tail.split("impl Default for SemanticAnalysisConfig").next())
        .expect("SemanticAnalysisConfig source should remain discoverable");
    for field in [
        "pub max_external_guess_iterations: Option<usize>",
        "pub enable_external_api_guesses: bool",
        "pub enable_control_flow_facts: bool",
        "pub enable_builtin_external_packs: bool",
    ] {
        assert!(
            !semantic_config_source.contains(field),
            "SemanticAnalysisConfig should expose accessors/builders, not raw public fields: {field}"
        );
    }
    assert!(
        semantic_source.contains("pub fn max_external_guess_iterations(self) -> Option<usize>")
    );
    assert!(semantic_source.contains("pub fn external_api_guesses_enabled(self) -> bool"));
    assert!(semantic_source.contains("pub fn control_flow_facts_enabled(self) -> bool"));
    assert!(semantic_source.contains("pub fn builtin_external_packs_enabled(self) -> bool"));
    assert!(semantic_source.contains(
        "pub fn with_max_external_guess_iterations(mut self, iterations: Option<usize>) -> Self"
    ));
    assert!(
        semantic_source
            .contains("pub fn with_external_api_guesses_enabled(mut self, enabled: bool) -> Self")
    );
    assert!(
        semantic_source
            .contains("pub fn with_control_flow_facts_enabled(mut self, enabled: bool) -> Self")
    );
    assert!(
        semantic_source.contains(
            "pub fn with_builtin_external_packs_enabled(mut self, enabled: bool) -> Self"
        )
    );
    let control_flow_diagnostic_source = rewrite_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ControlFlowRewriteDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl ControlFlowRewriteDiagnostic").next())
        .expect("rewrite::ControlFlowRewriteDiagnostic struct source should be present");
    assert!(!control_flow_diagnostic_source.contains("pub code: &'static str"));
    assert!(!control_flow_diagnostic_source.contains("pub message: String"));
    assert!(
        rewrite_source.contains("impl ControlFlowRewriteDiagnostic")
            && rewrite_source
                .contains("pub fn new(code: &'static str, message: impl Into<String>)")
            && rewrite_source.contains("pub fn code(&self) -> &'static str")
            && rewrite_source.contains("pub fn message(&self) -> &str"),
        "control-flow rewrite diagnostics should expose constructor/accessors, not raw fields"
    );
    let fact_diagnostic_source = facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FactDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl FactDiagnostic").next())
        .expect("facts::FactDiagnostic struct source should be present");
    assert!(!fact_diagnostic_source.contains("pub code: &'static str"));
    assert!(!fact_diagnostic_source.contains("pub message: String"));
    assert!(
        facts_source.contains("impl FactDiagnostic")
            && facts_source.contains("pub fn new(code: &'static str, message: impl Into<String>)")
            && facts_source.contains("pub fn code(&self) -> &'static str")
            && facts_source.contains("pub fn message(&self) -> &str")
            && facts_source.contains("pub fn into_parts(self) -> (&'static str, String)"),
        "fact diagnostics should expose constructor/accessors, not raw fields"
    );
}

#[test]
fn indexed_assignment_target_shape_consumes_inferred_array_shape() {
    let source = include_str!("../src/expr_shape.rs");

    assert!(source.contains("fn infer_indexed_assignment_target_shape"));
    assert!(source.contains("&infer_expr_shape_with_context(array, env, ctx)"));
    assert!(!source.contains("RustSlotShape::conservative(infer_expr_type(array"));
}

#[test]
fn statement_render_renders_local_types_with_facts() {
    let source = include_str!("../src/statement_render.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(source.contains("pub(crate) fn local_let_stmt_with_facts"));
    assert!(!source.contains("pub fn local_let_stmt_with_facts"));
    assert!(source.contains("fn local_let_stmt_with_facts_and_type_annotation"));
    assert!(!source.contains("pub fn local_let_stmt_with_facts_and_type_annotation"));
    assert!(
        source.contains(
            "pub(crate) fn local_let_stmt_with_facts_representations_and_type_annotation"
        )
    );
    assert!(
        !source.contains("pub fn local_let_stmt_with_facts_representations_and_type_annotation")
    );
    assert!(source.contains("rust_type_for_slot_with_facts"));
    assert!(!source.contains("rust_type_for_slot(&local.shape)"));
    assert!(lower_source.contains("local_let_stmt_with_facts"));
}

#[test]
fn statement_render_simple_constructors_stay_crate_local() {
    let source = include_str!("../src/statement_render.rs");

    for helper in [
        "assignment_stmt",
        "assignment_op_stmt",
        "expr_stmt",
        "if_stmt",
        "if_let_stmt",
        "while_stmt",
        "while_let_stmt",
        "break_stmt",
        "continue_stmt",
    ] {
        assert!(
            source.contains(&format!("pub(crate) fn {helper}")),
            "{helper} should remain crate-local"
        );
        assert!(
            !source.contains(&format!("pub fn {helper}")),
            "{helper} should not be part of the public test-facing API"
        );
    }
}

#[test]
fn conversion_lowering_cast_and_instanceof_helpers_stay_crate_local() {
    let source = include_str!("../src/conversion_lowering.rs");

    assert!(source.contains("pub(crate) fn apply_cast_conversion"));
    assert!(source.contains("pub(crate) fn apply_instanceof"));
    assert!(!source.contains("pub fn apply_cast_conversion"));
    assert!(!source.contains("pub fn apply_instanceof"));
}

#[test]
fn access_lowering_helpers_stay_crate_local() {
    let source = include_str!("../src/access_lowering.rs");

    assert!(source.contains("pub(crate) fn lower_bare_name"));
    assert!(source.contains("pub(crate) fn lower_field_access_with_facts"));
    assert!(!source.contains("pub fn lower_bare_name"));
    assert!(!source.contains("pub fn lower_field_access_with_facts"));
}

#[test]
fn array_lowering_expression_builder_stays_test_local() {
    let source = include_str!("../src/array_lowering.rs");
    let production_source = source
        .split("#[cfg(test)]")
        .next()
        .expect("array lowering production source should be present");

    assert!(source.contains("fn array_expr"));
    assert!(!production_source.contains("fn array_expr("));
    assert!(!source.contains("pub(crate) fn array_expr("));
    assert!(!source.contains("pub fn array_expr"));
}

#[test]
fn source_class_bound_trait_name_stays_crate_local() {
    let source = include_str!("../src/representation_lowering.rs");

    assert!(source.contains("pub(crate) fn source_class_bound_trait_name"));
    assert!(!source.contains("pub fn source_class_bound_trait_name"));
}

#[test]
fn stdlib_does_not_grow_non_jdk_domain_package_knowledge() {
    let stdlib_source = include_str!("../src/stdlib.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production stdlib source should be present");
    for forbidden in [
        "htsjdk",
        "gatk",
        "hellbender",
        "hdf5",
        "sqlite",
        "arrow",
        "spark",
        "genomicsdb",
        "disq",
        "picard",
        "barclay",
        "broadinstitute",
        "org.apache",
        "org.disq",
        "com.intel",
    ] {
        assert!(
            !stdlib_source.contains(forbidden),
            "non-JDK package knowledge belongs in javalibs or external packs, not stdlib.rs: {forbidden}"
        );
    }

    assert_eq!(stdlib_source.matches("cloud_storage_").count(), 2);
    assert_eq!(stdlib_source.matches("CloudStorage").count(), 2);

    let javalibs_source = include_str!("../src/javalibs.rs");
    assert!(javalibs_source.contains("pub(crate) fn is_builtin_external_facade_type"));
    assert!(javalibs_source.contains("pub(crate) fn broad_utils_dynamic_return_type"));
    assert!(javalibs_source.contains("pub(crate) fn broad_static_utility_return_type"));
    assert!(
        javalibs_source
            .contains("pub(crate) fn imported_static_method_returns_broad_utils_first_arg")
    );
    assert!(javalibs_source.contains("pub(crate) fn is_broad_utils_type_name"));
    assert!(javalibs_source.contains("pub(crate) fn needs_conversion_constructor_for_facade"));
    assert!(javalibs_source.contains("pub(crate) fn is_static_facade_method"));
    assert!(javalibs_source.contains("pub(crate) fn is_jdk_input_stream_facade_type"));
    assert!(javalibs_source.contains("pub(crate) fn is_known_java_subtype"));
    assert!(javalibs_source.contains("pub(crate) fn is_known_exception_supertype"));
    assert!(!javalibs_source.contains("pub fn is_builtin_external_facade_type"));
    assert!(!javalibs_source.contains("pub fn broad_utils_dynamic_return_type"));
    assert!(!javalibs_source.contains("pub fn broad_static_utility_return_type"));
    assert!(
        !javalibs_source.contains("pub fn imported_static_method_returns_broad_utils_first_arg")
    );
    assert!(!javalibs_source.contains("pub fn is_broad_utils_type_name"));
    assert!(!javalibs_source.contains("pub fn needs_conversion_constructor_for_facade"));
    assert!(!javalibs_source.contains("pub fn is_static_facade_method"));
    assert!(!javalibs_source.contains("pub fn is_jdk_input_stream_facade_type"));
    assert!(!javalibs_source.contains("pub fn is_known_java_subtype"));
    assert!(!javalibs_source.contains("pub fn is_known_exception_supertype"));
}

#[test]
fn stdlib_erased_display_downcast_collectors_stay_internal() {
    let stdlib_full_source = include_str!("../src/stdlib.rs");

    for helper in [
        "source_erased_object_to_string_downcasts",
        "platform_erased_object_to_string_downcasts",
        "external_erased_object_to_string_downcasts",
        "erased_display_to_string_downcast_arms",
    ] {
        assert!(stdlib_full_source.contains(&format!("fn {helper}")));
        assert!(!stdlib_full_source.contains(&format!("pub fn {helper}")));
        assert!(!stdlib_full_source.contains(&format!("pub(crate) fn {helper}")));
    }
    assert_eq!(
        stdlib_full_source
            .matches("erased_display_to_string_downcast_arms(")
            .count(),
        5,
        "platform, external, ordinary source, and concrete generic-source erased-display collectors should share one carrier-arm template"
    );
    assert!(stdlib_full_source.contains("pub(crate) fn platform_type_has_erased_display_support"));
    assert!(!stdlib_full_source.contains("pub fn platform_type_has_erased_display_support"));
    assert!(stdlib_full_source.contains("pub(crate) fn source_summary_method_return_nullability"));
    assert!(!stdlib_full_source.contains("pub fn source_summary_method_return_nullability"));
    assert!(
        stdlib_full_source.contains("pub(crate) fn external_facade_receiver_call_is_noop_method")
    );
    assert!(!stdlib_full_source.contains("pub fn external_facade_receiver_call_is_noop_method"));
    assert!(
        stdlib_full_source.contains("pub(crate) fn native_collection_method_name_mutates_receiver")
    );
    assert!(!stdlib_full_source.contains("pub fn native_collection_method_name_mutates_receiver"));
    for helper in [
        "external_api_method_role",
        "external_constructor_provenance_with_builtin_packs",
        "external_field_provenance_with_builtin_packs",
    ] {
        assert!(stdlib_full_source.contains(&format!("pub(crate) fn {helper}")));
        assert!(!stdlib_full_source.contains(&format!("pub fn {helper}")));
    }
    for helper in [
        "is_known_receiverless_static_utility_method",
        "has_receiver_agnostic_external_return_type",
        "is_known_object_inherited_method",
        "native_collection_receiver_mutates",
        "static_collection_argument_mutates",
    ] {
        assert!(stdlib_full_source.contains(&format!("pub(crate) fn {helper}")));
        assert!(!stdlib_full_source.contains(&format!("pub fn {helper}")));
    }
    assert!(stdlib_full_source.contains("pub(crate) fn java_type_is_array_blocking_queue"));
    assert!(!stdlib_full_source.contains("pub fn java_type_is_array_blocking_queue"));
    assert!(
        stdlib_full_source.contains("pub(crate) fn program_mentions_java_array_blocking_queue")
    );
    assert!(!stdlib_full_source.contains("pub fn program_mentions_java_array_blocking_queue"));
}

#[test]
fn control_flow_normalization_plans_are_owned_before_lowering() {
    let semantic_source = include_str!("../src/semantic_analysis.rs");
    let rewrite_source = include_str!("../src/rewrite.rs");
    let lower_source = include_str!("../src/lower.rs");
    let control_flow_lowering_source = include_str!("../src/control_flow_lowering.rs");

    assert!(rewrite_source.contains("pub struct MethodControlFlowPlan"));
    assert!(rewrite_source.contains("AssignmentInConditionPlan"));
    assert!(rewrite_source.contains("TailRecursiveLoopPlan"));
    let method_control_flow_plan_source = rewrite_source
        .split("pub struct MethodControlFlowPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub enum ControlFlowRewritePlan").next())
        .expect("MethodControlFlowPlan should remain discoverable");
    assert!(method_control_flow_plan_source.contains("pub fn new("));
    assert!(method_control_flow_plan_source.contains("pub fn method(&self) -> &MethodKey"));
    assert!(
        method_control_flow_plan_source
            .contains("pub fn rewrites(&self) -> &[ControlFlowRewritePlan]")
    );
    assert!(!method_control_flow_plan_source.contains("pub method: MethodKey"));
    assert!(!method_control_flow_plan_source.contains("pub rewrites: Vec<ControlFlowRewritePlan>"));
    let assignment_plan_source = rewrite_source
        .split("pub struct AssignmentInConditionPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub enum NullTestKind").next())
        .expect("AssignmentInConditionPlan should remain discoverable");
    assert!(assignment_plan_source.contains("pub fn new("));
    assert!(assignment_plan_source.contains("pub fn local_name(&self) -> &str"));
    assert!(assignment_plan_source.contains("pub fn source_call(&self) -> &str"));
    assert!(assignment_plan_source.contains("pub fn null_test(&self) -> NullTestKind"));
    assert!(
        assignment_plan_source
            .contains("pub fn normalized_shape(&self) -> AssignmentConditionNormalizedShape")
    );
    for field in [
        "pub local_name: String",
        "pub source_call: String",
        "pub null_test: NullTestKind",
        "pub normalized_shape: AssignmentConditionNormalizedShape",
    ] {
        assert!(
            !assignment_plan_source.contains(field),
            "AssignmentInConditionPlan should expose accessors, not raw public fields: {field}"
        );
    }
    let tail_plan_source = rewrite_source
        .split("pub struct TailRecursiveLoopPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub enum TailRecursiveNormalizedShape").next())
        .expect("TailRecursiveLoopPlan should remain discoverable");
    assert!(tail_plan_source.contains("pub fn new("));
    assert!(tail_plan_source.contains("pub fn method_name(&self) -> &str"));
    assert!(tail_plan_source.contains("pub fn arg_count(&self) -> usize"));
    assert!(
        tail_plan_source.contains("pub fn normalized_shape(&self) -> TailRecursiveNormalizedShape")
    );
    for field in [
        "pub method_name: String",
        "pub arg_count: usize",
        "pub normalized_shape: TailRecursiveNormalizedShape",
    ] {
        assert!(
            !tail_plan_source.contains(field),
            "TailRecursiveLoopPlan should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(semantic_source.contains("collect_control_flow_plans"));
    assert!(semantic_source.contains("assignment_in_condition_plan"));
    assert!(semantic_source.contains("tail_recursive_loop_plan"));
    assert!(semantic_source.contains("collect_recursion_fallback_diagnostics"));
    assert!(semantic_source.contains("recursion_tco_fallback"));
    assert!(!lower_source.contains("AssignmentInConditionPlan"));
    assert!(!lower_source.contains("TailRecursiveLoopPlan"));
    assert!(!lower_source.contains("LoopWithParameterReassignment"));
    assert!(!lower_source.contains("recursion_tco_fallback"));
    assert!(control_flow_lowering_source.contains("RustStmt::Loop"));
    assert!(control_flow_lowering_source.contains("RustStmt::If"));
    assert!(control_flow_lowering_source.contains("RustStmt::Assign"));
    assert!(control_flow_lowering_source.contains("RustStmt::Continue"));
    let tail_render_source = control_flow_lowering_source
        .split("pub(crate) struct TailRecursiveLoopRender")
        .nth(1)
        .and_then(|tail| tail.split("impl TailRecursiveLoopRender").next())
        .expect("TailRecursiveLoopRender should remain discoverable");
    for field in [
        "pub(crate) params:",
        "pub(crate) condition:",
        "pub(crate) base_return:",
        "pub(crate) next_args:",
    ] {
        assert!(
            !tail_render_source.contains(field),
            "TailRecursiveLoopRender should expose a constructor, not raw crate fields: {field}"
        );
    }
    assert!(control_flow_lowering_source.contains("pub(crate) fn new("));
    assert!(control_flow_lowering_source.contains("fn into_parts(self)"));
    assert!(!control_flow_lowering_source.contains("raw_stmt"));
    assert!(!control_flow_lowering_source.contains("emit_expr"));
    assert!(!control_flow_lowering_source.contains("format!(\"let mut"));
    assert!(!control_flow_lowering_source.contains("format!(\"    if"));
}

#[test]
fn coercion_plan_owns_instanceof_fallback_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let facts_source = include_str!("../src/facts.rs");

    assert!(source.contains("pub struct CoercionDiagnostic"));
    let coercion_diagnostic_source = source
        .split("pub struct CoercionDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl CoercionDiagnostic").next())
        .expect("CoercionDiagnostic source should remain discoverable");
    assert!(
        !coercion_diagnostic_source.contains("pub code:")
            && !coercion_diagnostic_source.contains("pub message:"),
        "CoercionDiagnostic should expose accessors, not raw public fields"
    );
    assert!(source.contains("pub fn new(code: &'static str, message: impl Into<String>)"));
    assert!(source.contains("pub fn code(&self) -> &'static str"));
    assert!(source.contains("pub fn message(&self) -> &str"));
    assert!(conversion_source.contains("pub enum InstanceOfFallbackPolicy"));
    assert!(conversion_source.contains("pub enum InstanceOfTargetPolicy"));
    assert!(!source.contains("pub enum InstanceOfFallbackPolicy"));
    assert!(!source.contains("pub enum InstanceOfTargetPolicy"));
    assert!(source.contains("ErasedObjectToDynTrait"));
    assert!(source.contains("ErasedObjectToBoxedDynTrait"));
    assert!(conversion_source.contains("pub enum InstanceOfCheck"));
    assert!(!source.contains("pub enum InstanceOfCheck"));
    assert!(source.contains("BoxedDynTraitType"));
    assert!(conversion_source.contains("policy: InstanceOfFallbackPolicy"));
    assert!(conversion_source.contains("pub fn instanceof_target_policy"));
    assert!(!conversion_source.contains("pub fn plan_instanceof_check("));
    assert!(conversion_source.contains("pub fn plan_instanceof_check_with_facts"));
    assert!(conversion_source.contains("pub struct HierarchyInstanceOfPolicy"));
    let hierarchy_instanceof_policy_source = conversion_source
        .split("pub struct HierarchyInstanceOfPolicy")
        .nth(1)
        .and_then(|tail| tail.split("pub fn hierarchy_instanceof_policy").next())
        .unwrap();
    assert!(conversion_source.contains(
        "pub(crate) fn new(actual: JavaType, target: JavaType, always_true: bool) -> Self"
    ));
    assert!(conversion_source.contains("pub fn actual(&self) -> &JavaType"));
    assert!(conversion_source.contains("pub fn target(&self) -> &JavaType"));
    assert!(conversion_source.contains("pub fn always_true(&self) -> bool"));
    assert!(!hierarchy_instanceof_policy_source.contains("pub actual: JavaType"));
    assert!(!hierarchy_instanceof_policy_source.contains("pub target: JavaType"));
    assert!(!hierarchy_instanceof_policy_source.contains("pub always_true: bool"));
    assert!(conversion_source.contains("pub fn hierarchy_instanceof_policy"));
    assert!(facts_source.contains("pub struct HierarchyReachabilityResult"));
    let hierarchy_reachability_result_source = facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct HierarchyReachabilityResult")
        .nth(1)
        .and_then(|tail| tail.split("impl HierarchyReachabilityResult").next())
        .expect("HierarchyReachabilityResult should remain discoverable");
    for field in [
        "pub edges: Vec<HierarchyEdgeFact>",
        "pub stats: WorklistStats",
        "pub diagnostics: Vec<HierarchyReachabilityDiagnostic>",
    ] {
        assert!(
            !hierarchy_reachability_result_source.contains(field),
            "HierarchyReachabilityResult should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(facts_source.contains("pub fn edges(&self) -> &[HierarchyEdgeFact]"));
    assert!(facts_source.contains("pub fn stats(&self) -> WorklistStats"));
    assert!(
        facts_source.contains("pub fn diagnostics(&self) -> &[HierarchyReachabilityDiagnostic]")
    );
    assert!(facts_source.contains(
        "Vec<HierarchyEdgeFact>,\n        WorklistStats,\n        Vec<HierarchyReachabilityDiagnostic>,"
    ));
    assert!(facts_source.contains("pub struct HierarchySubtypePolicy"));
    let hierarchy_subtype_policy_source = facts_source
        .split("pub struct HierarchySubtypePolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl HierarchySubtypePolicy").next())
        .expect("HierarchySubtypePolicy should remain discoverable");
    assert!(!hierarchy_subtype_policy_source.contains("pub reaches: bool"));
    assert!(facts_source.contains("pub fn reaches(&self) -> bool"));
    assert!(facts_source.contains("pub fn hierarchy_subtype_policy"));
    assert!(!source.contains("pub fn plan_instanceof_check("));
    assert!(!source.contains("pub fn instanceof_target_policy"));
    assert!(!source.contains("hierarchy_subtype_policy"));
    assert!(source.contains("unsupported_instanceof_shape"));
    assert!(source.contains("unsupported_instanceof_primitive_target"));
    assert!(source.contains("unsupported_instanceof_raw_or_wildcard_target"));
    assert!(source.contains("unsupported_erased_object_instanceof_dyn_trait"));
    assert!(source.contains("unsupported_erased_object_instanceof_boxed_dyn_trait"));
    assert!(source.contains("unsupported_instanceof_missing_runtime_target"));
    assert!(source.contains("instanceof_fallback_message_policy"));
    assert!(source.contains("render_instanceof_fallback_policy"));
}

#[test]
fn new_feature_responsibility_checklist_is_tracked() {
    let checklist = include_str!("../docs/new-feature-responsibility-checklist.md");

    assert!(checklist.contains("Java Semantics"));
    assert!(checklist.contains("Capabilities And External Facts"));
    assert!(checklist.contains("Object Representation"));
    assert!(checklist.contains("Signature And Slot Shape"));
    assert!(checklist.contains("Shape Projection"));
    assert!(checklist.contains("Coercion"));
    assert!(checklist.contains("Rendering And Lowering"));
    assert!(checklist.contains("Required Tests"));
    assert!(checklist.contains("ObjectRepresentationPolicy"));
    assert!(checklist.contains("SignatureBoundaryPolicy"));
    assert!(checklist.contains("SignatureCapabilityPolicy"));
    assert!(checklist.contains("lowering"));
    assert!(checklist.contains("must not recompute hierarchy"));
    assert!(checklist.contains("finite graph edges"));
    assert!(checklist.contains("bounded-domain facts"));
    assert!(checklist.contains("finite-budget diagnostic"));
    assert!(checklist.contains("recursively specialized Rust shapes"));
}

#[test]
fn recursive_code_policy_is_finite_fixpoint_not_lowering_expansion() {
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");
    let signature_source = include_str!("../src/signature.rs");
    let fact_saturation_source = include_str!("../src/fact_saturation.rs");

    assert!(design.contains("Recursive Java code"));
    assert!(design.contains("finite graph edges"));
    assert!(design.contains("bounded"));
    assert!(design.contains("finite-budget diagnostic"));
    assert!(design.contains("create unbounded path-specific facts"));
    assert!(signature_source.contains("signature_fixpoint_saturation_policy"));
    assert!(signature_source.contains("signature_fixpoint_iteration_budget"));
    assert!(signature_source.contains("signature_fixpoint_budget_exceeded"));
    assert!(signature_source.contains("FiniteSaturationPolicy"));
    assert!(fact_saturation_source.contains("FiniteFactDomain"));
    assert!(fact_saturation_source.contains("FiniteSaturationPolicy"));
    assert!(fact_saturation_source.contains("finite_fact_iteration_budget"));
    assert!(fact_saturation_source.contains("FiniteSaturationResult"));
    assert!(fact_saturation_source.contains("SaturationStep::Stable"));
}

#[test]
fn object_representation_fallback_matrix_is_documented() {
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");

    assert!(design.contains("### Object Representation Fallback Matrix"));
    assert!(design.contains("| Role | Concrete Struct | Closed Enum | Dyn Trait | Boxed Dyn Trait | Erased Object | Unsupported Hierarchy |"));
    for role in [
        "Public API param",
        "Public API return",
        "Unknown API param",
        "Unknown API return",
        "Storage/field",
        "Local",
        "Captured value",
        "External pack fact",
        "Non-object-safe interface",
        "Closed hierarchy",
        "Raw object / wildcard object",
    ] {
        assert!(design.contains(role), "missing matrix role {role}");
        let row = design
            .lines()
            .find(|line| line.starts_with(&format!("| {role} |")))
            .unwrap_or_else(|| panic!("missing matrix row {role}"));
        let cells = row
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(cells.len(), 7, "matrix row has wrong cell count: {row}");
        assert!(
            cells.iter().all(|cell| !cell.is_empty()),
            "matrix row has an empty cell: {row}"
        );
    }
    assert!(design.contains("`BoxedDynTrait` is intentionally not an `ObjectRepresentation`"));
    assert!(!design.contains("    BoxedDynTrait,\n    ClosedEnum,"));
}

#[test]
fn erased_object_runtime_contract_is_documented_and_shared() {
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");
    let coercion_source = include_str!("../src/coercion.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");

    assert!(design.contains("## Erased Object Runtime Contract"));
    for phrase in [
        "`ErasedObject` lowers to `Box<dyn std::any::Any>`",
        "storing, returning, and passing erased values preserves",
        "Box::new(value) as Box<dyn std::any::Any>",
        "downcast::<Target>()",
        "is::<Target>()",
        "erased-to-borrowed-dyn and erased-to-boxed-dyn recovery are unsupported",
        "anonymous erased boundaries and external-pack erased handles",
        "carrier instead of generating anonymous concrete implementations",
    ] {
        assert!(
            design.contains(phrase),
            "missing erased contract phrase {phrase}"
        );
    }
    assert!(coercion_source.contains("pub(crate) fn erased_object_box_projection"));
    assert!(coercion_source.contains("fn typed_method_name(method: &str, ty: &str)"));
    assert!(
        coercion_source.contains("typed_zero_arg_method_projection(receiver, \"is\", target_type)")
    );
    assert!(
        coercion_source
            .contains("typed_zero_arg_method_projection(receiver, \"downcast_ref\", target_type)")
    );
    assert!(!coercion_source.contains("format!(\"is::<{target_type}>\")"));
    assert!(
        slot_render_source
            .contains("DispatchShape::ErasedObject => Some(erased_object_rust_type())")
    );
}

#[test]
fn signature_boundary_policy_is_the_single_boundary_constraint_entrypoint() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("pub enum SignatureBoundaryKind"));
    assert!(source.contains("pub enum SignatureBoundaryRole"));
    assert!(!source.contains("pub struct SignatureBoundaryRolePolicy"));
    assert!(!source.contains("pub fn signature_boundary_role_policies"));
    assert!(!source.contains("pub fn signature_boundary_role_policy"));
    assert!(source.contains("pub struct SignatureMemberApiBoundaryPolicy"));
    assert!(source.contains("pub fn signature_member_api_boundary_policy"));
    assert!(!source.contains("fn member_is_api_boundary"));
    assert!(source.contains("pub struct SignatureBoundaryKindMerge"));
    assert!(source.contains("pub struct SignatureBoundaryKindPriorityPolicy"));
    assert!(source.contains("pub fn signature_boundary_kind_priority_policy"));
    assert!(source.contains("pub fn merge_signature_boundary_kind_policy"));
    let boundary_kind_priority_source = source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SignatureBoundaryKindPriorityPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureBoundaryKindPriorityPolicy").next())
        .expect("signature boundary kind priority policy should remain discoverable");
    assert!(!boundary_kind_priority_source.contains("pub requirement: SignatureBoundaryKind"));
    assert!(!boundary_kind_priority_source.contains("pub rank: u8"));
    assert!(source.contains("pub fn requirement(self) -> SignatureBoundaryKind"));
    assert!(source.contains("pub fn rank(self) -> u8"));
    let boundary_kind_merge_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureBoundaryKindMerge")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureBoundaryKindMerge").next())
        .expect("signature boundary kind merge should remain discoverable");
    assert!(!boundary_kind_merge_source.contains("pub kept: Option<SignatureBoundaryKind>"));
    assert!(!boundary_kind_merge_source.contains("pub rejected: Option<SignatureBoundaryKind>"));
    assert!(source.contains("pub fn into_kept(self) -> Option<SignatureBoundaryKind>"));
    assert!(source.contains("pub struct SignatureFlagMerge"));
    assert!(source.contains("pub fn merge_signature_flag_requirement_policy"));
    let flag_merge_source = source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SignatureFlagMerge")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureFlagMerge").next())
        .expect("signature flag merge should remain discoverable");
    assert!(!flag_merge_source.contains("pub kept: bool"));
    assert!(source.contains("pub fn kept(self) -> bool"));
    assert!(source.contains("pub enum SignatureBoundaryFallback"));
    assert!(source.contains("pub struct SignatureBoundaryRepresentationFactPolicy"));
    assert!(source.contains("pub struct SignatureBoundaryRepresentationPolicy"));
    let boundary_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureBoundaryPolicy").next())
        .expect("signature boundary policy should remain discoverable");
    assert!(!boundary_policy_source.contains("pub kind: SignatureBoundaryKind"));
    assert!(!boundary_policy_source.contains("pub java_ty: JavaType"));
    assert!(!boundary_policy_source.contains("pub fallback: SignatureBoundaryFallback"));
    assert!(!boundary_policy_source.contains("pub constraints: Vec<SignatureConstraint>"));
    assert!(source.contains("pub fn kind(&self) -> SignatureBoundaryKind"));
    assert!(source.contains("pub fn fallback(&self) -> SignatureBoundaryFallback"));
    let representation_fact_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureBoundaryRepresentationFactPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureBoundaryRepresentationFactPolicy").next())
        .expect("signature boundary representation fact policy should remain discoverable");
    assert!(!representation_fact_policy_source.contains("pub java_ty: JavaType"));
    assert!(!representation_fact_policy_source.contains("pub object_representation_facts: bool"));
    assert!(source.contains("pub fn object_representation_facts(&self) -> bool"));
    let representation_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureBoundaryRepresentationPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureBoundaryRepresentationPolicy").next())
        .expect("signature boundary representation policy should remain discoverable");
    assert!(!representation_policy_source.contains("pub java_ty: JavaType"));
    assert!(!representation_policy_source.contains("pub preserve_object_representation: bool"));
    assert!(source.contains("pub fn preserve_object_representation(&self) -> bool"));
    assert!(source.contains("pub struct SignatureNativeBoundaryPolicy"));
    let native_boundary_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureNativeBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureNativeBoundaryPolicy").next())
        .expect("signature native boundary policy should remain discoverable");
    assert!(!native_boundary_policy_source.contains("pub constraints: Vec<SignatureConstraint>"));
    assert!(source.contains("PreserveObjectRepresentationEscapingStorage"));
    assert!(source.contains("EscapingNativeStorage"));
    assert!(source.contains("pub fn signature_boundary_policy"));
    assert!(!source.contains("pub fn conservative_boundary_constraints"));
    assert!(!source.contains("pub fn signature_boundary_representation_fact_policy("));
    assert!(source.contains("signature_boundary_representation_fact_policy_with_effective_facts"));
    assert!(!source.contains("pub fn signature_boundary_representation_policy("));
    assert!(source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(source.contains("pub fn signature_boundary_policy_with_effective_facts"));
    assert!(!source.contains("pub fn signature_native_boundary_policy("));
    assert!(source.contains("pub fn signature_native_boundary_policy_with_effective_facts"));
    assert!(source.contains("signature_capability_policy_with_effective_facts(java_ty, facts)"));
    assert!(source.contains("fn signature_boundary_fallback_with_effective_facts"));
    assert!(!source.contains("pub fn signature_boundary_constraints("));
    assert!(source.contains("pub fn signature_boundary_constraints_with_effective_facts"));
    assert!(!source.contains("fn boundary_type_has_object_representation"));
    assert!(!source.contains("fn native_boundary_constraints"));
    assert!(source.contains("pub struct BindingBoundaryPolicy"));
    let binding_boundary_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct BindingBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl BindingBoundaryPolicy").next())
        .expect("binding boundary policy should remain discoverable");
    assert!(!binding_boundary_source.contains("pub binding_name: String"));
    assert!(!binding_boundary_source.contains("pub kind: Option<SignatureBoundaryKind>"));
    assert!(source.contains("pub(crate) fn binding_boundary_policy"));
    assert!(!source.contains("pub fn binding_boundary_policy"));
    assert!(source.contains("pub(crate) fn external_argument_binding_constraints"));
    assert!(!source.contains("pub fn external_argument_binding_constraints"));
    assert!(source.contains("pub(crate) fn external_call_return_binding_constraints"));
    assert!(!source.contains("pub fn external_call_return_binding_constraints"));
    assert!(source.contains("pub(crate) fn external_receiver_binding_constraints"));
    assert!(!source.contains("pub fn external_receiver_binding_constraints"));
    assert!(!source.contains("fn binding_boundary_kind"));
    assert!(source.contains("pub struct MethodParamBoundaryPolicy"));
    let method_param_boundary_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct MethodParamBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl MethodParamBoundaryPolicy").next())
        .expect("method param boundary policy should remain discoverable");
    assert!(!method_param_boundary_source.contains("pub type_name: String"));
    assert!(!method_param_boundary_source.contains("pub method_name: String"));
    assert!(!method_param_boundary_source.contains("pub param_name: String"));
    assert!(!method_param_boundary_source.contains("pub kind: Option<SignatureBoundaryKind>"));
    assert!(source.contains("pub fn method_param_boundary_policy"));
    assert!(!source.contains("fn method_param_boundary_kind"));
    assert!(source.contains("pub struct ConstructorParamBoundaryPolicy"));
    let constructor_param_boundary_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ConstructorParamBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl ConstructorParamBoundaryPolicy").next())
        .expect("constructor param boundary policy should remain discoverable");
    assert!(!constructor_param_boundary_source.contains("pub type_name: String"));
    assert!(!constructor_param_boundary_source.contains("pub param_name: String"));
    assert!(!constructor_param_boundary_source.contains("pub kind: Option<SignatureBoundaryKind>"));
    assert!(source.contains("pub fn constructor_param_boundary_policy"));
    assert!(!source.contains("fn constructor_param_boundary_kind"));
    assert!(source.contains("pub struct FieldBoundaryPolicy"));
    let field_boundary_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct FieldBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl FieldBoundaryPolicy").next())
        .expect("field boundary policy should remain discoverable");
    assert!(!field_boundary_source.contains("pub type_name: String"));
    assert!(!field_boundary_source.contains("pub field_name: String"));
    assert!(!field_boundary_source.contains("pub kind: Option<SignatureBoundaryKind>"));
    assert!(source.contains("pub fn field_boundary_policy"));
    assert!(!source.contains("fn field_boundary_kind"));
    assert!(source.contains("pub struct MethodReturnBoundaryPolicy"));
    let method_return_boundary_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct MethodReturnBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl MethodReturnBoundaryPolicy").next())
        .expect("method return boundary policy should remain discoverable");
    assert!(!method_return_boundary_source.contains("pub type_name: String"));
    assert!(!method_return_boundary_source.contains("pub method_name: String"));
    assert!(!method_return_boundary_source.contains("pub kind: Option<SignatureBoundaryKind>"));
    assert!(source.contains("pub fn method_return_boundary_policy"));
    assert!(source.contains("pub fn kind(&self) -> Option<SignatureBoundaryKind>"));
    assert!(!source.contains("fn method_return_boundary_kind"));
    assert!(!source.contains("fn binding_requires_conservative_boundary"));
    assert!(!source.contains("fn field_requires_unknown_boundary"));
    assert!(!source.contains("fn method_return_requires_conservative_boundary"));
    assert!(!source.contains("fn public_api_boundary_constraints"));
    assert!(!source.contains("fn escaping_boundary_constraints"));
}

#[test]
fn shape_projection_owns_borrowed_string_content_shape() {
    let coercion_source = include_str!("../src/coercion.rs");
    let coercion_production_source = coercion_source
        .split("#[cfg(test)]")
        .next()
        .expect("coercion production source should be available before tests");
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs");
    let shape_projection_source = include_str!("../src/shape_projection.rs");

    assert!(shape_projection_source.contains("pub fn borrowed_string_content_shape"));
    assert!(shape_projection_source.contains("pub fn owned_string_value_shape"));
    assert!(
        shape_projection_source.contains("pub(crate) fn apply_borrowed_string_content_projection")
    );
    assert!(shape_projection_source.contains("pub fn borrowed_sequence_element_shape"));
    assert!(
        shape_projection_source
            .contains("pub(crate) fn apply_borrowed_sequence_element_projection")
    );
    assert!(slot_policy_source.contains("BorrowedContentPolicy"));
    assert!(slot_policy_source.contains("BorrowedContentKind"));
    assert!(slot_policy_source.contains("BorrowedContentTypePolicy"));
    assert!(slot_policy_source.contains("borrowed_content_type_policy"));
    assert!(!slot_policy_source.contains("borrowed_content_policy_for_type"));
    assert!(!slot_policy_source.contains("fn can_borrow_content"));
    assert!(!slot_policy_source.contains("BorrowedViewPolicy"));
    assert!(!slot_policy_source.contains("BorrowedStringContent"));
    assert!(!stream_shape_source.contains("ViewShape::Str"));
    assert!(!stream_shape_source.contains("JavaType::String"));
    assert!(!slot_policy_source.contains("ViewShape::Str"));
    assert!(!slot_policy_source.contains("OwnershipShape::SharedBorrow"));
    assert!(!coercion_production_source.contains("element_shape.ownership = OwnershipShape"));
}

#[test]
fn signature_dispatch_requirement_projection_stays_role_policy_based() {
    let signature_source = include_str!("../src/signature.rs");

    assert!(signature_source.contains("dispatch_requirement_projection_policy"));
    assert!(signature_source.contains("fn dispatch_requirement_projection_role"));
    assert!(!signature_source.contains(
        "(DispatchRequirementKind::NeedsOwnedPolymorphicStorage, SignatureSlotRole::Storage)"
    ));
}

#[test]
fn signature_requirement_merge_policy_owns_axis_priority_symbols() {
    let source = include_str!("../src/signature.rs");
    let lattice_source = include_str!("../src/lattice.rs");

    assert!(source.contains("pub struct SignatureRequirementMerge"));
    assert!(!source.contains("pub struct SignatureOwnershipRequirementPriorityPolicy"));
    assert!(!source.contains("pub fn signature_ownership_requirement_priority_policy"));
    assert!(!source.contains("pub struct SignatureViewRequirementPriorityPolicy"));
    assert!(!source.contains("pub fn signature_view_requirement_priority_policy"));
    assert!(!source.contains("pub struct SignatureDispatchRequirementPriorityPolicy"));
    assert!(!source.contains("pub fn signature_dispatch_requirement_priority_policy"));
    assert!(source.contains("pub struct SignatureRequirementMerge"));
    let requirement_merge_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureRequirementMerge<T>")
        .nth(1)
        .and_then(|tail| tail.split("impl<T> SignatureRequirementMerge<T>").next())
        .expect("signature requirement merge should remain discoverable");
    assert!(!requirement_merge_source.contains("pub kept: T"));
    assert!(!requirement_merge_source.contains("pub rejected: Option<T>"));
    assert!(source.contains("pub fn kept(&self) -> &T"));
    assert!(source.contains("pub fn rejected(&self) -> Option<&T>"));
    assert!(source.contains("pub fn into_parts(self) -> (T, Option<T>)"));
    assert!(source.contains("pub struct SignatureFlagMerge"));
    assert!(source.contains("pub fn merge_signature_flag_requirement_policy"));
    let flag_merge_source = source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SignatureFlagMerge")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureFlagMerge").next())
        .expect("signature flag merge should remain discoverable");
    assert!(!flag_merge_source.contains("pub kept: bool"));
    assert!(source.contains("pub fn kept(self) -> bool"));
    assert!(source.contains("pub fn merge_ownership_requirement_policy"));
    assert!(source.contains("pub fn merge_view_requirement_policy"));
    assert!(source.contains("pub fn merge_dispatch_requirement_policy"));
    assert!(source.contains("join_ranked_lattice"));
    assert!(source.contains("ownership_lattice_rank"));
    let ranked_join_source = lattice_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub(crate) struct RankedLatticeJoin<T>")
        .nth(1)
        .and_then(|tail| tail.split("impl<T> RankedLatticeJoin<T>").next())
        .expect("ranked lattice join should remain discoverable");
    assert!(!ranked_join_source.contains("pub(crate) kept: T"));
    assert!(!ranked_join_source.contains("pub(crate) rejected: Option<T>"));
    assert!(lattice_source.contains("pub(crate) fn into_parts(self) -> (T, Option<T>)"));
    assert!(!source.contains("fn ownership_requirement_rank"));
    assert!(!source.contains("fn view_requirement_rank"));
    assert!(!source.contains("fn dispatch_requirement_rank"));
    assert!(!source.contains("fn storage_nullability_requirement_rank"));
}

#[test]
fn signature_owned_recovery_policy_owns_capability_symbols() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("pub enum SignatureOwnedRecoveryPolicy"));
    assert!(source.contains("pub struct SignatureOwnedRecoveryCapabilityPolicy"));
    let capability_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureOwnedRecoveryCapabilityPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureOwnedRecoveryCapabilityPolicy").next())
        .expect("signature owned recovery capability policy should remain discoverable");
    assert!(!capability_policy_source.contains("pub recovery: OwnedRecoveryCapability"));
    assert!(source.contains("pub fn recovery(&self) -> &OwnedRecoveryCapability"));
    assert!(source.contains("pub fn into_recovery(self) -> OwnedRecoveryCapability"));
    assert!(source.contains("ProvenByCloneBoxAdapter"));
    assert!(source.contains("ProvenByFactoryCopy"));
    assert!(source.contains("ProvenByImmutableSharedHandle"));
    assert!(source.contains("UnsupportedFacadeOwnedExternalHandle"));
    assert!(source.contains("UnsupportedFfiOwnedHandle"));
    assert!(!source.contains("pub fn signature_owned_recovery_capability_policy("));
    assert!(source.contains("signature_owned_recovery_capability_policy_with_effective_facts"));
    assert!(source.contains("pub fn signature_owned_recovery_policy"));
    assert!(source.contains("pub fn signature_owned_recovery_policy_with_effective_facts"));
    assert!(source.contains("signature_owned_recovery_policy_with_effective_facts(java_ty, constraints, &effective_facts)"));
    assert!(source.contains(
        "signature_owned_recovery_capability_policy_with_effective_facts(java_ty, facts)"
    ));
    assert!(source.contains("use crate::effective_facts::EffectiveFacts"));
}

#[test]
fn signature_capability_and_baseline_policy_symbols_stay_fact_aware() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("pub fn signature_capability_policy_with_effective_facts"));
    assert_eq!(source.matches("is_copy_type").count(), 1);
    assert!(source.contains("pub struct SignatureSlotBaselinePolicy"));
    let slot_baseline_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureSlotBaselinePolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureSlotBaselinePolicy").next())
        .expect("signature slot baseline policy should remain discoverable");
    assert!(!slot_baseline_policy_source.contains("pub constraints: Vec<SignatureConstraint>"));
    assert!(source.contains("pub fn signature_slot_baseline_policy"));
    assert!(source.contains("pub fn signature_slot_baseline_policy_with_effective_facts"));
    assert!(!source.contains("fn signature_slot_baseline_constraints("));
    assert!(source.contains("pub struct SignatureSlotDispatchRequirementPolicy"));
    let dispatch_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureSlotDispatchRequirementPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureSlotDispatchRequirementPolicy").next())
        .expect("signature slot dispatch requirement policy should remain discoverable");
    assert!(!dispatch_policy_source.contains("pub constraints: Vec<SignatureConstraint>"));
    assert!(source.contains("pub fn constraints(&self) -> &[SignatureConstraint]"));
    assert!(source.contains("pub fn into_constraints(self) -> Vec<SignatureConstraint>"));
}

#[test]
fn stream_shape_stream_source_semantics_stay_effective_fact_based() {
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");

    assert!(!stream_shape_source.contains("sequence_element_type("));
    assert!(stream_shape_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(stream_shape_source.contains("pub struct StreamSourceCapabilityPolicy"));
    let stream_source_capability_policy_source = stream_shape_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct StreamSourceCapabilityPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl StreamSourceCapabilityPolicy").next())
        .expect("StreamSourceCapabilityPolicy source should remain discoverable");
    assert!(
        stream_source_capability_policy_source.contains("capability_element: Option<JavaType>")
    );
    assert!(
        !stream_source_capability_policy_source
            .contains("pub capability_element: Option<JavaType>")
    );
    assert!(stream_shape_source.contains("pub fn capability_element(&self) -> Option<&JavaType>"));
    assert!(stream_shape_source.contains("pub fn stream_source_capability_policy"));
    assert!(stream_shape_source.contains("stream_source_capability_policy_with_effective_facts"));
    assert!(effective_facts_source.contains("pub fn sequence_element_type_for"));
    assert!(!stream_shape_source.contains("name == \"Integer\""));
    assert!(!stream_shape_source.contains("OwnershipShape"));
    assert!(!stream_shape_source.contains("ownership ="));
    assert!(stream_shape_source.contains("iteration_element_shape_with_effective_facts"));
    assert_eq!(
        stream_shape_source
            .matches("facts.sequence_element_type_for")
            .count(),
        1
    );
    assert!(stream_shape_source.contains("infer_stream_source_element_shape_with_facts"));
    assert!(stream_shape_source.contains("infer_stream_source_element_shape_with_effective_facts"));
    assert!(!stream_shape_source.contains("fn stream_source_element_type("));
}

#[test]
fn effective_fact_precedence_queries_are_audited_by_consumer_module() {
    fn assert_count(source_name: &str, source: &str, needle: &str, expected: usize) {
        assert_eq!(
            source.matches(needle).count(),
            expected,
            "{source_name} should contain {expected} occurrence(s) of {needle:?}"
        );
    }

    let object_model_source = include_str!("../src/object_model.rs");
    let representation_source = include_str!("../src/representation.rs");
    let signature_source = include_str!("../src/signature.rs");
    let shape_projection_source = include_str!("../src/shape_projection.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let coercion_source = include_str!("../src/coercion.rs");
    let conversion_plan_source = include_str!("../src/conversion_plan.rs");
    let semantic_analysis_source = include_str!("../src/semantic_analysis.rs");
    let facts_source = include_str!("../src/facts.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");
    let slot_shape_source = include_str!("../src/slot_shape.rs");

    assert!(effective_facts_source.contains("TypeCapabilities::conservative"));
    let type_identity_source = slot_shape_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct TypeIdentity")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct TypeCapabilities")
                .next()
        })
        .expect("TypeIdentity should remain discoverable");
    for field in [
        "pub declared_java_ty:",
        "pub runtime_bounds:",
        "pub generic_args:",
        "pub erasure:",
    ] {
        assert!(
            !type_identity_source.contains(field),
            "TypeIdentity should expose accessors instead of raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn declared_java_ty(&self) -> &JavaType",
        "pub fn runtime_bounds(&self) -> &[JavaType]",
        "pub fn generic_args(&self) -> &[JavaType]",
        "pub fn erasure(&self) -> Option<&JavaType>",
    ] {
        assert!(
            slot_shape_source.contains(accessor),
            "TypeIdentity accessor should remain available: {accessor}"
        );
    }
    let type_capabilities_source = slot_shape_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct TypeCapabilities")
        .nth(1)
        .and_then(|tail| tail.split("impl TypeCapabilities").next())
        .expect("TypeCapabilities should remain discoverable");
    for field in [
        "pub copy: bool",
        "pub clone: bool",
        "pub string_like: bool",
        "pub sequence_like: Option<JavaType>",
        "pub implements: Vec<JavaType>",
        "pub extends: Option<JavaType>",
        "pub final_type: bool",
        "pub object_safe_trait: bool",
    ] {
        assert!(
            !type_capabilities_source.contains(field),
            "TypeCapabilities should expose accessors instead of raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn is_copy(&self) -> bool",
        "pub fn is_clone(&self) -> bool",
        "pub fn is_string_like(&self) -> bool",
        "pub fn sequence_like(&self) -> Option<&JavaType>",
        "pub fn implements(&self) -> &[JavaType]",
        "pub fn extends(&self) -> Option<&JavaType>",
        "pub fn is_final_type(&self) -> bool",
        "pub fn is_object_safe_trait(&self) -> bool",
    ] {
        assert!(
            slot_shape_source.contains(accessor),
            "TypeCapabilities accessor should remain available: {accessor}"
        );
    }
    assert!(effective_facts_source.contains("pub struct EffectiveFact<T>"));
    let effective_fact_source = effective_facts_source
        .split("pub struct EffectiveFact<T>")
        .nth(1)
        .and_then(|tail| tail.split("impl<T> EffectiveFact<T>").next())
        .expect("EffectiveFact should remain discoverable");
    assert!(!effective_fact_source.contains("pub value: T"));
    assert!(!effective_fact_source.contains("pub provenance: FactProvenance"));
    assert!(effective_facts_source.contains("pub fn value(&self) -> &T"));
    assert!(effective_facts_source.contains("pub fn provenance(&self) -> FactProvenance"));
    assert!(effective_facts_source.contains("fn fact_precedence_key"));
    assert!(effective_facts_source.contains(
        "FactProvenance::BuiltinExternalModel | FactProvenance::ExternalLibraryPack => 2"
    ));
    assert!(effective_facts_source.contains("FactProvenance::UserOverride => 4"));
    assert!(effective_facts_source.contains("fn apply_type_capability_override"));
    assert!(effective_facts_source.contains("pub fn type_capabilities_for"));
    assert!(effective_facts_source.contains("object_safe_trait_fact_for_name_with_provenance"));
    assert!(effective_facts_source.contains("closed_hierarchy_fact_for_name_with_provenance"));
    assert!(effective_facts_source.contains("pub fn object_safety_fact_for_name"));
    assert!(!facts_source.contains("fn apply_type_capability_override"));
    assert!(facts_source.contains("EffectiveFacts::new(self).type_capabilities_for(java_ty)"));
    assert!(
        facts_source.contains("EffectiveFacts::new(self).owned_recovery_capability_for(java_ty)")
    );

    assert!(object_model_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(object_model_source.contains("plan_object_model_with_effective_facts"));
    assert_count(
        "object_model",
        object_model_source,
        "facts.hierarchy_edges_for_type",
        1,
    );
    assert_count("object_model", object_model_source, "extends_type_for", 1);
    assert_count(
        "object_model",
        object_model_source,
        "implemented_types_for",
        1,
    );
    assert_count(
        "object_model",
        object_model_source,
        "closed_hierarchy_fact_for_name",
        2,
    );
    assert_count(
        "object_model",
        object_model_source,
        "object_safe_trait_fact_for_name",
        1,
    );

    assert!(representation_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(!representation_source.contains("representation_hierarchy_fact_policy"));
    assert_count(
        "representation",
        representation_source,
        "extends_type_for",
        0,
    );
    assert_count(
        "representation",
        representation_source,
        "implemented_types_for",
        0,
    );
    assert_count(
        "representation",
        representation_source,
        "hierarchy_edges_for_type",
        0,
    );

    assert!(signature_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(
        signature_source
            .contains("signature_owned_recovery_capability_policy_with_effective_facts")
    );
    assert!(
        signature_source
            .contains("signature_boundary_representation_fact_policy_with_effective_facts")
    );
    assert!(signature_source.contains("signature_capability_policy_with_effective_facts"));
    assert_count(
        "signature",
        signature_source,
        "owned_recovery_capability_for",
        1,
    );
    assert_count(
        "signature",
        signature_source,
        "has_object_representation_facts",
        1,
    );
    assert_count("signature", signature_source, "is_copy_type", 1);

    assert!(shape_projection_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(
        shape_projection_source.contains("shape_projection_capability_policy_with_effective_facts")
    );
    assert_count(
        "shape_projection",
        shape_projection_source,
        "is_copy_type",
        1,
    );
    assert_count(
        "shape_projection",
        shape_projection_source,
        "sequence_element_type_for",
        1,
    );

    assert!(slot_policy_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(slot_policy_source.contains("borrowed_content_capability_policy_with_effective_facts"));
    assert_count("slot_policy", slot_policy_source, "is_string_like_type", 1);
    assert_count(
        "slot_policy",
        slot_policy_source,
        "sequence_element_type_for",
        1,
    );

    assert!(slot_render_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(slot_render_source.contains("slot_content_capability_policy_with_effective_facts"));
    assert_count("slot_render", slot_render_source, "is_string_like_type", 1);
    assert_count(
        "slot_render",
        slot_render_source,
        "sequence_element_type_for",
        1,
    );

    assert!(stream_shape_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(stream_shape_source.contains("stream_source_capability_policy_with_effective_facts"));
    assert_count(
        "stream_shape",
        stream_shape_source,
        "sequence_element_type_for",
        1,
    );

    assert!(!coercion_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_plan_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_plan_source.contains("coercion_content_policy_with_effective_facts"));
    assert!(conversion_plan_source.contains("coercion_shape_content_policy_with_effective_facts"));
    assert!(
        conversion_plan_source
            .contains("coercion_owned_recovery_capability_policy_with_effective_facts")
    );
    assert_count("coercion", coercion_source, "is_string_like_type", 0);
    assert_count(
        "conversion_plan",
        conversion_plan_source,
        "is_string_like_type",
        1,
    );
    assert_count(
        "conversion_plan",
        conversion_plan_source,
        "owned_recovery_capability_for",
        1,
    );

    assert_count(
        "semantic_analysis",
        semantic_analysis_source,
        "reachable_hierarchy_edges_for_type",
        1,
    );
}

#[test]
fn stream_lowering_borrows_lambda_string_views_structurally() {
    let stream_lowering_source = include_str!("../src/stream_lowering.rs");

    assert!(stream_lowering_source.contains("ensure_borrow_projection("));
    assert!(!stream_lowering_source.contains("ensure_borrow_projection_rendered"));
    assert!(stream_lowering_source.contains("BorrowProjectionUse::StringView"));
    assert!(!stream_lowering_source.contains("format!(\"{rendered}.as_str()\")"));
    assert!(
        stream_lowering_source
            .contains("pub(crate) fn render_stream_plan_with_optional_sequence_context")
    );
    assert!(
        !stream_lowering_source
            .contains("pub fn render_stream_plan_with_optional_sequence_context")
    );
    assert!(stream_lowering_source.contains("pub(crate) fn emit_stream_lambda_body_expr"));
    assert!(!stream_lowering_source.contains("pub fn emit_stream_lambda_body_expr"));
    assert!(stream_lowering_source.contains("fn emit_stream_lambda_body_with_context"));
    assert!(!stream_lowering_source.contains("pub fn emit_stream_lambda_body_with_context"));
}

#[test]
fn slot_render_policy_symbols_stay_effective_fact_aware() {
    let source = include_str!("../src/slot_render.rs");

    assert!(source.contains("pub enum NativeJavaRustTypePolicy"));
    assert!(!source.contains("pub fn native_java_rust_type_policy("));
    assert!(source.contains("pub fn native_java_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub(crate) struct NativeNamedJavaRustTypePolicy"));
    assert!(!source.contains("pub struct NativeNamedJavaRustTypePolicy"));
    let native_named_policy_source = source
        .split(
            "#[derive(Debug, Clone, PartialEq)]\npub(crate) struct NativeNamedJavaRustTypePolicy",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl NativeNamedJavaRustTypePolicy").next())
        .expect("native named Java/Rust type policy source should remain discoverable");
    assert!(native_named_policy_source.contains("name: String"));
    assert!(native_named_policy_source.contains("policy: NativeJavaRustTypePolicy"));
    assert!(!native_named_policy_source.contains("pub name: String"));
    assert!(!native_named_policy_source.contains("pub policy: NativeJavaRustTypePolicy"));
    assert!(!source.contains("fn name(&self) -> &str"));
    assert!(!source.contains("fn policy(&self) -> &NativeJavaRustTypePolicy"));
    assert!(source.contains("pub(crate) fn native_named_java_rust_type_policy"));
    assert!(!source.contains("pub fn native_named_java_rust_type_policy"));
    assert!(!source.contains("pub fn render_native_java_rust_type_policy("));
    assert!(
        source.contains("pub(crate) fn render_native_java_rust_type_policy_with_effective_facts")
    );
    assert!(!source.contains("\nfn native_named_java_rust_type_policy("));
    assert!(source.contains("pub enum SlotValueRustTypePolicy"));
    assert!(!source.contains("pub fn slot_value_rust_type_policy("));
    assert!(source.contains("pub fn slot_value_rust_type_policy_with_effective_facts"));
    assert!(!source.contains("pub fn render_slot_value_rust_type_policy("));
    assert!(
        source.contains("pub(crate) fn render_slot_value_rust_type_policy_with_effective_facts")
    );
    assert!(source.contains("pub enum SlotRustTypePolicy"));
    assert!(!source.contains("pub fn slot_rust_type_policy("));
    assert!(source.contains("pub fn slot_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub(crate) fn render_slot_rust_type_policy"));
    assert!(source.contains("EffectiveFacts::new(facts)"));
    assert_eq!(source.matches("facts.is_string_like_type").count(), 1);
    assert_eq!(source.matches("facts.sequence_element_type_for").count(), 1);
}

#[test]
fn slot_render_projects_erased_object_facts_without_representation_plan_lookup() {
    let effective_facts_source = include_str!("../src/effective_facts.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");

    assert!(effective_facts_source.contains("object_representation_override_strategy_for"));
    assert!(slot_render_source.contains("object_representation_override_strategy_for"));
    assert!(!slot_render_source.contains("representation_for("));
}

#[test]
fn slot_render_default_empty_slice_stays_structured() {
    let slot_render_source = include_str!("../src/slot_render.rs");

    assert!(
        !slot_render_source.contains(
            "SlotDefaultValuePolicy::EmptySliceView => RustExpr::Raw(\"&[]\".to_string())"
        ),
        "empty slice defaults should compose borrowed array literals structurally"
    );
}

#[test]
fn slot_render_default_policy_symbols_stay_effective_fact_aware() {
    let source = include_str!("../src/slot_render.rs");

    assert!(source.contains("pub enum SlotDefaultPolicy"));
    assert!(source.contains("pub enum SlotDefaultValuePolicy"));
    assert!(source.contains("pub struct SlotContentCapabilityPolicy"));
    assert!(source.contains("    string_content: bool"));
    assert!(source.contains("    sequence_element: Option<JavaType>"));
    assert!(!source.contains("pub string_content: bool"));
    assert!(!source.contains("pub sequence_element: Option<JavaType>"));
    assert!(source.contains("pub fn string_content(&self) -> bool"));
    assert!(source.contains("pub fn sequence_element(&self) -> Option<&JavaType>"));
    assert!(!source.contains("pub fn slot_content_capability_policy("));
    assert!(source.contains("pub fn slot_content_capability_policy_with_effective_facts"));
    assert!(source.contains("pub(crate) struct SlotNoDefaultMessagePolicy"));
    assert!(!source.contains("pub struct SlotNoDefaultMessagePolicy"));
    assert!(source.contains("pub(crate) fn slot_no_default_message_policy"));
    assert!(!source.contains("pub fn slot_no_default_message_policy"));
    assert!(!source.contains("pub fn slot_default_policy("));
    assert!(source.contains("pub fn slot_default_policy_with_effective_facts"));
    assert!(!source.contains("pub fn slot_default_value_policy("));
    assert!(source.contains("pub fn slot_default_value_policy_with_effective_facts"));
    assert!(source.contains("fn slot_default_policy_expr"));
    assert!(!source.contains("pub fn slot_default_policy_expr"));
    assert!(source.contains("fn slot_default_value_policy_expr"));
    assert!(!source.contains("pub fn slot_default_value_policy_expr"));
    assert!(!source.contains("pub fn render_slot_default_policy"));
    assert!(!source.contains("pub fn render_slot_default_value_policy"));
    assert!(source.contains("pub(crate) fn java_set_uses_hash_free_carrier"));
    assert!(source.contains("pub(crate) fn java_set_uses_hash_free_carrier_with_representations"));
    assert!(source.contains("pub(crate) fn java_set_uses_hash_free_carrier_with_effective_facts"));
}

#[test]
fn call_target_lowering_uses_shared_projection_helpers() {
    let call_lowering_source = include_str!("../src/call_lowering.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production call_lowering source should be present");
    let apply_call_target_source = call_lowering_source
        .split("pub fn apply_call_target")
        .nth(1)
        .expect("apply_call_target should exist")
        .split("fn mark_noop_facade_method_call_site")
        .next()
        .expect("apply_call_target should be bounded");

    assert!(apply_call_target_source.contains("function_call_projection"));
    assert!(apply_call_target_source.contains("receiver_method_call_projection_exact"));
    assert!(apply_call_target_source.contains("free_function_path_projection(rust_name)"));
    assert!(!apply_call_target_source.contains("function_call_projection(RustExpr::Var"));
    assert!(
        apply_call_target_source.contains("external_crate_path_projection(crate_name, rust_name)")
    );
    assert!(!apply_call_target_source.contains("RustExpr::Raw("));
    assert!(!apply_call_target_source.contains("emit::emit_expr"));
    assert!(call_lowering_source.contains("fn unit_expr() -> RustExpr"));
    assert!(call_lowering_source.contains(
        "function_call_projection(RustExpr::path([\"Default\", \"default\"]), Vec::new())"
    ));
    assert!(!call_lowering_source.contains("function: Box::new(RustExpr::path([\"Ok\"]))"));
    assert!(
        !call_lowering_source
            .contains("function: Box::new(RustExpr::path([\"Default\", \"default\"]))")
    );
    assert!(call_lowering_source.contains("RustExpr::Tuple(Vec::new())"));
    assert!(!call_lowering_source.contains("RustExpr::Raw(\"Ok(())\""));
    assert!(!call_lowering_source.contains("RustExpr::Raw(\"()\""));
    assert!(!call_lowering_source.contains("RustExpr::Raw(\"{ }\""));
    assert!(!call_lowering_source.contains("RustStmt::Raw("));
    assert!(
        !call_lowering_source.contains("emit::emit_expr"),
        "call lowering should not render RustExprs back to text while composing calls"
    );
    assert!(call_lowering_source.contains("pub(crate) fn apply_class_literal_token"));
    assert!(!call_lowering_source.contains("pub fn apply_class_literal_token"));
    let stdlib_source = include_str!("../src/stdlib.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production stdlib source should be present");
    assert!(stdlib_source.contains("pub(crate) struct ClassLiteralTokenPlan"));
    assert!(stdlib_source.contains("pub(crate) fn class_literal_token_plan"));
    assert!(!stdlib_source.contains("pub struct ClassLiteralTokenPlan"));
    assert!(!stdlib_source.contains("pub fn class_literal_token_plan"));
    let token_plan_source = stdlib_source
        .split("#[derive(Debug, Clone, PartialEq)]\npub(crate) struct ClassLiteralTokenPlan")
        .nth(1)
        .and_then(|tail| tail.split("impl ClassLiteralTokenPlan").next())
        .expect("class literal token plan should remain discoverable");
    assert!(!token_plan_source.contains("pub(crate) java_name: String"));
    assert!(!token_plan_source.contains("pub(crate) rust_type: RustType"));
    assert!(stdlib_source.contains("pub(crate) fn java_name(&self) -> &str"));
    assert!(stdlib_source.contains("pub(crate) fn rust_type(&self) -> &RustType"));
    assert!(call_lowering_source.contains("RustExpr::MarkedBlock"));
    assert!(!call_lowering_source.contains("RustExpr::Raw(format!(\n        \"{{ {} {} }}\""));
}

#[test]
fn exception_lowering_keeps_exception_paths_structured() {
    let source = include_str!("../src/exception_lowering.rs");
    let production_source = source
        .split("#[cfg(test)]")
        .next()
        .expect("exception lowering source should have a production prefix");
    let coercion_source = include_str!("../src/coercion.rs");

    assert!(!production_source.contains("JavaType"));
    assert!(!production_source.contains("exception_class_name"));
    assert!(!production_source.contains("JavaType::Named"));
    assert!(!production_source.contains("JavaType::Unknown"));
    let throw_policy_source = production_source
        .split("pub(crate) fn throw_stmt_with_result_policy")
        .nth(1)
        .and_then(|tail| tail.split("pub(crate) fn try_catch_stmt").next())
        .expect("throw result policy lowering should remain discoverable");
    assert!(production_source.contains("pub(crate) fn throw_stmt_with_result_policy"));
    assert!(!production_source.contains("pub fn throw_stmt_with_result_policy"));
    for helper in [
        "exception_result_type",
        "exception_try_ok_type",
        "return_stmt",
        "new_exception_expr",
        "new_exception_expr_with_cause",
        "exception_cause_or_self_if_instance_expr",
        "exception_expr_from_new_with_cause_hint",
    ] {
        assert!(
            production_source.contains(&format!("pub(crate) fn {helper}(")),
            "{helper} should stay crate-local"
        );
        assert!(
            !production_source.contains(&format!("pub fn {helper}(")),
            "{helper} should not be public API"
        );
    }
    for helper in ["throw_stmt", "exception_expr_from_new"] {
        assert!(
            production_source.contains(&format!("fn {helper}(")),
            "{helper} should remain inside exception lowering"
        );
        assert!(
            !production_source.contains(&format!("pub(crate) fn {helper}("))
                && !production_source.contains(&format!("pub fn {helper}(")),
            "{helper} should not cross the exception-lowering module boundary"
        );
    }
    assert!(production_source.contains("pub(crate) fn try_with_resources_finally_scope_stmt"));
    assert!(!production_source.contains("pub fn try_with_resources_finally_scope_stmt"));
    assert!(production_source.contains("pub(crate) fn try_with_resources_scope_stmt"));
    assert!(!production_source.contains("pub fn try_with_resources_scope_stmt"));
    assert!(production_source.contains("pub(crate) fn try_finally_scope_stmt"));
    assert!(!production_source.contains("pub fn try_finally_scope_stmt"));
    assert!(production_source.contains("pub(crate) fn try_catch_finally_scope_stmt"));
    assert!(!production_source.contains("pub fn try_catch_finally_scope_stmt"));
    assert!(production_source.contains("pub(crate) fn try_catch_stmt"));
    assert!(!production_source.contains("pub fn try_catch_stmt"));
    assert!(production_source.contains("pub(crate) fn try_catch_value_fallthrough_stmt"));
    assert!(!production_source.contains("pub fn try_catch_value_fallthrough_stmt"));
    assert!(production_source.contains("pub(crate) struct CatchRenderArm"));
    let catch_render_arm_source = production_source
        .split("#[derive(Debug, Clone, PartialEq)]\npub(crate) struct CatchRenderArm")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]")
                .next()
        })
        .expect("CatchRenderArm should remain discoverable");
    assert!(!catch_render_arm_source.contains("pub(crate) variable_name: String"));
    assert!(!catch_render_arm_source.contains("pub(crate) class_names: Vec<String>"));
    assert!(!catch_render_arm_source.contains("pub(crate) body: Vec<RustStmt>"));
    assert!(production_source.contains("pub(crate) fn variable_name(&self) -> &str"));
    assert!(production_source.contains("pub(crate) fn class_names(&self) -> &[String]"));
    assert!(production_source.contains("pub(crate) fn into_body(self) -> Vec<RustStmt>"));
    assert!(production_source.contains("pub(crate) struct ExternalExceptionConversion"));
    assert!(!production_source.contains("pub struct ExternalExceptionConversion"));
    let external_conversion_source = production_source
        .split("#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]\npub(crate) struct ExternalExceptionConversion")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn java_exception_support_item_with_facts")
                .next()
        })
        .expect("ExternalExceptionConversion should remain discoverable");
    assert!(!external_conversion_source.contains("pub(crate) java_type: String"));
    assert!(!external_conversion_source.contains("pub(crate) rust_type: String"));
    assert!(production_source.contains("pub(crate) fn java_type(&self) -> &str"));
    assert!(production_source.contains("pub(crate) fn rust_type(&self) -> &str"));
    assert!(production_source.contains("pub(crate) fn external_exception_conversions"));
    assert!(!production_source.contains("pub fn external_exception_conversions"));
    let exception_plan_source = include_str!("../src/exception_plan.rs");
    assert!(exception_plan_source.contains("pub(crate) fn try_resource_names"));
    assert!(!exception_plan_source.contains("pub fn try_resource_names"));
    assert!(exception_plan_source.contains("pub(crate) fn try_resources_are_declarations"));
    assert!(!exception_plan_source.contains("pub fn try_resources_are_declarations"));
    assert!(exception_plan_source.contains("pub struct CallableFallibility"));
    assert!(exception_plan_source.contains("    returns_result: bool"));
    assert!(exception_plan_source.contains("    reasons: Vec<FallibilityReason>"));
    assert!(!exception_plan_source.contains("pub returns_result: bool"));
    assert!(!exception_plan_source.contains("pub reasons: Vec<FallibilityReason>"));
    assert!(exception_plan_source.contains("pub fn returns_result(&self) -> bool"));
    assert!(exception_plan_source.contains("pub fn reasons(&self) -> &[FallibilityReason]"));
    let java_exception_support_source = production_source
        .split("pub struct JavaException {")
        .nth(1)
        .and_then(|tail| tail.split("impl JavaException").next())
        .expect("JavaException support source block");
    assert!(
        java_exception_support_source.contains("cause: Option<std::boxed::Box<JavaException>>")
    );
    assert!(java_exception_support_source.contains("class_name: &'static str"));
    assert!(java_exception_support_source.contains("message: String"));
    assert!(java_exception_support_source.contains("suppressed: Vec<JavaException>"));
    assert!(!java_exception_support_source.contains("pub class_name:"));
    assert!(!java_exception_support_source.contains("pub message:"));
    assert!(!java_exception_support_source.contains("pub cause:"));
    assert!(!java_exception_support_source.contains("pub suppressed:"));
    assert!(production_source.contains("pub fn class_name(&self) -> &'static str"));
    assert!(production_source.contains("pub fn message(&self) -> &str"));
    assert!(production_source.contains("pub fn cause(&self) -> Option<&JavaException>"));
    assert!(
        production_source.contains("pub fn add_suppressed(&mut self, exception: JavaException)")
    );
    assert!(
        production_source.contains("pub fn get_suppressed(&self) -> Vec<Option<JavaException>>")
    );
    assert!(throw_policy_source.contains("RustExpr::MacroCall"));
    assert!(
        !throw_policy_source.contains("RustStmt::Raw(format!")
            && !throw_policy_source.contains("emit_expr(&exception)"),
        "non-result throw lowering should keep the exception expression structured"
    );
    assert!(
        production_source.contains("postfix_receiver_projection(message)")
            && production_source.contains("postfix_receiver_projection(wrapper)")
            && !production_source.contains("fn raw_needs_method_receiver_parentheses")
            && !production_source.contains("RustExpr::Raw(format!(\"({trimmed})\""),
        "exception method receivers should delegate Raw recovery to the central coercion helper"
    );
    let postfix_receiver_source = coercion_source
        .split("pub(crate) fn postfix_receiver_projection")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn recover_parseable_raw_operand_projection")
                .next()
        })
        .expect("central raw postfix receiver helper should remain discoverable");
    assert!(
        postfix_receiver_source
            .contains("raw_receiver_projection(expr, raw_needs_postfix_receiver_parentheses)"),
        "postfix receiver parenthesizing should share the central raw receiver projection"
    );
    assert!(
        postfix_receiver_source.contains("raw_contains_top_level_postfix_receiver_operator(raw)")
            && postfix_receiver_source.contains("rendered_top_level_as_cast_parts(raw).is_some()")
            && !postfix_receiver_source.contains("char_indices()")
            && !postfix_receiver_source.contains("raw[idx..].starts_with(\"as\")"),
        "postfix receiver parenthesizing should reuse shared top-level raw scanners"
    );
    let raw_receiver_projection_source = coercion_source
        .split("fn raw_receiver_projection")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn postfix_receiver_projection")
                .next()
        })
        .expect("central raw receiver projection should remain discoverable");
    assert!(
        !coercion_source.contains("pub(crate) fn raw_receiver_projection"),
        "raw receiver projection should remain private to coercion.rs"
    );
    assert!(
        raw_receiver_projection_source
            .matches("recover_parseable_rendered_operand_projection")
            .count()
            == 2
            && !raw_receiver_projection_source.contains("RustExpr::Raw(trimmed.to_string())")
            && !raw_receiver_projection_source.contains("rendered_projection_or_raw_leaf(&raw)")
            && raw_receiver_projection_source
                .contains("recover_parseable_rendered_operand_projection(&raw)"),
        "central raw receiver projection should delegate parseable operand recovery to the named helper"
    );
    assert!(source.contains("fn java_exception_new_projection"));
    assert!(source.contains("fn java_exception_with_cause_projection"));
    assert!(source.contains("pub(crate) fn fallback_exception_expr"));
    assert!(!source.contains("pub fn fallback_exception_expr"));
    assert!(source.contains("function_call_projection("));
    assert!(source.contains("receiver_method_call_projection_exact("));
    assert!(source.contains("macro_call_projection(\"format\""));
    assert!(!source.contains("RustExpr::Raw(format!(\"JavaException::new"));
    assert!(!source.contains("RustExpr::Raw(format!(\"JavaException::with_cause"));
    assert!(!source.contains("RustExpr::Raw(format!(\"format!("));
    assert!(source.contains("fn uncaught_java_exception_panic_stmt(exception: RustExpr)"));
    assert!(
        source.contains("fn rust_expr_is_path_name")
            && !source.contains("rust_expr_is_path_or_raw_name")
            && !source.contains("RustExpr::Raw(name) => segments.len() == 1"),
        "try/catch Ok/Err rewrites should rely on structured path functions"
    );
    assert!(source.contains("fn ok_none_expr() -> RustExpr"));
    assert!(
        source.contains("fn unit_expr() -> RustExpr")
            && source.contains("RustExpr::Tuple(Vec::new())")
            && !source.contains("RustExpr::Raw(\"()\""),
        "unit exception results should use tuple IR rather than a raw unit literal"
    );
}

#[test]
fn stream_method_references_use_call_target_projection_helpers() {
    let stream_lowering_source = include_str!("../src/stream_lowering.rs");
    let method_ref_call_plan = stream_lowering_source
        .split("fn emit_stream_method_reference_body_through_call_plan")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn emit_stream_instance_method_reference_body_through_call_plan")
                .next()
        })
        .expect("stream method-reference call-plan helper should be present");
    assert!(method_ref_call_plan.contains("NativeJavaStaticMethod::ObjectsNonNull"));
    assert!(method_ref_call_plan.contains("param_shape.is_none_or"));
    assert!(method_ref_call_plan.contains("stream_call_target_projection("));
    assert!(
        method_ref_call_plan.contains("let lowered_args = adapt_stream_lambda_args_for_plan_exprs")
    );
    assert!(
        !method_ref_call_plan
            .contains("let rendered_args = adapt_stream_lambda_args_for_plan_exprs")
    );
    assert!(!method_ref_call_plan.contains("rendered_projection_or_raw_leaf(&rendered)"));
    assert!(!method_ref_call_plan.contains(".map(RustExpr::Raw)"));
    let instance_method_ref_call_plan = stream_lowering_source
        .split("fn emit_stream_instance_method_reference_body_through_call_plan")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn stream_method_reference_native_instance_receiver")
                .next()
        })
        .expect("stream instance method-reference call-plan helper should be present");
    assert!(instance_method_ref_call_plan.contains("stream_call_target_projection("));
    assert!(!instance_method_ref_call_plan.contains("Some(RustExpr::Raw(lowered_receiver))"));
}

#[test]
fn stream_nullable_map_carrier_rendering_stays_behind_projection_helpers() {
    let stream_lowering_source = include_str!("../src/stream_lowering.rs");
    let coercion_source = include_str!("../src/coercion.rs");

    assert!(
        !coercion_source.contains("render_nullable_stream_map_body"),
        "nullable stream map carrier rendering should stay behind nullable_stream_map_body_projection"
    );
    assert!(
        stream_lowering_source.contains("nullable_stream_map_body_projection"),
        "nullable stream map expression path should compose the carrier structurally"
    );
    assert!(
        !stream_lowering_source.contains("rendered_projection_or_raw_leaf(&nullable_body)"),
        "nullable stream map expression path should not rehydrate a rendered body through Raw"
    );
    assert!(
        !stream_lowering_source.contains("Some({final_expr})"),
        "stream lowering should not own nullable map Some(...) carrier spelling"
    );
}

#[test]
fn coercion_production_does_not_build_raw_expression_with_format() {
    let coercion_source = include_str!("../src/coercion.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .expect("production coercion source should be present");

    assert!(
        !coercion_source.contains("RustExpr::Raw(format!"),
        "production coercions should compose RustExpr nodes or use centralized raw-leaf recovery"
    );
    let unresolved_static_call_detector = coercion_source
        .split("fn expr_is_unresolved_static_owner_value_call")
        .nth(1)
        .and_then(|tail| tail.split("fn normalize_emitted_java_type_name").next())
        .expect("unresolved static-owner detector should remain discoverable");
    assert!(
        !unresolved_static_call_detector.contains("RustExpr::Raw("),
        "unresolved static-owner detection should use the structured macro expression, not raw text"
    );
}

#[test]
fn raw_leaf_predicates_use_named_ir_helper() {
    for (path, source) in [
        (
            "src/array_lowering.rs",
            include_str!("../src/array_lowering.rs"),
        ),
        (
            "src/call_lowering.rs",
            include_str!("../src/call_lowering.rs"),
        ),
        ("src/coercion.rs", include_str!("../src/coercion.rs")),
        (
            "src/conversion_lowering.rs",
            include_str!("../src/conversion_lowering.rs"),
        ),
        (
            "src/exception_lowering.rs",
            include_str!("../src/exception_lowering.rs"),
        ),
        (
            "src/item_assembly.rs",
            include_str!("../src/item_assembly.rs"),
        ),
        ("src/lower.rs", include_str!("../src/lower.rs")),
        (
            "src/operator_lowering.rs",
            include_str!("../src/operator_lowering.rs"),
        ),
        (
            "src/stream_lowering.rs",
            include_str!("../src/stream_lowering.rs"),
        ),
    ] {
        let production_source = production_source_without_cfg_test_items(source);
        for forbidden in [
            ".unshaped(), RustExpr::Raw(_))",
            "if let RustExpr::Raw(_) = ",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "{path} should use RustExpr::is_raw_leaf() for raw-leaf predicates instead of open-coding {forbidden:?}"
            );
        }
    }
}

#[test]
fn raw_ir_leaf_wrappers_are_not_public_api() {
    let item_assembly_source = include_str!("../src/item_assembly.rs");
    let statement_render_source = include_str!("../src/statement_render.rs");
    let rust_ir_source = include_str!("../src/rust_ir.rs");
    let types_source = include_str!("../src/types.rs");

    assert!(!rust_ir_source.contains("pub fn raw_leaf"));
    assert!(rust_ir_source.contains("pub(crate) fn raw_leaf"));
    assert!(!item_assembly_source.contains("fn raw_item"));
    assert!(!item_assembly_source.contains("pub fn raw_item"));
    assert!(!statement_render_source.contains("fn raw_stmt"));
    assert!(!statement_render_source.contains("pub fn raw_stmt"));
    for helper in [
        "raw_rust_generic_args",
        "raw_rust_vec_inner",
        "raw_rust_box_dyn_trait",
        "raw_rust_box_sized_inner",
        "raw_generic_type_head",
        "is_known_raw_generic_type",
        "is_raw_generic_counterpart",
        "is_unchecked_generic_counterpart",
    ] {
        assert!(
            !types_source.contains(&format!("pub fn {helper}")),
            "{helper} should not be public API"
        );
        assert!(
            types_source.contains(&format!("pub(crate) fn {helper}")),
            "{helper} should remain available only inside the crate"
        );
    }
    assert!(!types_source.contains("pub fn split_top_level_rust_type_args"));
    assert!(!types_source.contains("pub(crate) fn split_top_level_rust_type_args"));
    for helper in [
        "raw_rust_type_is_result",
        "raw_rust_type_is_option",
        "raw_rust_result_args",
        "raw_rust_option_inner",
    ] {
        assert!(
            !types_source.contains(&format!("pub fn {helper}")),
            "{helper} should not expose raw rendered-type parsing as public API"
        );
        assert!(
            types_source.contains(&format!("pub(crate) fn {helper}")),
            "{helper} should remain an internal parser behind rendered-type facades"
        );
    }
}

#[test]
fn liveness_collectors_are_not_public_api() {
    let lower_source = include_str!("../src/lower.rs");

    for helper in [
        "collect_expr_assigned_names_for_liveness",
        "collect_stmt_read_counts_for_liveness",
        "collect_expr_read_names_for_liveness",
    ] {
        assert!(
            lower_source.contains(&format!("pub(crate) fn {helper}")),
            "{helper} should stay available to internal liveness consumers"
        );
        assert!(
            !lower_source.contains(&format!("pub fn {helper}")),
            "{helper} should not be public API"
        );
    }
}

#[test]
fn fallback_call_name_adapters_are_not_public_api() {
    let call_plan_source = include_str!("../src/call_plan.rs");

    for name in [
        "apply_fallback_receiver_rust_name",
        "apply_fallback_self_rust_name",
        "apply_fallback_static_owner_rust_name",
        "rust_static_owner_type_name",
        "rust_static_owner_type_name_with_facts",
        "is_fallback_receiver_method",
        "uses_receiver_argument_lowering",
        "signature_owner_source",
        "receiver_overload_current_name",
        "into_fallback_receiver_with_rust_name",
        "non_self_trivial_accessor_target",
        "retarget_static_call_to_receiver_if",
        "projection_plan_for_option_shape",
        "external_call_receiver_type_key",
    ] {
        assert!(
            call_plan_source.contains(&format!("pub(crate) fn {name}")),
            "{name} should stay available to lowering"
        );
        assert!(
            !call_plan_source.contains(&format!("pub fn {name}")),
            "{name} should not be public API"
        );
    }
    assert!(call_plan_source.contains("pub(crate) enum CallTargetSignatureOwner"));
    assert!(!call_plan_source.contains("pub enum CallTargetSignatureOwner"));
    assert!(call_plan_source.contains("pub(crate) struct OptionalReceiverSlotProjectionPlan"));
    assert!(!call_plan_source.contains("pub struct OptionalReceiverSlotProjectionPlan"));
    let new_target_plan_source = call_plan_source
        .split("pub struct NewTargetPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub struct ExternalCrateNewTarget").next())
        .expect("NewTargetPlan should remain discoverable");
    assert!(new_target_plan_source.contains("pub fn new("));
    assert!(new_target_plan_source.contains("pub fn owner_shape(&self) -> &RustSlotShape"));
    assert!(new_target_plan_source.contains("pub fn rust_name(&self) -> &str"));
    assert!(
        new_target_plan_source
            .contains("pub fn external_crate(&self) -> Option<&ExternalCrateNewTarget>")
    );
    assert!(
        new_target_plan_source
            .contains("pub fn native_collection(&self) -> Option<NativeCollectionNewTarget>")
    );
    assert!(
        new_target_plan_source
            .contains("pub fn native_string(&self) -> Option<NativeStringNewTarget>")
    );
    assert!(new_target_plan_source.contains("pub(crate) fn rename_constructor("));
    for field in [
        "pub owner_shape: RustSlotShape",
        "pub rust_name: String",
        "pub external_crate: Option<ExternalCrateNewTarget>",
        "pub native_collection: Option<NativeCollectionNewTarget>",
        "pub native_string: Option<NativeStringNewTarget>",
    ] {
        assert!(
            !new_target_plan_source.contains(field),
            "NewTargetPlan should expose accessors, not raw public fields: {field}"
        );
    }
}

#[test]
fn coercion_fallback_message_policies_are_not_public_api() {
    let coercion_source = include_str!("../src/coercion.rs");

    for name in [
        "ownership_coercion_fallback_message_policy",
        "dispatch_coercion_fallback_message_policy",
        "instanceof_fallback_message_policy",
        "borrowed_element_recovery_fallback_message_policy",
    ] {
        assert!(
            coercion_source.contains(&format!("pub(crate) fn {name}")),
            "{name} should stay available to internal lowering"
        );
        assert!(
            !coercion_source.contains(&format!("pub fn {name}")),
            "{name} should not be public API"
        );
    }
}

#[test]
fn trivial_accessor_detection_is_not_a_lowering_special_case() {
    let lower_source = include_str!("../src/lower.rs");
    let call_plan_source = include_str!("../src/call_plan.rs");
    let semantic_source = include_str!("../src/semantic_analysis.rs");

    assert!(semantic_source.contains("trivial_getter_fact"));
    assert!(semantic_source.contains("trivial_setter_fact"));
    assert!(call_plan_source.contains("pub(crate) enum TrivialAccessorCallPlan"));
    assert!(!call_plan_source.contains("pub enum TrivialAccessorCallPlan"));
    assert!(call_plan_source.contains("pub(crate) fn plan_trivial_accessor_call"));
    assert!(!call_plan_source.contains("pub fn plan_trivial_accessor_call"));
    assert!(call_plan_source.contains("plan_call_expr"));
    assert!(call_plan_source.contains("plan_call_statement"));
    assert!(!lower_source.contains("plan_trivial_accessor_call"));
    assert!(lower_source.contains("plan_call_expr"));
    assert!(lower_source.contains("plan_call_statement"));
    assert!(!lower_source.contains("trivial_accessor_for"));
    assert!(!lower_source.contains("trivial_getter_field_for"));
    assert!(!lower_source.contains("trivial_setter_field_for"));
    assert!(!include_str!("../src/signature.rs").contains("trivial_accessor_for"));
    assert!(!include_str!("../src/signature.rs").contains("trivial_getter_field_for"));
    assert!(!include_str!("../src/signature.rs").contains("trivial_setter_field_for"));
    assert!(!include_str!("../src/facts.rs").contains("trivial_getter_field_for"));
    assert!(!include_str!("../src/facts.rs").contains("trivial_setter_field_for"));
}

#[test]
fn callable_analysis_shell_keeps_legacy_lowering_inputs_while_migrating() {
    let lower_source = include_str!("../src/lower.rs");
    let signature_source = include_str!("../src/signature.rs");
    let usage_source = include_str!("../src/usage.rs");
    let slot_policy_source = include_str!("../src/slot_policy.rs");
    let analysis_source = include_str!("../src/callable_analysis.rs");
    let lib_source = include_str!("../src/lib.rs");
    let java_signature_source = include_str!("../src/java_signature.rs");
    let graph_source = include_str!("../src/graph.rs");
    assert!(java_signature_source.contains("pub(crate) struct JavaCallableKey"));
    assert!(java_signature_source.contains("pub(crate) struct JavaCallableSlotKey"));
    assert!(java_signature_source.contains("pub(crate) enum JavaCallableKind"));
    assert!(java_signature_source.contains("pub(crate) enum JavaCallableStaticness"));
    assert!(!java_signature_source.contains("pub struct JavaCallableKey"));
    assert!(!java_signature_source.contains("pub struct JavaCallableSlotKey"));
    assert!(!java_signature_source.contains("pub enum JavaCallableKind"));
    assert!(!java_signature_source.contains("pub enum JavaCallableStaticness"));
    let java_callable_key_source = java_signature_source
        .split("pub(crate) struct JavaCallableKey")
        .nth(1)
        .and_then(|tail| tail.split("impl JavaCallableSlotKey").next())
        .expect("JavaCallableKey source should remain discoverable");
    let java_callable_slot_key_source = java_signature_source
        .split("pub(crate) struct JavaCallableSlotKey")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(Debug, Clone, Copy").next())
        .expect("JavaCallableSlotKey source should remain discoverable");

    assert!(lower_source.contains("usage: &UsageFacts"));
    assert!(!lower_source.contains("usage: &'a UsageFacts"));
    assert!(lower_source.contains("signatures: &'a SignaturePlan"));
    assert!(lower_source.contains("exceptions: &'a ProgramExceptionPlan"));
    assert!(signature_source.contains("methods_by_java_key: HashMap<JavaCallableKey"));
    assert!(signature_source.contains("JavaCallableKey::source_method_for_type"));
    let signature_method_key_source = signature_source
        .split("pub struct MethodKey")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]")
                .next()
        })
        .expect("signature MethodKey source should remain discoverable");
    assert!(!signature_method_key_source.contains("pub type_name: String"));
    assert!(!signature_method_key_source.contains("pub rust_name: String"));
    assert!(signature_method_key_source.contains("pub(crate) fn type_name(&self) -> &str"));
    assert!(!signature_method_key_source.contains("pub fn type_name(&self) -> &str"));
    assert!(!signature_method_key_source.contains("pub fn rust_name(&self) -> &str"));
    assert!(!signature_method_key_source.contains("fn rust_name(&self) -> &str"));
    let signature_constructor_key_source = signature_source
        .split("pub struct ConstructorKey")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]")
                .next()
        })
        .expect("signature ConstructorKey source should remain discoverable");
    assert!(!signature_constructor_key_source.contains("pub type_name: String"));
    assert!(!signature_constructor_key_source.contains("pub rust_name: String"));
    assert!(signature_constructor_key_source.contains("pub(crate) fn type_name(&self) -> &str"));
    assert!(signature_constructor_key_source.contains("pub(crate) fn rust_name(&self) -> &str"));
    assert!(!signature_constructor_key_source.contains("pub fn type_name(&self) -> &str"));
    assert!(!signature_constructor_key_source.contains("pub fn rust_name(&self) -> &str"));
    let signature_field_key_source = signature_source
        .split("pub struct FieldKey")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(Debug, Clone, PartialEq, Eq)]").next())
        .expect("signature FieldKey source should remain discoverable");
    assert!(!signature_field_key_source.contains("pub type_name: String"));
    assert!(!signature_field_key_source.contains("pub field_name: String"));
    assert!(signature_field_key_source.contains("pub(crate) fn type_name(&self) -> &str"));
    assert!(!signature_field_key_source.contains("pub fn type_name(&self) -> &str"));
    assert!(!signature_field_key_source.contains("pub fn field_name(&self) -> &str"));
    assert!(!signature_field_key_source.contains("fn field_name(&self) -> &str"));
    assert!(signature_source.contains("pub fn field_shape_for_declared_field("));
    assert!(signature_source.contains("pub fn required_declared_field_shape("));
    assert!(signature_source.contains("pub(crate) fn field_shape("));
    assert!(signature_source.contains("pub(crate) fn required_field_shape("));
    assert!(!signature_source.contains("pub fn field_shape("));
    assert!(!signature_source.contains("pub fn required_field_shape("));
    assert!(signature_source.contains("FieldKey::source_field(ty, field)"));
    assert!(signature_source.contains("FieldKey::rust_identity(type_name, field_name)"));
    let signature_dependency_graph_source = signature_source
        .split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureDependencyGraph")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureDependencySccAnalysis")
                .next()
        })
        .expect("signature dependency graph source should remain discoverable");
    assert!(
        !signature_dependency_graph_source.contains("pub nodes: HashSet<SignatureDependencyNode>")
    );
    assert!(!signature_dependency_graph_source.contains("pub edges: Vec<SignatureDependencyEdge>"));
    assert!(
        signature_dependency_graph_source
            .contains("pub fn nodes(&self) -> &HashSet<SignatureDependencyNode>")
    );
    assert!(
        signature_dependency_graph_source
            .contains("pub fn edges(&self) -> &[SignatureDependencyEdge]")
    );
    let signature_dependency_edge_source = signature_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureDependencyEdge")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureDependencyEdge").next())
        .expect("signature dependency edge source should remain discoverable");
    assert!(signature_dependency_edge_source.contains("from: SignatureDependencyNode"));
    assert!(signature_dependency_edge_source.contains("to: SignatureDependencyNode"));
    assert!(signature_dependency_edge_source.contains("kind: SignatureDependencyEdgeKind"));
    assert!(!signature_dependency_edge_source.contains("pub from: SignatureDependencyNode"));
    assert!(!signature_dependency_edge_source.contains("pub to: SignatureDependencyNode"));
    assert!(!signature_dependency_edge_source.contains("pub kind: SignatureDependencyEdgeKind"));
    assert!(signature_source.contains(
        "pub(crate) fn new(\n        from: SignatureDependencyNode,\n        to: SignatureDependencyNode,\n        kind: SignatureDependencyEdgeKind,"
    ));
    assert!(!signature_source.contains(
        "pub fn new(\n        from: SignatureDependencyNode,\n        to: SignatureDependencyNode,\n        kind: SignatureDependencyEdgeKind,"
    ));
    assert!(signature_source.contains("pub fn from(&self) -> &SignatureDependencyNode"));
    assert!(signature_source.contains("pub fn to(&self) -> &SignatureDependencyNode"));
    assert!(signature_source.contains("pub fn kind(&self) -> SignatureDependencyEdgeKind"));
    let signature_dependency_scc_source = signature_source
        .split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureDependencySccAnalysis")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct SignatureDependencySccStats")
                .next()
        })
        .expect("signature dependency SCC analysis source should remain discoverable");
    assert!(
        !signature_dependency_scc_source
            .contains("pub components: Vec<Vec<SignatureDependencyNode>>")
    );
    assert!(!signature_dependency_scc_source.contains("pub component_successors: Vec<Vec<usize>>"));
    assert!(
        !signature_dependency_scc_source
            .contains("pub component_by_node: HashMap<SignatureDependencyNode, usize>")
    );
    assert!(!signature_dependency_scc_source.contains("pub stats: SignatureDependencySccStats"));
    assert!(
        signature_dependency_scc_source
            .contains("pub fn components(&self) -> &[Vec<SignatureDependencyNode>]")
    );
    assert!(
        signature_dependency_scc_source
            .contains("pub fn component_successors(&self) -> &[Vec<usize>]")
    );
    assert!(
        signature_dependency_scc_source.contains(
            "pub fn component_by_node(&self) -> &HashMap<SignatureDependencyNode, usize>"
        )
    );
    let graph_scc_source = graph_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub(crate) struct StronglyConnectedComponents<N>")
        .nth(1)
        .and_then(|tail| tail.split("impl<N> StronglyConnectedComponents<N>").next())
        .expect("graph SCC result should remain discoverable");
    assert!(!graph_scc_source.contains("pub(crate) components: Vec<Vec<N>>"));
    assert!(!graph_scc_source.contains("pub(crate) component_by_node: HashMap<N, usize>"));
    assert!(!graph_scc_source.contains("pub(crate) component_successors: Vec<Vec<usize>>"));
    assert!(graph_source.contains("pub(crate) fn components(&self) -> &[Vec<N>]"));
    assert!(graph_source.contains("pub(crate) fn component_by_node(&self) -> &HashMap<N, usize>"));
    assert!(graph_source.contains("pub(crate) fn component_successors(&self) -> &[Vec<usize>]"));
    assert!(
        signature_dependency_scc_source
            .contains("pub fn stats(&self) -> SignatureDependencySccStats")
    );
    let signature_dependency_scc_stats_source = signature_source
        .split("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct SignatureDependencySccStats")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureSolverPassPlan")
                .next()
        })
        .expect("signature dependency SCC stats source should remain discoverable");
    for field in [
        "pub node_count: usize",
        "pub edge_count: usize",
        "pub component_count: usize",
        "pub cyclic_component_count: usize",
        "pub max_component_size: usize",
    ] {
        assert!(
            !signature_dependency_scc_stats_source.contains(field),
            "SignatureDependencySccStats should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn node_count(&self) -> usize",
        "pub fn edge_count(&self) -> usize",
        "pub fn component_count(&self) -> usize",
        "pub fn cyclic_component_count(&self) -> usize",
        "pub fn max_component_size(&self) -> usize",
    ] {
        assert!(signature_dependency_scc_stats_source.contains(accessor));
    }
    let signature_solver_pass_plan_source = signature_source
        .split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureSolverPassPlan")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct SignatureSolverPassStats")
                .next()
        })
        .expect("signature solver pass plan source should remain discoverable");
    assert!(!signature_solver_pass_plan_source.contains("pub component_order: Vec<usize>"));
    assert!(!signature_solver_pass_plan_source.contains("pub cyclic_components: HashSet<usize>"));
    assert!(!signature_solver_pass_plan_source.contains("pub mutable_components: HashSet<usize>"));
    assert!(!signature_solver_pass_plan_source.contains("pub stats: SignatureSolverPassStats"));
    assert!(
        signature_solver_pass_plan_source.contains("pub fn component_order(&self) -> &[usize]")
    );
    assert!(
        signature_solver_pass_plan_source
            .contains("pub fn cyclic_components(&self) -> &HashSet<usize>")
    );
    assert!(
        signature_solver_pass_plan_source
            .contains("pub fn mutable_components(&self) -> &HashSet<usize>")
    );
    assert!(
        signature_solver_pass_plan_source
            .contains("pub fn stats(&self) -> SignatureSolverPassStats")
    );
    let signature_solver_pass_stats_source = signature_source
        .split("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct SignatureSolverPassStats")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub enum SignatureCallableKey")
                .next()
        })
        .expect("signature solver pass stats source should remain discoverable");
    for field in [
        "pub planned_components: usize",
        "pub planned_mutable_components: usize",
        "pub cyclic_components: usize",
        "pub max_component_size: usize",
    ] {
        assert!(
            !signature_solver_pass_stats_source.contains(field),
            "SignatureSolverPassStats should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn planned_components(&self) -> usize",
        "pub fn planned_mutable_components(&self) -> usize",
        "pub fn cyclic_components(&self) -> usize",
        "pub fn max_component_size(&self) -> usize",
    ] {
        assert!(signature_solver_pass_stats_source.contains(accessor));
    }
    assert!(!usage_source.contains("pub(crate) methods_by_java_key: HashMap<JavaCallableKey"));
    assert!(!usage_source.contains("pub methods_by_java_key: HashMap<JavaCallableKey"));
    assert!(usage_source.contains("fn source_method_usage_by_java_key"));
    assert!(!usage_source.contains("pub(crate) fn source_method_usage_by_java_key"));
    assert!(usage_source.contains("pub(crate) fn insert_source_method_usage"));
    assert!(usage_source.contains("pub(crate) fn source_method_usage_has_collapsed_owner_alias"));
    assert!(!usage_source.contains("pub(crate) fn source_method_usage_java_keys"));
    assert!(!usage_source.contains("pub methods: HashMap<UsageMethodKey"));
    assert!(!usage_source.contains("pub constructors: HashMap<UsageConstructorKey"));
    assert!(usage_source.contains("fn method_usage_by_key"));
    assert!(usage_source.contains("fn constructor_usage_by_key"));
    assert!(!usage_source.contains("pub(crate) fn method_usage_by_key"));
    assert!(!usage_source.contains("pub(crate) fn constructor_usage_by_key"));
    assert!(usage_source.contains("pub(crate) fn current_rust_method_usage"));
    assert!(usage_source.contains("pub(crate) fn current_rust_constructor_usage"));
    assert!(!usage_source.contains("pub fn method_usage_by_key"));
    assert!(!usage_source.contains("pub fn constructor_usage_by_key"));
    assert!(usage_source.contains("#[cfg(test)]\n    pub(crate) fn insert_method_usage"));
    assert!(!usage_source.contains("pub fn insert_method_usage("));
    assert!(usage_source.contains("pub(crate) fn method_entries"));
    assert!(usage_source.contains("pub(crate) fn constructor_entries"));
    assert!(usage_source.contains("pub(crate) fn method_usages"));
    assert!(usage_source.contains("pub(crate) fn constructor_usages"));
    assert!(!usage_source.contains("pub fn method_entries"));
    assert!(!usage_source.contains("pub fn constructor_entries"));
    assert!(!usage_source.contains("pub fn method_usages"));
    assert!(!usage_source.contains("pub fn constructor_usages"));
    let usage_method_key_source = usage_source
        .split("pub struct UsageMethodKey")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]")
                .next()
        })
        .expect("UsageMethodKey source should remain discoverable");
    assert!(!usage_method_key_source.contains("pub type_name: String"));
    assert!(!usage_method_key_source.contains("pub method_name: String"));
    assert!(usage_method_key_source.contains("pub(crate) fn type_name(&self) -> &str"));
    assert!(usage_method_key_source.contains("pub(crate) fn method_name(&self) -> &str"));
    assert!(!usage_method_key_source.contains("pub fn type_name(&self) -> &str"));
    assert!(!usage_method_key_source.contains("pub fn method_name(&self) -> &str"));
    let usage_constructor_key_source = usage_source
        .split("pub struct UsageConstructorKey")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Default, PartialEq, Eq)]")
                .next()
        })
        .expect("UsageConstructorKey source should remain discoverable");
    assert!(!usage_constructor_key_source.contains("pub type_name: String"));
    assert!(!usage_constructor_key_source.contains("pub rust_name: String"));
    assert!(usage_constructor_key_source.contains("pub(crate) fn type_name(&self) -> &str"));
    assert!(usage_constructor_key_source.contains("pub(crate) fn rust_name(&self) -> &str"));
    assert!(!usage_constructor_key_source.contains("pub fn type_name(&self) -> &str"));
    assert!(!usage_constructor_key_source.contains("pub fn rust_name(&self) -> &str"));
    assert!(!java_callable_key_source.contains("pub owner: String"));
    assert!(!java_callable_key_source.contains("pub java_name: String"));
    assert!(!java_callable_key_source.contains("pub erased_param_types: Vec<String>"));
    assert!(java_callable_key_source.contains("pub(crate) fn slot(&self) -> JavaCallableSlotKey"));
    assert!(java_callable_key_source.contains("pub(crate) fn owner(&self) -> &str"));
    assert!(java_callable_key_source.contains("pub(crate) fn java_name(&self) -> &str"));
    assert!(!java_callable_key_source.contains("pub fn slot(&self) -> JavaCallableSlotKey"));
    assert!(!java_callable_key_source.contains("pub fn owner(&self) -> &str"));
    assert!(!java_callable_key_source.contains("pub fn java_name(&self) -> &str"));
    assert!(!java_callable_slot_key_source.contains("pub java_name: String"));
    assert!(!java_callable_slot_key_source.contains("pub erased_param_types: Vec<String>"));
    assert!(usage_source.contains("JavaCallableKey::source_method_for_type"));
    assert!(usage_source.contains("pub fn source_constructor_usage"));
    assert!(usage_source.contains("UsageMethodKey::source_method(ty, method)"));
    assert!(usage_source.contains("UsageMethodKey::legacy_java_name_source_method(ty, method)"));
    assert!(usage_source.contains("pub(crate) fn current_rust_method_name"));
    assert!(!usage_source.contains("method_usage_by_current_rust_name"));
    assert!(!usage_source.contains("pub fn insert_method_usage_for_current_rust_name"));
    assert!(!usage_source.contains("insert_method_usage_for_current_rust_name"));
    assert!(usage_source.contains("UsageConstructorKey::source_constructor(ty, ctor)"));
    assert!(usage_source.contains("pub(crate) fn current_rust_constructor_name"));
    assert!(!usage_source.contains("constructor_usage_by_current_rust_name"));
    assert!(!usage_source.contains("pub fn insert_constructor_usage_for_current_rust_name"));
    assert!(!usage_source.contains("insert_constructor_usage_for_current_rust_name"));
    assert!(usage_source.contains("pub(crate) fn source_method_for_type"));
    assert!(usage_source.contains("pub(crate) fn source_method("));
    assert!(usage_source.contains("pub(crate) fn source_constructor_for_type"));
    assert!(usage_source.contains("pub(crate) fn source_constructor("));
    assert!(!usage_source.contains("pub fn source_method_for_type"));
    assert!(!usage_source.contains("pub fn source_method("));
    assert!(!usage_source.contains("pub fn source_constructor_for_type"));
    assert!(!usage_source.contains("pub fn source_constructor("));
    assert!(usage_source.contains("pub(crate) fn resolved_method"));
    assert!(usage_source.contains("pub(crate) fn resolved_constructor"));
    assert!(usage_source.contains("pub(crate) fn method_identity"));
    assert!(usage_source.contains("pub(crate) fn constructor_identity"));
    assert!(usage_source.contains("pub(crate) fn external_constructor"));
    assert!(!usage_source.contains("pub fn resolved_method"));
    assert!(!usage_source.contains("pub fn resolved_constructor"));
    assert!(!usage_source.contains("pub fn method_identity("));
    assert!(!usage_source.contains("pub fn constructor_identity("));
    assert!(!usage_source.contains("pub fn external_constructor("));
    assert!(usage_source.contains("CallableKey::method_identity"));
    assert!(usage_source.contains("pub fn external_method("));
    assert!(usage_source.contains("pub fn method_identity_parts(&self)"));
    assert!(usage_source.contains("pub fn constructor_identity_parts(&self)"));
    assert!(usage_source.contains("pub fn external_method_parts(&self)"));
    assert!(usage_source.contains("pub fn external_constructor_parts(&self)"));
    assert!(usage_source.contains("pub fn is_lambda(&self) -> bool"));
    assert!(usage_source.contains("pub fn anonymous_method_parts(&self)"));
    let call_graph_source = usage_source
        .split("#[derive(Debug, Clone, Default, PartialEq)]\npub struct CallGraph")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]")
                .next()
        })
        .expect("usage::CallGraph source should remain discoverable");
    assert!(!call_graph_source.contains("pub nodes: HashSet<CallableKey>"));
    assert!(!call_graph_source.contains("pub edges: Vec<CallGraphEdge>"));
    assert!(call_graph_source.contains("pub fn nodes(&self) -> &HashSet<CallableKey>"));
    assert!(call_graph_source.contains("pub fn edges(&self) -> &[CallGraphEdge]"));
    assert!(
        call_graph_source
            .contains("#[cfg(test)]\n    pub(crate) fn from_edges(edges: Vec<CallGraphEdge>)")
    );
    assert!(!call_graph_source.contains("pub fn from_edges(edges: Vec<CallGraphEdge>)"));
    assert!(
        call_graph_source
            .contains("pub fn from_nodes(nodes: impl IntoIterator<Item = CallableKey>)")
    );
    let call_graph_edge_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct CallGraphEdge")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]")
                .next()
        })
        .expect("usage::CallGraphEdge source should remain discoverable");
    assert!(!call_graph_edge_source.contains("pub caller: CallableKey"));
    assert!(!call_graph_edge_source.contains("pub callee: CallableKey"));
    assert!(!call_graph_edge_source.contains("pub kind: CallGraphEdgeKind"));
    assert!(usage_source.contains(
        "#[cfg(test)]\n    pub(crate) fn new(caller: CallableKey, callee: CallableKey, kind: CallGraphEdgeKind) -> Self"
    ));
    assert!(!usage_source.contains(
        "pub fn new(caller: CallableKey, callee: CallableKey, kind: CallGraphEdgeKind) -> Self"
    ));
    assert!(usage_source.contains("pub fn caller(&self) -> &CallableKey"));
    assert!(usage_source.contains("pub fn callee(&self) -> &CallableKey"));
    assert!(usage_source.contains("pub fn kind(&self) -> CallGraphEdgeKind"));
    let usage_callable_key_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub enum CallableKey")
        .nth(1)
        .and_then(|tail| tail.split("impl CallableKey").next())
        .expect("usage::CallableKey enum source should remain discoverable");
    for variant in [
        "Method",
        "Constructor",
        "Lambda",
        "AnonymousMethod",
        "ExternalMethod",
        "ExternalConstructor",
    ] {
        assert!(
            usage_callable_key_source.contains(&format!("#[non_exhaustive]\n    {variant}")),
            "public usage::CallableKey::{variant} should be non-exhaustive so downstream callers cannot fabricate raw identity variants"
        );
    }
    for (path, source) in [(
        "tests/slot_shape_design.rs",
        include_str!("slot_shape_design.rs"),
    )] {
        assert!(
            !source.contains("new_java2rust::usage::CallGraph {"),
            "{path} should build public usage::CallGraph fixtures through named constructors instead of raw fields"
        );
        for raw_pattern in [
            "new_java2rust::usage::CallableKey::Method {",
            "new_java2rust::usage::CallableKey::Constructor {",
            "new_java2rust::usage::CallableKey::Lambda {",
            "new_java2rust::usage::CallableKey::AnonymousMethod {",
            "new_java2rust::usage::CallableKey::ExternalMethod {",
            "new_java2rust::usage::CallableKey::ExternalConstructor {",
        ] {
            assert!(
                !source.contains(raw_pattern),
                "{path} should inspect public usage::CallableKey values through query methods instead of raw enum fields"
            );
        }
    }
    assert!(analysis_source.contains("CallableKey::external_method("));
    assert!(analysis_source.contains("CallableKey::external_constructor("));
    assert!(
        !analysis_source.contains("CallableKey::ExternalMethod {")
            && !analysis_source.contains("CallableKey::ExternalConstructor {"),
        "CallableAnalysis should use named external callable constructors instead of raw enum literals"
    );
    assert!(analysis_source.contains("pub(crate) fn current_rust_method_usage("));
    assert!(analysis_source.contains("pub(crate) fn current_rust_constructor_usage("));
    assert!(usage_source.contains("UsageMethodKey::current_rust_method_name"));
    assert!(usage_source.contains("UsageConstructorKey::current_rust_constructor_name"));
    let analysis_production_source = analysis_source
        .split("\n#[cfg(test)]\nmod tests")
        .next()
        .expect("callable analysis production source should be present");
    assert!(!analysis_production_source.contains("UsageMethodKey::current_rust_method_name"));
    assert!(
        !analysis_production_source.contains("UsageConstructorKey::current_rust_constructor_name")
    );
    assert!(!lower_source.contains("UsageMethodKey::current_rust_method_name"));
    assert!(!lower_source.contains("UsageConstructorKey::current_rust_constructor_name"));
    assert!(!signature_source.contains("usage.methods_by_java_key"));
    assert!(!slot_policy_source.contains("usage.methods_by_java_key"));
    assert!(!lib_source.contains(".usage\n                    .source_method_usage_by_java_key"));
    for (path, source) in [
        ("src/callable_analysis.rs", analysis_source),
        ("src/lower.rs", lower_source),
        ("src/signature.rs", signature_source),
        ("src/slot_policy.rs", slot_policy_source),
        (
            "src/object_model.rs",
            include_str!("../src/object_model.rs"),
        ),
        ("tests/object_model.rs", include_str!("object_model.rs")),
        ("tests/semantic_facts.rs", include_str!("semantic_facts.rs")),
        (
            "tests/slot_shape_design.rs",
            include_str!("slot_shape_design.rs"),
        ),
    ] {
        assert!(
            !source.contains("UsageMethodKey {"),
            "{path} should use named UsageMethodKey constructors instead of two-string key literals"
        );
        assert!(
            !source.contains("UsageConstructorKey {"),
            "{path} should use named UsageConstructorKey constructors instead of two-string key literals"
        );
    }
    let fallibility_invariant_source = include_str!("../src/fallibility_invariant.rs");
    let fallibility_violation_source = fallibility_invariant_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub(crate) struct FallibilityViolation")
        .nth(1)
        .and_then(|tail| tail.split("impl FallibilityViolation").next())
        .expect("fallibility violation source should remain discoverable");
    assert!(!fallibility_violation_source.contains("pub caller: CallableKey"));
    assert!(!fallibility_violation_source.contains("pub callee: CallableKey"));
    assert!(fallibility_invariant_source.contains("pub(crate) fn caller(&self) -> &CallableKey"));
    assert!(fallibility_invariant_source.contains("pub(crate) fn callee(&self) -> &CallableKey"));
    assert!(fallibility_invariant_source.contains("pub(crate) fn fallibility_violations("));
    assert!(!fallibility_invariant_source.contains("pub struct FallibilityViolation"));
    assert!(!fallibility_invariant_source.contains("pub fn caller(&self) -> &CallableKey"));
    assert!(!fallibility_invariant_source.contains("pub fn callee(&self) -> &CallableKey"));
    assert!(!fallibility_invariant_source.contains("pub fn fallibility_violations("));
    for (path, source) in [
        (
            "src/exception_plan.rs",
            include_str!("../src/exception_plan.rs"),
        ),
        (
            "src/semantic_analysis.rs",
            include_str!("../src/semantic_analysis.rs"),
        ),
        (
            "src/fallibility_invariant.rs",
            include_str!("../src/fallibility_invariant.rs"),
        ),
        (
            "src/fallibility_invariant.rs",
            include_str!("../src/fallibility_invariant.rs")
                .split("#[cfg(test)]")
                .nth(1)
                .expect("fallibility invariant test module should be present"),
        ),
        ("tests/object_model.rs", include_str!("object_model.rs")),
        ("tests/semantic_facts.rs", include_str!("semantic_facts.rs")),
        (
            "tests/slot_shape_design.rs",
            include_str!("slot_shape_design.rs"),
        ),
    ] {
        assert!(
            !source.contains("MethodKey {"),
            "{path} should use named signature::MethodKey constructors instead of two-string key literals"
        );
        assert!(
            !source.contains("ConstructorKey {"),
            "{path} should use named signature::ConstructorKey constructors instead of two-string key literals"
        );
        assert!(
            !source.contains("FactMethodKey {"),
            "{path} should use named facts::MethodKey constructors instead of two-string key literals"
        );
        assert!(
            !source.contains("field: FieldKey {") && !source.contains("field: FactFieldKey {"),
            "{path} should use named facts::FieldKey constructors instead of two-string key literals"
        );
        assert!(
            !source.contains("CallableKey::Method {"),
            "{path} should use source/resolved constructors or inspect existing keys through accessors instead of direct method key literals"
        );
    }
    for (path, source) in [
        ("src/signature.rs", signature_source),
        ("tests/external_packs.rs", include_str!("external_packs.rs")),
    ] {
        let source_after_field_key_definition =
            source.split("impl FieldKey").nth(1).unwrap_or(source);
        assert!(
            !source_after_field_key_definition.contains("FieldKey {")
                && !source.contains("field: FactFieldKey {"),
            "{path} should use named facts::FieldKey constructors instead of raw field-key literals"
        );
    }
    let inherited_members_source = include_str!("../src/inherited_members.rs");
    let external_inherited_instance_field_source = inherited_members_source
        .split(
            "#[derive(Debug, Clone, PartialEq)]\npub(crate) struct ExternalInheritedInstanceField",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl ExternalInheritedInstanceField").next())
        .expect("ExternalInheritedInstanceField source should remain discoverable");
    let external_inherited_instance_method_source = inherited_members_source
        .split(
            "#[derive(Debug, Clone, PartialEq)]\npub(crate) struct ExternalInheritedInstanceMethod",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl ExternalInheritedInstanceMethod").next())
        .expect("ExternalInheritedInstanceMethod source should remain discoverable");
    let external_inherited_static_field_source = inherited_members_source
        .split("#[derive(Debug, Clone, PartialEq)]\npub(crate) struct ExternalInheritedStaticField")
        .nth(1)
        .and_then(|tail| tail.split("impl ExternalInheritedStaticField").next())
        .expect("ExternalInheritedStaticField source should remain discoverable");
    let inherited_instance_field_source = inherited_members_source
        .split("#[derive(Debug, Clone, PartialEq)]\npub(crate) struct InheritedInstanceField")
        .nth(1)
        .and_then(|tail| tail.split("impl InheritedInstanceField").next())
        .expect("InheritedInstanceField source should remain discoverable");
    let inherited_instance_field_ref_source = inherited_members_source
        .split(
            "#[derive(Debug, Clone, Copy, PartialEq)]\npub(crate) struct InheritedInstanceFieldRef",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl<'a> InheritedInstanceFieldRef").next())
        .expect("InheritedInstanceFieldRef source should remain discoverable");
    let inherited_instance_method_ref_source = inherited_members_source
        .split("#[derive(Debug, Clone, PartialEq)]\npub(crate) struct InheritedInstanceMethodRef")
        .nth(1)
        .and_then(|tail| tail.split("impl<'a> InheritedInstanceMethodRef").next())
        .expect("InheritedInstanceMethodRef source should remain discoverable");
    for field in [
        "pub declaring_type: String",
        "pub field_name: String",
        "pub rust_name: String",
        "pub field_type: JavaType",
        "pub field_nullability: Option<NullabilityState>",
    ] {
        assert!(
            !external_inherited_instance_field_source.contains(field),
            "ExternalInheritedInstanceField should expose accessors, not raw public fields: {field}"
        );
    }
    for field in [
        "pub declaring_type: String",
        "pub method_name: String",
        "pub rust_name: String",
        "pub arg_count: usize",
        "pub fact: ExternalApiEffectFact",
    ] {
        assert!(
            !external_inherited_instance_method_source.contains(field),
            "ExternalInheritedInstanceMethod should expose accessors, not raw public fields: {field}"
        );
    }
    for field in [
        "pub declaring_type: String",
        "pub field_name: String",
        "pub field_type: JavaType",
    ] {
        assert!(
            !external_inherited_static_field_source.contains(field),
            "ExternalInheritedStaticField should expose accessors, not raw public fields: {field}"
        );
    }
    for field in ["pub declaring_type: String", "pub field: FieldDecl"] {
        assert!(
            !inherited_instance_field_source.contains(field),
            "InheritedInstanceField should expose accessors, not raw public fields: {field}"
        );
    }
    for field in ["pub declaring_type: &'a str", "pub field: &'a FieldDecl"] {
        assert!(
            !inherited_instance_field_ref_source.contains(field),
            "InheritedInstanceFieldRef should expose accessors, not raw public fields: {field}"
        );
    }
    for field in [
        "pub declaring_type: &'a str",
        "pub declaring_ty: &'a TypeDecl",
        "pub method: &'a crate::java_ir::MethodDecl",
        "pub rust_name: String",
    ] {
        assert!(
            !inherited_instance_method_ref_source.contains(field),
            "InheritedInstanceMethodRef should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(inherited_members_source.contains("pub(crate) fn declaring_type(&self) -> &str"));
    assert!(inherited_members_source.contains("pub(crate) fn rust_name(&self) -> &str"));
    assert!(inherited_members_source.contains("pub(crate) fn field(&self) -> &FieldDecl"));
    assert!(
        inherited_members_source
            .contains("pub(crate) fn method(&self) -> &'a crate::java_ir::MethodDecl")
    );
    assert!(inherited_members_source.contains("pub(crate) fn field_type(&self) -> &JavaType"));
    assert!(
        inherited_members_source.contains("pub(crate) fn fact(&self) -> &ExternalApiEffectFact")
    );
    for public_surface in [
        "pub struct ExternalInheritedInstanceField",
        "pub struct ExternalInheritedStaticField",
        "pub struct ExternalInheritedInstanceMethod",
        "pub struct InheritedInstanceField",
        "pub struct InheritedInstanceFieldRef",
        "pub struct InheritedInstanceMethodRef",
        "pub fn inherited_instance_fields_for_type",
        "pub fn inherited_instance_method_refs_for_type",
        "pub fn external_inherited_instance_fields_for_type",
        "pub fn external_inherited_static_final_fields_for_type",
        "pub fn external_inherited_instance_methods_for_type",
        "pub fn inherited_instance_field_refs_for_type",
        "pub fn inherited_instance_field_refs_for_type_in_programs",
    ] {
        assert!(
            !inherited_members_source.contains(public_surface),
            "inherited-member identity helper should stay crate-local: {public_surface}"
        );
    }
    let member_access_plan_source = include_str!("../src/member_access_plan.rs");
    let constructor_plan_source = include_str!("../src/constructor_plan.rs");
    let external_packs_source = include_str!("../src/external_packs.rs");
    let resolver_benchmark_source = include_str!("../src/resolver_benchmark.rs");
    let stdlib_source = include_str!("../src/stdlib.rs");
    let resolver_benchmark_source_root_source = resolver_benchmark_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ResolverBenchmarkSourceRoot")
        .nth(1)
        .and_then(|tail| {
            tail.split(
                "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ResolverBenchmarkFactProvider",
            )
            .next()
        })
        .expect("ResolverBenchmarkSourceRoot source should remain discoverable");
    for field in [
        "pub path: PathBuf",
        "pub mode: DependencyMode",
        "pub package_prefixes: Vec<String>",
        "pub files_seen: usize",
    ] {
        assert!(
            !resolver_benchmark_source_root_source.contains(field),
            "ResolverBenchmarkSourceRoot should expose accessors, not raw public fields: {field}"
        );
    }
    let resolver_benchmark_fact_provider_source = resolver_benchmark_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ResolverBenchmarkFactProvider")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct UnresolvedTargetSummary")
                .next()
        })
        .expect("ResolverBenchmarkFactProvider source should remain discoverable");
    for field in [
        "pub name: String",
        "pub mode: DependencyMode",
        "pub facts: usize",
        "pub external_api_effect_facts: usize",
        "pub external_api_argument_effect_facts: usize",
        "pub external_crate_api_mapping_facts: usize",
        "pub external_crate_constructor_mapping_facts: usize",
        "pub external_constructor_facts: usize",
        "pub external_field_facts: usize",
        "pub hierarchy_override_facts: usize",
        "pub closed_hierarchy_facts: usize",
        "pub object_safety_facts: usize",
        "pub type_capability_facts: usize",
        "pub field_nullability_facts: usize",
        "pub exception_facts: usize",
        "pub constructor_recursion_facts: usize",
        "pub external_library_pack_facts: usize",
        "pub unsupported_external_api_facts: usize",
        "pub unsupported_external_constructor_facts: usize",
        "pub unsupported_external_field_facts: usize",
    ] {
        assert!(
            !resolver_benchmark_fact_provider_source.contains(field),
            "ResolverBenchmarkFactProvider should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(resolver_benchmark_source.contains("pub fn path(&self) -> &Path"));
    assert!(resolver_benchmark_source.contains("pub fn package_prefixes(&self) -> &[String]"));
    assert!(resolver_benchmark_source.contains("pub fn external_api_effect_facts(&self) -> usize"));
    let diagnostic_summary_source = resolver_benchmark_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct DiagnosticSummary")
        .nth(1)
        .and_then(|tail| tail.split("impl DiagnosticSummary").next())
        .expect("DiagnosticSummary source should remain discoverable");
    for field in [
        "pub code: &'static str",
        "pub message: String",
        "pub mode: Option<DependencyMode>",
        "pub count: usize",
        "pub sample_files: Vec<String>",
    ] {
        assert!(
            !diagnostic_summary_source.contains(field),
            "DiagnosticSummary should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn code(&self) -> &'static str",
        "pub fn message(&self) -> &str",
        "pub fn mode(&self) -> Option<DependencyMode>",
        "pub fn count(&self) -> usize",
        "pub fn sample_files(&self) -> &[String]",
    ] {
        assert!(
            resolver_benchmark_source.contains(accessor),
            "DiagnosticSummary should keep accessor API: {accessor}"
        );
    }
    let signature_benchmark_count_source = resolver_benchmark_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureBenchmarkCount")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureBenchmarkCount").next())
        .expect("SignatureBenchmarkCount source should remain discoverable");
    for field in ["pub label: String", "pub count: usize"] {
        assert!(
            !signature_benchmark_count_source.contains(field),
            "SignatureBenchmarkCount should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(resolver_benchmark_source.contains("pub fn label(&self) -> &str"));
    assert!(resolver_benchmark_source.contains("pub fn count(&self) -> usize"));
    let source_summary_recommendation_source = resolver_benchmark_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SourceSummaryRecommendation")
        .nth(1)
        .and_then(|tail| tail.split("impl SourceSummaryRecommendation").next())
        .expect("SourceSummaryRecommendation source should remain discoverable");
    for field in [
        "pub package_prefix: String",
        "pub target_type: String",
        "pub constructor_guesses: usize",
        "pub field_guesses: usize",
        "pub method_guesses: usize",
        "pub count: usize",
        "pub sample_files: Vec<String>",
    ] {
        assert!(
            !source_summary_recommendation_source.contains(field),
            "SourceSummaryRecommendation should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn package_prefix(&self) -> &str",
        "pub fn target_type(&self) -> &str",
        "pub fn constructor_guesses(&self) -> usize",
        "pub fn field_guesses(&self) -> usize",
        "pub fn method_guesses(&self) -> usize",
        "pub fn sample_files(&self) -> &[String]",
    ] {
        assert!(
            resolver_benchmark_source.contains(accessor),
            "SourceSummaryRecommendation should keep accessor API: {accessor}"
        );
    }
    let external_api_guess_summary_source = resolver_benchmark_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ExternalApiGuessSummary")
        .nth(1)
        .and_then(|tail| tail.split("impl ExternalApiGuessSummary").next())
        .expect("ExternalApiGuessSummary source should remain discoverable");
    for field in [
        "pub receiver_type: String",
        "pub method_name: String",
        "pub count: usize",
    ] {
        assert!(
            !external_api_guess_summary_source.contains(field),
            "ExternalApiGuessSummary should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn receiver_type(&self) -> &str",
        "pub fn method_name(&self) -> &str",
    ] {
        assert!(
            resolver_benchmark_source.contains(accessor),
            "ExternalApiGuessSummary should keep accessor API: {accessor}"
        );
    }
    for (summary_name, forbidden_fields, required_accessors) in [
        (
            "UnresolvedTargetSummary",
            &[
                "pub kind: UnresolvedTargetKind",
                "pub target_name: String",
                "pub count: usize",
            ][..],
            &[
                "pub fn kind(&self) -> UnresolvedTargetKind",
                "pub fn target_name(&self) -> &str",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "UnresolvedMethodReceiverSummary",
            &[
                "pub target_name: String",
                "pub receiver_type: String",
                "pub count: usize",
            ][..],
            &[
                "pub fn target_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "UnresolvedFieldReceiverSummary",
            &[
                "pub field_name: String",
                "pub receiver_type: String",
                "pub count: usize",
            ][..],
            &[
                "pub fn field_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "UnresolvedFieldReceiverFileSummary",
            &[
                "pub field_name: String",
                "pub receiver_type: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn field_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "UnresolvedFieldContextSummary",
            &[
                "pub field_name: String",
                "pub receiver_type: String",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn field_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "UnresolvedConstructorContextSummary",
            &[
                "pub target_name: String",
                "pub receiver_type: String",
                "pub arg_types: Vec<String>",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn target_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn arg_types(&self) -> &[String]",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "SourceDependencyMethodSummary",
            &[
                "pub receiver_type: String",
                "pub method_name: String",
                "pub source_mode: DependencyMode",
                "pub count: usize",
            ][..],
            &[
                "pub fn receiver_type(&self) -> &str",
                "pub fn method_name(&self) -> &str",
                "pub fn source_mode(&self) -> DependencyMode",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "SourceDependencyConstructorSummary",
            &[
                "pub type_name: String",
                "pub arity: usize",
                "pub source_mode: DependencyMode",
                "pub count: usize",
            ][..],
            &[
                "pub fn type_name(&self) -> &str",
                "pub fn arity(&self) -> usize",
                "pub fn source_mode(&self) -> DependencyMode",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "SourceDependencyFieldSummary",
            &[
                "pub type_name: String",
                "pub field_name: String",
                "pub source_mode: DependencyMode",
                "pub count: usize",
            ][..],
            &[
                "pub fn type_name(&self) -> &str",
                "pub fn field_name(&self) -> &str",
                "pub fn source_mode(&self) -> DependencyMode",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "SourceDependencyFieldContextSummary",
            &[
                "pub declaring_type: String",
                "pub field_name: String",
                "pub receiver_type: String",
                "pub source_mode: DependencyMode",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn declaring_type(&self) -> &str",
                "pub fn field_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn source_mode(&self) -> DependencyMode",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "SourceDependencyMethodContextSummary",
            &[
                "pub declaring_type: String",
                "pub method_name: String",
                "pub source_mode: DependencyMode",
                "pub arg_count: usize",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn declaring_type(&self) -> &str",
                "pub fn method_name(&self) -> &str",
                "pub fn source_mode(&self) -> DependencyMode",
                "pub fn arg_count(&self) -> usize",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "SourceDependencyConstructorContextSummary",
            &[
                "pub declaring_type: String",
                "pub source_mode: DependencyMode",
                "pub arg_count: usize",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn declaring_type(&self) -> &str",
                "pub fn source_mode(&self) -> DependencyMode",
                "pub fn arg_count(&self) -> usize",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "ExternalApiPackSummary",
            &[
                "pub receiver_type: String",
                "pub method_name: String",
                "pub mode: ExternalReportMode",
                "pub count: usize",
            ][..],
            &[
                "pub fn receiver_type(&self) -> &str",
                "pub fn method_name(&self) -> &str",
                "pub fn mode(&self) -> ExternalReportMode",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "ExternalApiMethodContextSummary",
            &[
                "pub receiver_type: String",
                "pub method_name: String",
                "pub mode: ExternalReportMode",
                "pub arg_count: usize",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn receiver_type(&self) -> &str",
                "pub fn method_name(&self) -> &str",
                "pub fn mode(&self) -> ExternalReportMode",
                "pub fn arg_count(&self) -> usize",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "ExternalApiConstructorContextSummary",
            &[
                "pub target_name: String",
                "pub receiver_type: String",
                "pub mode: ExternalReportMode",
                "pub arg_types: Vec<String>",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn target_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn mode(&self) -> ExternalReportMode",
                "pub fn arg_types(&self) -> &[String]",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
        (
            "ExternalPackConstructorSummary",
            &[
                "pub type_name: String",
                "pub arity: usize",
                "pub mode: ExternalReportMode",
                "pub count: usize",
            ][..],
            &[
                "pub fn type_name(&self) -> &str",
                "pub fn arity(&self) -> usize",
                "pub fn mode(&self) -> ExternalReportMode",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "ExternalConstructorGuessSummary",
            &[
                "pub type_name: String",
                "pub arity: usize",
                "pub count: usize",
            ][..],
            &[
                "pub fn type_name(&self) -> &str",
                "pub fn arity(&self) -> usize",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "ExternalFieldGuessSummary",
            &[
                "pub type_name: String",
                "pub field_name: String",
                "pub mode: ExternalReportMode",
                "pub count: usize",
            ][..],
            &[
                "pub fn type_name(&self) -> &str",
                "pub fn field_name(&self) -> &str",
                "pub fn mode(&self) -> ExternalReportMode",
                "pub fn count(&self) -> usize",
            ][..],
        ),
        (
            "ExternalApiFieldContextSummary",
            &[
                "pub field_name: String",
                "pub receiver_type: String",
                "pub mode: ExternalReportMode",
                "pub enclosing_type: String",
                "pub enclosing_member: String",
                "pub count: usize",
                "pub sample_files: Vec<String>",
            ][..],
            &[
                "pub fn field_name(&self) -> &str",
                "pub fn receiver_type(&self) -> &str",
                "pub fn mode(&self) -> ExternalReportMode",
                "pub fn enclosing_type(&self) -> &str",
                "pub fn enclosing_member(&self) -> &str",
                "pub fn count(&self) -> usize",
                "pub fn sample_files(&self) -> &[String]",
            ][..],
        ),
    ] {
        let struct_source = resolver_benchmark_source
            .split(&format!(
                "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct {summary_name}"
            ))
            .nth(1)
            .and_then(|tail| tail.split(&format!("impl {summary_name}")).next())
            .unwrap_or_else(|| panic!("{summary_name} source should remain discoverable"));
        for field in forbidden_fields {
            assert!(
                !struct_source.contains(field),
                "{summary_name} should expose accessors, not raw public fields: {field}"
            );
        }
        for accessor in required_accessors {
            assert!(
                resolver_benchmark_source.contains(accessor),
                "{summary_name} should keep accessor API: {accessor}"
            );
        }
    }
    assert!(
        constructor_plan_source
            .contains("pub(crate) fn plan_constructor_field_values_with_inherited_field_refs")
    );
    assert!(
        !constructor_plan_source
            .contains("pub fn plan_constructor_field_values_with_inherited_field_refs")
    );
    for (path, source) in [
        ("src/inherited_members.rs", inherited_members_source),
        ("src/member_access_plan.rs", member_access_plan_source),
        ("src/external_packs.rs", external_packs_source),
        ("src/resolver_benchmark.rs", resolver_benchmark_source),
        ("src/stdlib.rs", stdlib_source),
    ] {
        for direct_field_payload in [
            ".field.type_name.clone()",
            ".field.type_name.as_str()",
            ".field.type_name == ",
            "&fact.field.type_name",
            "&field.field.type_name",
            "&unsupported.field.type_name",
            ".field.field_name.clone()",
            ".field.field_name.as_str()",
            ".field.field_name == ",
            "&fact.field.field_name",
            "&field.field.field_name",
            "&unsupported.field.field_name",
        ] {
            assert!(
                !source.contains(direct_field_payload),
                "{path} should use facts::FieldKey accessors instead of public raw owner/name fields"
            );
        }
    }
    assert!(signature_source.contains("MethodKey::source_method(ty, method)"));
    assert!(signature_source.contains("MethodKey::rust_identity(type_name, rust_name)"));
    assert!(signature_source.contains("MethodKey::resolved_call(resolved)"));
    assert!(signature_source.contains("MethodKey::from_usage_key(usage_key)"));
    assert!(signature_source.contains("ConstructorKey::source_constructor(ty, ctor)"));
    assert!(signature_source.contains("ConstructorKey::rust_identity(type_name, rust_name)"));
    assert!(signature_source.contains("ConstructorKey::resolved_constructor(resolved)"));
    assert!(signature_source.contains("ConstructorKey::from_usage_key(usage_key)"));
    assert!(signature_source.contains("usage.source_constructor_usage(ty, ctor)"));
    assert!(usage_source.contains("source_method_rust_name(method)"));
    let facts_source = include_str!("../src/facts.rs");
    assert!(facts_source.contains("impl TypeKey"));
    assert!(facts_source.contains("pub fn named(type_name: impl Into<String>) -> Self"));
    assert!(facts_source.contains("pub fn as_str(&self) -> &str"));
    let type_key_struct_source = facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub struct TypeKey")
        .nth(1)
        .and_then(|tail| tail.split("impl TypeKey").next())
        .expect("facts::TypeKey struct source should be present");
    assert!(type_key_struct_source.contains("type_name: String"));
    assert!(!type_key_struct_source.contains("pub(crate) type_name: String"));
    assert!(!type_key_struct_source.contains("pub type_name: String"));
    assert!(facts_source.contains("impl MemberName"));
    assert!(facts_source.contains("pub fn named(member_name: impl Into<String>) -> Self"));
    assert!(facts_source.contains("pub struct MemberName(String);"));
    assert!(!facts_source.contains("pub struct MemberName(pub(crate) String);"));
    assert!(!facts_source.contains("pub struct MemberName(pub String);"));
    assert!(facts_source.contains("pub fn owner_type(&self) -> &str"));
    assert!(facts_source.contains("pub fn member_name(&self) -> &str"));
    assert!(facts_source.contains("pub fn params(&self) -> &[TypePattern]"));
    assert!(
        facts_source
            .contains("pub(crate) fn with_owner_type(mut self, owner: impl Into<String>) -> Self")
    );
    assert!(
        !facts_source
            .contains("pub fn with_owner_type(mut self, owner: impl Into<String>) -> Self")
    );
    for raw_callable_field in [
        "pub owner: TypeKey",
        "pub member: MemberName",
        "pub kind: CallableKind",
        "pub staticness: Staticness",
        "pub arity: usize",
        "pub params: Vec<TypePattern>",
        "pub generic_arity: Option<usize>",
    ] {
        assert!(
            !facts_source.contains(raw_callable_field),
            "facts::CallableKey should expose `{raw_callable_field}` through accessors"
        );
    }
    for crate_callable_field in [
        "pub(crate) owner: TypeKey",
        "pub(crate) member: MemberName",
        "pub(crate) kind: CallableKind",
        "pub(crate) staticness: Staticness",
        "pub(crate) arity: usize",
        "pub(crate) params: Vec<TypePattern>",
        "pub(crate) generic_arity: Option<usize>",
    ] {
        assert!(
            !facts_source.contains(crate_callable_field),
            "facts::CallableKey should expose `{crate_callable_field}` through accessors"
        );
    }
    assert!(facts_source.contains("impl TypePattern"));
    assert!(facts_source.contains("pub fn exact(type_name: impl Into<String>) -> Self"));
    assert!(facts_source.contains("pub fn erased(type_name: impl Into<String>) -> Self"));
    assert!(!facts_source.contains("TypePattern::Exact(TypeKey {"));
    assert!(!facts_source.contains("TypePattern::Erased(TypeKey {"));
    let hierarchy_reachability_diagnostic_source = facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct HierarchyReachabilityDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl HierarchyReachabilityDiagnostic").next())
        .expect("facts::HierarchyReachabilityDiagnostic struct source should be present");
    assert!(!hierarchy_reachability_diagnostic_source.contains("pub code: &'static str"));
    assert!(!hierarchy_reachability_diagnostic_source.contains("pub message: String"));
    assert!(
        facts_source.contains("impl HierarchyReachabilityDiagnostic")
            && facts_source
                .contains("pub(crate) fn new(code: &'static str, message: impl Into<String>)")
            && facts_source.contains("pub fn code(&self) -> &'static str")
            && facts_source.contains("pub fn message(&self) -> &str"),
        "hierarchy reachability diagnostics should expose public accessors, not a public constructor or raw fields"
    );
    let callable_key_constructor_source = facts_source
        .split("impl CallableKey")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub struct FieldKey")
                .next()
        })
        .expect("CallableKey constructor source should be present");
    assert!(callable_key_constructor_source.contains("owner: TypeKey::named(owner)"));
    assert!(callable_key_constructor_source.contains("member: MemberName::named(method)"));
    assert!(callable_key_constructor_source.contains("member: MemberName::named(\"<init>\")"));
    assert!(callable_key_constructor_source.contains("member: MemberName::named(field)"));
    assert!(!callable_key_constructor_source.contains("owner: TypeKey {"));
    assert!(!callable_key_constructor_source.contains("member: MemberName("));
    assert!(!include_str!("../src/stdlib.rs").contains("TypePattern::Exact(TypeKey {"));
    assert!(!include_str!("../src/stdlib.rs").contains("TypePattern::Erased(TypeKey {"));
    let fact_method_key_source = facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub struct MethodKey")
        .nth(1)
        .and_then(|tail| tail.split("impl MethodKey").next())
        .expect("facts::MethodKey struct source should be present");
    assert!(!fact_method_key_source.contains("pub type_name: String"));
    assert!(!fact_method_key_source.contains("pub method_name: String"));
    assert!(facts_source.contains("pub fn source_method("));
    assert!(facts_source.contains("pub(crate) fn with_method_name("));
    assert!(!facts_source.contains("pub fn with_method_name("));
    assert!(facts_source.contains("pub fn method_name(&self) -> &str"));
    assert!(
        facts_source
            .contains("pub(crate) fn external_api_effect_for_receiver_with_arity_or_rust_name")
    );
    assert!(
        !facts_source.contains("pub fn external_api_effect_for_receiver_with_arity_or_rust_name")
    );
    assert!(facts_source.contains("pub(crate) fn external_api_effect_for("));
    assert!(!facts_source.contains("pub fn external_api_effect_for("));
    assert!(facts_source.contains("pub(crate) fn external_api_argument_effect_for_key"));
    assert!(!facts_source.contains("pub fn external_api_argument_effect_for_key"));
    assert!(!facts_source.contains("fn external_api_effect_for_key"));
    assert!(!facts_source.contains("pub fn external_api_effect_for_key"));
    let external_packs_source = include_str!("../src/external_packs.rs");
    assert!(!external_packs_source.contains("fn external_api_effect_for_key"));
    assert!(!external_packs_source.contains("pub fn external_api_effect_for_key"));
    assert!(!external_packs_source.contains("fn external_constructor_for_key"));
    assert!(!external_packs_source.contains("pub fn external_constructor_for_key"));
    assert!(!external_packs_source.contains("fn external_field_for_key"));
    assert!(!external_packs_source.contains("pub fn external_field_for_key"));
    assert!(!facts_source.contains("fn external_constructor_for_key"));
    assert!(!facts_source.contains("pub fn external_constructor_for_key"));
    assert!(!facts_source.contains("fn external_field_for_key"));
    assert!(!facts_source.contains("pub fn external_field_for_key"));
    let fact_constructor_key_source = facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub struct ConstructorKey")
        .nth(1)
        .and_then(|tail| tail.split("impl ConstructorKey").next())
        .expect("facts::ConstructorKey struct source should be present");
    assert!(!fact_constructor_key_source.contains("pub type_name: String"));
    assert!(!fact_constructor_key_source.contains("pub rust_name: String"));
    assert!(facts_source.contains("pub fn rust_name(&self) -> &str"));
    assert!(facts_source.contains("impl FieldKey"));
    let fact_field_key_source = facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub struct FieldKey")
        .nth(1)
        .and_then(|tail| tail.split("impl FieldKey").next())
        .expect("facts::FieldKey struct source should be present");
    assert!(!fact_field_key_source.contains("pub type_name: String"));
    assert!(!fact_field_key_source.contains("pub field_name: String"));
    assert!(facts_source.contains(
        "pub fn named(type_name: impl Into<String>, field_name: impl Into<String>) -> Self"
    ));
    assert!(facts_source.contains("pub fn source_field("));
    assert!(
        include_str!("../src/semantic_analysis.rs").contains("FieldKey::source_field(ty, field)")
    );
    assert!(
        include_str!("../src/semantic_analysis.rs")
            .contains("FieldKey::named(ty.name.clone(), field_name)")
    );
    assert!(
        include_str!("../src/semantic_analysis.rs")
            .contains("FieldKey::named(type_name.clone(), fact.target_name.clone())")
    );
    assert!(
        include_str!("../src/resolve.rs")
            .contains("FieldKey::named(type_name.clone(), field.name.clone())")
    );
    let resolve_source = include_str!("../src/resolve.rs");
    assert!(resolve_source.contains("FieldKey::named("));
    assert!(resolve_source.contains("declaration.owner.clone()"));
    assert!(resolve_source.contains("declaration.field.name.clone()"));
    assert!(include_str!("../src/stdlib.rs").contains("FieldKey::named(owner, field_name)"));
    assert!(
        include_str!("../src/external_facts.rs").contains("FieldKey::named(type_name, field_name)")
    );
    let external_facts_source = include_str!("../src/external_facts.rs");
    let external_parse_source = external_facts_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ExternalFactsParse")
        .nth(1)
        .and_then(|tail| tail.split("impl ExternalFactsParse").next())
        .expect("ExternalFactsParse source should remain discoverable");
    assert!(!external_parse_source.contains("pub facts: FactSet"));
    assert!(!external_parse_source.contains("pub diagnostics: DiagnosticBag"));
    assert!(external_facts_source.contains("pub fn facts(&self) -> &FactSet"));
    assert!(external_facts_source.contains("pub fn diagnostics(&self) -> &DiagnosticBag"));
    assert!(external_facts_source.contains("pub fn into_parts(self) -> (FactSet, DiagnosticBag)"));
    let external_pack_manifest_parse_source = include_str!("../src/external_packs.rs")
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct ExternalPackManifestParse")
        .nth(1)
        .and_then(|tail| tail.split("impl ExternalPackManifestParse").next())
        .expect("ExternalPackManifestParse source should remain discoverable");
    assert!(!external_pack_manifest_parse_source.contains("pub facts: FactSet"));
    assert!(!external_pack_manifest_parse_source.contains("pub diagnostics: DiagnosticBag"));
    assert!(include_str!("../src/external_packs.rs").contains("pub fn facts(&self) -> &FactSet"));
    assert!(
        include_str!("../src/external_packs.rs")
            .contains("pub fn diagnostics(&self) -> &DiagnosticBag")
    );
    assert!(
        include_str!("../src/external_packs.rs")
            .contains("pub fn into_parts(self) -> (FactSet, DiagnosticBag)")
    );
    assert!(
        include_str!("../src/external_packs.rs")
            .contains("pub(crate) fn builtin_external_pack_manifest_slice()")
    );
    assert!(
        include_str!("../src/external_packs.rs")
            .contains("static MANIFESTS: OnceLock<Vec<ExternalPackManifest>>")
    );
    let external_packs_production = include_str!("../src/external_packs.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production external_packs source should be present");
    for (path, source) in [
        ("src/facts.rs", facts_source),
        ("src/call_plan.rs", include_str!("../src/call_plan.rs")),
        ("src/stdlib.rs", include_str!("../src/stdlib.rs")),
        ("src/external_packs.rs", external_packs_production),
        ("src/usage.rs", include_str!("../src/usage.rs")),
        ("src/signature.rs", include_str!("../src/signature.rs")),
        ("src/lower.rs", include_str!("../src/lower.rs")),
    ] {
        for pattern in [
            ".owner.type_name",
            ".member.0",
            "abi.key.arity ",
            "abi.key.arity ==",
            "abi.key.kind ",
            "abi.key.kind ==",
            "abi.key.params.",
            "abi.key.staticness ",
            "fact.key.arity ",
            "fact.key.arity ==",
            "fact.key.kind ",
            "fact.key.kind ==",
            "fact.key.params.",
            "fact.key.staticness ",
            "key.params.as_slice()",
            "key.params.first()",
            "key.params.get(",
            "&key.params",
            "type_key.type_name",
        ] {
            assert!(
                !source.contains(pattern),
                "{path} should ask callable/type keys through accessors instead of `{pattern}`"
            );
        }
    }
    assert!(external_packs_production.contains("FieldKey::named(type_name, field_name)"));
    assert!(
        external_packs_production.contains("FieldKey::named(field.type_name, field.field_name)")
    );
    assert!(external_packs_production.contains("FieldKey::named(self.type_name, self.field_name)"));
    assert!(!include_str!("../src/semantic_analysis.rs").contains("field: FieldKey {"));
    assert!(!include_str!("../src/resolve.rs").contains("field: FieldKey {"));
    assert!(!include_str!("../src/stdlib.rs").contains("field: FieldKey {"));
    assert!(!include_str!("../src/external_facts.rs").contains("field: FieldKey {"));
    assert!(!external_packs_production.contains("field: FieldKey {"));
    assert!(
        !usage_source.contains("unwrap_or_else(|| crate::parser::to_snake_case(&method.name))")
    );
    let exception_source = include_str!("../src/exception_plan.rs");
    assert!(exception_source.contains("source_method_rust_name(method)"));
    assert!(
        !exception_source.contains("unwrap_or_else(|| crate::parser::to_snake_case(&method.name))")
    );
    assert!(
        exception_source.contains("pub(crate) fn mark_returns_result_for_test"),
        "test-only fallibility-plan mutation hook should stay inside the crate"
    );
    assert!(
        !exception_source.contains("pub fn mark_returns_result_for_test"),
        "test-only fallibility-plan mutation hook should not be public API"
    );
    let java_signature_source = include_str!("../src/java_signature.rs");
    assert!(java_signature_source.contains("pub(crate) fn source_method_rust_name"));
    assert!(java_signature_source.contains("pub(crate) fn source_method_overload_rust_name"));
    assert!(java_signature_source.contains("pub(crate) fn erased_param_types"));
    assert!(java_signature_source.contains("pub(crate) fn erased_type_name"));
    assert!(java_signature_source.contains("pub(crate) fn method_staticness"));
    assert!(!java_signature_source.contains("pub fn source_method_rust_name"));
    assert!(!java_signature_source.contains("pub fn source_method_overload_rust_name"));
    assert!(!java_signature_source.contains("pub fn erased_param_types"));
    assert!(!java_signature_source.contains("pub fn erased_type_name"));
    assert!(!java_signature_source.contains("pub fn method_staticness"));
    assert!(lower_source.contains("CallableAnalysis::new(usage, &exceptions, &fallibility)"));
    assert!(lower_source.contains(
        "lower_program_with_semantic_plans_and_support_context(\n    program: &Program,\n    mut diagnostics: DiagnosticBag,\n    facts: &FactSet,\n    usage: &UsageFacts,\n    representations: &RepresentationPlan,\n    signatures: &SignaturePlan,"
    ));
}

#[test]
fn callable_analysis_shell_reaches_lowering_context() {
    let lib_source = include_str!("../src/lib.rs");
    let analysis_source = include_str!("../src/callable_analysis.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(lib_source.contains("mod callable_analysis;"));
    assert!(!lib_source.contains("pub mod callable_analysis;"));
    assert!(analysis_source.contains("pub(crate) struct CallableAnalysis<'a>"));
    assert!(!analysis_source.contains("pub struct CallableAnalysis<'a>"));
    for field in ["usage", "exceptions", "fallibility"] {
        assert!(
            !analysis_source.contains(&format!("pub {field}:")),
            "CallableAnalysis should expose query methods, not public backing field {field}"
        );
    }
    assert!(!analysis_source.contains("signatures:"));
    assert!(!analysis_source.contains("call_graph:"));
    assert!(!analysis_source.contains("fn method_usage("));
    assert!(!analysis_source.contains("pub fn method_usage("));
    assert!(!analysis_source.contains("method_usage_by_java_key("));
    assert!(!analysis_source.contains("pub fn method_usage_by_java_key("));
    assert!(analysis_source.contains("pub(crate) fn binding_use_count_for_rust_method_family("));
    assert!(analysis_source.contains("pub(crate) fn binding_facts_for_rust_method_family("));
    assert!(!analysis_source.contains("method_signature_by_java_key("));
    assert!(!analysis_source.contains("fn constructor_usage("));
    assert!(!analysis_source.contains("pub fn constructor_usage("));
    assert!(analysis_source.contains("pub(crate) fn callable_returns_result("));
    assert!(analysis_source.contains("pub(crate) fn method_exception_plan("));
    assert!(analysis_source.contains("pub(crate) fn external_method_returns_result("));
    assert!(!analysis_source.contains("pub fn callable_returns_result("));
    assert!(!analysis_source.contains("pub fn method_exception_plan("));
    assert!(!analysis_source.contains("pub fn external_method_returns_result("));
    assert!(!analysis_source.contains("pub fn method_usage_for_source_method("));
    assert!(!analysis_source.contains("pub fn method_signature_for_source_method("));
    assert!(!analysis_source.contains("pub fn source_method_returns_result("));
    assert!(!analysis_source.contains("JavaCallableKey"));
    assert!(lower_source.contains("callable_analysis: &'a CallableAnalysis<'a>"));
    assert!(lower_source.contains("current_method_decl: Option<&'a MethodDecl>"));
    assert!(
        lower_source.contains("let call_graph = crate::usage::build_call_graph")
            && lower_source.contains("let fallibility = plan_program_fallibility")
            && lower_source.contains(
                "let callable_analysis = CallableAnalysis::new(usage, &exceptions, &fallibility);"
            )
    );
    assert!(lower_source.contains(".callable_analysis\n            .current_rust_method_usage("));
    let call_plan_source = include_str!("../src/call_plan.rs");
    assert!(call_plan_source.contains("pub fn external_facade_method_rust_name_for_abi"));
    assert!(call_plan_source.contains("pub fn external_facade_constructor_rust_name_for_abi"));
    assert!(call_plan_source.contains("pub(crate) fn unresolved_constructor_rust_name_for_facts"));
    assert!(call_plan_source.contains("pub(crate) fn external_method_rust_name"));
    assert!(!call_plan_source.contains("pub fn unresolved_constructor_rust_name_for_facts"));
    assert!(!call_plan_source.contains("pub fn external_method_rust_name("));
    assert!(include_str!("../src/main.rs").contains("external_facade_method_rust_name_for_abi"));
    assert!(
        include_str!("../src/main.rs").contains("external_facade_constructor_rust_name_for_abi")
    );
    let resolved_source_arg_plan = lower_source
        .split("fn emitted_source_resolved_call_argument_plan")
        .nth(1)
        .and_then(|tail| tail.split("fn resolved_source_method_for_call").next())
        .expect("resolved source call argument planner should be present");
    assert!(resolved_source_arg_plan.contains("method_signature_for_resolved_source_call"));
    let resolved_source_signature_helper = lower_source
        .split("fn method_signature_for_resolved_source_call")
        .nth(1)
        .and_then(|tail| tail.split("fn method_effective_rust_name").next())
        .expect("resolved source signature helper should be present");
    assert!(
        resolved_source_signature_helper.contains("signatures.method_signature(owner, method)")
    );
    assert!(!resolved_source_signature_helper.contains("JavaCallableKey::source_method_for_type"));
    assert!(!resolved_source_signature_helper.contains("method_signature_by_java_key"));
    let resolved_static_fallback = lower_source
        .split("fn source_context_resolved_static_method_fallback_signature")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn non_emitted_static_context_fallback_signature_for_method")
                .next()
        })
        .expect("resolved static source fallback planner should be present");
    assert!(resolved_static_fallback.contains("resolved_source_method_for_call"));
    assert!(resolved_static_fallback.contains("ctx.signatures.method_signature(ty, method)"));
    assert!(!resolved_static_fallback.contains("JavaCallableKey::source_method_for_owner"));
    assert!(!resolved_static_fallback.contains("method_signature_by_java_key"));
    let resolved_varargs = lower_source
        .split("fn lower_resolved_varargs_call_args")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn lower_resolved_constructor_varargs_args")
                .next()
        })
        .expect("resolved varargs lowering should be present");
    assert!(resolved_varargs.contains("method_signature_for_resolved_source_call"));
    assert!(resolved_varargs.contains("method_signature_by_rust_name"));
    let signature_source = include_str!("../src/signature.rs");
    assert!(signature_source.contains("pub(crate) fn method_signature_by_rust_name"));
    assert!(signature_source.contains("pub(crate) fn constructor_signature_by_rust_name"));
    assert!(signature_source.contains("pub(crate) fn method_arg_shape_by_rust_name"));
    assert!(signature_source.contains("pub(crate) fn constructor_arg_shape_by_rust_name"));
    assert!(signature_source.contains("pub(crate) fn method_return_shape_by_rust_name"));
    assert!(
        signature_source
            .contains("pub(crate) fn has_method_signature_by_base_name_accepting_arity")
    );
    assert!(
        signature_source.contains("pub(crate) fn method_signature_by_base_name_accepting_arity")
    );
    assert!(signature_source.contains("pub(crate) fn method_signature_by_base_name_and_arity"));
    assert!(signature_source.contains("pub(crate) fn method_signature_by_base_name_and_arg_types"));
    assert!(
        signature_source
            .contains("pub(crate) fn method_signature_by_base_name_and_arg_types_with_facts")
    );
    assert!(
        signature_source.contains("#[cfg(test)]\n    pub(crate) fn method_signature_by_java_key")
    );
    assert!(signature_source.contains("pub(crate) fn resolved_source_method_signature"));
    assert!(signature_source.contains("pub(crate) fn resolved_source_method_return_shape"));
    assert!(!signature_source.contains("pub fn method_signature_by_rust_name"));
    assert!(!signature_source.contains("pub fn constructor_signature_by_rust_name"));
    assert!(!signature_source.contains("pub fn method_arg_shape_by_rust_name"));
    assert!(!signature_source.contains("pub fn constructor_arg_shape_by_rust_name"));
    assert!(!signature_source.contains("pub fn method_return_shape_by_rust_name"));
    assert!(!signature_source.contains("pub fn has_method_signature_by_base_name_accepting_arity"));
    assert!(!signature_source.contains("pub fn method_signature_by_base_name_accepting_arity"));
    assert!(!signature_source.contains("pub fn method_signature_by_base_name_and_arity"));
    assert!(!signature_source.contains("pub fn method_signature_by_base_name_and_arg_types"));
    assert!(
        !signature_source.contains("pub fn method_signature_by_base_name_and_arg_types_with_facts")
    );
    assert!(!signature_source.contains("pub fn method_signature_by_java_key"));
    assert!(!signature_source.contains("fn method_return_shape_by_java_key"));
    let expr_shape_source = include_str!("../src/expr_shape.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production expr_shape source should be present");
    assert!(expr_shape_source.contains("resolved_source_method_return_shape(call, source_name)"));
    assert!(!expr_shape_source.contains("JavaCallableKey::resolved_source_method"));
    assert!(!expr_shape_source.contains("JavaOwnerKey::resolved_source"));
    assert!(
        lower_source.contains(".callable_analysis\n            .current_rust_constructor_usage(")
    );
    assert!(lower_source.contains(".binding_use_count_for_rust_method_family("));
    assert!(lower_source.contains(".binding_facts_for_rust_method_family("));
    assert!(
        !lower_source.contains(".usage\n            .methods_by_rust_name")
            && !lower_source.contains(".usage.methods_by_rust_name"),
        "lowering should ask CallableAnalysis for legacy Rust-name family lookups"
    );
    let usage_source = include_str!("../src/usage.rs");
    assert!(
        !usage_source
            .contains("pub methods_by_rust_name: HashMap<UsageMethodKey, MethodUsageFacts>")
    );
    assert!(
        !usage_source
            .contains("pub(crate) methods_by_rust_name: HashMap<UsageMethodKey, MethodUsageFacts>")
    );
    assert!(usage_source.contains("pub(crate) fn binding_facts_for_rust_method_family"));
    assert!(usage_source.contains("fn rust_method_family_key_matches"));
    assert!(!usage_source.contains("pub(crate) fn rust_name_method_entries"));
    assert!(analysis_source.contains(".binding_facts_for_rust_method_family("));
    assert!(!analysis_source.contains(".rust_name_method_entries()"));
    assert!(!usage_source.contains("pub(crate) bindings: HashMap<String, BindingUseFacts>"));
    assert!(
        !usage_source.contains("pub(crate) bindings_by_id: HashMap<BindingId, BindingUseFacts>")
    );
    assert!(!usage_source.contains("pub bindings: HashMap<String, BindingUseFacts>"));
    assert!(!usage_source.contains("pub bindings_by_id: HashMap<BindingId, BindingUseFacts>"));
    let binding_use_facts_source = usage_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct BindingUseFacts")
        .nth(1)
        .and_then(|tail| tail.split("impl BindingUseFacts").next())
        .expect("BindingUseFacts source should remain discoverable");
    for field in [
        "pub name: String",
        "pub binding: Option<BindingId>",
        "pub java_ty: JavaType",
        "pub uses: Vec<BindingUse>",
    ] {
        assert!(
            !binding_use_facts_source.contains(field),
            "BindingUseFacts should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(usage_source.contains("pub fn name(&self) -> &str"));
    assert!(usage_source.contains("pub fn binding(&self) -> Option<&BindingId>"));
    assert!(usage_source.contains("pub fn java_ty(&self) -> &JavaType"));
    assert!(usage_source.contains("pub fn uses(&self) -> &[BindingUse]"));
    assert!(usage_source.contains("#[cfg(test)]\n    pub(crate) fn new("));
    assert!(!usage_source.contains("pub fn new(\n        name: impl Into<String>"));
    assert!(lower_source.contains(
        ".callable_analysis\n        .external_method_returns_result(receiver_type, name, arg_count)"
    ));
    assert!(lower_source.contains(".source_method_identity_returns_result("));
    assert!(
        !lower_source
            .contains(".callable_returns_result(&crate::usage::CallableKey::method_identity("),
        "lowering should ask CallableAnalysis for source-method result identity instead of fabricating CallableKey::method_identity directly"
    );
    assert!(
        !lower_source.contains("CallableKey::resolved_method("),
        "lowering should ask CallableAnalysis for resolved source-method result identity instead of fabricating CallableKey::resolved_method directly"
    );
    assert!(
        !lower_source.contains("CallableKey::source_constructor_for_type(")
            && !lower_source.contains("CallableKey::resolved_constructor(")
            && !lower_source.contains("CallableKey::ExternalConstructor"),
        "lowering should ask CallableAnalysis for constructor result identity instead of fabricating constructor CallableKey values directly"
    );
    assert!(lower_source.contains(
        "fn plan_call_exception_propagation_with_lower_context(\n    receiver: Option<&Expr>,"
    ));
    assert!(lower_source.contains("source_method_call_returns_result_with_exception_plan"));
    let binding_use_count_helper = lower_source
        .split("fn current_callable_binding_use_count_for_id")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn current_callable_binding_has_later_read")
                .next()
        })
        .expect("current callable use-count helper should be present");
    assert!(binding_use_count_helper.contains("current_method_decl"));
    assert!(binding_use_count_helper.contains("source_method_usage_for_resolved_owner"));
    assert!(!binding_use_count_helper.contains("JavaCallableKey::source_method_for_owner"));
    assert!(!binding_use_count_helper.contains("method_usage_by_java_key"));
    assert!(binding_use_count_helper.contains("binding_facts_for_id(binding_id)"));
    assert!(
        !binding_use_count_helper.contains("bindings_by_id.get(binding_id)"),
        "move/clone use counts should ask MethodUsageFacts to validate binding-id facts"
    );
    assert!(
        !binding_use_count_helper.contains(".bindings.get("),
        "move/clone use counts must not rejoin locals through the compatibility name map"
    );
    let binding_facts_helper = lower_source
        .split("fn current_callable_binding_facts")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn current_callable_binding_recovery_shape")
                .next()
        })
        .expect("current callable binding facts helper should be present");
    assert!(binding_facts_helper.contains("current_method_decl"));
    assert!(binding_facts_helper.contains("source_method_usage_for_resolved_owner"));
    assert!(!binding_facts_helper.contains("JavaCallableKey::source_method_for_owner"));
    assert!(!binding_facts_helper.contains("method_usage_by_java_key"));
    assert!(binding_facts_helper.contains("binding_facts_for_id(binding_id)"));
    assert!(
        !binding_facts_helper.contains("bindings_by_id.get(binding_id)"),
        "lowering binding fact queries should ask MethodUsageFacts to validate binding-id facts"
    );
    assert!(
        !binding_facts_helper.contains(".bindings.get("),
        "lowering binding fact queries must not rejoin locals through the compatibility name map"
    );
    assert!(
        !binding_facts_helper.contains("bindings.contains_key("),
        "lowering binding fact queries must not probe the compatibility name map"
    );
    let local_plan_helper = lower_source
        .split("fn local_plan_for_declaration")
        .nth(1)
        .and_then(|tail| tail.split("fn refine_local_plan_from_initializer").next())
        .expect("local plan helper should be present");
    assert!(local_plan_helper.contains("local_plan_by_binding"));
    assert!(local_plan_helper.contains("idless_compatibility_local_plan_for_declaration"));
    assert!(
        !local_plan_helper.contains("binding) {\n        return ctx\n            .signatures\n            .local_plan_by_binding(ctx.current_local_shapes, binding);\n    }\n    initializer?;"),
        "bound declarations must not fall through to the id-less local-plan compatibility cursor"
    );
    let idless_local_plan_helper = lower_source
        .split("fn idless_compatibility_local_plan_for_declaration")
        .nth(1)
        .and_then(|tail| tail.split("fn refine_local_plan_from_initializer").next())
        .expect("id-less local plan compatibility helper should be present");
    assert!(idless_local_plan_helper.contains("initializer?;"));
    assert!(idless_local_plan_helper.contains("idless_compatibility_local_plan_for_declared_type"));
    let facts_source = include_str!("../src/facts.rs");
    let call_plan_source = include_str!("../src/call_plan.rs");
    let resolved_arg_shape_helper = call_plan_source
        .split("fn resolved_call_signature_arg_shapes_by_java_key")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn resolved_call_signature_arg_shapes_by_args")
                .next()
        })
        .expect("resolved Java-key arg-shape helper should be present");
    assert!(
        resolved_arg_shape_helper.contains("resolved_source_method_signature(call, source_name)")
    );
    assert!(!resolved_arg_shape_helper.contains("JavaCallableKey"));
    assert!(!resolved_arg_shape_helper.contains("JavaOwnerKey::resolved_source"));
    assert!(!resolved_arg_shape_helper.contains("method_signature_by_java_key"));
    let resolved_with_facts = call_plan_source
        .split("pub fn plan_call_arguments_with_facts(")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub fn plan_call_arguments_with_facts_and_args")
                .next()
        })
        .expect("resolved call argument planner should be present");
    assert!(resolved_with_facts.contains("resolved_call_signature_arg_shapes_by_java_key"));
    let resolved_with_args = call_plan_source
        .split("pub fn plan_call_arguments_with_facts_and_args(")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn explicit_facade_arg_slots_for_resolved")
                .next()
        })
        .expect("resolved call argument planner with args should be present");
    assert!(resolved_with_args.contains("resolved_call_signature_arg_shapes_by_java_key"));
    for internal_helper in [
        "external_call_lowered_type_with_external_index",
        "external_receiver_call_return_type_with_external_index",
        "resource_close_declares_result_with_external_index",
        "external_call_returns_rust_result",
        "external_call_returns_rust_result_with_external_index",
        "external_call_returns_rust_result_for_receiver",
        "external_call_has_thrown_effect_for_receiver",
        "external_constructor_returns_rust_result",
        "external_receiver_overload_is_modeled",
        "external_constructor_is_modeled",
    ] {
        assert!(
            call_plan_source.contains(&format!("pub(crate) fn {internal_helper}")),
            "{internal_helper} should stay crate-local"
        );
        assert!(
            !call_plan_source.contains(&format!("pub fn {internal_helper}")),
            "{internal_helper} should not be public API"
        );
    }
    assert!(call_plan_source.contains(".facade_abi_for_key(&key)"));
    assert!(facts_source.contains("pub(crate) fn facade_abi_for_key"));
    assert!(!facts_source.contains("pub fn facade_abi_for_key"));
    let slot_result_helper = lower_source
        .split("fn slot_expr_needs_result_propagation_from_shape_env")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn source_method_call_returns_result_with_exception_plan")
                .next()
        })
        .expect("slot result propagation helper should be present");
    assert!(slot_result_helper.contains("external_call_returns_rust_result_for_receiver"));
    assert!(!slot_result_helper.contains("facade_abi_for_key"));
    assert!(!slot_result_helper.contains("external_rust_result_for_receiver"));
    let for_each_lambda_body = lower_source
        .split("fn lower_for_each_lambda_body_expr")
        .nth(1)
        .and_then(|tail| tail.split("fn lower_accessor_field").next())
        .expect("forEach lambda body lowerer should be present");
    assert!(for_each_lambda_body.contains("resolved.rust_name.clone()"));
    assert!(for_each_lambda_body.contains("plan_call_arguments_with_facts_and_args"));
    assert!(for_each_lambda_body.contains("lower_args_with_plan"));
    assert!(!lower_source.contains("fn lower_same_object_call_args"));
    assert!(!lower_source.contains("fn lower_same_object_call_arg_with_shape"));
}

#[test]
fn semantic_facts_own_nullability_and_external_api_heuristics_before_lowering() {
    let lower_source = include_str!("../src/lower.rs");
    let signature_source = include_str!("../src/signature.rs");
    let semantic_source = include_str!("../src/semantic_analysis.rs");

    assert!(semantic_source.contains("collect_field_nullability_facts"));
    assert!(semantic_source.contains("FiniteSaturationPolicy"));
    assert!(semantic_source.contains("SaturationStep"));
    assert!(semantic_source.contains("collect_external_api_effect_facts"));
    assert!(semantic_source.contains("collect_method_effect_facts"));
    assert!(semantic_source.contains("method_receiver_mutability_effect"));
    assert!(semantic_source.contains("method_return_nullability_effect"));
    assert!(signature_source.contains("method_effect_for"));
    assert!(signature_source.contains("receiver_constraints_with_facts"));
    assert!(signature_source.contains("return_nullability"));
    assert!(signature_source.contains("external_receiver_binding_constraints"));
    assert!(!signature_source.contains("block_mutates_self"));
    assert!(!signature_source.contains("stmt_mutates_self"));
    assert!(!lower_source.contains("field_nullability_for"));
    assert!(!lower_source.contains("external_api_effect_for"));
    assert!(!lower_source.contains("external_constructor_for("));
    assert!(!lower_source.contains("external_field_for"));
    assert!(!lower_source.contains("starts_with(\"set\")"));
    assert!(!lower_source.contains("starts_with(\"get\")"));
}

#[test]
fn map_get_return_adaptation_uses_native_lowered_type_plan() {
    let lower_source = include_str!("../src/lower.rs");
    let function = lower_source
        .split("fn lower_map_get_call_expr_for_slot")
        .nth(1)
        .expect("map get slot lowering helper should exist")
        .split("fn lower_map_get_to_array_call_expr_for_slot")
        .next()
        .expect("map get slot lowering helper should be followed by toArray helper");

    assert!(function.contains("native_collection_lowered_type"));
    assert!(function.contains("plan_lowered_slot_conversion"));
    assert!(!function.contains("JavaDisplayPlan::OptionalBorrowedValue"));
    assert!(!function.contains("map_type_args_from_java_type(&receiver_ty)"));
    assert!(!function.contains("borrowed_sequence_element_shape(value_ty)"));
}

#[test]
fn representation_lowering_owns_dyn_trait_impl_compatibility_policy() {
    let source = include_str!("../src/representation_lowering.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(source.contains("trait_impl_method_compatibility_error"));
    assert!(source.contains("pub(crate) enum TraitImplMethodFallbackPolicy"));
    assert!(source.contains("pub(crate) enum TraitImplMethodCompatibilityError"));
    assert!(source.contains("pub(crate) struct TraitImplMethodMessagePolicy"));
    assert!(!source.contains("pub enum TraitImplMethodFallbackPolicy"));
    assert!(!source.contains("pub enum TraitImplMethodCompatibilityError"));
    assert!(!source.contains("pub struct TraitImplMethodMessagePolicy"));
    assert!(source.contains("pub(crate) fn trait_impl_method_message_policy"));
    assert!(!source.contains("pub fn trait_impl_method_message_policy"));
    assert!(source.contains("fn compile_error_body(&self, signature: &str) -> String"));
    assert!(!source.contains("pub fn compile_error_body"));
    assert!(source.contains("implementing_method_candidates"));
    assert!(source.contains("incompatible Java interface method implementation"));
    assert!(source.contains("parameter count does not match interface"));
    assert!(source.contains("pub(crate) fn type_emission_for_with_facts"));
    assert!(!source.contains("pub fn type_emission_for_with_facts"));
    assert!(source.contains("pub(crate) fn type_emission_for_program_with_facts_and_exceptions"));
    assert!(source.contains("fn raw_type_item_for_with_facts"));
    assert!(!source.contains("pub fn raw_type_item_for_with_facts"));
    assert!(source.contains("pub(crate) fn raw_type_item_for_program_with_facts_and_exceptions"));
    assert!(source.contains("trait_method_receiver_plan"));
    assert!(source.contains("pub(crate) fn trait_impl_emissions_for_type_with_facts"));
    assert!(!source.contains("pub fn trait_impl_emissions_for_type_with_facts"));
    assert!(
        source.contains("pub(crate) fn trait_impl_emissions_for_type_with_facts_and_exceptions")
    );
    assert!(source.contains("pub(crate) fn source_hierarchy_carrier_item_for_type"));
    assert!(source.contains("pub(crate) fn source_class_bound_trait_item_for_type"));
    assert!(source.contains("pub(crate) fn source_class_bound_trait_impl_emissions_for_type"));
    assert!(source.contains("rust_type_for_slot_with_facts"));
    assert!(!source.contains("rust_type_for_slot(&"));
    assert!(lower_source.contains("raw_type_item_for_program_with_facts_and_exceptions"));
    assert!(!lower_source.contains("type_emission_for_with_facts"));
    assert!(!lower_source.contains("RepresentationTypeEmission"));
    assert!(!lower_source.contains("RawRustItem"));
    assert!(lower_source.contains("trait_impl_emissions_for_type_with_facts"));
    assert!(!lower_source.contains("incompatible Java interface method implementation"));
}

#[test]
fn coercion_plan_owns_invalid_cast_runtime_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let lower_source = include_str!("../src/lower.rs");
    let types_source = include_str!("../src/types.rs");

    assert!(conversion_source.contains("pub enum SyntaxSlotCoercionPolicy"));
    assert!(!source.contains("pub enum SyntaxSlotCoercionPolicy"));
    assert!(conversion_source.contains("pub enum Slot"));
    assert!(conversion_source.contains("pub fn assignment_slot"));
    assert!(conversion_source.contains("pub fn argument_slot"));
    assert!(conversion_source.contains("pub fn condition_slot"));
    assert!(conversion_source.contains("pub fn index_slot"));
    assert!(conversion_source.contains("pub fn assignment_slot_for_target"));
    assert!(conversion_source.contains("pub struct AssignmentValuePlan"));
    let assignment_value_plan_source = conversion_source
        .split("pub struct AssignmentValuePlan")
        .nth(1)
        .and_then(|tail| tail.split("pub enum AssignmentStatementPlan").next())
        .unwrap();
    assert!(conversion_source.contains("pub(crate) fn new(slot: Slot) -> Self"));
    assert!(conversion_source.contains("pub fn slot(&self) -> &Slot"));
    assert!(conversion_source.contains("pub fn into_slot(self) -> Slot"));
    assert!(!assignment_value_plan_source.contains("pub slot: Slot"));
    assert!(conversion_source.contains("pub fn assignment_value_plan_for_shape"));
    assert!(conversion_source.contains("pub fn assignment_value_plan_for_target"));
    assert!(conversion_source.contains("pub enum AssignmentStatementPlan"));
    assert!(conversion_source.contains("pub fn assignment_statement_plan_for_target"));
    assert!(conversion_source.contains("pub fn return_slot"));
    assert!(conversion_source.contains("pub enum ReturnValuePlan"));
    assert!(conversion_source.contains("pub fn return_value_plan"));
    assert!(!source.contains("pub enum Slot"));
    assert!(!source.contains("pub fn assignment_slot"));
    assert!(!source.contains("pub fn argument_slot"));
    assert!(!source.contains("pub fn condition_slot"));
    assert!(!source.contains("pub fn index_slot"));
    assert!(!source.contains("pub fn assignment_slot_for_target"));
    assert!(!source.contains("pub struct AssignmentValuePlan"));
    assert!(!source.contains("pub fn assignment_value_plan_for_shape"));
    assert!(!source.contains("pub fn assignment_value_plan_for_target"));
    assert!(!source.contains("pub enum AssignmentStatementPlan"));
    assert!(!source.contains("pub fn assignment_statement_plan_for_target"));
    assert!(!source.contains("pub fn return_slot"));
    assert!(!source.contains("pub enum ReturnValuePlan"));
    assert!(!source.contains("pub fn return_value_plan"));
    assert!(lower_source.contains("return_value_plan"));
    assert!(!lower_source.contains("return_slot"));
    assert!(lower_source.contains("assignment_value_plan_for_shape"));
    assert!(lower_source.contains("assignment_value_plan_for_target"));
    assert!(lower_source.contains("assignment_statement_plan_for_target"));
    assert!(!lower_source.contains("assignment_slot(shape"));
    assert!(!lower_source.contains("assignment_slot_for_target"));
    assert!(conversion_source.contains("pub struct SyntaxCoercionPlan"));
    let syntax_coercion_plan_source = conversion_source
        .split("pub struct SyntaxCoercionPlan")
        .nth(1)
        .and_then(|tail| tail.split("pub fn syntax_coercion_plan_with_facts").next())
        .unwrap();
    assert!(conversion_source.contains(
        "pub(crate) fn new(actual: JavaType, slot: Slot, policy: SyntaxSlotCoercionPolicy) -> Self"
    ));
    assert!(conversion_source.contains("pub fn actual(&self) -> &JavaType"));
    assert!(conversion_source.contains("pub fn slot(&self) -> &Slot"));
    assert!(conversion_source.contains("pub fn policy(&self) -> &SyntaxSlotCoercionPolicy"));
    assert!(!syntax_coercion_plan_source.contains("pub actual: JavaType"));
    assert!(!syntax_coercion_plan_source.contains("pub slot: Slot"));
    assert!(!syntax_coercion_plan_source.contains("pub policy: SyntaxSlotCoercionPolicy"));
    assert!(!source.contains("pub struct SyntaxCoercionPlan"));
    assert!(!conversion_source.contains("pub fn syntax_coercion_plan("));
    assert!(conversion_source.contains("pub fn syntax_coercion_plan_with_facts"));
    assert!(!source.contains("pub fn syntax_coercion_plan("));
    assert!(!source.contains("pub fn syntax_coercion_plan_with_facts"));
    assert!(source.contains("pub fn apply_syntax_coercion_plan"));
    assert!(source.contains("impl SyntaxCoercionPlan"));
    assert!(source.contains("pub fn plan_coercion_with_facts"));
    assert!(source.contains("crate::conversion_plan::syntax_coercion_plan_with_facts"));
    assert!(
        conversion_source.contains("SlotConversionPlan::Syntax(syntax_coercion_plan_with_facts")
    );
    assert!(source.contains("apply_syntax_coercion_plan(expr, plan)"));
    assert!(!conversion_source.contains("pub fn syntax_slot_coercion_policy("));
    assert!(conversion_source.contains("fn syntax_slot_coercion_policy_from_content"));
    assert!(!source.contains("pub fn syntax_slot_coercion_policy("));
    assert!(conversion_source.contains("pub enum DispatchCoercionPolicy"));
    assert!(!conversion_source.contains("pub fn dispatch_coercion_policy("));
    assert!(conversion_source.contains("pub fn dispatch_coercion_policy_with_facts"));
    assert!(!source.contains("pub enum DispatchCoercionPolicy"));
    assert!(!source.contains("pub fn dispatch_coercion_policy("));
    assert!(!source.contains("pub fn dispatch_coercion_policy_with_facts"));
    assert!(!source.contains("pub enum BorrowedDynToOwnedBoxedDynRecoveryPolicy"));
    assert!(!source.contains("pub enum DynCloneBoxAdapterRequirement"));
    assert!(!source.contains("pub enum DynCloneBoxAdapterGenerationPolicy"));
    assert!(conversion_source.contains("CloneBoxDynTrait"));
    assert!(source.contains("Coercion::CloneBoxDynTrait"));
    assert!(source.contains("pub struct UnsupportedCoercionMessagePolicy"));
    let unsupported_coercion_message_policy_source = source
        .split("pub struct UnsupportedCoercionMessagePolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl UnsupportedCoercionMessagePolicy").next())
        .expect("UnsupportedCoercionMessagePolicy source should remain discoverable");
    assert!(source.contains("pub fn new(code: &'static str, message: impl Into<String>) -> Self"));
    assert!(source.contains("pub fn code(&self) -> &'static str"));
    assert!(source.contains("pub fn message(&self) -> &str"));
    assert!(!unsupported_coercion_message_policy_source.contains("pub code: &'static str"));
    assert!(!unsupported_coercion_message_policy_source.contains("pub message: String"));
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");
    assert!(design.contains("## Boxed Dyn Clone-Box Requirements"));
    assert!(design.contains("DispatchCoercionPolicy::CloneBoxDynTrait"));
    assert!(design.contains("Coercion::CloneBoxDynTrait"));
    assert!(conversion_source.contains("pub struct CoercionContentPolicy"));
    let coercion_content_policy_source = conversion_source
        .split("pub struct CoercionContentPolicy")
        .nth(1)
        .and_then(|tail| tail.split("pub struct CoercionShapeContentPolicy").next())
        .unwrap();
    assert!(
        conversion_source
            .contains("pub(crate) fn new(java_ty: JavaType, string_content: bool) -> Self")
    );
    assert!(conversion_source.contains("pub fn java_ty(&self) -> &JavaType"));
    assert!(conversion_source.contains("pub fn string_content(&self) -> bool"));
    assert!(!coercion_content_policy_source.contains("pub java_ty: JavaType"));
    assert!(!coercion_content_policy_source.contains("pub string_content: bool"));
    assert!(conversion_source.contains("pub fn coercion_content_policy("));
    assert!(conversion_source.contains("pub struct CoercionShapeContentPolicy"));
    let coercion_shape_content_policy_source = conversion_source
        .split("pub struct CoercionShapeContentPolicy")
        .nth(1)
        .and_then(|tail| tail.split("pub fn rust_nominal_type_name_policy").next())
        .unwrap();
    assert!(!coercion_shape_content_policy_source.contains("pub java_ty: JavaType"));
    assert!(!coercion_shape_content_policy_source.contains("pub string_content: bool"));
    assert!(!conversion_source.contains("pub fn coercion_shape_content_policy("));
    assert!(
        conversion_source.contains("pub fn coercion_shape_content_policy_with_effective_facts")
    );
    assert!(!source.contains("pub struct CoercionContentPolicy"));
    assert!(!source.contains("pub fn coercion_content_policy("));
    assert!(!source.contains("pub struct CoercionShapeContentPolicy"));
    assert!(!source.contains("pub fn coercion_shape_content_policy("));
    assert!(conversion_source.contains("pub struct OptionalElementTargetMatchPolicy"));
    let optional_element_match_source = conversion_source
        .split(
            "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct OptionalElementTargetMatchPolicy",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl OptionalElementTargetMatchPolicy").next())
        .expect("OptionalElementTargetMatchPolicy source should remain discoverable");
    assert!(optional_element_match_source.contains("matches: bool"));
    assert!(!optional_element_match_source.contains("pub matches: bool"));
    assert!(conversion_source.contains("impl OptionalElementTargetMatchPolicy"));
    assert!(conversion_source.contains("pub fn optional_element_target_match_policy"));
    assert!(!source.contains("pub struct OptionalElementTargetMatchPolicy"));
    assert!(!source.contains("pub fn optional_element_target_match_policy"));
    assert!(conversion_source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
    let borrowed_element_slot_adapter_source = conversion_source
        .split(
            "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct BorrowedElementSlotAdapterPolicy",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl BorrowedElementSlotAdapterPolicy").next())
        .expect("BorrowedElementSlotAdapterPolicy source should remain discoverable");
    let borrowed_element_slot_adapter_impl_source = conversion_source
        .split("impl BorrowedElementSlotAdapterPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn borrowed_element_adapter_policy_with_unwrap")
                .next()
        })
        .expect("BorrowedElementSlotAdapterPolicy impl source should remain discoverable");
    assert!(borrowed_element_slot_adapter_impl_source.contains("pub(crate) fn new("));
    assert!(!borrowed_element_slot_adapter_impl_source.contains("pub fn new("));
    for field in [
        "pub target: RustSlotShape",
        "pub carrier_shape: RustSlotShape",
        "pub element_shape: RustSlotShape",
        "pub carrier: BorrowedElementCarrier",
        "pub matches: bool",
        "pub adapter: Option<BorrowedElementAdapterPolicy>",
    ] {
        assert!(
            !borrowed_element_slot_adapter_source.contains(field),
            "BorrowedElementSlotAdapterPolicy should expose policy accessors, not raw public fields: {field}"
        );
    }
    assert!(conversion_source.contains("pub fn target(&self) -> &RustSlotShape"));
    assert!(conversion_source.contains("pub fn carrier_shape(&self) -> &RustSlotShape"));
    assert!(conversion_source.contains("pub fn element_shape(&self) -> &RustSlotShape"));
    assert!(conversion_source.contains("pub fn carrier(&self) -> BorrowedElementCarrier"));
    assert!(conversion_source.contains("pub fn matches(&self) -> bool"));
    assert!(
        conversion_source
            .contains("pub fn adapter(&self) -> Option<&BorrowedElementAdapterPolicy>")
    );
    let borrowed_element_adapter_policy_source = conversion_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct BorrowedElementAdapterPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl BorrowedElementAdapterPolicy").next())
        .expect("BorrowedElementAdapterPolicy source should remain discoverable");
    for field in [
        "pub carrier: BorrowedElementCarrier",
        "pub element_shape: RustSlotShape",
        "pub recovery: BorrowedElementRecoveryPolicy",
        "pub unwrap_carrier: bool",
    ] {
        assert!(
            !borrowed_element_adapter_policy_source.contains(field),
            "BorrowedElementAdapterPolicy should expose policy accessors, not raw public fields: {field}"
        );
    }
    assert!(conversion_source.contains("pub fn carrier(&self) -> BorrowedElementCarrier"));
    assert!(conversion_source.contains("pub fn element_shape(&self) -> &RustSlotShape"));
    assert!(conversion_source.contains("pub fn recovery(&self) -> &BorrowedElementRecoveryPolicy"));
    assert!(conversion_source.contains("pub fn unwrap_carrier(&self) -> bool"));
    let borrowed_facade_return_policy_source = conversion_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct RustBorrowedFacadeReturnPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split(
                "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct RustOwnedFacadeReturnPolicy",
            )
            .next()
        })
        .expect("RustBorrowedFacadeReturnPolicy source should remain discoverable");
    for field in [
        "pub target: RustSlotShape",
        "pub recovery: RustBorrowedFacadeReturnRecovery",
    ] {
        assert!(
            !borrowed_facade_return_policy_source.contains(field),
            "RustBorrowedFacadeReturnPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(borrowed_facade_return_policy_source.contains("pub(crate) fn new("));
    assert!(!borrowed_facade_return_policy_source.contains("pub fn new("));
    assert!(
        borrowed_facade_return_policy_source.contains("pub fn target(&self) -> &RustSlotShape")
    );
    assert!(
        borrowed_facade_return_policy_source
            .contains("pub fn recovery(&self) -> RustBorrowedFacadeReturnRecovery")
    );
    let owned_facade_return_policy_source = conversion_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct RustOwnedFacadeReturnPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
                .next()
        })
        .expect("RustOwnedFacadeReturnPolicy source should remain discoverable");
    assert!(!owned_facade_return_policy_source.contains("pub target: RustSlotShape"));
    assert!(owned_facade_return_policy_source.contains("pub(crate) fn new("));
    assert!(!owned_facade_return_policy_source.contains("pub fn new("));
    assert!(owned_facade_return_policy_source.contains("pub fn target(&self) -> &RustSlotShape"));
    let unsigned_byte_array_return_policy_source = conversion_source
        .split(
            "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct RustUnsignedByteArrayReturnPolicy",
        )
        .nth(1)
        .and_then(|tail| {
            tail.split(
                "#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum BorrowedElementCarrier",
            )
            .next()
        })
        .expect("RustUnsignedByteArrayReturnPolicy source should remain discoverable");
    assert!(!unsigned_byte_array_return_policy_source.contains("pub optional: bool"));
    assert!(unsigned_byte_array_return_policy_source.contains("pub(crate) fn new("));
    assert!(!unsigned_byte_array_return_policy_source.contains("pub fn new("));
    assert!(unsigned_byte_array_return_policy_source.contains("pub fn optional(&self) -> bool"));
    assert!(!source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
    assert!(conversion_source.contains("pub fn borrowed_element_slot_adapter_policy"));
    assert!(!source.contains("pub fn borrowed_element_slot_adapter_policy"));
    assert!(source.contains("fn apply_borrowed_element_slot_adapter"));
    let borrowed_element_adapter_source = source
        .split("fn apply_borrowed_element_slot_adapter")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn borrowed_option_adapter_should_unwrap_by_move")
                .next()
        })
        .expect("borrowed element slot adapter should remain discoverable");
    assert!(
        borrowed_element_adapter_source
            .contains("recover_borrowed_element_slot_adapter_input(expr)")
            && borrowed_element_adapter_source
                .contains("fn recover_borrowed_element_slot_adapter_input")
            && borrowed_element_adapter_source
                .contains("recover_rendered_method_receiver_expr(&rendered)")
            && !borrowed_element_adapter_source
                .contains("recover_parseable_raw_operand_projection(RustExpr::Raw(rendered))")
            && !borrowed_element_adapter_source
                .contains("rendered_plus_expr_projection_or_raw_leaf(&rendered)")
            && borrowed_element_adapter_source
                .contains("unwrap_or_else(|| RustExpr::raw_leaf(rendered))")
            && !borrowed_element_adapter_source
                .contains("unwrap_or_else(|| RustExpr::Raw(rendered))"),
        "borrowed element slot adapter should centralize parseable raw input recovery before applying adapter policy"
    );
    assert!(conversion_source.contains("pub enum SlotConversionPlan"));
    assert!(!source.contains("pub enum SlotConversionPlan"));
    assert!(!conversion_source.contains("pub fn slot_conversion_plan("));
    assert!(conversion_source.contains("pub fn slot_conversion_plan_with_facts"));
    assert!(!conversion_source.contains("pub fn lowered_slot_conversion_plan("));
    assert!(conversion_source.contains("pub fn lowered_slot_conversion_plan_with_facts"));
    assert!(!source.contains("pub fn slot_conversion_plan("));
    assert!(!source.contains("pub fn slot_conversion_plan_with_facts"));
    assert!(!source.contains("pub fn lowered_slot_conversion_plan("));
    assert!(!source.contains("pub fn lowered_slot_conversion_plan_with_facts"));
    assert!(source.contains("pub fn apply_slot_conversion_plan"));
    assert!(source.contains("SlotConversionPlan::Shape"));
    assert!(source.contains("SlotConversionPlan::Syntax"));
    assert!(source.contains("SlotConversionPlan::BorrowedElement"));
    assert!(source.contains("crate::conversion_plan::lowered_slot_conversion_plan_with_facts"));
    let lowered_expr_source = source
        .split("pub struct LoweredExpr")
        .nth(1)
        .and_then(|tail| tail.split("impl LoweredExpr").next())
        .expect("LoweredExpr source should remain discoverable");
    assert!(
        !lowered_expr_source.contains("pub expr:") && !lowered_expr_source.contains("pub actual:"),
        "LoweredExpr should expose accessors, not raw public fields"
    );
    assert!(source.contains("pub fn new(expr: RustExpr, actual: LoweredType) -> Self"));
    assert!(source.contains("pub fn expr(&self) -> &RustExpr"));
    assert!(source.contains("pub fn actual(&self) -> &LoweredType"));
    assert!(source.contains("pub fn into_expr(self) -> RustExpr"));
    assert!(source.contains("apply_slot_conversion_plan(lowered.into_expr(), &plan)"));
    assert!(conversion_source.contains("pub struct ShapeCoercionPlan"));
    assert!(!source.contains("pub struct ShapeCoercionPlan"));
    assert!(!conversion_source.contains("pub fn shape_coercion_plan("));
    assert!(conversion_source.contains("pub fn shape_coercion_plan_with_facts"));
    assert!(!source.contains("pub fn shape_coercion_plan"));
    assert!(!source.contains("pub fn shape_coercion_plan_with_facts"));
    assert!(source.contains("pub fn apply_shape_coercion_plan"));
    assert!(source.contains("impl ShapeCoercionPlan"));
    assert!(source.contains("pub fn plan_shape_coercion_with_facts"));
    assert!(source.contains("crate::conversion_plan::shape_coercion_plan_with_facts"));
    assert!(source.contains("apply_shape_coercion_plan(expr, plan)"));
    assert!(conversion_source.contains("pub enum OptionalShapeCoercionPolicy"));
    assert!(!conversion_source.contains("pub fn optional_shape_coercion_policy("));
    assert!(conversion_source.contains("pub fn optional_shape_coercion_policy_with_facts"));
    assert!(conversion_source.contains("element_plan: Box<ShapeCoercionPlan>"));
    assert!(conversion_source.contains("fn optional_element_slot_shape"));
    assert!(!source.contains("pub enum OptionalShapeCoercionPolicy"));
    assert!(!source.contains("pub fn optional_shape_coercion_policy("));
    assert!(!source.contains("pub fn optional_shape_coercion_policy_with_facts"));
    assert!(source.contains("pub(crate) fn non_identity_coercions"));
    assert!(!source.contains("pub fn non_identity_coercions"));
    assert!(!source.contains("element_coercions: Vec<Coercion>"));
    assert!(conversion_source.contains("pub struct VecElementTargetMatchPolicy"));
    let vec_element_match_source = conversion_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct VecElementTargetMatchPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl VecElementTargetMatchPolicy").next())
        .expect("VecElementTargetMatchPolicy source should remain discoverable");
    assert!(vec_element_match_source.contains("matches: bool"));
    assert!(!vec_element_match_source.contains("pub matches: bool"));
    assert!(conversion_source.contains("impl VecElementTargetMatchPolicy"));
    assert!(conversion_source.contains("pub fn vec_element_target_match_policy"));
    assert!(!source.contains("pub struct VecElementTargetMatchPolicy"));
    assert!(!source.contains("pub fn vec_element_target_match_policy"));
    assert!(conversion_source.contains("pub struct SetElementTargetMatchPolicy"));
    let set_element_match_source = conversion_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SetElementTargetMatchPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SetElementTargetMatchPolicy").next())
        .expect("SetElementTargetMatchPolicy source should remain discoverable");
    assert!(set_element_match_source.contains("matches: bool"));
    assert!(!set_element_match_source.contains("pub matches: bool"));
    assert!(conversion_source.contains("impl SetElementTargetMatchPolicy"));
    assert!(conversion_source.contains("pub fn set_element_target_match_policy"));
    assert!(!source.contains("pub struct SetElementTargetMatchPolicy"));
    assert!(!source.contains("pub fn set_element_target_match_policy"));
    assert!(source.contains("dispatch_coercion_fallback_message_policy"));
    assert!(source.contains("unsupported_dispatch_coercion"));
    assert!(conversion_source.contains("pub enum ViewCoercionPolicy"));
    assert!(conversion_source.contains("pub fn view_coercion_policy"));
    assert!(conversion_source.contains("pub fn view_coercion_policy_with_effective_facts"));
    assert!(!source.contains("pub enum ViewCoercionPolicy"));
    assert!(!source.contains("pub fn view_coercion_policy"));
    assert!(!source.contains("pub fn view_coercion_policy_with_effective_facts"));
    assert!(conversion_source.contains("pub enum InvalidCastPolicy"));
    assert!(conversion_source.contains("pub enum JavaCastSemanticsPolicy"));
    assert!(!conversion_source.contains("pub fn java_cast_semantics_policy("));
    assert!(conversion_source.contains("pub fn java_cast_semantics_policy_with_facts"));
    assert!(conversion_source.contains("pub enum JavaTypeConversionKind"));
    assert!(conversion_source.contains("pub struct JavaTypeConversionPlan"));
    let java_type_conversion_plan_source = conversion_source
        .split("pub struct JavaTypeConversionPlan")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub fn java_type_conversion_plan_from_evidence")
                .next()
        })
        .unwrap();
    assert!(conversion_source.contains(
        "pub(crate) fn new(actual: JavaType, target: JavaType, kind: JavaTypeConversionKind) -> Self"
    ));
    assert!(conversion_source.contains("pub fn actual(&self) -> &JavaType"));
    assert!(conversion_source.contains("pub fn target(&self) -> &JavaType"));
    assert!(conversion_source.contains("pub fn kind(&self) -> &JavaTypeConversionKind"));
    assert!(!java_type_conversion_plan_source.contains("pub actual: JavaType"));
    assert!(!java_type_conversion_plan_source.contains("pub target: JavaType"));
    assert!(!java_type_conversion_plan_source.contains("pub kind: JavaTypeConversionKind"));
    assert!(conversion_source.contains("pub fn java_type_conversion_plan_from_evidence"));
    assert!(conversion_source.contains("pub fn java_cast_type_conversion_plan"));
    assert!(types_source.contains("pub fn is_numeric_primitive_type"));
    assert!(types_source.contains("pub fn contains_wildcard_type"));
    assert!(types_source.contains("pub(crate) fn is_raw_generic_counterpart"));
    assert!(types_source.contains("pub(crate) fn is_unchecked_generic_counterpart"));
    assert!(types_source.contains("pub fn is_optional_or_result_type"));
    assert!(!source.contains("pub enum InvalidCastPolicy"));
    assert!(!source.contains("pub enum JavaCastSemanticsPolicy"));
    assert!(!source.contains("pub fn java_cast_semantics_policy("));
    assert!(!source.contains("pub enum JavaTypeConversionKind"));
    assert!(!source.contains("pub struct JavaTypeConversionPlan"));
    assert!(!source.contains("pub fn java_type_conversion_plan_from_evidence"));
    assert!(!source.contains("pub fn java_cast_type_conversion_plan"));
    assert!(conversion_source.contains("pub struct RustNominalTypeNamePolicy"));
    let nominal_policy_source = conversion_source
        .split("pub struct RustNominalTypeNamePolicy")
        .nth(1)
        .and_then(|tail| tail.split("pub enum RuntimeTypeTargetPolicy").next())
        .unwrap();
    assert!(
        conversion_source
            .contains("pub(crate) fn new(java_ty: JavaType, target_type: Option<String>) -> Self")
    );
    assert!(conversion_source.contains("pub fn java_ty(&self) -> &JavaType"));
    assert!(conversion_source.contains("pub fn target_type(&self) -> Option<&str>"));
    assert!(conversion_source.contains("pub fn into_target_type(self) -> Option<String>"));
    assert!(!nominal_policy_source.contains("pub java_ty: JavaType"));
    assert!(!nominal_policy_source.contains("pub target_type: Option<String>"));
    assert!(conversion_source.contains("pub fn rust_nominal_type_name_policy"));
    assert!(conversion_source.contains("pub enum RuntimeTypeTargetPolicy"));
    assert!(conversion_source.contains("pub fn runtime_type_target_policy"));
    assert!(!source.contains("pub struct RustNominalTypeNamePolicy"));
    assert!(!source.contains("pub fn rust_nominal_type_name_policy"));
    assert!(!source.contains("pub enum RuntimeTypeTargetPolicy"));
    assert!(!source.contains("pub fn runtime_type_target_policy"));
    assert!(conversion_source.contains("pub struct PrimitiveWideningPolicy"));
    let primitive_widening_policy_source = conversion_source
        .split("pub struct PrimitiveWideningPolicy")
        .nth(1)
        .and_then(|tail| tail.split("pub struct CoercionContentPolicy").next())
        .unwrap();
    assert!(conversion_source.contains(
        "pub(crate) fn new(from: PrimitiveType, to: PrimitiveType, widens: bool) -> Self"
    ));
    assert!(conversion_source.contains("pub fn from(&self) -> PrimitiveType"));
    assert!(conversion_source.contains("pub fn to(&self) -> PrimitiveType"));
    assert!(conversion_source.contains("pub fn widens(&self) -> bool"));
    assert!(!primitive_widening_policy_source.contains("pub from: PrimitiveType"));
    assert!(!primitive_widening_policy_source.contains("pub to: PrimitiveType"));
    assert!(!primitive_widening_policy_source.contains("pub widens: bool"));
    assert!(conversion_source.contains("pub fn primitive_widening_policy"));
    assert!(!conversion_source.contains("pub struct PrimitiveRankPolicy"));
    assert!(!conversion_source.contains("pub fn primitive_rank_policy"));
    assert!(!source.contains("pub struct PrimitiveWideningPolicy"));
    assert!(!source.contains("pub fn primitive_widening_policy"));
    assert!(!source.contains("pub struct PrimitiveRankPolicy"));
    assert!(!source.contains("pub fn primitive_rank_policy"));
    assert!(source.contains("pub(crate) struct RustPrimitiveTypeNamePolicy"));
    let primitive_type_name_policy_source = source
        .split("pub(crate) struct RustPrimitiveTypeNamePolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl RustPrimitiveTypeNamePolicy").next())
        .expect("RustPrimitiveTypeNamePolicy source should remain discoverable");
    assert!(source.contains("pub(crate) fn new(rust_name: &'static str) -> Self"));
    assert!(source.contains("pub(crate) fn rust_name(&self) -> &'static str"));
    assert!(!primitive_type_name_policy_source.contains("pub(crate) rust_name: &'static str"));
    assert!(source.contains("pub(crate) fn rust_primitive_type_name_policy"));
    assert!(!source.contains("pub struct RustPrimitiveTypeNamePolicy"));
    assert!(!source.contains("pub fn rust_primitive_type_name_policy"));
    assert!(!source.contains("fn rust_nominal_type_name("));
    assert!(!source.contains("fn option_target_matches("));
    assert!(!source.contains("fn vec_target_matches("));
    assert!(!source.contains("fn apply_borrowed_option_element_adapter"));
    assert!(!source.contains("fn apply_borrowed_vec_element_adapter"));
    assert!(!source.contains("fn primitive_widening("));
    assert!(!source.contains("fn primitive_rank("));
    assert!(!source.contains("fn rust_primitive_name("));
    assert!(source.contains("MissingRuntimeTarget"));
    assert!(source.contains("unsupported_dispatch_missing_runtime_target"));
    assert!(source.contains("invalid_cast: InvalidCastPolicy"));
    assert!(source.contains("DowncastBoxedDynTrait"));
    assert!(source.contains("DowncastErasedObject"));
    assert!(source.contains("apply_boxed_dyn_trait_downcast"));
    assert!(source.contains("invalid_cast_policy_expr"));
    assert!(conversion_source.contains("semantics: JavaCastSemanticsPolicy"));
    let scalar_slot_cast_policy_source = conversion_source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct RustScalarSlotCastPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split(
                "#[derive(Debug, Clone, PartialEq, Eq)]\npub struct RustBorrowedFacadeReturnPolicy",
            )
            .next()
        })
        .expect("RustScalarSlotCastPolicy source should remain discoverable");
    for field in [
        "pub rust_type_name: String",
        "pub target_primitive: PrimitiveType",
    ] {
        assert!(
            !scalar_slot_cast_policy_source.contains(field),
            "RustScalarSlotCastPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(scalar_slot_cast_policy_source.contains("pub(crate) fn new("));
    assert!(!scalar_slot_cast_policy_source.contains("pub fn new("));
    assert!(scalar_slot_cast_policy_source.contains("pub fn rust_type_name(&self) -> &str"));
    assert!(
        scalar_slot_cast_policy_source.contains("pub fn target_primitive(&self) -> PrimitiveType")
    );
    assert!(conversion_source.contains("PrimitiveNumericCast"));
    assert!(conversion_source.contains("CheckedReferenceDowncast"));
    assert!(conversion_source.contains("UncheckedGenericCast"));
    assert!(conversion_source.contains("RawOrWildcardCast"));
    assert!(conversion_source.contains("OptionalResultCast"));
    assert!(conversion_source.contains("DiagnosticOnlyFallback"));
}

#[test]
fn coercion_plan_owns_clone_recovery_policy() {
    let source = include_str!("../src/coercion.rs");
    let lower_source = include_str!("../src/lower.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let external_packs_source = include_str!("../src/external_packs.rs");
    let facts_source = include_str!("../src/facts.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");

    assert!(conversion_source.contains("pub enum CloneRecoveryPolicy"));
    assert!(conversion_source.contains("pub enum OwnershipCoercionPolicy"));
    assert!(conversion_source.contains("pub enum OwnershipCoercionFallbackPolicy"));
    assert!(!source.contains("pub enum CloneRecoveryPolicy"));
    assert!(!source.contains("pub enum OwnershipCoercionPolicy"));
    assert!(!source.contains("pub enum OwnershipCoercionFallbackPolicy"));
    assert!(!source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(conversion_source.contains("pub fn ownership_coercion_policy"));
    assert!(conversion_source.contains("ownership_coercion_policy_with_effective_facts"));
    assert!(conversion_source.contains("pub struct CoercionOwnedRecoveryCapabilityPolicy"));
    assert!(!conversion_source.contains("pub fn coercion_owned_recovery_capability_policy("));
    assert!(
        conversion_source
            .contains("coercion_owned_recovery_capability_policy_with_effective_facts")
    );
    let recovery_policy_source = conversion_source
        .split("pub struct CoercionOwnedRecoveryCapabilityPolicy")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub enum BorrowedElementRecoveryFallbackPolicy")
                .next()
        })
        .expect("owned recovery capability policy source should remain discoverable");
    for field in ["pub java_ty:", "pub recovery:"] {
        assert!(
            !recovery_policy_source.contains(field),
            "CoercionOwnedRecoveryCapabilityPolicy should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(recovery_policy_source.contains("pub fn java_ty(&self) -> &JavaType"));
    assert!(recovery_policy_source.contains("pub fn recovery(&self) -> OwnedRecoveryCapability"));
    assert!(facts_source.contains("pub struct OwnedRecoveryCapabilityFact"));
    assert!(facts_source.contains("owned_recovery_capability_facts"));
    assert!(facts_source.contains("CloneBoxAdapter"));
    assert!(facts_source.contains("FactoryCopy"));
    assert!(facts_source.contains("ImmutableSharedHandle"));
    assert!(facts_source.contains("FacadeOwnedExternalHandle"));
    assert!(facts_source.contains("FfiOwnedHandle"));
    assert!(external_packs_source.contains("external-pack-owned-recovery"));
    assert!(external_packs_source.contains("push_external_pack_owned_recovery_fact"));
    assert!(external_packs_source.contains("parse_external_pack_owned_recovery_capability"));
    assert!(external_packs_source.contains("owned_recovery_capability_facts"));
    assert!(!source.contains("pub fn ownership_coercion_policy"));
    assert!(!source.contains("pub fn ownership_coercion_policy_with_effective_facts"));
    assert!(!source.contains("pub struct CoercionOwnedRecoveryCapabilityPolicy"));
    assert!(!source.contains("pub fn coercion_owned_recovery_capability_policy("));
    assert!(conversion_source.contains("OwnershipCoercionPolicy::RecoverOwnedCopyValue"));
    assert!(source.contains("Coercion::CopyBorrowedValue"));
    assert!(conversion_source.contains("OwnershipCoercionPolicy::RecoverOwnedWithClone"));
    assert!(conversion_source.contains("BorrowedToOwnedNonCloneable"));
    assert!(conversion_source.contains("BorrowedToFacadeOwnedExternalHandle"));
    assert!(conversion_source.contains("BorrowedToFfiOwnedHandle"));
    assert!(source.contains("ownership_coercion_fallback_message_policy"));
    assert!(conversion_source.contains("policy: CloneRecoveryPolicy"));
    assert!(conversion_source.contains("ProvenCloneCapability"));
    assert!(effective_facts_source.contains("pub fn owned_recovery_capability_for"));
    assert!(effective_facts_source.contains("pub fn owned_recovery_capability_fact_for"));
    assert_eq!(
        conversion_source
            .matches("facts.owned_recovery_capability_for")
            .count(),
        1
    );
    assert!(conversion_source.contains("ownership_recovery_policy_from_capability"));
    assert!(source.contains("clone_audit_required"));
    assert!(source.contains("unsupported_ownership_coercion"));
    assert!(source.contains("unsupported_borrowed_to_owned_non_cloneable"));
    assert!(source.contains("render_clone_recovery_policy"));
    assert!(!source.contains("fn rendered_expr_has_clone_recovery"));
    assert!(!source.contains("fn rendered_expr_has_direct_clone_recovery"));
    assert!(source.contains("pub(crate) fn rust_expr_has_direct_clone_recovery"));
    assert!(
        !source.contains("rendered_expr_has_direct_clone_recovery(&emit::emit_expr(expr))"),
        "direct clone-recovery detection should inspect structured projections and raw leaves without re-rendering the full expression"
    );
    assert!(source.contains("pub(crate) fn apply_audited_clone_recovery"));
    assert!(lower_source.contains("enum CloneRecoveryDecision"));
    assert!(lower_source.contains("ApplyRecoveryCoercion"));
    assert!(lower_source.contains("coercion: Coercion"));
    assert!(lower_source.contains("fn clone_recovery_decision_for_argument"));
    assert!(lower_source.contains("fn clone_recovery_decision_for_value"));
    assert!(lower_source.contains("fn audited_clone_recovery_decision"));
    assert!(lower_source.contains("apply_clone_recovery_decision"));
    assert!(lower_source.contains("Coercion::CloneRecovery"));
    assert!(
        lower_source.contains("fn clone_recovery_idempotence_matches(\n    lowered: &RustExpr")
    );
    assert!(lower_source.contains("crate::coercion::rust_expr_has_clone_recovery(lowered)"));
    assert!(lower_source.contains("rust_expr_has_direct_clone_recovery(lowered)"));
    assert!(lower_source.contains("RustExprShape::OwnedRecovery"));
    assert!(lower_source.contains("fn direct_optional_clone_recovery_unwrap_expr"));
    let direct_optional_clone_recovery_source = lower_source
        .split("fn direct_optional_clone_recovery_unwrap_expr")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn rust_expr_has_optional_unwrap_or_java_null_expect")
                .next()
        })
        .expect("direct optional clone recovery helper should remain discoverable");
    assert!(
        direct_optional_clone_recovery_source.contains("method != \"unwrap\"")
            && direct_optional_clone_recovery_source
                .contains("optional_clone_recovery_owned_element_projection")
            && direct_optional_clone_recovery_source.contains("direct_optional_clone_receiver")
            && !direct_optional_clone_recovery_source.contains("emit::emit_expr")
            && !direct_optional_clone_recovery_source
                .contains("replace_optional_clone_recovery_unwrap"),
        "direct optional clone recovery should inspect method-call structure before rendered fallback"
    );
    let apply_clone_recovery_decision_source = lower_source
        .split("fn apply_clone_recovery_decision")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn direct_optional_clone_recovery_unwrap_expr")
                .next()
        })
        .expect("clone recovery decision application should remain discoverable");
    assert!(
        apply_clone_recovery_decision_source.contains("direct_optional_clone_recovery_unwrap_expr")
            && !apply_clone_recovery_decision_source.contains("emit::emit_expr(&lowered)")
            && !apply_clone_recovery_decision_source
                .contains("replace_optional_clone_recovery_unwrap"),
        "clone recovery decision application should stay on structured optional unwrap recovery"
    );
    assert!(
        !lower_source.contains("fn optional_clone_recovery_expr(rendered: String) -> RustExpr")
    );
    assert!(
        lower_source.contains("fn rust_expr_has_optional_unwrap_or_java_null_expect")
            && !lower_source.contains("rendered_expr_has_optional_unwrap_or_java_null_expect"),
        "lowering should guard optional unwraps structurally instead of rendering lowered expressions"
    );
    let optional_clone_stmt_source = lower_source
        .split("fn recover_optional_clone_recovery_stmt")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn recover_first_optional_clone_recovery_field")
                .next()
        })
        .expect("optional clone recovery statement helper should remain discoverable");
    assert!(
        optional_clone_stmt_source.contains("RustStmt::Raw(text)")
            && optional_clone_stmt_source.contains("raw_structured_stmt_projection(&text)")
            && !optional_clone_stmt_source.contains("| RustStmt::Raw(_) => None"),
        "optional clone recovery should recover parseable raw statements before preserving opaque raw"
    );
    let optional_unwrap_stmt_predicate_source = lower_source
        .split("fn rust_stmt_has_optional_unwrap_or_java_null_expect")
        .nth(1)
        .and_then(|tail| tail.split("fn clone_recovery_idempotence_matches").next())
        .expect("optional unwrap statement predicate should remain discoverable");
    assert!(
        optional_unwrap_stmt_predicate_source.contains("RustStmt::Raw(text)")
            && optional_unwrap_stmt_predicate_source
                .contains("raw_structured_stmt_projection(text)")
            && !optional_unwrap_stmt_predicate_source.contains("| RustStmt::Raw(_) => false"),
        "optional unwrap statement detection should inspect parseable raw statements structurally"
    );
    let unshaped_argument = lower_source
        .split("fn lower_unshaped_argument")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn lower_current_self_cast_unshaped_argument")
                .next()
        })
        .expect("unshaped argument lowering should exist");
    assert!(unshaped_argument.contains("apply_clone_recovery_decision"));
    assert!(!unshaped_argument.contains("apply_audited_clone_recovery"));
    let preserving_args = lower_source
        .split("fn lower_unshaped_external_facade_preserving_args")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn external_facade_preserving_arg_needs_clone")
                .next()
        })
        .expect("external facade preserving argument lowering should exist");
    assert!(preserving_args.contains("apply_clone_recovery_decision"));
    assert!(!preserving_args.contains("render_audited_clone_recovery"));
    assert!(lower_source.contains("struct MoveUseContext"));
    assert!(lower_source.contains("MoveUseContext::new(ctx)"));
    assert!(!lower_source.contains("render_audited_clone_recovery"));
    assert!(!lower_source.contains("fn rendered_expr_has_clone_recovery"));
    assert!(!lower_source.contains("fn rendered_expr_has_direct_clone_recovery"));
    assert!(!lower_source.contains("fn render_audited_clone_recovery"));
    let coercion_production_source = source
        .split("#[cfg(test)]")
        .next()
        .expect("coercion production source should be discoverable");
    assert!(
        !coercion_production_source.contains("pub fn render_clone_projection")
            && !coercion_production_source.contains("pub fn render_audited_clone_recovery")
            && !coercion_production_source.contains("fn render_clone_projection_expr"),
        "clone recovery composition should use structured apply_* helpers in production"
    );
    assert!(
        !coercion_production_source
            .contains("rendered_expr_has_optional_unwrap_or_java_null_expect"),
        "optional unwrap/null-expect detection should stay behind structured lowering helpers, with rendered compatibility only in tests"
    );
    assert!(
        !coercion_production_source.contains("typed_single_input_callback_projection"),
        "typed single-input callback rendering should stay test-only; production uses typed_single_input_callback_expr"
    );
    assert!(
        source.contains("#[cfg(test)]\nfn typed_single_input_callback_projection"),
        "rendered callback compatibility wrapper should be private to coercion tests"
    );
}

#[test]
fn coercion_plan_owns_borrowed_element_recovery_fallback_policy() {
    let source = include_str!("../src/coercion.rs");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");

    assert!(conversion_source.contains("pub enum BorrowedElementRecoveryPolicy"));
    assert!(!source.contains("pub enum BorrowedElementRecoveryPolicy"));
    assert!(conversion_source.contains("pub enum BorrowedElementCarrier"));
    assert!(conversion_source.contains("pub struct BorrowedElementAdapterPolicy"));
    assert!(!source.contains("pub enum BorrowedElementCarrier"));
    assert!(!source.contains("pub struct BorrowedElementAdapterPolicy"));
    assert!(!conversion_source.contains("pub fn borrowed_element_adapter_policy("));
    assert!(
        conversion_source.contains("pub(crate) fn borrowed_element_adapter_policy_with_unwrap")
    );
    assert!(!conversion_source.contains("pub fn borrowed_element_adapter_policy_with_unwrap"));
    assert!(!source.contains("pub fn borrowed_element_adapter_policy("));
    assert!(source.contains("fn apply_borrowed_element_adapter"));
    assert!(conversion_source.contains("pub fn borrowed_element_recovery_policy"));
    assert!(conversion_source.contains("borrowed_element_recovery_policy_with_effective_facts"));
    assert!(conversion_source.contains("pub enum BorrowedElementRecoveryFallbackPolicy"));
    assert!(conversion_source.contains("CompileError"));
    assert!(conversion_source.contains("NonCloneableElement"));
    assert!(conversion_source.contains("policy: CloneRecoveryPolicy"));
    assert!(conversion_source.contains("borrowed_element_recovery_policy_from_capability"));
    assert!(!source.contains("pub fn borrowed_element_recovery_policy"));
    assert!(!source.contains("pub fn borrowed_element_recovery_policy_with_effective_facts"));
    assert!(!source.contains("pub enum BorrowedElementRecoveryFallbackPolicy"));
    assert!(source.contains("borrowed_element_clone_audit_required"));
    assert!(source.contains("borrowed_element_recovery_fallback_message_policy"));
    assert!(source.contains("unsupported_borrowed_element_non_cloneable"));
    assert!(source.contains("render_borrowed_element_recovery_fallback_policy"));
    assert!(!source.contains("fn shape_has_string_content"));
    assert!(conversion_source.contains("coercion_content_policy_with_effective_facts"));
    assert!(conversion_source.contains("coercion_shape_content_policy_with_effective_facts"));
    assert!(!source.contains("pub fn coercion_content_policy_with_effective_facts"));
    assert!(!source.contains("pub fn coercion_shape_content_policy_with_effective_facts"));
    assert!(conversion_source.contains("view_coercion_policy_with_effective_facts"));
    assert!(!source.contains("pub fn view_coercion_policy_with_effective_facts"));
    assert!(effective_facts_source.contains("pub fn is_string_like_type"));
    assert!(
        conversion_source
            .contains("coercion_owned_recovery_capability_policy_with_effective_facts")
    );
    assert!(conversion_source.contains("pub fn borrowed_element_recovery_policy("));
    assert!(!source.contains("pub fn borrowed_element_recovery_policy("));
    assert!(conversion_source.contains("borrowed_element_recovery_policy(element_shape, facts)"));
    assert_eq!(
        conversion_source
            .matches("facts.owned_recovery_capability_for")
            .count(),
        1
    );
    assert!(!source.contains(".copy"));
    assert!(!source.contains(".clone {"));
    assert!(!source.contains(".string_like"));
}

#[test]
fn lower_consumes_cast_and_instanceof_plans_without_rechecking_representation_facts() {
    let lower_source = include_str!("../src/lower.rs");
    let cast_instanceof_branch = lower_source
        .rsplit("Expr::Cast { ty, expr } =>")
        .next()
        .and_then(|tail| tail.split("Expr::ClassLiteral(ty)").next())
        .expect("cast and instanceof lowering branch should be present");
    let conversion_source = include_str!("../src/conversion_plan.rs");
    let lowering_source = include_str!("../src/conversion_lowering.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("conversion lowering production source should be present");

    assert!(conversion_source.contains("project_java_value_shape(ctx.representations"));
    assert!(conversion_source.contains("ctx.facts().unwrap_or(&default_facts)"));
    assert!(
        conversion_source.contains("plan_instanceof_check_with_facts(&actual, &target, facts)")
    );
    assert!(
        conversion_source
            .contains("java_cast_semantics_policy_with_facts(&actual, &target, &coercion, facts)")
    );

    assert!(cast_instanceof_branch.contains("plan_cast_conversion_from_actual("));
    assert!(cast_instanceof_branch.contains("apply_cast_conversion("));
    assert!(cast_instanceof_branch.contains("plan_instanceof("));
    assert!(cast_instanceof_branch.contains("apply_instanceof("));
    assert!(
        cast_instanceof_branch.contains("lower_expr_with_refinements(expr, env, ctx, refinements)")
    );
    assert!(cast_instanceof_branch.contains("ExprShapeContext::new("));
    assert!(cast_instanceof_branch.contains("Some(ctx.representations)"));
    assert!(cast_instanceof_branch.contains("Some(ctx.facts)"));
    assert!(!cast_instanceof_branch.contains("ObjectRepresentation"));
    assert!(!cast_instanceof_branch.contains("representation_for("));
    assert!(!cast_instanceof_branch.contains("dispatch_for("));
    assert!(!cast_instanceof_branch.contains("HierarchyStrategy"));
    assert!(!cast_instanceof_branch.contains("hierarchy_subtype_policy"));
    assert!(!cast_instanceof_branch.contains("reachable_hierarchy_edges"));
    assert!(!cast_instanceof_branch.contains("type_capability_overrides"));
    assert!(!cast_instanceof_branch.contains("instanceof_target_policy"));
    assert!(!cast_instanceof_branch.contains("plan_instanceof_check"));
    assert!(!cast_instanceof_branch.contains("JavaCastSemanticsPolicy"));
    assert!(!cast_instanceof_branch.contains("java_cast_semantics_policy"));
    assert!(!cast_instanceof_branch.contains("InvalidCastPolicy"));

    assert!(lowering_source.contains("apply_shape_coercion_plan(expr, plan.coercion())"));
    assert!(lowering_source.contains("instanceof_check_projection(expr, plan.check())"));
    assert!(!lowering_source.contains("ObjectRepresentation"));
    assert!(!lowering_source.contains("HierarchyStrategy"));
    assert!(!lowering_source.contains("hierarchy_subtype_policy"));
    assert!(!lowering_source.contains("java_cast_semantics_policy"));
}

#[test]
fn signature_solver_collects_param_constraints_before_applying_shapes() {
    let source = include_str!("../src/signature.rs");

    assert!(source.contains("pub enum SignatureConstraint"));
    assert!(source.contains("RequiredOwnership"));
    assert!(source.contains("RequiredView"));
    assert!(source.contains("RequiredDispatch"));
    assert!(source.contains("pub enum SignatureSlotRole"));
    assert!(source.contains("pub struct SignatureCapabilityPolicy"));
    let capability_policy_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureCapabilityPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureCapabilityPolicy").next())
        .expect("signature capability policy should remain discoverable");
    assert!(!capability_policy_source.contains("pub java_ty: JavaType"));
    assert!(!capability_policy_source.contains("pub capability_ownership: Option<OwnershipShape>"));
    assert!(!capability_policy_source.contains("pub constraints: Vec<SignatureConstraint>"));
    assert!(source.contains("pub fn java_ty(&self) -> &JavaType"));
    assert!(source.contains("pub fn capability_ownership(&self) -> Option<OwnershipShape>"));
    assert!(source.contains("pub fn constraints(&self) -> &[SignatureConstraint]"));
    assert!(source.contains("pub struct SignatureShapeRequest"));
    let shape_request_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureShapeRequest")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureShapeRequest").next())
        .expect("signature shape request should remain discoverable");
    assert!(!shape_request_source.contains("pub java_ty: JavaType"));
    assert!(!shape_request_source.contains("pub role: SignatureSlotRole"));
    assert!(!shape_request_source.contains("pub constraints: Vec<SignatureConstraint>"));
    assert!(!shape_request_source.contains("pub type_params: Vec<String>"));
    assert!(source.contains("pub fn into_parts(\n        self,\n    ) -> (\n        JavaType,"));
    assert!(source.contains("pub struct MergedSignatureConstraints"));
    assert!(source.contains("pub struct SignatureConstraintConflict"));
    let constraint_conflict_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureConstraintConflict")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureConstraintConflict").next())
        .expect("signature constraint conflict should remain discoverable");
    assert!(!constraint_conflict_source.contains("pub axis: SignatureConstraintAxis"));
    assert!(!constraint_conflict_source.contains("pub kept: SignatureConstraint"));
    assert!(!constraint_conflict_source.contains("pub rejected: SignatureConstraint"));
    assert!(source.contains("pub fn axis(&self) -> SignatureConstraintAxis"));
    assert!(source.contains("pub fn kept(&self) -> &SignatureConstraint"));
    assert!(source.contains("pub fn rejected(&self) -> &SignatureConstraint"));
    assert!(source.contains("pub enum SignatureConstraintAxis"));
    assert!(source.contains("from_constraints"));
    assert!(source.contains("apply_to_shape"));
    assert!(source.contains("requires_mutable_binding"));
    assert!(source.contains("conflicts"));
    assert!(source.contains("pub struct SignatureConstraintState"));
    let constraint_state_source = source
        .split(
            "#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureConstraintState",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureConstraintState").next())
        .expect("signature constraint state source should remain discoverable");
    assert!(constraint_state_source.contains("methods: HashMap<MethodKey, MethodConstraintSet>"));
    assert!(
        !constraint_state_source.contains("pub methods: HashMap<MethodKey, MethodConstraintSet>")
    );
    assert!(
        constraint_state_source
            .contains("constructors: HashMap<ConstructorKey, ConstructorConstraintSet>")
    );
    assert!(
        !constraint_state_source
            .contains("pub constructors: HashMap<ConstructorKey, ConstructorConstraintSet>")
    );
    assert!(
        constraint_state_source.contains("fields: HashMap<FieldKey, Vec<SignatureConstraint>>")
    );
    assert!(
        !constraint_state_source
            .contains("pub fields: HashMap<FieldKey, Vec<SignatureConstraint>>")
    );
    assert!(source.contains("pub fn methods(&self) -> &HashMap<MethodKey, MethodConstraintSet>"));
    assert!(source.contains(
        "pub fn constructors(&self) -> &HashMap<ConstructorKey, ConstructorConstraintSet>"
    ));
    assert!(
        source.contains("pub fn fields(&self) -> &HashMap<FieldKey, Vec<SignatureConstraint>>")
    );
    assert!(source.contains("pub struct SignatureFixpointResult"));
    let fixpoint_result_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureFixpointResult")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureFixpointResult").next())
        .expect("signature fixpoint result source should remain discoverable");
    assert!(fixpoint_result_source.contains("constraints: SignatureConstraintState"));
    assert!(!fixpoint_result_source.contains("pub constraints: SignatureConstraintState"));
    assert!(fixpoint_result_source.contains("iterations: usize"));
    assert!(!fixpoint_result_source.contains("pub iterations: usize"));
    assert!(fixpoint_result_source.contains("diagnostics: Vec<SignatureDiagnostic>"));
    assert!(!fixpoint_result_source.contains("pub diagnostics: Vec<SignatureDiagnostic>"));
    assert!(fixpoint_result_source.contains("dependency_sccs: SignatureDependencySccAnalysis"));
    assert!(
        !fixpoint_result_source.contains("pub dependency_sccs: SignatureDependencySccAnalysis")
    );
    assert!(fixpoint_result_source.contains("pass_plan: SignatureSolverPassPlan"));
    assert!(!fixpoint_result_source.contains("pub pass_plan: SignatureSolverPassPlan"));
    assert!(source.contains("pub fn constraints(&self) -> &SignatureConstraintState"));
    assert!(source.contains("pub fn iterations(&self) -> usize"));
    assert!(source.contains("pub fn diagnostics(&self) -> &[SignatureDiagnostic]"));
    assert!(source.contains("pub fn into_constraints_iterations_diagnostics("));
    assert!(source.contains("pub fn dependency_sccs(&self) -> &SignatureDependencySccAnalysis"));
    assert!(source.contains("pub fn pass_plan(&self) -> &SignatureSolverPassPlan"));
    assert!(source.contains("pub struct SignatureDiagnostic"));
    let signature_diagnostic_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureDiagnostic").next())
        .expect("signature diagnostic should remain discoverable");
    assert!(!signature_diagnostic_source.contains("pub code: &'static str"));
    assert!(!signature_diagnostic_source.contains("pub message: String"));
    assert!(source.contains("pub fn code(&self) -> &'static str"));
    assert!(source.contains("pub fn message(&self) -> &str"));
    assert!(source.contains("pub struct SignatureSolverProgressEvent"));
    let progress_event_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureSolverProgressEvent")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureSolverProgressEvent").next())
        .expect("signature solver progress event should remain discoverable");
    assert!(!progress_event_source.contains("pub phase: &'static str"));
    assert!(!progress_event_source.contains("pub status: &'static str"));
    assert!(!progress_event_source.contains("pub elapsed_ms: u128"));
    assert!(!progress_event_source.contains("pub detail: String"));
    assert!(source.contains("pub fn phase(&self) -> &'static str"));
    assert!(source.contains("pub fn status(&self) -> &'static str"));
    assert!(source.contains("pub fn elapsed_ms(&self) -> u128"));
    assert!(source.contains("pub fn detail(&self) -> &str"));
    assert!(source.contains("pub struct ProfiledSignaturePlan"));
    let profiled_plan_source = source
        .split("#[derive(Debug, Clone)]\npub struct ProfiledSignaturePlan")
        .nth(1)
        .and_then(|tail| tail.split("impl ProfiledSignaturePlan").next())
        .expect("profiled signature plan should remain discoverable");
    assert!(!profiled_plan_source.contains("pub plan: SignaturePlan"));
    assert!(!profiled_plan_source.contains("pub profile: SignatureSolverProfile"));
    assert!(source.contains("pub fn into_parts(self) -> (SignaturePlan, SignatureSolverProfile)"));
    assert!(source.contains("pub struct SignatureSaturationReport"));
    let saturation_report_source = source
        .split(
            "#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureSaturationReport",
        )
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureSaturationReport").next())
        .expect("signature saturation report should remain discoverable");
    assert!(!saturation_report_source.contains("pub domain_stats: SignatureFixpointDomainStats"));
    assert!(!saturation_report_source.contains("pub iterations: usize"));
    assert!(!saturation_report_source.contains("pub diagnostics: usize"));
    assert!(
        !saturation_report_source.contains("pub dependency_stats: SignatureDependencySccStats")
    );
    assert!(!saturation_report_source.contains("pub pass_stats: SignatureSolverPassStats"));
    assert!(!saturation_report_source.contains("pub join_conflicts: usize"));
    assert!(!saturation_report_source.contains("pub top_fallback_reasons"));
    assert!(!saturation_report_source.contains("pub top_conflict_axes"));
    assert!(source.contains("pub fn domain_stats(&self) -> SignatureFixpointDomainStats"));
    assert!(source.contains(
        "pub fn into_top_counts(self) -> (Vec<SignatureReportCount>, Vec<SignatureReportCount>)"
    ));
    assert!(source.contains("pub struct SignaturePropagationArmProfile"));
    let propagation_arm_profile_source = source
        .split("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct SignaturePropagationArmProfile")
        .nth(1)
        .and_then(|tail| tail.split("impl SignaturePropagationArmProfile").next())
        .expect("signature propagation arm profile should remain discoverable");
    assert!(!propagation_arm_profile_source.contains("pub sweeps: usize"));
    assert!(!propagation_arm_profile_source.contains("pub changed_sweeps: usize"));
    assert!(!propagation_arm_profile_source.contains("pub elapsed_ns: u128"));
    assert!(source.contains("pub fn sweeps(&self) -> usize"));
    assert!(source.contains("pub fn changed_sweeps(&self) -> usize"));
    assert!(source.contains("pub fn elapsed_ns(&self) -> u128"));
    assert!(source.contains("pub struct SignatureSolverExecutionStats"));
    let execution_stats_source = source
        .split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureSolverExecutionStats")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureSolverExecutionStats").next())
        .expect("signature solver execution stats should remain discoverable");
    assert!(!execution_stats_source.contains("pub component_sweeps: usize"));
    assert!(!execution_stats_source.contains("pub changed_component_sweeps: usize"));
    assert!(!execution_stats_source.contains("pub state_clones: usize"));
    assert!(!execution_stats_source.contains("pub state_clone_ms: u128"));
    assert!(!execution_stats_source.contains("pub propagation_sweep_ms: u128"));
    assert!(!execution_stats_source.contains("pub component_rollback_ms: u128"));
    assert!(!execution_stats_source.contains("pub component_change_check_ms: u128"));
    assert!(!execution_stats_source.contains("pub propagation_arms: BTreeMap"));
    assert!(source.contains("pub struct SignatureSolverProfile"));
    let solver_profile_source = source
        .split("#[derive(Debug, Clone, Default, PartialEq, Eq)]\npub struct SignatureSolverProfile")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureSolverProfile").next())
        .expect("signature solver profile should remain discoverable");
    assert!(!solver_profile_source.contains("pub collect_constraints_ms: u128"));
    assert!(!solver_profile_source.contains("pub dependency_graph_ms: u128"));
    assert!(!solver_profile_source.contains("pub scc_analysis_ms: u128"));
    assert!(!solver_profile_source.contains("pub pass_plan_ms: u128"));
    assert!(!solver_profile_source.contains("pub fixpoint_ms: u128"));
    assert!(!solver_profile_source.contains("pub plan_build_ms: u128"));
    assert!(!solver_profile_source.contains("pub propagation_arms: BTreeMap"));
    assert!(source.contains("pub fn collect_constraints_ms(&self) -> u128"));
    assert!(source.contains("pub fn propagation_arms(&self) -> &BTreeMap"));
    let domain_stats_source = source
        .split("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]\npub struct SignatureFixpointDomainStats")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureFixpointDomainStats").next())
        .expect("signature fixpoint domain stats should remain discoverable");
    assert!(!domain_stats_source.contains("pub slot_count: usize"));
    assert!(!domain_stats_source.contains("pub unique_constraint_count: usize"));
    assert!(!domain_stats_source.contains("pub iteration_budget: usize"));
    assert!(source.contains("pub struct SignatureReportCount"));
    let report_count_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureReportCount")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureReportCount").next())
        .expect("signature report count should remain discoverable");
    assert!(!report_count_source.contains("pub label: String"));
    assert!(!report_count_source.contains("pub count: usize"));
    assert!(source.contains("pub fn label(&self) -> &str"));
    assert!(source.contains("pub fn count(&self) -> usize"));
    assert!(source.contains("pub fn signature_saturation_report"));
    assert!(source.contains("pub fn signature_constraint_state_conflict_diagnostics"));
    assert!(source.contains("pub struct SignatureConstraintConflictDiagnostic"));
    let conflict_diagnostic_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct SignatureConstraintConflictDiagnostic")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureConstraintConflictDiagnostic").next())
        .expect("signature constraint conflict diagnostic should remain discoverable");
    assert!(!conflict_diagnostic_source.contains("pub axis: SignatureConstraintAxis"));
    assert!(!conflict_diagnostic_source.contains("pub kept: SignatureConstraint"));
    assert!(!conflict_diagnostic_source.contains("pub rejected: SignatureConstraint"));
    assert!(
        !conflict_diagnostic_source.contains("pub kept_provenance: SignatureConstraintProvenance")
    );
    assert!(
        !conflict_diagnostic_source
            .contains("pub rejected_provenance: SignatureConstraintProvenance")
    );
    assert!(!conflict_diagnostic_source.contains("pub message: String"));
    assert!(source.contains("pub fn axis(&self) -> SignatureConstraintAxis"));
    assert!(source.contains("pub fn kept(&self) -> &SignatureConstraint"));
    assert!(source.contains("pub fn rejected(&self) -> &SignatureConstraint"));
    assert!(source.contains("pub fn message(&self) -> &str"));
    assert!(source.contains("pub struct MethodConstraintSet"));
    assert!(source.contains("pub struct ConstructorConstraintSet"));
    let method_constraint_set_source = source
        .split("#[derive(Debug, Clone, Default)]\npub struct MethodConstraintSet")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Default)]\npub struct ConstructorConstraintSet")
                .next()
        })
        .expect("method constraint set source should remain discoverable");
    for field in [
        "pub public_api: bool",
        "pub param_names: Vec<String>",
        "pub param_index_by_name: HashMap<String, usize>",
        "pub param_java_types: Vec<JavaType>",
        "pub params: Vec<Vec<SignatureConstraint>>",
        "pub return_constraints: Vec<SignatureConstraint>",
        "pub locals: Vec<LocalConstraintSet>",
        "pub local_index_by_name: HashMap<String, usize>",
        "pub local_index_by_binding: HashMap<BindingId, usize>",
        "pub receiver: Vec<ReceiverConstraint>",
    ] {
        assert!(
            !method_constraint_set_source.contains(field),
            "MethodConstraintSet should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn param_names(&self) -> &[String]",
        "pub fn params(&self) -> &[Vec<SignatureConstraint>]",
        "pub fn return_constraints(&self) -> &[SignatureConstraint]",
        "pub fn locals(&self) -> &[LocalConstraintSet]",
        "pub fn with_return_constraints(mut self, constraints: Vec<SignatureConstraint>) -> Self",
    ] {
        assert!(source.contains(accessor));
    }
    let constructor_constraint_set_source = source
        .split("#[derive(Debug, Clone, Default)]\npub struct ConstructorConstraintSet")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct LocalConstraintSet")
                .next()
        })
        .expect("constructor constraint set source should remain discoverable");
    for field in [
        "pub public_api: bool",
        "pub param_names: Vec<String>",
        "pub param_index_by_name: HashMap<String, usize>",
        "pub params: Vec<Vec<SignatureConstraint>>",
        "pub locals: Vec<LocalConstraintSet>",
        "pub local_index_by_name: HashMap<String, usize>",
        "pub local_index_by_binding: HashMap<BindingId, usize>",
    ] {
        assert!(
            !constructor_constraint_set_source.contains(field),
            "ConstructorConstraintSet should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn param_names(&self) -> &[String]",
        "pub fn params(&self) -> &[Vec<SignatureConstraint>]",
        "pub fn locals(&self) -> &[LocalConstraintSet]",
    ] {
        assert!(source.contains(accessor));
    }
    assert!(source.contains("pub struct SignatureMemberApiBoundaryPolicy"));
    assert!(source.contains("pub fn signature_member_api_boundary_policy"));
    let member_api_policy_source = source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SignatureMemberApiBoundaryPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureMemberApiBoundaryPolicy").next())
        .expect("signature member api boundary policy should remain discoverable");
    assert!(!member_api_policy_source.contains("pub public_api: bool"));
    assert!(source.contains("pub fn public_api(self) -> bool"));
    assert!(source.contains("public_api: signature_member_api_boundary_policy(&ctor.modifiers)"));
    assert!(source.contains("pub struct LocalConstraintSet"));
    let local_constraint_set_source = source
        .split("#[derive(Debug, Clone, PartialEq, Eq)]\npub struct LocalConstraintSet")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum ReceiverConstraint")
                .next()
        })
        .expect("local constraint set source should remain discoverable");
    for field in [
        "pub binding: BindingId",
        "pub name: String",
        "pub java_ty: JavaType",
        "pub constraints: Vec<SignatureConstraint>",
    ] {
        assert!(
            !local_constraint_set_source.contains(field),
            "LocalConstraintSet should expose accessors, not raw public fields: {field}"
        );
    }
    for accessor in [
        "pub fn binding(&self) -> &BindingId",
        "pub fn name(&self) -> &str",
        "pub fn java_ty(&self) -> &JavaType",
        "pub fn constraints(&self) -> &[SignatureConstraint]",
    ] {
        assert!(source.contains(accessor));
    }
    for (struct_header, impl_header, forbidden_fields, required_accessors) in [
        (
            "pub struct MethodSignaturePlan",
            "impl MethodSignaturePlan",
            &[
                "pub params: Vec<ParamPlan>",
                "pub return_shape: Option<RustSlotShape>",
                "pub locals: Vec<LocalPlan>",
                "pub receiver: ReceiverPlan",
            ][..],
            &[
                "pub fn new(",
                "pub fn params(&self) -> &[ParamPlan]",
                "pub fn return_shape(&self) -> Option<&RustSlotShape>",
                "pub fn locals(&self) -> &[LocalPlan]",
                "pub fn receiver(&self) -> ReceiverPlan",
            ][..],
        ),
        (
            "pub struct ConstructorSignaturePlan",
            "impl ConstructorSignaturePlan",
            &["pub params: Vec<ParamPlan>", "pub locals: Vec<LocalPlan>"][..],
            &[
                "pub fn new(params: Vec<ParamPlan>, locals: Vec<LocalPlan>) -> Self",
                "pub fn params(&self) -> &[ParamPlan]",
                "pub fn locals(&self) -> &[LocalPlan]",
            ][..],
        ),
        (
            "pub struct ParamPlan",
            "impl ParamPlan",
            &[
                "pub name: String",
                "pub java_ty: JavaType",
                "pub is_varargs: bool",
                "pub shape: RustSlotShape",
                "pub binding_mutable: bool",
            ][..],
            &[
                "pub fn name(&self) -> &str",
                "pub fn java_ty(&self) -> &JavaType",
                "pub fn is_varargs(&self) -> bool",
                "pub fn shape(&self) -> &RustSlotShape",
                "pub fn binding_mutable(&self) -> bool",
            ][..],
        ),
        (
            "pub struct LocalPlan",
            "impl LocalPlan",
            &[
                "pub binding: BindingId",
                "pub name: String",
                "pub java_ty: JavaType",
                "pub shape: RustSlotShape",
                "pub binding_mutable: bool",
            ][..],
            &[
                "pub fn binding(&self) -> &BindingId",
                "pub fn name(&self) -> &str",
                "pub fn java_ty(&self) -> &JavaType",
                "pub fn shape(&self) -> &RustSlotShape",
                "pub fn binding_mutable(&self) -> bool",
            ][..],
        ),
    ] {
        let struct_source = source
            .split(struct_header)
            .nth(1)
            .and_then(|tail| tail.split(impl_header).next())
            .expect("signature plan struct source should remain discoverable");
        for field in forbidden_fields {
            assert!(
                !struct_source.contains(field),
                "{struct_header} should expose accessors, not raw public fields: {field}"
            );
        }
        for accessor in required_accessors {
            assert!(
                source.contains(accessor),
                "{struct_header} should keep accessor API: {accessor}"
            );
        }
    }
    assert!(source.contains("pub enum ReceiverConstraint"));
    assert!(source.contains("pub enum SignatureConstraintPropagationKind"));
    assert!(source.contains("pub struct SignatureConstraintPropagationPolicy"));
    let propagation_policy_source = source
        .split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub struct SignatureConstraintPropagationPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl SignatureConstraintPropagationPolicy").next())
        .expect("signature constraint propagation policy should remain discoverable");
    assert!(!propagation_policy_source.contains("pub kind: SignatureConstraintPropagationKind"));
    assert!(source.contains("pub fn kind(self) -> SignatureConstraintPropagationKind"));
    assert!(source.contains("signature_shape_constraint_propagation_policy"));
    assert!(source.contains("signature_slot_constraint_propagation_policy"));
    assert!(source.contains("collect_signature_constraints"));
    assert!(source.contains("solve_signature_constraints_to_fixpoint"));
    assert!(source.contains("solve_signature_constraints_to_fixpoint_result"));
    assert!(source.contains("signature_fixpoint_saturation_policy"));
    assert!(source.contains("signature_fixpoint_iteration_budget"));
    assert!(source.contains("FiniteSaturationPolicy"));
    assert!(source.contains("SaturationStep"));
    assert!(source.contains("signature_capability_policy"));
    assert!(source.contains("capability_diagnostics_for_signature_constraints"));
    assert!(source.contains("signature_clone_recovery_requires_audit"));
    assert!(!source.contains("capability_baseline_constraints"));
    assert!(source.contains("propagate_call_site_constraints"));
    assert!(source.contains("propagate_constructor_call_site_constraints"));
    assert!(source.contains("propagate_call_argument_constraints_to_callees"));
    assert!(source.contains("propagate_constructor_call_argument_constraints_to_callees"));
    assert!(source.contains("propagate_call_result_constraints_to_bindings"));
    assert!(source.contains("propagate_constructor_call_result_constraints_to_bindings"));
    assert!(source.contains("propagate_call_result_constraints_to_fields"));
    assert!(source.contains("propagate_constructor_call_result_constraints_to_fields"));
    assert!(source.contains("propagate_method_local_alias_constraints"));
    assert!(source.contains("propagate_method_local_alias_source_constraints"));
    assert!(source.contains("propagate_constructor_local_alias_constraints"));
    assert!(source.contains("propagate_constructor_local_alias_source_constraints"));
    assert!(source.contains("propagate_method_field_storage_constraints"));
    assert!(source.contains("propagate_constructor_field_storage_constraints"));
    assert!(source.contains("propagate_method_return_binding_constraints"));
    assert!(source.contains("propagate_method_binding_return_constraints"));
    assert!(source.contains("add_method_param_constraint"));
    assert!(source.contains("add_method_return_constraint"));
    assert!(source.contains("method_param_shape_constraints"));
    assert!(source.contains("constructor_param_shape_constraints"));
    assert!(source.contains("method_return_shape_constraints"));
    assert!(source.contains("legacy_name_set_local_mutability_constraints"));
    assert!(source.contains("if binding_id.is_none()"));
    assert!(source.contains("field_shape_constraints"));
    assert!(source.contains("field_shape_constraints_with_facts"));
    assert!(source.contains("signature_member_api_boundary_policy"));
    assert!(!source.contains("fn member_is_api_boundary"));
    assert!(source.contains("receiver_constraints_with_usage"));
    assert!(!source.contains("pub fn receiver_constraints(method"));
    assert!(!source.contains("pub fn solve_signature_shape("));
    assert!(source.contains("solve_signature_shape_with_facts"));
    assert!(source.contains("apply_signature_constraints"));
    assert_eq!(source.matches("param_shape_for_java_type").count(), 2);
    assert_eq!(source.matches("return_shape_for_java_type").count(), 2);
    assert_eq!(source.matches("storage_shape_for_java_type").count(), 3);

    let slot_policy_source = include_str!("../src/slot_policy.rs");
    assert!(!slot_policy_source.contains("| CallArgumentBoundary::InternalSameType"));

    let lib_source = include_str!("../src/lib.rs");
    let fact_saturation_source = include_str!("../src/fact_saturation.rs");
    assert!(fact_saturation_source.contains("pub struct FiniteFactDomain"));
    assert!(fact_saturation_source.contains("pub struct FiniteSaturationPolicy"));
    let finite_domain_source = fact_saturation_source
        .split("pub struct FiniteFactDomain")
        .nth(1)
        .and_then(|tail| tail.split("pub struct FiniteSaturationPolicy").next())
        .expect("FiniteFactDomain should remain discoverable");
    assert!(!finite_domain_source.contains("pub declared_slots: usize"));
    assert!(!finite_domain_source.contains("pub value_lattice_height: usize"));
    assert!(fact_saturation_source.contains("pub fn declared_slots(self) -> usize"));
    assert!(fact_saturation_source.contains("pub fn value_lattice_height(self) -> usize"));
    let finite_policy_source = fact_saturation_source
        .split("pub struct FiniteSaturationPolicy")
        .nth(1)
        .and_then(|tail| tail.split("impl FiniteFactDomain").next())
        .expect("FiniteSaturationPolicy should remain discoverable");
    assert!(!finite_policy_source.contains("pub domain: FiniteFactDomain"));
    assert!(fact_saturation_source.contains("pub fn domain(self) -> FiniteFactDomain"));
    let worklist_policy_source = fact_saturation_source
        .split("pub(crate) struct WorklistSaturationPolicy")
        .nth(1)
        .and_then(|tail| tail.split("pub(crate) struct WorklistInserter").next())
        .expect("WorklistSaturationPolicy should remain discoverable");
    assert!(!worklist_policy_source.contains("pub max_processed_items: usize"));
    assert!(fact_saturation_source.contains("pub(crate) fn max_processed_items(self) -> usize"));
    assert!(fact_saturation_source.contains("fn finite_fact_iteration_budget"));
    assert!(!fact_saturation_source.contains("pub fn finite_fact_iteration_budget"));
    assert!(fact_saturation_source.contains("pub(crate) enum SaturationStep"));
    assert!(fact_saturation_source.contains("pub(crate) struct FiniteSaturationResult"));
    let finite_result_source = fact_saturation_source
        .split("pub(crate) struct FiniteSaturationResult")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]")
                .next()
        })
        .expect("FiniteSaturationResult should remain discoverable");
    for field in [
        "pub state: T",
        "pub iterations: usize",
        "pub diagnostics: Vec<D>",
    ] {
        assert!(
            !finite_result_source.contains(field),
            "FiniteSaturationResult should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(fact_saturation_source.contains("pub(crate) fn iterations(&self) -> usize"));
    assert!(
        fact_saturation_source.contains("pub(crate) fn into_parts(self) -> (T, usize, Vec<D>)")
    );
    let worklist_stats_source = fact_saturation_source
        .split("pub struct WorklistStats")
        .nth(1)
        .and_then(|tail| tail.split("#[derive(Debug, Clone, PartialEq, Eq)]").next())
        .expect("WorklistStats should remain discoverable");
    for field in [
        "pub processed_items: usize",
        "pub inserted_facts: usize",
        "pub max_queue_len: usize",
    ] {
        assert!(
            !worklist_stats_source.contains(field),
            "WorklistStats should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(fact_saturation_source.contains("pub fn processed_items(self) -> usize"));
    assert!(fact_saturation_source.contains("pub fn inserted_facts(self) -> usize"));
    assert!(fact_saturation_source.contains("pub fn max_queue_len(self) -> usize"));
    let worklist_result_source = fact_saturation_source
        .split("pub(crate) struct WorklistSaturationResult")
        .nth(1)
        .and_then(|tail| {
            tail.split("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
                .next()
        })
        .expect("WorklistSaturationResult should remain discoverable");
    for field in [
        "pub state: T",
        "pub stats: WorklistStats",
        "pub diagnostics: Vec<D>",
    ] {
        assert!(
            !worklist_result_source.contains(field),
            "WorklistSaturationResult should expose accessors, not raw public fields: {field}"
        );
    }
    assert!(
        fact_saturation_source
            .contains("pub(crate) fn into_parts(self) -> (T, WorklistStats, Vec<D>)")
    );
    assert!(fact_saturation_source.contains("pub(crate) fn saturate_finite"));
    for plain_public in [
        "pub enum SaturationStep",
        "pub struct FiniteSaturationResult",
        "pub struct WorklistSaturationResult",
        "pub struct WorklistSaturationPolicy",
        "pub struct WorklistInserter",
        "pub fn saturate_worklist",
        "pub fn saturate_worklist_with_seen",
        "pub fn saturate_finite",
    ] {
        assert!(
            !fact_saturation_source.contains(plain_public),
            "fact_saturation is a private module; {plain_public} should be crate-local"
        );
    }
    assert!(lib_source.contains("mod fact_saturation;"));
    assert!(!lib_source.contains("pub mod fact_saturation;"));
}

#[test]
fn call_lowering_renders_constructor_owner_types_with_facts() {
    let source = include_str!("../src/call_lowering.rs");
    let coercion_source = include_str!("../src/coercion.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(source.contains("pub fn apply_new_target_with_facts"));
    assert!(source.contains("pub fn apply_new_target_with_facts_and_representations"));
    assert!(source.contains("constructor_owner_associated_path_projection("));
    assert!(source.contains("rust_type_associated_path_projection("));
    let constructor_owner_path_source = source
        .split("fn constructor_owner_associated_path_projection")
        .nth(1)
        .and_then(|tail| tail.split("fn boxed_primitive_constructor_owner").next())
        .expect("constructor owner associated path helper should remain discoverable");
    assert!(
        constructor_owner_path_source.contains("JavaType::Named")
            && constructor_owner_path_source.contains("rust_name_for_java_named_type(name)")
            && constructor_owner_path_source
                .contains("owner_associated_path_projection(&owner, member)")
            && constructor_owner_path_source
                .contains("rust_type_for_slot_with_facts(owner_shape, facts)")
            && constructor_owner_path_source.contains("rust_type_associated_path_projection(")
            && !constructor_owner_path_source.contains("RustExpr::path(["),
        "constructor owner associated paths should render named owners without storage decoration, while non-named owners still derive a structured RustType with facts"
    );
    assert!(!source.contains("fn constructor_owner_type_associated_path_expr"));
    assert!(!constructor_owner_path_source.contains("emit_type"));
    assert!(coercion_source.contains("pub(crate) fn rust_type_associated_path_projection"));
    assert!(coercion_source.contains("RustExpr::GenericPath"));
    assert!(!source.contains("RustExpr::Path(vec![crate::emit::emit_type(&other)"));
    assert!(!source.contains("emit_type(&other), member.to_string()"));
    assert!(source.contains("boxed_primitive_type(name.rsplit('.').next().unwrap_or(name))"));
    assert!(!source.contains("let owner_type = constructor_owner_path_type"));
    assert!(!source.contains("owner_type == \"String\""));
    assert!(!source.contains("constructor_owner_path_type(&target.owner_shape)"));
    assert!(!source.contains("rust_type_for_slot_with_facts(owner_shape, &FactSet::default())"));
    assert!(!source.contains("emit_type(&rust_type_for_slot_with_facts(&target.owner_shape"));
    assert!(!source.contains("emit_type(&rust_type_for_slot_with_facts("));
    assert!(!source.contains("rust_type_for_slot(&target.owner_shape)"));
    assert!(lower_source.contains("apply_new_target_with_facts_and_representations("));
    assert!(lower_source.contains("ctx.representations"));
}

#[test]
fn external_api_support_passes_share_missing_return_surface_policy() {
    let source = include_str!("../src/stdlib.rs");
    let support_impl = source
        .split("impl ExternalApiFacadeSupport {")
        .nth(1)
        .expect("external facade support impl should exist");
    assert!(
        support_impl.contains("fn external_api_fact_return_surface"),
        "{}",
        support_impl
    );
    for function_name in [
        "fn add_used_external_api_facts",
        "fn add_builtin_used_external_api_facts",
        "fn add_static_external_api_facts_for_surface_types",
    ] {
        let body = support_impl
            .split(function_name)
            .nth(1)
            .and_then(|rest| rest.split("\n    fn ").next())
            .unwrap_or_else(|| panic!("{function_name} should exist"));
        assert!(
            body.contains("external_api_fact_return_surface"),
            "{function_name} should route missing return facts through the shared policy:\n{body}"
        );
        assert!(
            !body.contains(".unwrap_or_else(|| \"()\".to_string())"),
            "{function_name} must not locally treat all missing return facts as void:\n{body}"
        );
    }
}
