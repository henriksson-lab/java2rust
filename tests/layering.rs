#[test]
fn plan_layers_keep_rust_rendering_out_of_source_analysis() {
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
            include_str!("../src/signature.rs"),
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
    assert!(!source.contains("pub fn java_member_access_policy("));
    assert!(source.contains("pub fn java_member_access_policy_with_facts"));
    assert!(!source.contains("pub fn plan_member_access("));
    assert!(source.contains("pub fn plan_member_access_with_facts"));
    assert!(source.contains("let policy = java_member_access_policy_with_facts"));
    assert!(source.contains("project_java_value_shape"));
    assert!(!source.contains("builtin_external_library_packs"));
    assert!(!source.contains("PrimitiveType"));
    assert!(!source.contains("JavaType::Primitive"));
    assert!(!source.contains("RustSlotShape::conservative"));
    assert!(!source.contains("is_length_receiver"));
}

#[test]
fn string_concat_layers_keep_semantics_and_rendering_separate() {
    let semantic_source = include_str!("../src/string_concat.rs");
    let plan_source = include_str!("../src/string_concat_plan.rs");
    let render_source = include_str!("../src/string_concat_render.rs");

    assert!(!semantic_source.contains("rust_ir"));
    assert!(!semantic_source.contains("emit::"));
    assert!(!semantic_source.contains("format!("));
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
    assert!(stream_source.contains("use crate::conversion_plan::LoweredType"));
    assert!(stream_source.contains("LoweredType::collected_sequence_from_element_shape"));
}

#[test]
fn aliasing_uses_binding_fact_view_instead_of_legacy_name_map() {
    let source = include_str!("../src/aliasing.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production aliasing source should be present");

    assert!(
        !source.contains(".bindings.get("),
        "alias analysis should consume all_binding_facts() or binding ids instead of the legacy name map"
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
    assert!(source.contains("pub fn plan_simple_return_switch"));
    assert!(source.contains("pub fn plan_simple_return_switch_case"));
}

#[test]
fn shape_axis_free_lowering_layers_do_not_choose_shape_axes() {
    let lowering_sources = [
        include_str!("../src/access_lowering.rs"),
        include_str!("../src/call_lowering.rs"),
        include_str!("../src/conversion_lowering.rs"),
        include_str!("../src/exception_lowering.rs"),
        include_str!("../src/literal_lowering.rs"),
        include_str!("../src/loop_lowering.rs"),
        include_str!("../src/string_concat_render.rs"),
        include_str!("../src/switch_lowering.rs"),
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
    let lowering_source = include_str!("../src/conversion_lowering.rs");

    assert!(!source.contains("rust_ir"));
    assert!(!source.contains("RustExpr"));
    assert!(!source.contains("apply_expr_to_shape"));
    assert!(!source.contains("apply_instanceof_check"));
    assert!(source.contains("pub struct ShapeCoercionPlan"));
    assert!(source.contains("pub struct SyntaxCoercionPlan"));
    assert!(source.contains("pub enum InstanceOfCheck"));
    assert!(source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
    assert!(source.contains("pub coercion: ShapeCoercionPlan"));
    assert!(source.contains("pub check: InstanceOfCheck"));
    assert!(source.contains("pub enum LoweredType"));
    assert!(source.contains("pub enum SlotConversionPlan"));
    assert!(source.contains("pub struct SlotExprConversionPlan"));
    assert!(source.contains("pub struct LoweredSlotExprConversionPlan"));
    assert!(source.contains("pub conversion: SlotConversionPlan"));
    assert!(source.contains("pub fn actual_shape_for_slot_expr"));
    assert!(source.contains("pub fn plan_slot_conversion"));
    assert!(source.contains("pub fn plan_lowered_slot_conversion"));
    assert!(source.contains("shape_coercion_plan_with_facts"));
    assert!(source.contains("plan_instanceof_check"));
    assert!(source.contains("slot_conversion_plan_with_facts"));
    assert!(source.contains("lowered_slot_conversion_plan_with_facts"));
    assert!(lowering_source.contains("apply_shape_coercion_plan(expr, &plan.coercion)"));
    assert!(lowering_source.contains("instanceof_check_projection(expr, &plan.check)"));
    assert!(lowering_source.contains("pub fn apply_slot_expr_conversion"));
    assert!(lowering_source.contains("pub fn apply_lowered_slot_expr_conversion"));
    assert!(lowering_source.contains("pub fn apply_lowered_slot_conversion_to_expr"));
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

    assert!(source.contains("pub fn method_return_type_with_facts"));
    assert!(!source.contains("pub fn method_return_type("));
    assert!(!source.contains("rust_type_for_slot(&shape)"));
    assert!(!source.contains("fn raw_rust_expr_mentions_identifier"));
    assert!(!source.contains("fn valid_raw_move_identifier"));
    assert!(source.contains("recover_parseable_raw_operand_projection"));
}

#[test]
fn lower_orchestrator_delegates_static_storage_defaults_to_item_assembly() {
    let source = include_str!("../src/lower.rs");
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

    assert!(propagation.contains("let Some(resolved) = &call_site.resolved"));
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
fn idless_local_binding_compatibility_surface_stays_explicit() {
    let lower_source = include_str!("../src/lower.rs");
    let expr_shape_source = include_str!("../src/expr_shape.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production expr_shape source should be present");
    let signature_source = include_str!("../src/signature.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production signature source should be present");

    assert_eq!(
        lower_source
            .matches("next_local_plan_for_declared_type")
            .count(),
        1,
        "lowering should have exactly one id-less local-plan compatibility call"
    );
    assert!(lower_source.contains("fn idless_compatibility_local_plan_for_declaration"));
    assert!(lower_source.contains(
        "idless_compatibility_local_plan_for_declaration(name, ty, initializer, env, ctx)"
    ));
    assert!(expr_shape_source.contains("pub fn next_local_plan_for_declared_type"));
    assert_eq!(
        expr_shape_source
            .matches("next_local_plan_for_declared_type")
            .count(),
        1,
        "expr_shape should expose the compatibility primitive without adding production callers"
    );
    assert!(
        !expr_shape_source.contains("local_plan_occurrences")
            && !expr_shape_source.contains("consume_local_plan_occurrence"),
        "id-less local-plan compatibility must fail closed by unique exact match, not advance a shared occurrence cursor"
    );

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
    assert!(
        !signature_source
            .contains("let duplicate_names = collect_duplicate_declared_local_names(block);")
    );
}

#[test]
fn signature_uses_usage_binding_policy_for_legacy_name_fallback() {
    let signature_source = include_str!("../src/signature.rs");
    let usage_source = include_str!("../src/usage.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production usage source should be present");

    assert!(
        !signature_source.contains(".bindings.get("),
        "signature should ask MethodUsageFacts for binding lookup instead of reading the legacy name map directly"
    );
    assert!(
        signature_source.contains("binding_for_declaration_with_name_fallback"),
        "signature's id-less fallback should route through the centralized MethodUsageFacts policy"
    );
    assert!(
        usage_source.contains("pub fn binding_for_declaration_with_name_fallback"),
        "MethodUsageFacts should own the explicit legacy-name fallback policy"
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
    let slot_policy_source = include_str!("../src/slot_policy.rs");

    assert!(usage_source.contains("pub struct CallSiteUse"));
    assert!(usage_source.contains("pub call_sites: Vec<CallSiteUse>"));
    assert!(usage_source.contains("pub struct MethodApiBoundaryPolicy"));
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

    assert!(semantic_source.contains("SemanticDiagnostic"));
    assert!(semantic_source.contains("semantic_external_api_effect_budget_exhausted"));
    assert!(lib_source.contains("for diagnostic in semantic.diagnostics"));
    assert!(lib_source.contains(".warning(diagnostic.code, diagnostic.message)"));
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

    assert!(source.contains("pub fn local_let_stmt_with_facts"));
    assert!(source.contains("rust_type_for_slot_with_facts"));
    assert!(!source.contains("rust_type_for_slot(&local.shape)"));
    assert!(lower_source.contains("local_let_stmt_with_facts"));
}

#[test]
fn stdlib_does_not_grow_non_jdk_domain_package_knowledge() {
    let stdlib_source = include_str!("../src/stdlib.rs");
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
    assert!(conversion_source.contains("pub fn hierarchy_instanceof_policy"));
    assert!(facts_source.contains("pub struct HierarchySubtypePolicy"));
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
    assert!(coercion_source.contains("pub fn erased_object_box_projection"));
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
    assert!(source.contains("pub struct SignatureFlagMerge"));
    assert!(source.contains("pub fn merge_signature_flag_requirement_policy"));
    assert!(source.contains("pub enum SignatureBoundaryFallback"));
    assert!(source.contains("pub struct SignatureBoundaryRepresentationFactPolicy"));
    assert!(source.contains("pub struct SignatureBoundaryRepresentationPolicy"));
    assert!(source.contains("pub struct SignatureNativeBoundaryPolicy"));
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
    assert!(source.contains("pub fn binding_boundary_policy"));
    assert!(!source.contains("fn binding_boundary_kind"));
    assert!(source.contains("pub struct MethodParamBoundaryPolicy"));
    assert!(source.contains("pub fn method_param_boundary_policy"));
    assert!(!source.contains("fn method_param_boundary_kind"));
    assert!(source.contains("pub struct ConstructorParamBoundaryPolicy"));
    assert!(source.contains("pub fn constructor_param_boundary_policy"));
    assert!(!source.contains("fn constructor_param_boundary_kind"));
    assert!(source.contains("pub struct FieldBoundaryPolicy"));
    assert!(source.contains("pub fn field_boundary_policy"));
    assert!(!source.contains("fn field_boundary_kind"));
    assert!(source.contains("pub struct MethodReturnBoundaryPolicy"));
    assert!(source.contains("pub fn method_return_boundary_policy"));
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
    assert!(shape_projection_source.contains("pub fn apply_borrowed_string_content_projection"));
    assert!(shape_projection_source.contains("pub fn borrowed_sequence_element_shape"));
    assert!(shape_projection_source.contains("pub fn apply_borrowed_sequence_element_projection"));
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

    assert!(source.contains("pub struct SignatureRequirementMerge"));
    assert!(!source.contains("pub struct SignatureOwnershipRequirementPriorityPolicy"));
    assert!(!source.contains("pub fn signature_ownership_requirement_priority_policy"));
    assert!(!source.contains("pub struct SignatureViewRequirementPriorityPolicy"));
    assert!(!source.contains("pub fn signature_view_requirement_priority_policy"));
    assert!(!source.contains("pub struct SignatureDispatchRequirementPriorityPolicy"));
    assert!(!source.contains("pub fn signature_dispatch_requirement_priority_policy"));
    assert!(source.contains("pub struct SignatureFlagMerge"));
    assert!(source.contains("pub fn merge_signature_flag_requirement_policy"));
    assert!(source.contains("pub fn merge_ownership_requirement_policy"));
    assert!(source.contains("pub fn merge_view_requirement_policy"));
    assert!(source.contains("pub fn merge_dispatch_requirement_policy"));
    assert!(source.contains("join_ranked_lattice"));
    assert!(source.contains("ownership_lattice_rank"));
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
    assert!(source.contains("pub fn signature_slot_baseline_policy"));
    assert!(source.contains("pub fn signature_slot_baseline_policy_with_effective_facts"));
    assert!(!source.contains("fn signature_slot_baseline_constraints("));
}

#[test]
fn stream_shape_stream_source_semantics_stay_effective_fact_based() {
    let stream_shape_source = include_str!("../src/stream_shape.rs");
    let effective_facts_source = include_str!("../src/effective_facts.rs");

    assert!(!stream_shape_source.contains("sequence_element_type("));
    assert!(stream_shape_source.contains("use crate::effective_facts::EffectiveFacts"));
    assert!(stream_shape_source.contains("pub struct StreamSourceCapabilityPolicy"));
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

    assert!(effective_facts_source.contains("TypeCapabilities::conservative"));
    assert!(effective_facts_source.contains("pub struct EffectiveFact<T>"));
    assert!(effective_facts_source.contains("pub provenance: FactProvenance"));
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
}

#[test]
fn slot_render_policy_symbols_stay_effective_fact_aware() {
    let source = include_str!("../src/slot_render.rs");

    assert!(source.contains("pub enum NativeJavaRustTypePolicy"));
    assert!(!source.contains("pub fn native_java_rust_type_policy("));
    assert!(source.contains("pub fn native_java_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub struct NativeNamedJavaRustTypePolicy"));
    assert!(source.contains("pub fn native_named_java_rust_type_policy"));
    assert!(!source.contains("pub fn render_native_java_rust_type_policy("));
    assert!(source.contains("pub fn render_native_java_rust_type_policy_with_effective_facts"));
    assert!(!source.contains("\nfn native_named_java_rust_type_policy("));
    assert!(source.contains("pub enum SlotValueRustTypePolicy"));
    assert!(!source.contains("pub fn slot_value_rust_type_policy("));
    assert!(source.contains("pub fn slot_value_rust_type_policy_with_effective_facts"));
    assert!(!source.contains("pub fn render_slot_value_rust_type_policy("));
    assert!(source.contains("pub fn render_slot_value_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub enum SlotRustTypePolicy"));
    assert!(!source.contains("pub fn slot_rust_type_policy("));
    assert!(source.contains("pub fn slot_rust_type_policy_with_effective_facts"));
    assert!(source.contains("pub fn render_slot_rust_type_policy"));
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
    assert!(!source.contains("pub fn slot_content_capability_policy("));
    assert!(source.contains("pub fn slot_content_capability_policy_with_effective_facts"));
    assert!(source.contains("pub struct SlotNoDefaultMessagePolicy"));
    assert!(source.contains("pub fn slot_no_default_message_policy"));
    assert!(!source.contains("pub fn slot_default_policy("));
    assert!(source.contains("pub fn slot_default_policy_with_effective_facts"));
    assert!(!source.contains("pub fn slot_default_value_policy("));
    assert!(source.contains("pub fn slot_default_value_policy_with_effective_facts"));
    assert!(source.contains("pub fn slot_default_policy_expr"));
    assert!(source.contains("pub fn slot_default_value_policy_expr"));
    assert!(!source.contains("pub fn render_slot_default_policy"));
    assert!(!source.contains("pub fn render_slot_default_value_policy"));
}

#[test]
fn call_target_lowering_uses_shared_projection_helpers() {
    let call_lowering_source = include_str!("../src/call_lowering.rs");
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
    assert!(call_lowering_source.contains("RustExpr::MarkedBlock"));
    assert!(!call_lowering_source.contains("RustExpr::Raw(format!(\n        \"{{ {} {} }}\""));
}

#[test]
fn exception_lowering_keeps_exception_paths_structured() {
    let source = include_str!("../src/exception_lowering.rs");
    let coercion_source = include_str!("../src/coercion.rs");

    assert!(!source.contains("JavaType"));
    assert!(!source.contains("exception_class_name"));
    assert!(!source.contains("JavaType::Named"));
    assert!(!source.contains("JavaType::Unknown"));
    let throw_policy_source = source
        .split("pub fn throw_stmt_with_result_policy")
        .nth(1)
        .and_then(|tail| tail.split("pub fn try_catch_stmt").next())
        .expect("throw result policy lowering should remain discoverable");
    assert!(throw_policy_source.contains("RustExpr::MacroCall"));
    assert!(
        !throw_policy_source.contains("RustStmt::Raw(format!")
            && !throw_policy_source.contains("emit_expr(&exception)"),
        "non-result throw lowering should keep the exception expression structured"
    );
    assert!(
        source.contains("parenthesize_raw_postfix_receiver(message)")
            && source.contains("parenthesize_raw_postfix_receiver(wrapper)")
            && !source.contains("fn raw_needs_method_receiver_parentheses")
            && !source.contains("RustExpr::Raw(format!(\"({trimmed})\""),
        "exception method receivers should delegate Raw recovery to the central coercion helper"
    );
    let postfix_receiver_source = coercion_source
        .split("pub(crate) fn parenthesize_raw_postfix_receiver")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub fn recover_parseable_raw_operand_projection")
                .next()
        })
        .expect("central raw postfix receiver helper should remain discoverable");
    assert!(
        postfix_receiver_source
            .contains("raw_receiver_projection(expr, raw_needs_postfix_receiver_parentheses)"),
        "postfix receiver parenthesizing should share the central raw receiver projection"
    );
    let raw_receiver_projection_source = coercion_source
        .split("pub(crate) fn raw_receiver_projection")
        .nth(1)
        .and_then(|tail| {
            tail.split("pub(crate) fn parenthesize_raw_postfix_receiver")
                .next()
        })
        .expect("central raw receiver projection should remain discoverable");
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
