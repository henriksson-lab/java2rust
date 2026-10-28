use crate::effective_facts::EffectiveFacts;
use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, PrimitiveType};
use crate::shape_projection::iteration_element_shape_with_effective_facts;
use crate::slot_shape::RustSlotShape;
use crate::types::{TypeEnv, infer_expr_type, optional_element_type, stream_element_type};

/// Immediate stream creation and terminal traversal are different effects.
/// The same typed source classification also supplies cursor capabilities.
pub(crate) fn visit_retained_stream_sources(
    expr: &Expr,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    mut visit: impl FnMut(&Expr, RustSlotShape, bool),
) {
    let Some(facts) = ctx.facts() else {
        return;
    };
    crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |node| {
        if let Some(plan) =
            crate::stream::build_stream_plan_with_shape_context(node, env, ctx, facts)
        {
            let source = plan.source().expr();
            let parent = crate::expr_shape::infer_expr_shape_with_context(source, env, ctx);
            if crate::call_plan::retained_native_iteration_element_type(&parent, facts).is_some()
                || crate::stream::native_stored_stream_source(source, env.type_env(), facts)
            {
                visit(source, parent, true);
            }
        }
        if let Expr::Call {
            receiver: Some(source),
            name,
            args,
            ..
        } = node
            && matches!(name.as_str(), "map" | "filter")
            && args.len() == 1
            && crate::stream::native_stored_stream_source(source, env.type_env(), facts)
        {
            visit(
                source,
                crate::expr_shape::infer_expr_shape_with_context(source, env, ctx),
                true,
            );
            let callback = crate::expr_shape::infer_expr_shape_with_context(&args[0], env, ctx);
            if callback.optional_element_type().is_some()
                || matches!(&args[0], Expr::Literal(crate::java_ir::Literal::Null))
            {
                visit(&args[0], callback, false);
            }
        }
        if let Expr::Call {
            receiver: Some(owner),
            name,
            args,
            ..
        } = node
            && let Expr::Var(owner) = owner.as_ref()
            && ((name == "of"
                && crate::stdlib::canonical_external_receiver_type_key(owner) == "Stream")
                || (name == "stream"
                    && crate::stdlib::canonical_external_receiver_type_key(owner) == "Arrays"))
            && !crate::call_plan::java_type_owner_is_parsed_source(owner, env.type_env(), facts)
            && let [source] = args.as_slice()
        {
            let parent = crate::expr_shape::infer_expr_shape_with_context(source, env, ctx);
            let ty = parent
                .optional_element_type()
                .unwrap_or_else(|| parent.java_ty.clone());
            if matches!(ty, JavaType::Array(_)) {
                visit(source, parent, false);
            }
        }
        if let Expr::Call {
            receiver: Some(source),
            name,
            args,
            ..
        } = node
            && args.is_empty()
            && matches!(
                name.as_str(),
                "stream" | "parallelStream" | "parallel_stream"
            )
        {
            let parent = crate::expr_shape::infer_expr_shape_with_context(source, env, ctx);
            if crate::call_plan::retained_native_iteration_element_type(&parent, facts).is_some() {
                visit(source, parent, false);
            }
        }
    });
}

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
    // The resolver has already substituted a source declaration's generic
    // return type. A fallback external facade must not erase that information
    // (or make a non-sequence source method look like a sequence).
    if let Expr::Call {
        resolved: Some(call),
        ..
    } = source
        && crate::call_plan::java_type_owner_is_parsed_source(&call.declaring_type, env, facts)
    {
        let effective_facts = EffectiveFacts::new(facts);
        return infer_stream_source_element_shape_with_effective_facts(
            source,
            env,
            &effective_facts,
        );
    }
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

/// A solved source slot can refine element nullability without changing the
/// Java collection's declared type. Preserve that physical element contract
/// before planning callbacks, rather than repairing it in individual renderers.
pub(crate) fn infer_stream_source_element_shape_with_context(
    source: &Expr,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let source_shape = crate::expr_shape::infer_expr_shape_with_context(source, env, ctx);
    if let Some(element) = crate::shape_projection::retained_iteration_element_shape(
        &source_shape,
        facts,
        ctx.representations(),
    ) {
        return Some(element);
    }
    let declared = infer_stream_source_element_shape_with_facts(source, env.type_env(), facts)?;
    let effective_facts = EffectiveFacts::new(facts);
    if let Some(element) = stream_array_element_shape(&source_shape, &effective_facts) {
        return Some(element);
    }
    let source_ty = source_shape
        .optional_element_type()
        .unwrap_or_else(|| source_shape.java_ty.clone());
    let physical_element = stream_element_type(&source_ty)
        .or_else(|| stream_source_capability_policy(&source_ty, facts).into_capability_element());
    let mut element = match physical_element {
        Some(element) if element != declared.java_ty => {
            crate::shape_projection::iteration_element_shape(element, facts)
        }
        _ => declared,
    };
    element.inherit_native_collection_child(&source_shape, 0);
    Some(element)
}

pub fn infer_stream_source_element_shape_with_effective_facts(
    source: &Expr,
    env: &TypeEnv,
    facts: &EffectiveFacts,
) -> Option<RustSlotShape> {
    let source_shape = RustSlotShape::for_storage(infer_expr_type(source, env));
    if let Some(element) = stream_array_element_shape(&source_shape, facts) {
        return Some(element);
    }
    stream_source_element_type_with_effective_facts(source, env, facts)
        .map(|ty| iteration_element_shape_with_effective_facts(ty, facts))
}

fn stream_array_element_shape(
    source: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> Option<RustSlotShape> {
    let element = source.optional_element_type();
    let ty = element.as_ref().unwrap_or(&source.java_ty);
    if matches!(ty, JavaType::Array(_)) {
        crate::shape_projection::indexed_read_shape_for_access_with_effective_facts(source, facts)
    } else {
        None
    }
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
        ..
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
