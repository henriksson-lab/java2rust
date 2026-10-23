use crate::coercion::{
    OptionalReceiverProjection, associated_const_projection, field_projection,
    owner_associated_path_projection, project_optional_receiver, self_associated_path_projection,
    static_storage_accessor_projection,
};
use crate::expr_shape::ShapeEnv;
use crate::java_ir::Expr;
use crate::member_access_plan::{MemberAccessOp, MemberAccessPlan};
use crate::rust_ir::RustExpr;
use crate::types::TypeEnv;

pub(crate) fn lower_bare_name(name: &str, env: &ShapeEnv) -> RustExpr {
    // Declaration/binding evidence wins over capitalization: Java instance
    // fields may legally use constant-like or type-like names.
    if env.local_type(name).is_none() && env.is_instance_field(name) {
        let field_name = env.instance_field_access_name(name).unwrap_or(name);
        field_projection(RustExpr::Var("self".to_string()), field_name)
    } else if bare_name_local_type(name, env).is_none()
        && let Some(static_name) = env.static_storage_field_access_name(name)
    {
        static_storage_accessor_for(static_name, env)
    } else if bare_name_local_type(name, env).is_none()
        && let Some(static_name) = env.static_field_access_name(name)
    {
        // A field with no external owner is declared in the lexical frame.
        // An import of the same spelling must not redirect that declaration.
        if let Some(owner) = env.static_field_access_owner(name).map(|owner| {
            if env.current_type() == Some(owner) {
                owner
            } else {
                static_member_import_owner(name, env).unwrap_or(owner)
            }
        }) {
            owner_associated_path_projection(
                &rust_name_for_static_member_owner(owner, env),
                static_name,
            )
        } else {
            self_associated_path_projection(static_name)
        }
    } else if let Some(access_name) = env.binding_access_name(name) {
        RustExpr::Var(access_name.to_string())
    } else {
        RustExpr::Var(crate::parser::to_snake_case(name))
    }
}

pub(crate) fn lower_bare_name_with_facts(
    name: &str,
    env: &ShapeEnv,
    facts: &crate::facts::FactSet,
) -> RustExpr {
    if bare_name_local_type(name, env).is_none()
        && env.field_type(name).is_none()
        && let Some(owner) = static_member_import_owner(name, env)
    {
        let receiver = Expr::Var(owner.to_string());
        if let Some(plan) = crate::member_access_plan::plan_member_access_with_facts(
            &receiver,
            name,
            env.type_env(),
            facts,
        ) && matches!(plan.op(), MemberAccessOp::MappedStaticValue { .. })
        {
            return apply_member_access_plan(&plan, RustExpr::Var(owner.to_string()));
        }
    }
    lower_bare_name(name, env)
}

fn rust_name_for_static_member_owner(owner: &str, env: &ShapeEnv) -> String {
    let simple = owner
        .rsplit(['.', '$'])
        .find(|segment| !segment.is_empty())
        .unwrap_or(owner);
    if env.type_env().static_type_alias(simple) == Some(owner) {
        crate::java_naming::rust_name_for_java_named_type(simple)
    } else {
        crate::java_naming::rust_name_for_java_named_type(owner)
    }
}

fn static_member_import_owner<'a>(name: &str, env: &'a ShapeEnv) -> Option<&'a str> {
    env.type_env().static_member_import_owner(name).or_else(|| {
        env.type_env()
            .static_member_import_owner(&crate::parser::to_snake_case(name))
    })
}

fn bare_name_local_type<'a>(name: &str, env: &'a ShapeEnv) -> Option<&'a crate::java_ir::JavaType> {
    env.local_type(name).or_else(|| {
        let alias = crate::parser::to_snake_case(name);
        (alias != name).then(|| env.local_type(&alias)).flatten()
    })
}

/// A static-storage read, qualified with the declaring type when that is not
/// the type being lowered
/// ([0037](../claude_audits/done/0037-nested-class-outer-static-read-is-capture.md)).
fn static_storage_accessor_for(static_name: &str, env: &ShapeEnv) -> RustExpr {
    match env.static_storage_accessor_qualifier(static_name) {
        Some(owner) => {
            crate::coercion::qualified_static_storage_accessor_projection(&owner, static_name)
        }
        None => static_storage_accessor_projection(static_name),
    }
}

pub(crate) fn lower_field_access_with_facts(
    receiver: &Expr,
    field: &str,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
    facts: &crate::facts::FactSet,
) -> RustExpr {
    if let Some(projected) =
        lower_shared_local_field_access(receiver, field, type_env, lowered_receiver.clone(), facts)
    {
        return projected;
    }
    if let Expr::Var(owner) = receiver {
        let canonical_owner = crate::stdlib::canonical_external_receiver_type_key(owner);
        if crate::call_plan::implicit_java_lang_static_owner_is_unshadowed(
            owner,
            &canonical_owner,
            type_env,
        ) && let Some(value) =
            crate::stdlib::known_jdk_static_constant_value(&canonical_owner, field)
        {
            // Java implicitly imports java.lang. Render its primitive constants
            // directly so a batch peer's unrelated nested type with the same
            // leaf cannot retarget the access during Rust owner assembly.
            return RustExpr::Raw(value.to_string());
        }
    }
    if let Some(plan) =
        crate::member_access_plan::plan_member_access_with_facts(receiver, field, type_env, facts)
    {
        let lowered_receiver =
            lower_optional_sequence_member_receiver(receiver, type_env, &plan, lowered_receiver);
        let lowered_receiver = lower_nullable_array_element_member_receiver(
            receiver,
            type_env,
            &plan,
            lowered_receiver,
        );
        apply_member_access_plan(&plan, lowered_receiver)
    } else {
        let lowered_receiver =
            lower_nullable_array_element_field_receiver(receiver, type_env, lowered_receiver);
        field_projection(lowered_receiver, field)
    }
}

fn lower_shared_local_field_access(
    receiver: &Expr,
    field: &str,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
    facts: &crate::facts::FactSet,
) -> Option<RustExpr> {
    let receiver_ty = crate::types::infer_expr_type(receiver, type_env);
    let crate::java_ir::JavaType::Named { name, .. } = receiver_ty else {
        return None;
    };
    let placement = facts.storage_placement_for(&name);
    if !placement.shared() || !placement.interior_mutable() || placement.sync() {
        return None;
    }
    Some(field_projection(
        crate::coercion::receiver_method_call_projection_exact(
            lowered_receiver,
            "borrow",
            Vec::new(),
        ),
        &crate::parser::to_snake_case(field),
    ))
}

fn lower_nullable_array_element_field_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
) -> RustExpr {
    lower_nullable_array_element_receiver(receiver, type_env, lowered_receiver)
}

fn lower_nullable_array_element_member_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    plan: &MemberAccessPlan,
    lowered_receiver: RustExpr,
) -> RustExpr {
    if !matches!(
        plan.op(),
        MemberAccessOp::RustField { .. } | MemberAccessOp::RustGetter { .. }
    ) {
        return lowered_receiver;
    }
    lower_nullable_array_element_receiver(receiver, type_env, lowered_receiver)
}

fn lower_nullable_array_element_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    lowered_receiver: RustExpr,
) -> RustExpr {
    let Expr::ArrayAccess(array, _) = receiver else {
        return lowered_receiver;
    };
    let array_ty = crate::types::infer_expr_type(array, type_env);
    let Some(element_ty) = crate::types::sequence_element_type(&array_ty) else {
        return lowered_receiver;
    };
    if matches!(
        element_ty,
        crate::java_ir::JavaType::Primitive(_)
            | crate::java_ir::JavaType::Void
            | crate::java_ir::JavaType::Array(_)
    ) || crate::types::optional_element_type(&element_ty).is_some()
    {
        return lowered_receiver;
    }
    project_optional_receiver(lowered_receiver, OptionalReceiverProjection::Shared)
}

fn lower_optional_sequence_member_receiver(
    receiver: &Expr,
    type_env: &TypeEnv,
    plan: &MemberAccessPlan,
    lowered_receiver: RustExpr,
) -> RustExpr {
    if !matches!(
        plan.op(),
        MemberAccessOp::LenAsI32
            | MemberAccessOp::Array2RowsAsI32
            | MemberAccessOp::Array2ColsAsI32
    ) {
        return lowered_receiver;
    }
    let receiver_ty = crate::types::infer_expr_type(receiver, type_env);
    let Some(inner_ty) = crate::types::optional_element_type(&receiver_ty) else {
        return lowered_receiver;
    };
    if crate::types::sequence_element_type(&inner_ty).is_none()
        && crate::types::ndarray_array2_element_type(&inner_ty).is_none()
    {
        return lowered_receiver;
    }
    crate::coercion::project_optional_receiver(
        lowered_receiver,
        crate::coercion::OptionalReceiverProjection::Shared,
    )
}

fn apply_member_access_plan(plan: &MemberAccessPlan, receiver: RustExpr) -> RustExpr {
    match plan.op() {
        MemberAccessOp::LenAsI32 => crate::coercion::len_i32_projection(receiver),
        MemberAccessOp::Array2RowsAsI32 => crate::coercion::array2_rows_i32_projection(receiver),
        MemberAccessOp::Array2ColsAsI32 => crate::coercion::array2_cols_i32_projection(receiver),
        MemberAccessOp::StaticConstant { owner } => {
            let owner = if owner.contains('.') {
                crate::java_naming::rust_name_for_java_named_type(owner)
            } else {
                owner.clone()
            };
            associated_const_projection(&owner, &rust_associated_const_ident(plan.member()))
        }
        MemberAccessOp::MappedStaticValue { rust_value } => {
            RustExpr::Grouped(Box::new(RustExpr::Raw(rust_value.clone())))
        }
        MemberAccessOp::RustField { rust_name } => field_projection(receiver, rust_name),
        MemberAccessOp::RustGetter { rust_name } => {
            crate::coercion::receiver_method_call_projection_exact(receiver, rust_name, Vec::new())
        }
    }
}

fn rust_associated_const_ident(name: &str) -> String {
    if matches!(name, "in") {
        "r#in".to_string()
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::external_packs::parse_external_pack_manifest;
    use crate::facts::FactSet;
    use crate::java_ir::{JavaType, PrimitiveType};
    use crate::parser::parse_expr;
    use crate::slot_shape::RustSlotShape;
    use crate::types::TypeEnv;

    fn render_bare_name(name: &str, env: &ShapeEnv) -> String {
        crate::emit::emit_expr(&lower_bare_name(name, env))
    }

    #[test]
    fn mapped_static_values_reuse_pack_expressions_and_respect_source_owners() {
        let parsed = parse_external_pack_manifest(
            "external-pack dep.token dep.Token rust-crate-mapping mapped target=dep_runtime rust-type=Token\nexternal-pack-field dep.token dep.Token CURRENT mapped type=dep.Token value=dep_runtime::current()\n",
        );
        assert!(
            !parsed.diagnostics().has_errors(),
            "{:?}",
            parsed.diagnostics()
        );
        let mut env = TypeEnv::default();
        env.insert_static_type_alias("Token", "dep.Token");
        let owner = Expr::Var("Token".into());
        let plan = crate::member_access_plan::plan_member_access_with_facts(
            &owner,
            "CURRENT",
            &env,
            parsed.facts(),
        )
        .unwrap();
        assert!(matches!(
            plan.op(),
            MemberAccessOp::MappedStaticValue { .. }
        ));
        assert_eq!(
            crate::emit::emit_expr(&apply_member_access_plan(
                &plan,
                RustExpr::Var("Token".into())
            )),
            "(dep_runtime::current())"
        );
        let source = crate::parser::parse_java_source(
            "package dep; class Token { static int CURRENT = 7; }",
        );
        let mut emitted = crate::source_emission::SourceEmissionPlan::default();
        emitted.add_program(&source.program);
        env.set_source_emission(std::sync::Arc::new(emitted));
        let plan = crate::member_access_plan::plan_member_access_with_facts(
            &owner,
            "CURRENT",
            &env,
            parsed.facts(),
        )
        .unwrap();
        assert!(!matches!(
            plan.op(),
            MemberAccessOp::MappedStaticValue { .. }
        ));
    }

    #[test]
    fn lexical_static_field_shadows_imported_mapped_value() {
        let parsed = parse_external_pack_manifest(
            "external-pack dep.token dep.Token rust-crate-mapping mapped target=dep_runtime\nexternal-pack-field dep.token dep.Token CURRENT mapped type=int value=dep_runtime::current()\n",
        );
        let mut env = ShapeEnv::new();
        env.set_current_type("Probe");
        env.insert_static_member_import("CURRENT", "dep.Token");
        let shape = RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int));
        env.insert_static_field_shape("CURRENT", shape.clone());
        assert_eq!(
            crate::emit::emit_expr(&lower_bare_name_with_facts("CURRENT", &env, parsed.facts())),
            "Self::CURRENT"
        );
        env.insert_static_field_shape_for_owner_with_access_name(
            "Probe", "CURRENT", "CURRENT", shape,
        );
        assert_eq!(
            crate::emit::emit_expr(&lower_bare_name_with_facts("CURRENT", &env, parsed.facts())),
            "Probe::CURRENT"
        );
    }

    #[test]
    fn bare_name_rendering_is_test_local_compatibility() {
        let mut env = ShapeEnv::new();
        env.insert_field("count", JavaType::Primitive(PrimitiveType::Int));
        env.insert_static_field_shape(
            "LIMIT",
            RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
        );
        env.insert_static_field_shape_for_owner_with_access_name(
            "b.Item",
            "KIND",
            "KIND",
            RustSlotShape::for_storage(JavaType::String),
        );
        env.insert_static_member_import("KIND", "a.Item");
        env.insert_local("local", JavaType::Primitive(PrimitiveType::Int));

        assert_eq!(render_bare_name("count", &env), "self.count");
        assert_eq!(render_bare_name("local", &env), "local");
        assert_eq!(render_bare_name("LIMIT", &env), "Self::LIMIT");
        assert_eq!(render_bare_name("KIND", &env), "AItem::KIND");

        assert_eq!(
            lower_bare_name("count", &env),
            RustExpr::Field(
                Box::new(RustExpr::Var("self".to_string())),
                "count".to_string()
            )
        );
        assert_eq!(
            lower_bare_name("local", &env),
            RustExpr::Var("local".to_string())
        );
        assert_eq!(
            lower_bare_name("LIMIT", &env),
            RustExpr::Path(vec!["Self".to_string(), "LIMIT".to_string()])
        );
        assert_eq!(
            lower_bare_name("KIND", &env),
            RustExpr::Path(vec!["AItem".to_string(), "KIND".to_string()])
        );

        let mut imported_env = ShapeEnv::new();
        imported_env.insert_static_field_shape_for_owner_with_access_name(
            "org.example.Constants",
            "LIMIT",
            "LIMIT",
            RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
        );
        imported_env.insert_static_member_import("LIMIT", "org.example.Constants");
        imported_env.insert_static_type_alias("Constants", "org.example.Constants");
        assert_eq!(render_bare_name("LIMIT", &imported_env), "Constants::LIMIT");
    }

    #[test]
    fn field_access_lowering_uses_member_access_plans() {
        let mut type_env = TypeEnv::new();
        type_env.insert_local("values", JavaType::Array(Box::new(JavaType::String)));
        type_env.insert_type_field("Owner", "CONSTANT", JavaType::String);
        type_env.insert_local(
            "callback",
            JavaType::Named {
                name: "BatchCompletionCallbackFunctionArgument".to_string(),
                args: Vec::new(),
            },
        );

        assert_eq!(
            lower_field_access_with_facts(
                &parse_expr("values"),
                "length",
                &type_env,
                RustExpr::Var("values".to_string()),
                &FactSet::default(),
            ),
            RustExpr::Cast {
                expr: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("values".to_string())),
                    method: "len".to_string(),
                    args: Vec::new(),
                }),
                ty: "i32".to_string(),
            }
        );
        assert_eq!(
            lower_field_access_with_facts(
                &parse_expr("callback"),
                "batchCount",
                &type_env,
                RustExpr::Var("callback".to_string()),
                &FactSet::default(),
            ),
            RustExpr::Field(
                Box::new(RustExpr::Var("callback".to_string())),
                "batch_count".to_string()
            )
        );
        assert_eq!(
            lower_field_access_with_facts(
                &parse_expr("helper"),
                "name",
                &type_env,
                RustExpr::Var("helper".to_string()),
                &FactSet::default(),
            ),
            RustExpr::Field(
                Box::new(RustExpr::Var("helper".to_string())),
                "name".to_string()
            )
        );
        assert_eq!(
            lower_field_access_with_facts(
                &Expr::Var("Owner".to_string()),
                "CONSTANT",
                &type_env,
                RustExpr::Var("owner".to_string()),
                &FactSet::default(),
            ),
            RustExpr::Path(vec!["Owner".to_string(), "CONSTANT".to_string()])
        );
    }

    #[test]
    fn rust_crate_mapped_external_static_fields_render_associated_constants() {
        let facts = parse_external_pack_manifest(
            r#"
external-pack hdf5 HDF5File rust-crate-mapping mapped target=hdf5_facade
external-pack hdf5 HDF5File.OpenMode rust-crate-mapping mapped target=hdf5_facade
external-pack hdf5 OpenMode rust-crate-mapping mapped target=hdf5_facade
external-pack-field hdf5 HDF5File OpenMode mapped
external-pack-field hdf5 HDF5File.OpenMode CREATE mapped
external-pack-field hdf5 OpenMode READ_ONLY mapped
"#,
        )
        .facts()
        .clone();

        let hdf5_open_mode = Expr::FieldAccess(
            Box::new(Expr::Var("HDF5File".to_string())),
            "OpenMode".to_string(),
        );
        assert_eq!(
            lower_field_access_with_facts(
                &hdf5_open_mode,
                "CREATE",
                &TypeEnv::default(),
                RustExpr::Raw("ignored".to_string()),
                &facts,
            ),
            RustExpr::Path(vec![
                "hdf5_facade::OpenMode".to_string(),
                "CREATE".to_string(),
            ])
        );
        assert_eq!(
            lower_field_access_with_facts(
                &Expr::Var("OpenMode".to_string()),
                "READ_ONLY",
                &TypeEnv::default(),
                RustExpr::Raw("ignored".to_string()),
                &facts,
            ),
            RustExpr::Path(vec![
                "hdf5_facade::OpenMode".to_string(),
                "READ_ONLY".to_string(),
            ])
        );
    }
}
