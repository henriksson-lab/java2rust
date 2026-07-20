use crate::effective_facts::EffectiveFacts;
use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, PrimitiveType};
use crate::shape_projection::{
    borrowed_string_content_shape, iteration_element_shape_with_effective_facts,
    owned_string_value_shape,
};
use crate::slot_shape::RustSlotShape;
use crate::stdlib::{is_borrowed_string_view_method, is_owned_string_result_method};
use crate::types::{TypeEnv, infer_expr_type, optional_element_type, stream_element_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSourceCapabilityPolicy {
    pub source_ty: JavaType,
    pub capability_element: Option<JavaType>,
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
        source_ty: source_ty.clone(),
        capability_element: facts.sequence_element_type_for(source_ty),
    }
}

pub fn infer_stream_lambda_body_shape(expr: &Expr, default: &RustSlotShape) -> RustSlotShape {
    if stream_lambda_body_is_borrowed_str(expr) {
        return borrowed_string_content_shape();
    }
    if stream_lambda_body_is_owned_string(expr) {
        return owned_string_value_shape();
    }
    if stream_lambda_body_is_i32(expr) {
        return RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int));
    }
    default.clone()
}

pub fn infer_stream_map_to_int_shape(_expr: &Expr) -> RustSlotShape {
    RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int))
}

pub fn infer_stream_source_element_shape(source: &Expr, env: &TypeEnv) -> Option<RustSlotShape> {
    infer_stream_source_element_shape_with_facts(source, env, &FactSet::default())
}

pub fn infer_stream_source_element_shape_with_facts(
    source: &Expr,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<RustSlotShape> {
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

fn stream_lambda_body_is_borrowed_str(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            name,
            args,
            ..
        } if is_borrowed_string_view_method(name, args.len())
    )
}

fn stream_lambda_body_is_owned_string(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            name,
            args,
            ..
        } if is_owned_string_result_method(name, args.len())
    )
}

fn stream_lambda_body_is_i32(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.len() == 1
            && matches!(
                receiver.as_ref(),
                Expr::Var(owner)
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "Integer"
                        && name == "parseInt"
            )
    ) || matches!(
        expr,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.len() == 1
            && matches!(
                receiver.as_ref(),
                Expr::Var(owner)
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "Math"
                        && name == "toIntExact"
            )
    )
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
        .or(capability.capability_element)
        .or_else(|| {
            let element_ty = optional_element_type(&source_ty)?;
            let capability =
                stream_source_capability_policy_with_effective_facts(&element_ty, facts);
            stream_element_type(&element_ty).or(capability.capability_element)
        })
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
