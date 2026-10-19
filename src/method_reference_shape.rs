use crate::facts::{ExternalApiRole, FactSet, ReceiverAbi};
use crate::java_ir::{Expr, JavaType, PrimitiveType};
use crate::method_reference_plan::{MethodReferencePlan, plan_method_reference};
use crate::slot_shape::RustSlotShape;
use crate::types::TypeEnv;

/// Resolve the two legal interpretations of a type-qualified unary reference
/// using the same source signatures for rendering and callback effects.
pub(crate) fn source_unbound_or_static_reference_signature<'a>(
    target: &str,
    method: &str,
    input: &RustSlotShape,
    env: &TypeEnv,
    signatures: &'a crate::signature::SignaturePlan,
    facts: Option<&FactSet>,
) -> Option<(String, String, &'a crate::signature::MethodSignaturePlan)> {
    let owner = env.static_type_alias(target).unwrap_or(target);
    let unbound = signatures
        .method_signature_by_base_name_accepting_arity(owner, method, 0)
        .filter(|(_, signature)| signature.receiver() != crate::signature::ReceiverPlan::None)
        .map(|(name, signature)| (owner.to_string(), name, signature));
    let args = std::slice::from_ref(&input.java_ty);
    let static_call = facts
        .and_then(|facts| {
            signatures
                .method_signature_by_base_name_and_arg_types_with_facts(owner, method, args, facts)
        })
        .or_else(|| signatures.method_signature_by_base_name_and_arg_types(owner, method, args))
        .filter(|(_, signature)| signature.receiver() == crate::signature::ReceiverPlan::None)
        .map(|(name, signature)| (owner.to_string(), name, signature));
    match (unbound, static_call) {
        (Some(selected), None) | (None, Some(selected)) => Some(selected),
        _ => None,
    }
}

pub(crate) fn single_input_reference_has_nullable_receiver(
    target: &str,
    method: &str,
    input: &RustSlotShape,
    env: &crate::expr_shape::ShapeEnv,
    signatures: &crate::signature::SignaturePlan,
    facts: &FactSet,
) -> bool {
    if input.optional_element_type().is_none()
        || method == "new"
        || matches!(target, "this" | "self")
        || crate::method_reference_plan::bound_method_reference_receiver(target, env).is_some()
    {
        return false;
    }
    if let Some((_, _, signature)) = source_unbound_or_static_reference_signature(
        target,
        method,
        input,
        env.type_env(),
        signatures,
        Some(facts),
    ) {
        return signature.receiver() != crate::signature::ReceiverPlan::None;
    }
    let Some(receiver_ty) = method_reference_receiver_type_for_call_plan_with_context(
        target,
        method,
        Some(input),
        facts,
        env.type_env(),
    ) else {
        return false;
    };
    let mut types = env.type_env().clone();
    let name = "__java_method_ref_receiver";
    types.insert_local(name.to_string(), receiver_ty);
    let receiver = Expr::Var(name.to_string());
    matches!(
        crate::call_plan::plan_call_target_with_facts_and_arity(
            Some(&receiver),
            method,
            0,
            None,
            facts,
            &types,
        ),
        crate::call_plan::CallTargetPlan::NativeJavaInstance { .. }
    )
}

pub(crate) fn infer_single_input_method_reference_output_shape_with_context(
    target: &str,
    method: &str,
    input_shape: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    if let Some(shape) =
        infer_method_reference_output_shape_from_call_plan(target, method, input_shape, env, facts)
    {
        return Some(shape);
    }
    let plan = plan_method_reference(target, method);
    infer_method_reference_output_shape_for_plan(&plan, input_shape)
}

fn infer_method_reference_output_shape_from_call_plan(
    target: &str,
    method: &str,
    input_shape: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    if matches!(target, "this" | "self") {
        return None;
    }
    let mut lambda_env = env.clone();
    lambda_env.insert_local(
        "__java_method_ref_value".to_string(),
        input_shape.java_ty.clone(),
    );
    if let Some(receiver_ty) = method_reference_receiver_type_for_call_plan_with_context(
        target,
        method,
        Some(input_shape),
        facts,
        env,
    ) {
        lambda_env.insert_local("__java_method_ref_value".to_string(), receiver_ty);
        let receiver = Expr::Var("__java_method_ref_value".to_string());
        let target_plan = crate::call_plan::plan_call_target_with_facts_and_arity(
            Some(&receiver),
            method,
            0,
            None,
            facts,
            &lambda_env,
        );
        if let Some(shape) = method_reference_output_shape_from_call_plan(
            &target_plan,
            Some(&receiver),
            method,
            0,
            facts,
            &lambda_env,
        ) {
            return Some(shape);
        }
    }
    let owner = Expr::Var(target.to_string());
    let target_plan = crate::call_plan::plan_call_target_with_facts_and_arity(
        Some(&owner),
        method,
        1,
        None,
        facts,
        &lambda_env,
    );
    method_reference_output_shape_from_call_plan(
        &target_plan,
        Some(&owner),
        method,
        1,
        facts,
        &lambda_env,
    )
}

fn method_reference_output_shape_from_call_plan(
    target_plan: &crate::call_plan::CallTargetPlan<'_>,
    receiver: Option<&Expr>,
    method: &str,
    arg_count: usize,
    facts: &FactSet,
    env: &TypeEnv,
) -> Option<RustSlotShape> {
    if matches!(
        target_plan,
        crate::call_plan::CallTargetPlan::NativeJavaInstance { .. }
            | crate::call_plan::CallTargetPlan::NativeJavaStatic { .. }
    ) {
        return crate::call_plan::native_java_call_lowered_type(target_plan)
            .map(|lowered| lowered.into_slot_shape());
    }
    if matches!(
        target_plan,
        crate::call_plan::CallTargetPlan::ExternalFacadeReceiver { .. }
            | crate::call_plan::CallTargetPlan::ExternalCrateReceiver { .. }
            | crate::call_plan::CallTargetPlan::FallbackReceiver { .. }
    ) {
        return crate::call_plan::external_call_lowered_type(
            receiver, method, arg_count, facts, env, None,
        )
        .map(|lowered| lowered.into_slot_shape());
    }
    None
}

pub(crate) fn method_reference_receiver_type_for_call_plan(
    target: &str,
    input_shape: Option<&RustSlotShape>,
) -> Option<JavaType> {
    crate::call_plan::native_java_unbound_method_reference_receiver_type(target).or_else(|| {
        input_shape.and_then(|shape| object_method_reference_receiver_type(target, shape))
    })
}

pub(crate) fn method_reference_receiver_type_for_call_plan_with_facts(
    target: &str,
    method: &str,
    input_shape: Option<&RustSlotShape>,
    facts: &FactSet,
) -> Option<JavaType> {
    method_reference_receiver_type_for_call_plan_with_context(
        target,
        method,
        input_shape,
        facts,
        &TypeEnv::new(),
    )
}

pub(crate) fn method_reference_receiver_type_for_call_plan_with_context(
    target: &str,
    method: &str,
    input_shape: Option<&RustSlotShape>,
    facts: &FactSet,
    types: &TypeEnv,
) -> Option<JavaType> {
    method_reference_receiver_type_for_call_plan(target, input_shape)
        .or_else(|| {
            let input = input_shape?;
            let ty = input
                .optional_element_type()
                .unwrap_or_else(|| input.java_ty.clone());
            // Only a scoped generic declaration is evidence for open Object
            // dispatch. A matching nominal spelling alone must not activate
            // external getter heuristics for an otherwise unknown owner.
            (matches!(&ty, JavaType::Named { name, .. } if name == target)
                && types.is_unbounded_object_type_parameter(&ty))
            .then(|| input.java_ty.clone())
        })
        .or_else(|| {
            let input = input_shape?;
            let effective = crate::effective_facts::EffectiveFacts::new(facts);
            effective.retained_native_collection_kind(&JavaType::Named {
                name: target.to_string(),
                args: Vec::new(),
            })?;
            effective.retained_native_collection_kind(&input.java_ty)?;
            Some(input.java_ty.clone())
        })
        .or_else(|| {
            facade_abi_receiver_type_for_method_reference(target, method, facts)
                .or_else(|| external_api_receiver_type_for_method_reference(target, method, facts))
                .map(|name| JavaType::Named {
                    name,
                    args: Vec::new(),
                })
        })
}

fn facade_abi_receiver_type_for_method_reference(
    target: &str,
    method: &str,
    facts: &FactSet,
) -> Option<String> {
    let owner = crate::stdlib::canonical_external_receiver_type_key(target);
    crate::java_naming::java_method_name_aliases(method)
        .into_iter()
        .find_map(|method| {
            facts
                .facade_abi_for_external_method(&owner, &method, 0)
                .filter(|abi| {
                    matches!(
                        abi.receiver,
                        ReceiverAbi::BorrowedSelf
                            | ReceiverAbi::MutBorrowedSelf
                            | ReceiverAbi::OwnedSelf
                    )
                })
                .map(|_| owner.clone())
        })
}

fn external_api_receiver_type_for_method_reference(
    target: &str,
    method: &str,
    facts: &FactSet,
) -> Option<String> {
    let owner = crate::stdlib::canonical_external_receiver_type_key(target);
    let external_index = facts.external_lookup_index();
    crate::java_naming::java_method_name_aliases(method)
        .into_iter()
        .find_map(|method| {
            external_index
                .external_api_effect_for_receiver_with_arity(Some(&owner), &method, Some(0))
                .cloned()
                .or_else(|| {
                    crate::external_packs::external_api_effect_for_receiver_with_arity(
                        Some(&owner),
                        &method,
                        Some(0),
                    )
                })
                .filter(|effect| {
                    matches!(
                        effect.role,
                        ExternalApiRole::Getter | ExternalApiRole::BooleanGetter
                    )
                })
                .map(|effect| effect.receiver_type.unwrap_or_else(|| owner.clone()))
        })
}

fn object_method_reference_receiver_type(
    target: &str,
    input_shape: &RustSlotShape,
) -> Option<JavaType> {
    if matches!(target, "Object" | "java.lang.Object") {
        Some(input_shape.java_ty.clone())
    } else {
        None
    }
}

pub(crate) fn infer_method_reference_output_shape_for_plan(
    plan: &MethodReferencePlan,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    match plan {
        MethodReferencePlan::AssociatedFunction { owner, method }
            if method_reference_plan_owner_simple_name(owner.as_str()) == "Integer"
                && method == "int_value" =>
        {
            Some(RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            )))
        }
        MethodReferencePlan::AssociatedFunction { owner, method }
            if (method_reference_plan_owner_simple_name(owner.as_str()) == "String"
                && method == "value_of")
                || (method_reference_plan_owner_simple_name(owner.as_str()) == "Object"
                    && method == "to_string") =>
        {
            Some(crate::shape_projection::owned_string_value_shape())
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            infer_unbound_accessor_method_reference_fallback_shape(method, input_shape)
        }
        _ => None,
    }
}

fn method_reference_plan_owner_simple_name(owner: &str) -> String {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(owner);
    canonical
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(owner)
        .to_string()
}

fn infer_unbound_accessor_method_reference_fallback_shape(
    method: &str,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    if !crate::java_naming::java_accessor_method_reference_fallback_name(method) {
        return None;
    }
    let mut shape = input_shape.clone();
    shape.java_ty = JavaType::Unknown(method.to_string());
    Some(shape)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{
        CallableKey, ExternalApiEffectFact, FacadeAbiFact, FacadeSlot, FactProvenance,
        NullabilityState, ReceiverMutabilityFact, ResultAbi, Staticness,
    };

    #[test]
    fn method_reference_receiver_type_uses_native_and_object_fallbacks() {
        assert_eq!(
            method_reference_receiver_type_for_call_plan(
                "String",
                Some(&RustSlotShape::for_storage(JavaType::String)),
            ),
            Some(JavaType::String)
        );
        assert_eq!(
            method_reference_receiver_type_for_call_plan(
                "Object",
                Some(&RustSlotShape::for_storage(JavaType::Unknown(
                    "NameHolder".to_string(),
                ))),
            ),
            Some(JavaType::Unknown("NameHolder".to_string()))
        );
    }

    #[test]
    fn method_reference_output_shape_uses_native_accessors_and_external_facts() {
        let mut generic_env = TypeEnv::new();
        generic_env.insert_type_param_declaration("T", None);
        let generic_input =
            RustSlotShape::for_storage(crate::types::optional_java_type(JavaType::Named {
                name: "T".to_string(),
                args: Vec::new(),
            }));
        let generic_result = infer_single_input_method_reference_output_shape_with_context(
            "T",
            "toString",
            &generic_input,
            &generic_env,
            &FactSet::default(),
        )
        .expect("unbound generic references must use the selected virtual call contract");
        assert_eq!(
            generic_result.optional_element_type(),
            Some(JavaType::String)
        );
        assert_eq!(
            infer_single_input_method_reference_output_shape_with_context(
                "Integer",
                "intValue",
                &RustSlotShape::for_storage(JavaType::String),
                &TypeEnv::new(),
                &FactSet::default(),
            ),
            Some(RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            )))
        );
        assert_eq!(
            infer_single_input_method_reference_output_shape_with_context(
                "Object",
                "toString",
                &RustSlotShape::for_storage(JavaType::String),
                &TypeEnv::new(),
                &FactSet::default(),
            ),
            Some(crate::shape_projection::owned_string_value_shape())
        );
        assert_eq!(
            infer_single_input_method_reference_output_shape_with_context(
                "NameHolder",
                "getName",
                &RustSlotShape::for_storage(JavaType::Unknown("NameHolder".to_string())),
                &TypeEnv::new(),
                &FactSet::default(),
            )
            .map(|shape| shape.java_ty),
            Some(JavaType::Unknown("get_name".to_string()))
        );

        let mut external_method_ref_facts = FactSet::default();
        external_method_ref_facts
            .external_api_effect_facts
            .push(ExternalApiEffectFact {
                method_name: "label".to_string(),
                receiver_type: Some("FacadeValue".to_string()),
                arg_count: Some(0),
                parameter_patterns: None,
                role: ExternalApiRole::Getter,
                receiver_mutability: ReceiverMutabilityFact::ReadOnly,
                return_type: Some(JavaType::String),
                return_nullability: NullabilityState::DefinitelyNonNull,
                thrown: Vec::new(),
                provenance: FactProvenance::UserOverride,
            });
        external_method_ref_facts
            .facade_abi_facts
            .push(FacadeAbiFact {
                key: CallableKey::method(
                    "FacadeValue".to_string(),
                    "label".to_string(),
                    Staticness::Unknown,
                    0,
                ),
                rust_path: "FacadeValueFacade::label_text".to_string(),
                receiver: ReceiverAbi::BorrowedSelf,
                args: Vec::new(),
                return_slot: FacadeSlot::Owned,
                result: ResultAbi::Plain,
                borrow_source: None,
                borrowed_return_recovery: None,
                provenance: FactProvenance::UserOverride,
            });
        let facade_value_shape = RustSlotShape::for_storage(JavaType::Named {
            name: "FacadeValue".to_string(),
            args: Vec::new(),
        });
        assert_eq!(
            infer_single_input_method_reference_output_shape_with_context(
                "FacadeValue",
                "label",
                &facade_value_shape,
                &TypeEnv::new(),
                &external_method_ref_facts,
            ),
            Some(crate::shape_projection::owned_string_value_shape())
        );

        let mut external_getter_method_ref_facts = FactSet::default();
        external_getter_method_ref_facts
            .external_api_effect_facts
            .push(ExternalApiEffectFact {
                method_name: "getName".to_string(),
                receiver_type: Some("ExternalRecord".to_string()),
                arg_count: Some(0),
                parameter_patterns: None,
                role: ExternalApiRole::Getter,
                receiver_mutability: ReceiverMutabilityFact::ReadOnly,
                return_type: Some(JavaType::String),
                return_nullability: NullabilityState::DefinitelyNonNull,
                thrown: Vec::new(),
                provenance: FactProvenance::UserOverride,
            });
        assert_eq!(
            infer_single_input_method_reference_output_shape_with_context(
                "ExternalRecord",
                "getName",
                &RustSlotShape::for_storage(JavaType::Named {
                    name: "ExternalRecord".to_string(),
                    args: Vec::new(),
                }),
                &TypeEnv::new(),
                &external_getter_method_ref_facts,
            ),
            Some(crate::shape_projection::owned_string_value_shape())
        );
        assert_eq!(
            infer_single_input_method_reference_output_shape_with_context(
                "ExternalRecord",
                "getName",
                &RustSlotShape::for_storage(JavaType::Named {
                    name: "ExternalRecord".to_string(),
                    args: Vec::new(),
                }),
                &TypeEnv::new(),
                &FactSet::default(),
            )
            .map(|shape| shape.java_ty),
            Some(JavaType::Unknown("get_name".to_string()))
        );
    }
}
