use crate::effective_facts::EffectiveFacts;
use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, PrimitiveType};
use crate::shape_projection::iteration_element_shape_with_effective_facts;
use crate::slot_shape::RustSlotShape;
use crate::types::{TypeEnv, infer_expr_type, optional_element_type, stream_element_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSourceCapabilityPolicy {
    capability_element: Option<JavaType>,
}

impl StreamSourceCapabilityPolicy {
    pub fn capability_element(&self) -> Option<&JavaType> {
        self.capability_element.as_ref()
    }

    fn into_capability_element(self) -> Option<JavaType> {
        self.capability_element
    }
}

pub fn stream_source_capability_policy(
    source_ty: &JavaType,
    facts: &FactSet,
) -> StreamSourceCapabilityPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    stream_source_capability_policy_with_effective_facts(source_ty, &effective_facts)
}

pub fn stream_source_capability_policy_with_effective_facts(
    source_ty: &JavaType,
    facts: &EffectiveFacts,
) -> StreamSourceCapabilityPolicy {
    StreamSourceCapabilityPolicy {
        capability_element: facts.sequence_element_type_for(source_ty),
    }
}

pub fn infer_stream_lambda_body_shape_with_facts(
    expr: &Expr,
    default: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> RustSlotShape {
    if let Some(shape) = infer_stream_lambda_call_shape_from_shared_plan(expr, default, env, facts)
    {
        return shape;
    }
    default.clone()
}

pub fn infer_stream_map_to_int_shape(_expr: &Expr) -> RustSlotShape {
    RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int))
}

pub fn infer_stream_source_element_shape_with_facts(
    source: &Expr,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    if let Some(element_ty) = external_call_stream_source_element_type(source, env, facts) {
        let effective_facts = EffectiveFacts::new(facts);
        return Some(iteration_element_shape_with_effective_facts(
            element_ty,
            &effective_facts,
        ));
    }
    let effective_facts = EffectiveFacts::new(facts);
    infer_stream_source_element_shape_with_effective_facts(source, env, &effective_facts)
}

pub fn infer_stream_source_element_shape_with_effective_facts(
    source: &Expr,
    env: &TypeEnv,
    facts: &EffectiveFacts,
) -> Option<RustSlotShape> {
    stream_source_element_type_with_effective_facts(source, env, facts)
        .map(|ty| iteration_element_shape_with_effective_facts(ty, facts))
}

fn infer_stream_lambda_call_shape_from_shared_plan(
    expr: &Expr,
    default: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    let mut lambda_env;
    let env = if let Expr::Var(receiver_name) = receiver.as_ref() {
        if env.local_type(receiver_name).is_none()
            && !stream_receiver_is_static_utility_owner(receiver_name, name, args.len(), facts)
        {
            lambda_env = env.clone();
            lambda_env.insert_local(receiver_name.clone(), default.java_ty.clone());
            &lambda_env
        } else {
            env
        }
    } else {
        env
    };
    let target = crate::call_plan::plan_call_target_with_facts_and_arity(
        Some(receiver),
        name,
        args.len(),
        resolved.as_ref(),
        facts,
        env,
    );
    if let Some(lowered) = crate::call_plan::native_java_call_lowered_type(&target) {
        return Some(lowered.into_slot_shape());
    }
    crate::call_plan::external_call_lowered_type(Some(receiver), name, args.len(), facts, env, None)
        .map(|lowered| lowered.into_slot_shape())
}

fn stream_receiver_is_static_utility_owner(
    receiver_name: &str,
    method_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> bool {
    let owner = crate::stdlib::canonical_external_receiver_type_key(receiver_name);
    crate::stdlib::external_api_method_role(Some(&owner), method_name, arg_count)
        != crate::facts::ExternalApiRole::Unknown
        || facts
            .external_api_effect_for_receiver_with_arity(Some(&owner), method_name, Some(arg_count))
            .is_some()
        || facts
            .facade_abi_for_external_method(&owner, method_name, arg_count)
            .is_some()
}

fn stream_source_element_type_with_effective_facts(
    source: &Expr,
    env: &TypeEnv,
    facts: &EffectiveFacts,
) -> Option<JavaType> {
    if is_int_stream_range_source(source) {
        return Some(JavaType::Primitive(PrimitiveType::Int));
    }
    let source_ty = infer_expr_type(source, env);
    let capability = stream_source_capability_policy_with_effective_facts(&source_ty, facts);
    stream_element_type(&source_ty)
        .or_else(|| capability.into_capability_element())
        .or_else(|| {
            let element_ty = optional_element_type(&source_ty)?;
            let capability =
                stream_source_capability_policy_with_effective_facts(&element_ty, facts);
            stream_element_type(&element_ty).or_else(|| capability.into_capability_element())
        })
}

fn external_call_stream_source_element_type(
    source: &Expr,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<JavaType> {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = source
    else {
        return None;
    };
    let lowered = crate::call_plan::external_call_lowered_type(
        receiver.as_deref(),
        name,
        args.len(),
        facts,
        env,
        None,
    )?;
    let java_ty = lowered.java_ty()?;
    stream_element_type(java_ty)
}

fn is_int_stream_range_source(source: &Expr) -> bool {
    matches!(
        source,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.len() == 2
            && matches!(receiver.as_ref(), Expr::Var(owner)
                if crate::stdlib::canonical_external_receiver_type_key(owner) == "IntStream")
            && matches!(name.as_str(), "range" | "range_closed")
    )
}
