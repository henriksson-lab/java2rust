use crate::facts::{CallableKey, FactSet, ReceiverAbi, Staticness};
use crate::java_ir::{Expr, JavaType, PrimitiveType};
use crate::method_reference_plan::{MethodReferencePlan, plan_method_reference};
use crate::slot_shape::RustSlotShape;
use crate::types::TypeEnv;

pub fn infer_single_input_method_reference_output_shape(
    target: &str,
    method: &str,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    infer_single_input_method_reference_output_shape_with_context(
        target,
        method,
        input_shape,
        &TypeEnv::new(),
        &FactSet::default(),
    )
}

pub fn infer_single_input_method_reference_output_shape_with_context(
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
    if let Some(receiver_ty) = method_reference_receiver_type_for_call_plan_with_facts(
        target,
        method,
        Some(input_shape),
        facts,
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
    ) {
        return crate::call_plan::external_call_lowered_type(
            receiver, method, arg_count, facts, env, None,
        )
        .map(|lowered| lowered.into_slot_shape());
    }
    None
}

pub fn method_reference_receiver_type_for_call_plan(
    target: &str,
    input_shape: Option<&RustSlotShape>,
) -> Option<JavaType> {
    crate::call_plan::native_java_unbound_method_reference_receiver_type(target).or_else(|| {
        input_shape.and_then(|shape| object_method_reference_receiver_type(target, shape))
    })
}

pub fn method_reference_receiver_type_for_call_plan_with_facts(
    target: &str,
    method: &str,
    input_shape: Option<&RustSlotShape>,
    facts: &FactSet,
) -> Option<JavaType> {
    method_reference_receiver_type_for_call_plan(target, input_shape).or_else(|| {
        facade_abi_receiver_type_for_method_reference(target, method, facts).map(|name| {
            JavaType::Named {
                name,
                args: Vec::new(),
            }
        })
    })
}

fn facade_abi_receiver_type_for_method_reference(
    target: &str,
    method: &str,
    facts: &FactSet,
) -> Option<String> {
    let owner = crate::stdlib::canonical_external_receiver_type_key(target);
    method_reference_method_name_candidates(method)
        .into_iter()
        .find_map(|method| {
            let key = CallableKey::method(owner.clone(), method, Staticness::Unknown, 0);
            facts
                .facade_abi_for_key(&key)
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

fn method_reference_method_name_candidates(method: &str) -> Vec<String> {
    let snake = crate::parser::to_snake_case(method);
    if snake == method {
        vec![method.to_string()]
    } else {
        vec![method.to_string(), snake]
    }
}

pub fn object_method_reference_receiver_type(
    target: &str,
    input_shape: &RustSlotShape,
) -> Option<JavaType> {
    if crate::stdlib::canonical_external_receiver_type_key(target) == "Object"
        && matches!(target, "Object" | "java.lang.Object")
    {
        Some(input_shape.java_ty.clone())
    } else {
        None
    }
}

pub fn infer_method_reference_output_shape_for_plan(
    plan: &MethodReferencePlan,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    match plan {
        MethodReferencePlan::AssociatedFunction { owner, method }
            if owner == "Integer" && method == "int_value" =>
        {
            Some(RustSlotShape::for_storage(JavaType::Primitive(
                PrimitiveType::Int,
            )))
        }
        MethodReferencePlan::AssociatedFunction { owner, method }
            if (owner == "String" && method == "value_of")
                || (owner == "Object" && method == "to_string") =>
        {
            Some(crate::shape_projection::owned_string_value_shape())
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            infer_unbound_accessor_method_reference_fallback_shape(method, input_shape)
        }
        _ => None,
    }
}

fn infer_unbound_accessor_method_reference_fallback_shape(
    method: &str,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    if !is_java_accessor_method_reference_fallback(method) {
        return None;
    }
    let mut shape = input_shape.clone();
    shape.java_ty = JavaType::Unknown(method.to_string());
    Some(shape)
}

fn is_java_accessor_method_reference_fallback(method: &str) -> bool {
    method == "get" || method.starts_with("get_")
}
