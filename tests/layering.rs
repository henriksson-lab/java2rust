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
fn legacy_usage_binding_name_map_is_owned_by_usage_module() {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let forbidden = [
        ".bindings.get(",
        ".bindings.get_mut(",
        ".bindings.insert(",
        ".bindings.remove(",
        ".bindings_by_id.get(",
        ".bindings_by_id.get_mut(",
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
        for needle in forbidden {
            assert!(
                !production_source.contains(needle),
                "{} must use MethodUsageFacts accessors or all_binding_facts() instead of touching {needle}",
                path.display()
            );
        }
    }
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
            .matches("idless_compatibility_local_plan_for_declared_type")
            .count(),
        1,
        "lowering should have exactly one id-less local-plan compatibility call"
    );
    assert!(lower_source.contains("fn idless_compatibility_local_plan_for_declaration"));
    assert!(lower_source.contains(
        "idless_compatibility_local_plan_for_declaration(name, ty, initializer, env, ctx)"
    ));
    assert!(
        expr_shape_source
            .contains("pub(crate) fn idless_compatibility_local_plan_for_declared_type")
    );
    assert!(expr_shape_source.contains("pub(crate) fn fallback_local_shape_from_declared_type"));
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
        .split("#[cfg(test)]")
        .next()
        .expect("production usage source should be present");
    let slot_policy_source = include_str!("../src/slot_policy.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production slot_policy source should be present");

    assert!(
        !signature_source.contains(".bindings.get("),
        "signature should ask MethodUsageFacts for binding lookup instead of reading the legacy name map directly"
    );
    assert!(
        signature_source.contains("binding_for_declaration_with_name_fallback"),
        "signature's id-less fallback should route through the centralized MethodUsageFacts policy"
    );
    assert!(
        !signature_source.contains("binding_for_declaration(param.binding.as_ref(), &param.name)"),
        "signature must make parameter name-fallback compatibility explicit at each call site"
    );
    assert!(
        !slot_policy_source
            .contains("binding_for_declaration(param.binding.as_ref(), &param.name)"),
        "slot policy must make parameter name-fallback compatibility explicit at each call site"
    );
    assert!(
        usage_source.contains("pub(crate) fn binding_for_declaration_with_name_fallback"),
        "MethodUsageFacts should own the explicit legacy-name fallback policy without exporting it as public API"
    );
    assert!(
        usage_source.contains("pub(crate) fn snapshot_visible_binding_for_scope")
            && usage_source.contains("pub(crate) fn hide_visible_binding_for_scope")
            && usage_source.contains("pub(crate) fn restore_visible_binding_for_scope")
            && usage_source.contains("pub(crate) fn record_visible_binding_use"),
        "MethodUsageFacts scope/name-map maintenance helpers should stay crate-local"
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
        slot_policy_source.contains("pub(crate) fn legacy_idless_method_assigns_to_binding"),
        "the remaining name-only assignment scan should advertise its id-less compatibility scope"
    );
    assert_eq!(
        slot_policy_source
            .matches("legacy_idless_method_assigns_to_binding")
            .count(),
        2,
        "slot-policy legacy assignment scan should have one id-less caller and one helper definition"
    );
    assert!(
        slot_policy_source
            .contains("if param.binding.is_some() {\n        if !binding\n            .uses")
            && slot_policy_source.contains(
                "} else {\n        if !method_exactly_returns_param(method, &param.source_name)"
            )
            && slot_policy_source
                .contains("legacy_idless_method_assigns_to_binding(method, &param.source_name)"),
        "slot-policy legacy assignment scan must stay behind the id-less parameter branch"
    );
    assert_eq!(
        signature_source
            .matches("legacy_idless_method_assigns_to_binding")
            .count(),
        2,
        "signature's legacy assignment scan use should stay explicit and localized"
    );
    assert!(
        signature_source.contains(
            "let compatibility_name_returned = param.binding.is_none()\n            && exactly_returned_param_names.contains(param.source_name.as_str())\n            && !legacy_idless_method_assigns_to_binding(method, &param.source_name);"
        ),
        "signature's legacy assignment scan must stay behind the id-less parameter predicate"
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
    let java_signature_source = include_str!("../src/java_signature.rs");
    let main_source = include_str!("../src/main.rs");
    let slot_render_source = include_str!("../src/slot_render.rs");
    assert!(java_signature_source.contains("pub(crate) struct JavaOwnerKey"));
    assert!(java_signature_source.contains("pub fn source_method_for_type"));
    assert!(java_signature_source.contains("pub(crate) fn source_method_for_owner"));
    assert!(java_signature_source.contains("pub(crate) fn resolved_source_method"));
    assert!(
        java_signature_source.contains("pub(crate) fn source_method(method: &MethodDecl) -> Self")
    );
    assert!(!java_signature_source.contains("pub struct JavaOwnerKey"));
    assert!(!java_signature_source.contains("pub fn source_method(method: &MethodDecl) -> Self"));
    assert!(
        slot_render_source.contains("pub(crate) fn rust_name_for_java_named_type"),
        "slot_render should not publish a second public Java type-name projection API"
    );
    assert!(
        main_source.contains("new_java2rust::java_naming::rust_name_for_java_named_type")
            && !main_source.contains("new_java2rust::slot_render::rust_name_for_java_named_type"),
        "the CLI should use the canonical java_naming projection instead of slot_render's compatibility wrapper"
    );

    let usage_source = include_str!("../src/usage.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production usage source should be present");
    assert!(usage_source.contains("pub(crate) fn source_method_usage_for_owner"));

    for (module, source, required) in [
        (
            "aliasing",
            include_str!("../src/aliasing.rs"),
            "JavaCallableKey::resolved_source_method",
        ),
        (
            "storage_strategy",
            include_str!("../src/storage_strategy.rs"),
            "source_method_usage_for_owner",
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
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("production source should be present");
        assert!(
            production.contains(required),
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

    let aliasing_source = include_str!("../src/aliasing.rs")
        .split("#[cfg(test)]")
        .next()
        .expect("production aliasing source should be present");
    assert!(
        !aliasing_source.contains(".methods\n            .get(&UsageMethodKey"),
        "aliasing has ResolvedCall identity and should not re-key receiver mutation by plain strings"
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
    assert!(source.contains("pub struct NativeNamedJavaRustTypePolicy"));
    assert!(source.contains("pub fn native_named_java_rust_type_policy"));
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
    assert!(!source.contains("pub fn slot_content_capability_policy("));
    assert!(source.contains("pub fn slot_content_capability_policy_with_effective_facts"));
    assert!(source.contains("pub struct SlotNoDefaultMessagePolicy"));
    assert!(source.contains("pub fn slot_no_default_message_policy"));
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
        .and_then(|tail| tail.split("pub fn try_catch_stmt").next())
        .expect("throw result policy lowering should remain discoverable");
    assert!(production_source.contains("pub(crate) fn throw_stmt_with_result_policy"));
    assert!(!production_source.contains("pub fn throw_stmt_with_result_policy"));
    assert!(throw_policy_source.contains("RustExpr::MacroCall"));
    assert!(
        !throw_policy_source.contains("RustStmt::Raw(format!")
            && !throw_policy_source.contains("emit_expr(&exception)"),
        "non-result throw lowering should keep the exception expression structured"
    );
    assert!(
        production_source.contains("parenthesize_raw_postfix_receiver(message)")
            && production_source.contains("parenthesize_raw_postfix_receiver(wrapper)")
            && !production_source.contains("fn raw_needs_method_receiver_parentheses")
            && !production_source.contains("RustExpr::Raw(format!(\"({trimmed})\""),
        "exception method receivers should delegate Raw recovery to the central coercion helper"
    );
    let postfix_receiver_source = coercion_source
        .split("pub(crate) fn parenthesize_raw_postfix_receiver")
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
fn raw_ir_leaf_wrappers_are_not_public_api() {
    let item_assembly_source = include_str!("../src/item_assembly.rs");
    let statement_render_source = include_str!("../src/statement_render.rs");
    let types_source = include_str!("../src/types.rs");

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
        "split_top_level_rust_type_args",
    ] {
        assert!(
            !types_source.contains(&format!("pub fn {helper}")),
            "{helper} should not be public API"
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

    assert!(lower_source.contains("usage: &UsageFacts"));
    assert!(!lower_source.contains("usage: &'a UsageFacts"));
    assert!(lower_source.contains("signatures: &'a SignaturePlan"));
    assert!(lower_source.contains("exceptions: &'a ProgramExceptionPlan"));
    assert!(signature_source.contains("methods_by_java_key: HashMap<JavaCallableKey"));
    assert!(signature_source.contains("JavaCallableKey::source_method_for_type"));
    assert!(usage_source.contains("pub methods_by_java_key: HashMap<JavaCallableKey"));
    assert!(usage_source.contains("JavaCallableKey::source_method_for_type"));
    assert!(usage_source.contains("source_method_rust_name(method)"));
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
    assert!(
        lower_source
            .contains("CallableAnalysis::new(usage, signatures, &exceptions, &fallibility)")
    );
    assert!(lower_source.contains(
        "lower_program_with_semantic_plans_and_support_context(\n    program: &Program,\n    mut diagnostics: DiagnosticBag,\n    facts: &FactSet,\n    usage: &UsageFacts,\n    representations: &RepresentationPlan,\n    signatures: &SignaturePlan,"
    ));
}

#[test]
fn callable_analysis_shell_reaches_lowering_context() {
    let lib_source = include_str!("../src/lib.rs");
    let analysis_source = include_str!("../src/callable_analysis.rs");
    let lower_source = include_str!("../src/lower.rs");

    assert!(lib_source.contains("pub mod callable_analysis;"));
    assert!(analysis_source.contains("pub struct CallableAnalysis<'a>"));
    for field in ["usage", "signatures", "exceptions", "fallibility"] {
        assert!(
            !analysis_source.contains(&format!("pub {field}:")),
            "CallableAnalysis should expose query methods, not public backing field {field}"
        );
    }
    assert!(!analysis_source.contains("call_graph:"));
    assert!(analysis_source.contains("pub fn method_usage("));
    assert!(analysis_source.contains("pub fn method_usage_by_java_key("));
    assert!(analysis_source.contains("pub(crate) fn binding_use_count_for_rust_method_family("));
    assert!(analysis_source.contains("pub(crate) fn binding_facts_for_rust_method_family("));
    assert!(analysis_source.contains("pub fn method_signature_by_java_key("));
    assert!(analysis_source.contains("pub fn constructor_usage("));
    assert!(analysis_source.contains("pub fn callable_returns_result("));
    assert!(analysis_source.contains("pub fn method_exception_plan("));
    assert!(analysis_source.contains("pub fn external_method_returns_result("));
    assert!(!analysis_source.contains("pub fn method_usage_for_source_method("));
    assert!(!analysis_source.contains("pub fn method_signature_for_source_method("));
    assert!(!analysis_source.contains("pub fn source_method_returns_result("));
    assert!(analysis_source.contains("JavaCallableKey"));
    assert!(lower_source.contains("callable_analysis: &'a CallableAnalysis<'a>"));
    assert!(lower_source.contains("current_method_decl: Option<&'a MethodDecl>"));
    assert!(
        lower_source.contains("let call_graph = crate::usage::build_call_graph")
            && lower_source.contains("let fallibility = plan_program_fallibility")
            && lower_source.contains(
                "let callable_analysis = CallableAnalysis::new(usage, signatures, &exceptions, &fallibility);"
            )
    );
    assert!(lower_source.contains(".callable_analysis\n            .method_usage(&exact_key)"));
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
    assert!(resolved_source_signature_helper.contains("JavaCallableKey::source_method_for_type"));
    assert!(resolved_source_signature_helper.contains("method_signature_by_java_key"));
    let resolved_static_fallback = lower_source
        .split("fn source_context_resolved_static_method_fallback_signature")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn non_emitted_static_context_fallback_signature_for_method")
                .next()
        })
        .expect("resolved static source fallback planner should be present");
    assert!(resolved_static_fallback.contains("resolved_source_method_for_call"));
    assert!(resolved_static_fallback.contains("JavaCallableKey::source_method_for_owner"));
    assert!(resolved_static_fallback.contains("method_signature_by_java_key"));
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
    assert!(!signature_source.contains("pub fn method_signature_by_rust_name"));
    assert!(!signature_source.contains("pub fn constructor_signature_by_rust_name"));
    assert!(!signature_source.contains("pub fn method_arg_shape_by_rust_name"));
    assert!(!signature_source.contains("pub fn constructor_arg_shape_by_rust_name"));
    assert!(!signature_source.contains("pub fn method_return_shape_by_rust_name"));
    assert!(
        lower_source
            .contains(".callable_analysis\n            .constructor_usage(&UsageConstructorKey")
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
        usage_source
            .contains("pub(crate) methods_by_rust_name: HashMap<UsageMethodKey, MethodUsageFacts>")
    );
    assert!(
        !usage_source
            .contains("pub methods_by_rust_name: HashMap<UsageMethodKey, MethodUsageFacts>")
    );
    assert!(usage_source.contains("pub(crate) bindings: HashMap<String, BindingUseFacts>"));
    assert!(
        usage_source.contains("pub(crate) bindings_by_id: HashMap<BindingId, BindingUseFacts>")
    );
    assert!(!usage_source.contains("pub bindings: HashMap<String, BindingUseFacts>"));
    assert!(!usage_source.contains("pub bindings_by_id: HashMap<BindingId, BindingUseFacts>"));
    assert!(lower_source.contains(
        ".callable_analysis\n        .external_method_returns_result(receiver_type, name, arg_count)"
    ));
    assert!(lower_source.contains(".callable_returns_result(&crate::usage::CallableKey::Method"));
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
    assert!(binding_use_count_helper.contains("JavaCallableKey::source_method_for_owner"));
    assert!(binding_use_count_helper.contains("method_usage_by_java_key"));
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
    assert!(binding_facts_helper.contains("JavaCallableKey::source_method_for_owner"));
    assert!(binding_facts_helper.contains("method_usage_by_java_key"));
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
    let call_plan_source = include_str!("../src/call_plan.rs");
    let resolved_arg_shape_helper = call_plan_source
        .split("fn resolved_call_signature_arg_shapes_by_java_key")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn resolved_call_signature_arg_shapes_by_args")
                .next()
        })
        .expect("resolved Java-key arg-shape helper should be present");
    assert!(resolved_arg_shape_helper.contains("JavaCallableKey"));
    assert!(resolved_arg_shape_helper.contains("JavaOwnerKey::resolved_source"));
    assert!(resolved_arg_shape_helper.contains("resolved_source_method"));
    assert!(resolved_arg_shape_helper.contains("method_signature_by_java_key"));
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
    assert!(call_plan_source.contains("pub fn external_call_returns_rust_result_for_receiver"));
    for internal_helper in [
        "external_call_lowered_type_with_external_index",
        "external_receiver_call_return_type_with_external_index",
        "resource_close_declares_result_with_external_index",
        "external_call_returns_rust_result_with_external_index",
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
    assert!(call_plan_source.contains("facts.facade_abi_for_key(&key)"));
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
    assert!(source.contains("pub enum TraitImplMethodFallbackPolicy"));
    assert!(source.contains("pub enum TraitImplMethodCompatibilityError"));
    assert!(source.contains("pub struct TraitImplMethodMessagePolicy"));
    assert!(source.contains("pub fn trait_impl_method_message_policy"));
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
    assert!(!lower_source.contains("assignment_slot("));
    assert!(!lower_source.contains("assignment_slot_for_target"));
    assert!(conversion_source.contains("pub struct SyntaxCoercionPlan"));
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
    let design = include_str!("../docs/rust-slot-shape-and-object-representation.md");
    assert!(design.contains("## Boxed Dyn Clone-Box Requirements"));
    assert!(design.contains("DispatchCoercionPolicy::CloneBoxDynTrait"));
    assert!(design.contains("Coercion::CloneBoxDynTrait"));
    assert!(conversion_source.contains("pub struct CoercionContentPolicy"));
    assert!(conversion_source.contains("pub fn coercion_content_policy("));
    assert!(conversion_source.contains("pub struct CoercionShapeContentPolicy"));
    assert!(!conversion_source.contains("pub fn coercion_shape_content_policy("));
    assert!(
        conversion_source.contains("pub fn coercion_shape_content_policy_with_effective_facts")
    );
    assert!(!source.contains("pub struct CoercionContentPolicy"));
    assert!(!source.contains("pub fn coercion_content_policy("));
    assert!(!source.contains("pub struct CoercionShapeContentPolicy"));
    assert!(!source.contains("pub fn coercion_shape_content_policy("));
    assert!(conversion_source.contains("pub struct OptionalElementTargetMatchPolicy"));
    assert!(conversion_source.contains("pub fn optional_element_target_match_policy"));
    assert!(!source.contains("pub struct OptionalElementTargetMatchPolicy"));
    assert!(!source.contains("pub fn optional_element_target_match_policy"));
    assert!(conversion_source.contains("pub struct BorrowedElementSlotAdapterPolicy"));
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
    assert!(source.contains("apply_slot_conversion_plan(lowered.expr, &plan)"));
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
    assert!(conversion_source.contains("pub fn vec_element_target_match_policy"));
    assert!(!source.contains("pub struct VecElementTargetMatchPolicy"));
    assert!(!source.contains("pub fn vec_element_target_match_policy"));
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
    assert!(conversion_source.contains("pub fn rust_nominal_type_name_policy"));
    assert!(conversion_source.contains("pub enum RuntimeTypeTargetPolicy"));
    assert!(conversion_source.contains("pub fn runtime_type_target_policy"));
    assert!(!source.contains("pub struct RustNominalTypeNamePolicy"));
    assert!(!source.contains("pub fn rust_nominal_type_name_policy"));
    assert!(!source.contains("pub enum RuntimeTypeTargetPolicy"));
    assert!(!source.contains("pub fn runtime_type_target_policy"));
    assert!(conversion_source.contains("pub struct PrimitiveWideningPolicy"));
    assert!(conversion_source.contains("pub fn primitive_widening_policy"));
    assert!(!conversion_source.contains("pub struct PrimitiveRankPolicy"));
    assert!(!conversion_source.contains("pub fn primitive_rank_policy"));
    assert!(!source.contains("pub struct PrimitiveWideningPolicy"));
    assert!(!source.contains("pub fn primitive_widening_policy"));
    assert!(!source.contains("pub struct PrimitiveRankPolicy"));
    assert!(!source.contains("pub fn primitive_rank_policy"));
    assert!(source.contains("pub(crate) struct RustPrimitiveTypeNamePolicy"));
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
    assert!(source.contains("fn rendered_expr_has_clone_recovery"));
    assert!(!source.contains("pub fn rendered_expr_has_clone_recovery"));
    assert!(source.contains("fn rendered_expr_has_direct_clone_recovery"));
    assert!(!source.contains("pub fn rendered_expr_has_direct_clone_recovery"));
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
    assert!(conversion_source.contains("pub fn borrowed_element_adapter_policy_with_unwrap"));
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
    assert!(conversion_source.contains("ctx.facts.unwrap_or"));
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
    assert!(cast_instanceof_branch.contains("representations: Some(ctx.representations)"));
    assert!(cast_instanceof_branch.contains("facts: Some(ctx.facts)"));
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

    assert!(lowering_source.contains("apply_shape_coercion_plan(expr, &plan.coercion)"));
    assert!(lowering_source.contains("instanceof_check_projection(expr, &plan.check)"));
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
    assert!(source.contains("pub struct SignatureShapeRequest"));
    assert!(source.contains("pub struct MergedSignatureConstraints"));
    assert!(source.contains("pub struct SignatureConstraintConflict"));
    assert!(source.contains("pub enum SignatureConstraintAxis"));
    assert!(source.contains("from_constraints"));
    assert!(source.contains("apply_to_shape"));
    assert!(source.contains("requires_mutable_binding"));
    assert!(source.contains("conflicts"));
    assert!(source.contains("pub struct SignatureConstraintState"));
    assert!(source.contains("pub struct SignatureFixpointResult"));
    assert!(source.contains("pub struct SignatureSaturationReport"));
    assert!(source.contains("pub fn signature_saturation_report"));
    assert!(source.contains("pub fn signature_constraint_state_conflict_diagnostics"));
    assert!(source.contains("pub struct MethodConstraintSet"));
    assert!(source.contains("pub struct ConstructorConstraintSet"));
    assert!(source.contains("pub struct SignatureMemberApiBoundaryPolicy"));
    assert!(source.contains("pub fn signature_member_api_boundary_policy"));
    assert!(source.contains("public_api: signature_member_api_boundary_policy(&ctor.modifiers)"));
    assert!(source.contains("pub struct LocalConstraintSet"));
    assert!(source.contains("java_ty: JavaType"));
    assert!(source.contains("pub enum ReceiverConstraint"));
    assert!(source.contains("pub enum SignatureConstraintPropagationKind"));
    assert!(source.contains("pub struct SignatureConstraintPropagationPolicy"));
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

    let fact_saturation_source = include_str!("../src/fact_saturation.rs");
    assert!(fact_saturation_source.contains("pub struct FiniteFactDomain"));
    assert!(fact_saturation_source.contains("pub struct FiniteSaturationPolicy"));
    assert!(fact_saturation_source.contains("pub fn finite_fact_iteration_budget"));
    assert!(fact_saturation_source.contains("pub enum SaturationStep"));
    assert!(fact_saturation_source.contains("pub struct FiniteSaturationResult"));
    assert!(fact_saturation_source.contains("pub fn saturate_finite"));
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
    assert!(!source.contains("emit_type"));
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
