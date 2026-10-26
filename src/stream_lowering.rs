use crate::call_plan::{
    native_java_instance_receiver_type_hint, plan_native_java_static_owner_call,
    resolved_call_arg_target_type,
};
use crate::callable_analysis::CallableAnalysis;
use crate::coercion::{
    BorrowProjectionUse, IndexToUsizeProjection, LoweredExpr, SequenceIterationProjection,
    StreamLambdaArgumentSlotPlan, StreamLambdaParamProjection, apply_clone_projection,
    arrays_stream_range_projection_with_result_context, borrowed_collection_flat_map_projection,
    current_receiver_method_call_projection, ensure_borrow_projection, field_projection,
    function_call_projection, index_to_usize_projection, indexed_access_projection,
    len_i32_projection, nullable_stream_map_body_projection,
    optional_non_null_filter_map_projection, optional_string_view_projection,
    owner_associated_path_projection, propagate_result_projection, receiver_method_call_projection,
    receiver_method_call_projection_exact, rust_string_literal_projection,
    self_associated_path_projection, sequence_iteration_source_projection,
    sequence_iteration_source_projection_with_result_context, shared_borrow_expr,
    stream_collect_to_map_projection, stream_filter_op_projection, stream_flat_map_op_projection,
    stream_join_terminal_projection, stream_lambda_argument_slot_plan, stream_lambda_param_expr,
    stream_map_op_projection_with_param, stream_match_terminal_projection,
    stream_prefixed_flat_map_op_projection, stream_reduce_terminal_projection,
    string_equality_operand_projection, typed_collect_projection,
    unwrap_java_null_receiver_projection,
};
use crate::conversion_lowering::apply_lowered_slot_conversion_to_expr;
use crate::conversion_plan::{
    LoweredType, Slot, effective_stream_item_shape, optional_element_slot_shape_with_facts,
    plan_lowered_slot_conversion, storage_shape_for_java_type,
};
use crate::expr_shape::{
    ExprShapeContext, ShapeEnv, idless_compatibility_local_plan_for_declared_type,
    infer_expr_shape_with_context,
};
use crate::facts::{ExternalFactLookupIndex, FactSet, ReceiverMutabilityFact};
use crate::java_ir::{BinaryOp, CallKind, Expr, JavaType, Literal, Stmt, SwitchLabel, UnaryOp};
use crate::literal_lowering::literal_to_rust;
use crate::method_reference_shape::method_reference_receiver_type_for_call_plan_with_context;
use crate::operator_lowering::{binary_op_to_rust, stream_unary_prefix};
use crate::rust_ir::{RustClosureBody, RustClosureParam, RustExpr, RustStmt};
use crate::shape_projection::{borrowed_string_content_shape, shape_is_borrowed_string_content};
use crate::signature::{LocalPlan, SignaturePlan};
use crate::slot_render::rust_type_for_slot_with_facts;
use crate::slot_shape::{OwnershipShape, RustSlotShape};
use crate::stream::{
    FilterPlan, FlatMapPlan, LambdaPlan, StreamLambdaBody, StreamOp, StreamPlan,
    StreamSourceIteration, StreamSourceRange, StreamSubPipeline, StreamTerminal,
};
use crate::stream_shape::infer_stream_source_element_shape_with_facts;
use crate::types::TypeEnv;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamLambdaValue {
    FilterBorrowed,
    BorrowedItem,
    OwnedItem,
}

fn stream_lambda_value_expr(value: StreamLambdaValue, param: &str) -> RustExpr {
    stream_lambda_param_expr(stream_lambda_param_projection(value), param, false)
}

fn stream_identity_value_projection(param_name: &str, element_shape: &RustSlotShape) -> RustExpr {
    if element_shape.is_shared_borrowed_value() {
        let borrowed = stream_lambda_value_expr(StreamLambdaValue::BorrowedItem, param_name);
        if element_shape.capabilities().is_copy() {
            borrowed
        } else {
            apply_clone_projection(borrowed)
        }
    } else {
        RustExpr::Var(param_name.to_string())
    }
}

fn stream_lambda_receiver_expr(value: StreamLambdaValue, param: &str) -> RustExpr {
    stream_lambda_param_expr(stream_lambda_param_projection(value), param, true)
}

fn stream_lambda_param_projection(value: StreamLambdaValue) -> StreamLambdaParamProjection {
    match value {
        StreamLambdaValue::FilterBorrowed => StreamLambdaParamProjection::FilterBorrowed,
        StreamLambdaValue::BorrowedItem => StreamLambdaParamProjection::BorrowedItem,
        StreamLambdaValue::OwnedItem => StreamLambdaParamProjection::OwnedItem,
    }
}

fn stream_lambda_borrowed_item_expr(expr: &RustExpr, param: &str) -> bool {
    expr == &stream_lambda_value_expr(StreamLambdaValue::BorrowedItem, param)
}

fn stream_compile_error_expr(message: &str) -> RustExpr {
    crate::unsupported::compile_error_expr(message)
}

#[derive(Debug, Clone, Copy)]
pub struct StreamRenderContext<'a> {
    env: Option<&'a ShapeEnv>,
    facts: Option<&'a FactSet>,
    external_index: Option<&'a ExternalFactLookupIndex<'a>>,
    signatures: Option<&'a SignaturePlan>,
    current_local_shapes: Option<&'a [LocalPlan]>,
    callable_analysis: Option<&'a CallableAnalysis<'a>>,
    current_type: Option<&'a str>,
    self_receiver_available: bool,
    propagate_java_exceptions: bool,
    single_use_owned_input: bool,
    representations: Option<&'a crate::representation::RepresentationPlan>,
}

impl<'a> StreamRenderContext<'a> {
    pub fn new(env: &'a ShapeEnv) -> Self {
        Self {
            env: Some(env),
            facts: None,
            external_index: None,
            signatures: None,
            current_local_shapes: None,
            callable_analysis: None,
            current_type: None,
            self_receiver_available: false,
            propagate_java_exceptions: false,
            single_use_owned_input: false,
            representations: None,
        }
    }

    pub fn new_with_facts(env: &'a ShapeEnv, facts: &'a FactSet) -> Self {
        Self {
            env: Some(env),
            facts: Some(facts),
            external_index: None,
            signatures: None,
            current_local_shapes: None,
            callable_analysis: None,
            current_type: None,
            self_receiver_available: false,
            propagate_java_exceptions: false,
            single_use_owned_input: false,
            representations: None,
        }
    }

    pub fn new_with_facts_and_self_receiver(
        env: &'a ShapeEnv,
        facts: &'a FactSet,
        signatures: &'a SignaturePlan,
        external_index: &'a ExternalFactLookupIndex<'a>,
        current_type: Option<&'a str>,
        self_receiver_available: bool,
        propagate_java_exceptions: bool,
    ) -> Self {
        Self {
            env: Some(env),
            facts: Some(facts),
            external_index: Some(external_index),
            signatures: Some(signatures),
            current_local_shapes: None,
            callable_analysis: None,
            current_type,
            self_receiver_available,
            propagate_java_exceptions,
            single_use_owned_input: false,
            representations: None,
        }
    }

    pub(crate) fn new_with_facts_analysis_and_self_receiver(
        env: &'a ShapeEnv,
        facts: &'a FactSet,
        signatures: &'a SignaturePlan,
        callable_analysis: &'a CallableAnalysis<'a>,
        external_index: &'a ExternalFactLookupIndex<'a>,
        current_type: Option<&'a str>,
        self_receiver_available: bool,
        propagate_java_exceptions: bool,
    ) -> Self {
        Self {
            env: Some(env),
            facts: Some(facts),
            external_index: Some(external_index),
            signatures: Some(signatures),
            current_local_shapes: None,
            callable_analysis: Some(callable_analysis),
            current_type,
            self_receiver_available,
            propagate_java_exceptions,
            single_use_owned_input: false,
            representations: None,
        }
    }

    pub fn empty() -> Self {
        Self {
            env: None,
            facts: None,
            external_index: None,
            signatures: None,
            current_local_shapes: None,
            callable_analysis: None,
            current_type: None,
            self_receiver_available: false,
            propagate_java_exceptions: false,
            single_use_owned_input: false,
            representations: None,
        }
    }

    fn with_env<'b>(self, env: &'b ShapeEnv) -> StreamRenderContext<'b>
    where
        'a: 'b,
    {
        StreamRenderContext {
            env: Some(env),
            facts: self.facts,
            external_index: self.external_index,
            signatures: self.signatures,
            current_local_shapes: self.current_local_shapes,
            callable_analysis: self.callable_analysis,
            current_type: self.current_type,
            self_receiver_available: self.self_receiver_available,
            propagate_java_exceptions: self.propagate_java_exceptions,
            single_use_owned_input: self.single_use_owned_input,
            representations: self.representations,
        }
    }

    pub(crate) fn with_local_shapes(mut self, locals: &'a [LocalPlan]) -> Self {
        self.current_local_shapes = Some(locals);
        self
    }

    pub(crate) fn with_representations(
        mut self,
        representations: &'a crate::representation::RepresentationPlan,
    ) -> Self {
        self.representations = Some(representations);
        self
    }

    fn bare_name_expr(self, name: &str, locals: &[&str]) -> RustExpr {
        if let Some((owner, member)) = name.rsplit_once('.')
            && is_static_constant_member(member)
        {
            let owner = stream_static_raw_owner_name(owner, self);
            return owner_associated_path_projection(&owner, member);
        }
        if let Some((owner, member)) = name.rsplit_once("::")
            && is_static_constant_member(member)
        {
            let owner = stream_static_raw_owner_name(owner, self);
            return owner_associated_path_projection(&owner, member);
        }
        if locals.iter().any(|local| *local == name) {
            if let Some(access_name) = self.env.and_then(|env| env.binding_access_name(name)) {
                return RustExpr::Var(access_name.to_string());
            }
            return RustExpr::Var(crate::parser::to_snake_case(name));
        }
        self.env
            .map(|env| crate::access_lowering::lower_bare_name(name, env))
            .unwrap_or_else(|| RustExpr::Var(crate::parser::to_snake_case(name)))
    }
}

impl Default for StreamRenderContext<'_> {
    fn default() -> Self {
        Self::empty()
    }
}

fn plan_stream_lambda_call_target<'a>(
    receiver: Option<&'a Expr>,
    name: &'a str,
    arg_count: usize,
    resolved: Option<&'a crate::java_ir::ResolvedCall>,
    facts: &FactSet,
    external_index: Option<&ExternalFactLookupIndex<'_>>,
    type_env: &crate::types::TypeEnv,
) -> crate::call_plan::CallTargetPlan<'a> {
    if let Some(external_index) = external_index {
        crate::call_plan::plan_call_target_with_external_index_and_arity(
            receiver,
            name,
            arg_count,
            resolved,
            facts,
            external_index,
            type_env,
        )
    } else {
        crate::call_plan::plan_call_target_with_facts_and_arity(
            receiver, name, arg_count, resolved, facts, type_env,
        )
    }
}

fn emit_stream_lambda_shared_native_call(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    resolved: Option<&crate::java_ir::ResolvedCall>,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<StreamLambdaRenderedExpr> {
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let mut typed_lambda_env = ctx
        .env
        .map(|env| env.type_env().clone())
        .unwrap_or_default();
    let type_env = if let Expr::Var(receiver_name) = receiver {
        if receiver_name == param {
            if let Some(param_shape) = param_shape {
                typed_lambda_env.insert_local(param.to_string(), param_shape.java_ty.clone());
                &typed_lambda_env
            } else if let Some(receiver_ty) =
                native_java_instance_receiver_type_hint(name, args.len())
            {
                typed_lambda_env.insert_local(param.to_string(), receiver_ty);
                &typed_lambda_env
            } else {
                &typed_lambda_env
            }
        } else if locals.iter().any(|local| *local == receiver_name)
            && let Some(receiver_ty) = native_java_instance_receiver_type_hint(name, args.len())
        {
            typed_lambda_env.insert_local(receiver_name.clone(), receiver_ty);
            &typed_lambda_env
        } else {
            &typed_lambda_env
        }
    } else {
        &typed_lambda_env
    };
    let target = plan_stream_lambda_call_target(
        Some(receiver),
        name,
        args.len(),
        resolved,
        facts,
        ctx.external_index,
        type_env,
    );
    let precedence = match target {
        crate::call_plan::CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::ObjectsEquals,
        }
        | crate::call_plan::CallTargetPlan::NativeJavaInstance {
            method: crate::call_plan::NativeJavaInstanceMethod::Equals,
            ..
        } => stream_binary_precedence(BinaryOp::Eq),
        crate::call_plan::CallTargetPlan::NativeJavaStatic {
            method: crate::call_plan::NativeJavaStaticMethod::MathToIntExact,
        }
        | crate::call_plan::CallTargetPlan::NativeJavaInstance {
            method: crate::call_plan::NativeJavaInstanceMethod::StringLen,
            ..
        } => 6,
        crate::call_plan::CallTargetPlan::NativeJavaStatic { .. }
        | crate::call_plan::CallTargetPlan::NativeJavaInstance { .. }
        | crate::call_plan::CallTargetPlan::NativeCollectionMethod { .. } => 11,
        _ => return None,
    };
    let native_equals_receiver_is_stream_param = matches!(
        (&target, receiver),
        (
            crate::call_plan::CallTargetPlan::NativeJavaInstance {
                method: crate::call_plan::NativeJavaInstanceMethod::Equals,
                ..
            },
            Expr::Var(receiver_name)
        ) if receiver_name == param && !matches!(value, StreamLambdaValue::OwnedItem)
    );
    let lowered_receiver = emit_stream_lambda_receiver_with_locals_expr(
        receiver,
        param,
        param_rust,
        value,
        locals,
        ctx,
        param_shape,
    );
    let lowered_args: Vec<RustExpr> = emit_stream_lambda_call_args_for_receiver_exprs(
        Some(receiver),
        name,
        args,
        resolved,
        param,
        param_rust,
        value,
        locals,
        ctx,
        param_shape,
    )?
    .into_iter()
    .enumerate()
    .map(|(index, arg)| {
        if native_equals_receiver_is_stream_param && index == 0 {
            shared_borrow_expr(arg.expr)
        } else {
            arg.expr
        }
    })
    .collect();
    let lowered_receiver = if native_equals_receiver_is_stream_param
        && param_shape.is_none_or(|shape| shape.optional_element_type().is_none())
    {
        Some(stream_lambda_value_expr(
            StreamLambdaValue::BorrowedItem,
            param_rust,
        ))
    } else {
        lowered_receiver
    };
    // A missing rendered operand is not evidence for a static call. Preserve
    // the target plan's receiver requirement and let unsupported operands
    // fall back, rather than passing None to an instance-only projection.
    let lowered_receiver = if target.receiver_expr().is_some() {
        Some(lowered_receiver?)
    } else {
        None
    };
    if matches!(
        target,
        crate::call_plan::CallTargetPlan::NativeJavaInstance {
            method: crate::call_plan::NativeJavaInstanceMethod::VirtualObjectToString,
            ..
        }
    ) {
        let shape = stream_lambda_arg_actual_shape(receiver, param, locals, ctx, param_shape)?;
        let is_param = matches!(receiver, Expr::Var(name) if name == param);
        let mut operand = lowered_receiver?;
        if is_param && matches!(value, StreamLambdaValue::FilterBorrowed) {
            // Filter lends an additional callback reference, not another
            // layer of Java storage or an independently reified Any value.
            operand = RustExpr::Unary {
                op: "*".to_string(),
                expr: Box::new(operand),
            };
        }
        let borrowed_storage = is_param && !matches!(value, StreamLambdaValue::OwnedItem);
        let result = crate::coercion::generic_object_string_result_projection(
            operand,
            &shape,
            borrowed_storage,
        );
        return Some(stream_lambda_expr(
            stream_result_boundary(result, ctx),
            precedence,
        ));
    }
    if let crate::call_plan::CallTargetPlan::NativeCollectionMethod { method, .. } = &target
        && let Some(shape) =
            stream_lambda_arg_actual_shape(receiver, param, locals, ctx, param_shape)
        && shape.native_allocation.shared()
        && let Some(kind) = crate::effective_facts::EffectiveFacts::new(facts)
            .retained_native_collection_kind(&shape.java_ty)
    {
        return Some(stream_lambda_expr(
            crate::lower::render_retained_native_collection_call(
                *method,
                lowered_receiver?,
                &lowered_args,
                &shape,
                kind,
                ctx.propagate_java_exceptions,
                None,
                ctx.representations.and_then(|representations| {
                    crate::shape_projection::retained_native_collection_result_shape(
                        &shape,
                        *method,
                        facts,
                        representations,
                    )
                }),
            )?,
            precedence,
        ));
    }
    let expr = if matches!(
        target,
        crate::call_plan::CallTargetPlan::NativeJavaInstance {
            method: crate::call_plan::NativeJavaInstanceMethod::Equals,
            ..
        }
    ) && args.len() == 1
        && let Some(argument_shape) =
            stream_lambda_arg_actual_shape(&args[0], param, locals, ctx, param_shape)
        && argument_shape.optional_element_type() == Some(JavaType::String)
        && let Some(receiver_shape) = ctx.env.map(|env| {
            infer_expr_shape_with_context(
                receiver,
                env,
                ExprShapeContext::new(ctx.signatures, None, ctx.facts),
            )
        })
        && receiver_shape.optional_element_type().is_none()
        && receiver_shape.java_ty == JavaType::String
        && let (Some(lowered_receiver), [lowered_argument]) =
            (&lowered_receiver, lowered_args.as_slice())
    {
        // The general stream call renderer lowers native `equals` to `==`,
        // but it previously compared the receiver's `&str` directly with an
        // emitted `Option<String>` local.  Compare matching borrowed views;
        // this preserves Java's false-for-null argument semantics without
        // dereferencing the nullable argument.
        RustExpr::Binary {
            op: "==".to_string(),
            left: Box::new(function_call_projection(
                RustExpr::path(["Some"]),
                vec![string_equality_operand_projection(
                    lowered_receiver.clone(),
                    shape_is_borrowed_string_content(&receiver_shape),
                )],
            )),
            right: Box::new(optional_string_view_projection(lowered_argument.clone())),
        }
    } else {
        if let Some(lowered_receiver) = lowered_receiver {
            stream_instance_receiver_call_projection(
                receiver,
                name,
                args,
                resolved,
                param,
                locals,
                ctx,
                param_shape,
                lowered_receiver,
                lowered_args,
                &target,
                |receiver, args| stream_call_target_projection(&target, Some(receiver), args, ctx),
            )
        } else {
            stream_call_target_projection(&target, None, lowered_args, ctx)
        }
    };
    Some(stream_lambda_expr(expr, precedence))
}

fn stream_call_target_projection(
    target: &crate::call_plan::CallTargetPlan<'_>,
    receiver: Option<RustExpr>,
    args: Vec<RustExpr>,
    ctx: StreamRenderContext<'_>,
) -> RustExpr {
    let result = crate::call_lowering::apply_call_target(target, receiver, args);
    if crate::call_plan::intrinsic_call_result_abi(target)
        == Some(crate::facts::ResultAbi::ResultWrapped)
    {
        stream_result_boundary(result, ctx)
    } else {
        result
    }
}

#[allow(clippy::too_many_arguments)]
fn stream_instance_receiver_call_projection(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    resolved: Option<&crate::java_ir::ResolvedCall>,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
    lowered_receiver: RustExpr,
    lowered_args: Vec<RustExpr>,
    target: &crate::call_plan::CallTargetPlan<'_>,
    call: impl FnOnce(RustExpr, Vec<RustExpr>) -> RustExpr,
) -> RustExpr {
    let native_functional = ctx.facts.and_then(|facts| {
        let shape = stream_lambda_arg_actual_shape(receiver, param, locals, ctx, param_shape)?;
        let ty = shape.optional_element_type().unwrap_or(shape.java_ty);
        let functional =
            crate::effective_facts::EffectiveFacts::new(facts).native_functional_carrier(&ty)?;
        (functional.method() == name && functional.inputs().len() == args.len())
            .then_some(functional)
    });
    let call = |receiver, args| {
        if let Some(functional) = native_functional {
            stream_result_boundary(
                receiver_method_call_projection_exact(
                    receiver,
                    &functional.result_method_name(),
                    args,
                ),
                ctx,
            )
        } else {
            call(receiver, args)
        }
    };
    let optional_shape = (target.adapts_optional_instance_receiver()
        || matches!(
            target,
            crate::call_plan::CallTargetPlan::NativeCollectionMethod { .. }
        ))
    .then(|| stream_lambda_arg_actual_shape(receiver, param, locals, ctx, param_shape))
    .flatten()
    .filter(|shape| shape.optional_element_type().is_some());
    let Some(option_shape) = optional_shape else {
        return call(lowered_receiver, lowered_args);
    };
    let element_ty = option_shape.optional_element_type().expect("checked above");
    let mutable = target.mutates_receiver()
        || matches!(target, crate::call_plan::CallTargetPlan::NativeCollectionMethod { method, .. } if method.mutates_receiver())
        || stream_source_call_signature(
            Some(receiver),
            name,
            args,
            resolved,
            param,
            param_shape,
            ctx,
        )
        .is_some_and(|(_, _, signature)| {
            signature.receiver() == crate::signature::ReceiverPlan::Mutable
        });
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let receiver_plan =
        crate::call_plan::optional_receiver_slot_plan(element_ty, mutable, false, facts);
    let projection = receiver_plan.projection_plan_for_option_shape(option_shape, facts);
    let staged_receiver = if mutable {
        crate::coercion::force_mutable_borrow_projection(lowered_receiver)
    } else {
        crate::coercion::force_shared_borrow_projection(lowered_receiver)
    };
    crate::coercion::staged_instance_call_projection(
        staged_receiver,
        lowered_args,
        |receiver, args| {
            let receiver =
                crate::conversion_lowering::apply_fallible_optional_receiver_slot_projection(
                    receiver,
                    receiver_plan.projection(),
                    projection.element_shape(),
                    projection.conversion(),
                );
            call(receiver, args)
        },
    )
}

#[derive(Debug, Clone)]
struct StreamLambdaRenderedExpr {
    expr: RustExpr,
    precedence: u8,
}

impl StreamLambdaRenderedExpr {
    fn expr(&self) -> &RustExpr {
        &self.expr
    }

    #[cfg(test)]
    fn rendered(&self) -> String {
        crate::emit::emit_expr(&self.expr)
    }
}

fn stream_lambda_expr(expr: RustExpr, precedence: u8) -> StreamLambdaRenderedExpr {
    StreamLambdaRenderedExpr { expr, precedence }
}

fn stream_callback_result_expr(body: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::generic_path(
            ["Ok"],
            vec![
                crate::rust_ir::RustType::Named("_".to_string()),
                crate::rust_ir::RustType::Named("JavaException".to_string()),
            ],
            None::<String>,
        ),
        vec![body],
    )
}

pub(crate) fn emit_stream_lambda_body_expr(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
) -> Option<RustExpr> {
    emit_stream_lambda_body_with_locals_prec(
        expr,
        param,
        param,
        value,
        &[param],
        StreamRenderContext::empty(),
        None,
        0,
        false,
    )
    .map(|lowered| lowered.expr)
}

#[cfg(test)]
fn emit_stream_lambda_body_with_context(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    emit_stream_lambda_body_with_locals(expr, param, value, &[param], ctx, None)
}

#[cfg(test)]
fn emit_stream_lambda_body_with_locals(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<String> {
    emit_stream_lambda_body_with_locals_prec(
        expr,
        param,
        param,
        value,
        locals,
        ctx,
        param_shape,
        0,
        false,
    )
    .map(|lowered| lowered.rendered())
}

fn emit_stream_lambda_body_with_locals_prec(
    expr: &Expr,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
    parent_prec: u8,
    right_child: bool,
) -> Option<StreamLambdaRenderedExpr> {
    let mut lowered = emit_stream_lambda_body_with_locals_prec_impl(
        expr,
        param,
        param_rust,
        value,
        locals,
        ctx,
        param_shape,
        parent_prec,
        right_child,
    )?;
    if let Expr::Call {
        receiver,
        name,
        args,
        resolved,
        ..
    } = expr
        && let Some((owner, rust_name, _)) = stream_source_call_signature(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            param,
            param_shape,
            ctx,
        )
        && ctx.callable_analysis.is_some_and(|analysis| {
            analysis.source_method_identity_returns_result(owner, rust_name)
        })
    {
        lowered.expr = stream_result_boundary(lowered.expr, ctx);
        lowered.precedence = 11;
    }
    Some(lowered)
}

fn emit_stream_lambda_body_with_locals_prec_impl(
    expr: &Expr,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
    parent_prec: u8,
    right_child: bool,
) -> Option<StreamLambdaRenderedExpr> {
    if let Some(env) = ctx.env
        && let Some(plan) =
            crate::string_concat_plan::build_string_concat_plan(expr, env.type_env())
    {
        let args = plan
            .args()
            .iter()
            .map(|arg| {
                emit_stream_lambda_body_with_locals_prec(
                    arg,
                    param,
                    param_rust,
                    value,
                    locals,
                    ctx,
                    param_shape,
                    0,
                    false,
                )
                .map(|lowered| lowered.expr)
            })
            .collect::<Option<Vec<_>>>()?;
        return parenthesize_stream_expr_if_needed(
            stream_lambda_expr(
                crate::string_concat_render::string_concat_expr(&plan, args),
                12,
            ),
            parent_prec,
            right_child,
            None,
        );
    }
    let lowered = match expr {
        Expr::This if ctx.self_receiver_available => {
            stream_lambda_expr(RustExpr::Var("self".to_string()), 12)
        }
        Expr::Literal(lit) => stream_lambda_expr(RustExpr::Literal(literal_to_rust(lit)), 12),
        Expr::Raw(raw) => {
            let expr = stream_raw_expr(raw, ctx);
            let precedence = crate::emit::expr_precedence(&expr);
            stream_lambda_expr(expr, precedence)
        }
        Expr::Var(name) if name == param => {
            let mut projected = stream_lambda_value_expr(value, param_rust);
            if let Some(shape) = param_shape
                && shape.capabilities().is_copy()
            {
                // Value projection has already removed the iterator's loan.
                // Passing its old borrowed slot to coercion would dereference
                // a second time. Record the physical Copy value we produced.
                let mut actual = shape.clone();
                actual.ownership = OwnershipShape::Owned;
                projected = RustExpr::shaped_as(
                    crate::rust_ir::RustExprShape::OwnedRecovery,
                    Some(actual),
                    projected,
                );
            }
            stream_lambda_expr(projected, 12)
        }
        Expr::Var(name) => stream_lambda_expr(ctx.bare_name_expr(name, locals), 12),
        Expr::Cast { ty, expr } => {
            let plan = stream_lambda_cast_plan(ty, expr, param, locals, ctx, param_shape)?;
            // Checked casts and primitive unboxing still use their existing
            // exception-aware fallback. Identity casts must retain the
            // cursor's solved physical carrier, including nullable T.
            if !matches!(
                plan.semantics(),
                crate::conversion_plan::JavaCastSemanticsPolicy::Identity
            ) {
                return None;
            }
            let inner = emit_stream_lambda_body_with_locals_prec(
                expr,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?;
            stream_lambda_expr(
                crate::conversion_lowering::apply_cast_conversion(inner.expr, &plan),
                12,
            )
        }
        Expr::Binary { op, left, right }
            if matches!(op, BinaryOp::Eq | BinaryOp::Ne)
                && (matches!(left.as_ref(), Expr::Literal(Literal::Null))
                    || matches!(right.as_ref(), Expr::Literal(Literal::Null))) =>
        {
            let null_on_left = matches!(left.as_ref(), Expr::Literal(Literal::Null));
            let operand = if null_on_left { right } else { left };
            let shape = stream_lambda_arg_actual_shape(operand, param, locals, ctx, param_shape)?;
            if shape.optional_element_type().is_none() {
                return None;
            }
            let operand = emit_stream_lambda_body_with_locals_prec(
                operand,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?;
            stream_lambda_expr(
                crate::operator_lowering::lower_null_comparison_expr(
                    *op,
                    operand.expr,
                    null_on_left,
                )?,
                11,
            )
        }
        Expr::Binary {
            op: BinaryOp::UShr, ..
        } => stream_lambda_expr(
            stream_compile_error_expr("unsupported Java unsigned right shift in stream lambda"),
            12,
        ),
        Expr::Binary { op, left, right } => {
            let prec = stream_binary_precedence(*op);
            let mut numeric_env = ctx
                .env
                .map(|env| env.type_env().clone())
                .unwrap_or_default();
            if let Some(shape) = param_shape {
                // Primitive lambda parameters are represented as values. A
                // non-null boxed primitive has that same physical carrier;
                // nullable unboxing remains a separate checked operation.
                let mut ty = shape.java_ty.clone();
                if let JavaType::Named { name, args } = &ty
                    && args.is_empty()
                    && let Some(primitive) = crate::stdlib::boxed_primitive_type(name)
                {
                    ty = JavaType::Primitive(primitive);
                }
                numeric_env.insert_local(param, ty);
            }
            let numeric_plan = crate::operator_lowering::plan_numeric_binary_promotion(
                *op,
                &crate::types::infer_expr_type(left, &numeric_env),
                &crate::types::infer_expr_type(right, &numeric_env),
            );
            let left = emit_stream_lambda_body_with_locals_prec(
                left,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
                prec,
                false,
            )?;
            let right = emit_stream_lambda_body_with_locals_prec(
                right,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
                prec,
                true,
            )?;
            if let Some(plan) = numeric_plan {
                let expr = if let Some(checked) =
                    crate::operator_lowering::lower_checked_java_integer_div_rem_expr(
                        *op,
                        left.expr.clone(),
                        right.expr.clone(),
                        plan.clone(),
                    ) {
                    stream_result_boundary(checked, ctx)
                } else {
                    crate::operator_lowering::lower_numeric_binary_expr(
                        *op, left.expr, right.expr, plan,
                    )
                };
                let precedence = crate::emit::expr_precedence(&expr);
                stream_lambda_expr(expr, precedence)
            } else {
                stream_lambda_expr(stream_binary_expr_projection(*op, left, right), prec)
            }
        }
        Expr::Unary { op, expr } => {
            let op = stream_unary_prefix(*op)?;
            let expr = emit_stream_lambda_body_with_locals_prec(
                expr,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
                10,
                false,
            )?;
            stream_lambda_expr(stream_unary_expr_projection(op, expr), 10)
        }
        Expr::FieldAccess(receiver, field) => {
            if is_static_constant_member(field)
                && let Some(owner) = stream_static_owner_expr_type_name(receiver, locals, ctx)
                    .or_else(|| stream_static_constant_owner_name(receiver, locals, ctx))
            {
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(owner_associated_path_projection(&owner, field), 12),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if let Some((expr, own_prec)) = emit_stream_lambda_member_access_through_plan_expr(
                receiver,
                field,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            ) {
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(expr, own_prec),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if field == "length" {
                let receiver = emit_stream_lambda_receiver_with_locals_expr(
                    receiver,
                    param,
                    param_rust,
                    value,
                    locals,
                    ctx,
                    param_shape,
                )?;
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(len_i32_projection(receiver), 6),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            let receiver = emit_stream_lambda_receiver_with_locals_expr(
                receiver,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            )?;
            stream_lambda_expr(
                field_projection(receiver, &crate::parser::to_snake_case(field)),
                11,
            )
        }
        Expr::ArrayAccess(array, index) => {
            let array = emit_stream_lambda_receiver_with_locals_expr(
                array,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            )?;
            let index = emit_stream_lambda_body_with_locals_prec(
                index,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?
            .expr;
            stream_lambda_expr(
                indexed_access_projection(
                    array,
                    index,
                    IndexToUsizeProjection::StreamLambdaArgument,
                ),
                11,
            )
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            resolved,
            ..
        } => {
            if args.is_empty()
                && let Some(env) = ctx.env
                && let Some((index, shape)) =
                    crate::expr_shape::native_map_entry_accessor_shape(receiver, name, env)
            {
                let receiver = emit_stream_lambda_receiver_with_locals_expr(
                    receiver,
                    param,
                    param_rust,
                    value,
                    locals,
                    ctx,
                    param_shape,
                )?;
                return Some(stream_lambda_expr(
                    RustExpr::shaped_as(
                        crate::rust_ir::RustExprShape::SlotAdapted,
                        Some(shape),
                        crate::coercion::tuple_field_projection(receiver, index),
                    ),
                    12,
                ));
            }
            if let Some(rendered) = emit_stream_lambda_shared_native_call(
                receiver,
                name,
                args,
                resolved.as_ref(),
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            ) {
                return parenthesize_stream_expr_if_needed(
                    rendered,
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if let Some(resolved) = resolved
                && matches!(resolved.kind, CallKind::Static)
            {
                let lowered_args: Vec<_> = emit_stream_lambda_call_args_for_receiver_exprs(
                    Some(receiver),
                    name,
                    args,
                    Some(resolved),
                    param,
                    param_rust,
                    value,
                    locals,
                    ctx,
                    param_shape,
                )?
                .into_iter()
                .map(|arg| arg.expr)
                .collect();
                if let Some(expr) = stream_native_static_owner_call_expr(
                    &resolved.declaring_type,
                    name,
                    args,
                    lowered_args.clone(),
                    ctx,
                ) {
                    return parenthesize_stream_expr_if_needed(
                        stream_lambda_expr(expr, 11),
                        parent_prec,
                        right_child,
                        None,
                    );
                }
                let expr = function_call_projection(
                    owner_associated_path_projection(
                        &stream_static_owner_type_name_with_context(&resolved.declaring_type, ctx),
                        &resolved.rust_name,
                    ),
                    lowered_args,
                );
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(expr, 11),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if let Some(owner) = stream_static_owner_expr_type_name(receiver, locals, ctx) {
                let lowered_args: Vec<_> = emit_stream_lambda_call_args_for_receiver_exprs(
                    Some(receiver),
                    name,
                    args,
                    None,
                    param,
                    param_rust,
                    value,
                    locals,
                    ctx,
                    param_shape,
                )?
                .into_iter()
                .map(|arg| arg.expr)
                .collect();
                if let Some(owner_name) = stream_static_owner_expr_java_name(receiver, locals, ctx)
                    && let Some(expr) = stream_native_static_owner_call_expr(
                        &owner_name,
                        name,
                        args,
                        lowered_args.clone(),
                        ctx,
                    )
                {
                    return parenthesize_stream_expr_if_needed(
                        stream_lambda_expr(expr, 11),
                        parent_prec,
                        right_child,
                        None,
                    );
                }
                let expr = function_call_projection(
                    owner_associated_path_projection(&owner, &crate::parser::to_snake_case(name)),
                    lowered_args,
                );
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(expr, 11),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if matches!(name.as_str(), "newBuilder" | "new_builder")
                && args.is_empty()
                && let Expr::Var(receiver_name) = receiver.as_ref()
                && ctx.env.is_none_or(|env| {
                    env.local_type(receiver_name).is_none()
                        && env.field_type(receiver_name).is_none()
                })
            {
                let owner = stream_static_owner_type_name_with_context(receiver_name, ctx);
                let expr = function_call_projection(
                    owner_associated_path_projection(&owner, "new_builder"),
                    Vec::new(),
                );
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(expr, 11),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            let lowered_args = emit_stream_lambda_call_args_for_receiver_exprs(
                Some(receiver),
                name,
                args,
                resolved.as_ref(),
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            )?
            .into_iter()
            .map(|arg| arg.expr)
            .collect();
            let lowered_receiver = emit_stream_lambda_receiver_with_locals_expr(
                receiver,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            )?;
            let selected_source = stream_source_call_signature(
                Some(receiver),
                name,
                args,
                resolved.as_ref(),
                param,
                param_shape,
                ctx,
            );
            let method = selected_source
                .as_ref()
                .map(|(_, rust_name, _)| rust_name.as_str())
                .or_else(|| {
                    resolved
                        .as_ref()
                        .filter(|call| call.kind == CallKind::Instance)
                        .map(|call| call.rust_name.as_str())
                })
                .unwrap_or(name);
            let default_facts;
            let facts = match ctx.facts {
                Some(facts) => facts,
                None => {
                    default_facts = FactSet::default();
                    &default_facts
                }
            };
            let mut types = ctx
                .env
                .map(|env| env.type_env().clone())
                .unwrap_or_default();
            if let Some(shape) = param_shape {
                types.insert_local(param.to_string(), shape.java_ty.clone());
            }
            let target = plan_stream_lambda_call_target(
                Some(receiver),
                name,
                args.len(),
                resolved.as_ref(),
                facts,
                ctx.external_index,
                &types,
            );
            stream_lambda_expr(
                stream_instance_receiver_call_projection(
                    receiver,
                    name,
                    args,
                    resolved.as_ref(),
                    param,
                    locals,
                    ctx,
                    param_shape,
                    lowered_receiver,
                    lowered_args,
                    &target,
                    |receiver, args| receiver_method_call_projection(receiver, method, args),
                ),
                11,
            )
        }
        Expr::Call {
            receiver: None,
            name,
            args,
            resolved,
            ..
        } => {
            let lowered_args = emit_stream_lambda_call_args_for_resolved_exprs(
                name,
                args,
                resolved.as_ref(),
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            )?;
            let call_args = lowered_args
                .into_iter()
                .map(|arg| arg.expr)
                .collect::<Vec<_>>();
            let source_signature = stream_source_call_signature(
                None,
                name,
                args,
                resolved.as_ref(),
                param,
                param_shape,
                ctx,
            );
            let method_name = source_signature
                .as_ref()
                .map(|(_, rust_name, _)| rust_name.clone())
                .or_else(|| resolved.as_ref().map(|resolved| resolved.rust_name.clone()))
                .unwrap_or_else(|| name.clone());
            let method = crate::parser::to_snake_case(method_name.as_str());
            if let Some((owner, _, signature)) = source_signature
                && matches!(signature.receiver(), crate::signature::ReceiverPlan::None)
            {
                return Some(stream_lambda_expr(
                    function_call_projection(
                        owner_associated_path_projection(
                            &stream_static_raw_owner_name(&owner, ctx),
                            &method,
                        ),
                        call_args,
                    ),
                    11,
                ));
            }
            match resolved.as_ref().map(|resolved| resolved.kind) {
                Some(CallKind::Instance) => stream_lambda_expr(
                    current_receiver_method_call_projection(&method, call_args),
                    11,
                ),
                Some(CallKind::Static) => stream_lambda_expr(
                    function_call_projection(self_associated_path_projection(&method), call_args),
                    11,
                ),
                None if ctx.self_receiver_available
                    && ctx.env.is_none_or(|env| {
                        env.type_env().static_member_import_owner(name).is_none()
                            && env
                                .type_env()
                                .static_member_import_owner(&crate::parser::to_snake_case(name))
                                .is_none()
                    }) =>
                {
                    stream_lambda_expr(
                        current_receiver_method_call_projection(&method, call_args),
                        11,
                    )
                }
                None => stream_lambda_expr(
                    function_call_projection(RustExpr::Var(method), call_args),
                    11,
                ),
            }
        }
        Expr::New {
            ty, args, resolved, ..
        } => {
            let rust_name = resolved
                .as_ref()
                .map(|resolved| resolved.rust_name.clone())
                .or_else(|| {
                    ctx.facts.map(|facts| {
                        crate::call_plan::unresolved_constructor_rust_name_for_facts(
                            ty,
                            facts,
                            args.len(),
                        )
                    })
                })
                .unwrap_or_else(|| "new".to_string());
            let owner = stream_lambda_type_name(ty)?;
            let lowered_args = emit_stream_lambda_args_with_locals_prec(
                args,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            )?
            .into_iter()
            .map(|arg| arg.expr)
            .collect();
            stream_lambda_expr(
                function_call_projection(
                    owner_associated_path_projection(&owner, &rust_name),
                    lowered_args,
                ),
                11,
            )
        }
        _ => return None,
    };
    parenthesize_stream_expr_if_needed(
        lowered,
        parent_prec,
        right_child,
        match expr {
            Expr::Binary { op, .. } => Some(*op),
            _ => None,
        },
    )
}

fn emit_stream_lambda_member_access_through_plan_expr(
    receiver: &Expr,
    field: &str,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<(RustExpr, u8)> {
    let env = ctx.env?;
    let mut typed_lambda_env = env.type_env().clone();
    if let Some(param_shape) = param_shape {
        typed_lambda_env.insert_local(param.to_string(), param_shape.java_ty.clone());
    }

    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let plan = crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        field,
        &typed_lambda_env,
        facts,
    )?;
    let own_prec = match plan.op() {
        crate::member_access_plan::MemberAccessOp::LenAsI32
        | crate::member_access_plan::MemberAccessOp::Array2RowsAsI32
        | crate::member_access_plan::MemberAccessOp::Array2ColsAsI32 => 6,
        crate::member_access_plan::MemberAccessOp::StaticConstant { .. }
        | crate::member_access_plan::MemberAccessOp::MappedStaticValue { .. } => 12,
        crate::member_access_plan::MemberAccessOp::RustField { .. }
        | crate::member_access_plan::MemberAccessOp::RustGetter { .. } => 11,
    };
    let lowered_receiver = emit_stream_lambda_receiver_with_locals_expr(
        receiver,
        param,
        param_rust,
        value,
        locals,
        ctx,
        param_shape,
    )?;
    Some((
        crate::access_lowering::lower_field_access_with_facts(
            receiver,
            field,
            &typed_lambda_env,
            lowered_receiver,
            facts,
        ),
        own_prec,
    ))
}

fn stream_raw_expr(raw: &str, ctx: StreamRenderContext<'_>) -> RustExpr {
    if let Some((owner, member)) = raw_java_static_member_access(raw) {
        if let Some(owner) = stream_known_static_raw_owner_name(owner, ctx) {
            return owner_associated_path_projection(&owner, member);
        }
    }
    crate::coercion::recover_operator_expr_or_raw_leaf(raw)
}

fn stream_known_static_raw_owner_name(owner: &str, ctx: StreamRenderContext<'_>) -> Option<String> {
    if let Some(alias) = ctx
        .env
        .and_then(|env| env.type_env().static_type_alias(owner))
    {
        return Some(stream_static_owner_type_name_with_context(alias, ctx));
    }

    let canonical = crate::stdlib::canonical_external_receiver_type_key(owner);
    if canonical != owner || is_known_builtin_static_owner(&canonical) {
        return Some(stream_static_owner_type_name_with_context(owner, ctx));
    }

    None
}

fn stream_static_raw_owner_name(owner: &str, ctx: StreamRenderContext<'_>) -> String {
    if let Some(alias) = ctx
        .env
        .and_then(|env| env.type_env().static_type_alias(owner))
    {
        stream_static_owner_type_name_with_context(alias, ctx)
    } else {
        stream_static_owner_type_name_with_context(owner, ctx)
    }
}

fn raw_java_static_member_access(raw: &str) -> Option<(&str, &str)> {
    let (owner, member) = raw.rsplit_once('.')?;
    if !java_static_owner_path(owner) || !rust_identifier_part(member) {
        return None;
    }
    Some((owner, member))
}

fn is_known_builtin_static_owner(owner: &str) -> bool {
    matches!(
        owner,
        "Arrays"
            | "Collections"
            | "Collectors"
            | "Constants"
            | "Files"
            | "Integer"
            | "Math"
            | "Objects"
            | "StrictMath"
            | "System"
            | "Thread"
    )
}

fn java_static_owner_path(owner: &str) -> bool {
    owner
        .rsplit('.')
        .next()
        .is_some_and(java_static_owner_ident)
        && owner.split('.').all(rust_identifier_part)
}

fn java_static_owner_ident(owner: &str) -> bool {
    owner
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        && rust_identifier_part(owner)
}

fn rust_identifier_part(part: &str) -> bool {
    let mut chars = part.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

pub fn render_stream_plan_with_context(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    ctx: StreamRenderContext<'_>,
) -> Option<LoweredExpr> {
    render_stream_plan_with_optional_sequence_context(
        plan,
        lowered_source,
        ctx,
        plan.source().optional_sequence(),
        None,
    )
}

pub(crate) fn render_stream_plan_with_optional_sequence_context(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    ctx: StreamRenderContext<'_>,
    source_optional_sequence: bool,
    materialization_target: Option<&RustSlotShape>,
) -> Option<LoweredExpr> {
    if let Some(lowered) = render_stream_plan_structured(
        plan,
        lowered_source.clone(),
        ctx,
        source_optional_sequence,
        materialization_target,
    ) {
        return Some(lowered);
    }
    if let Some(lowered) = render_no_op_stream_plan_structured(
        plan,
        lowered_source.clone(),
        source_optional_sequence,
        materialization_target,
    ) {
        return Some(lowered);
    }
    None
}

fn render_stream_plan_structured(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    ctx: StreamRenderContext<'_>,
    source_optional_sequence: bool,
    materialization_target: Option<&RustSlotShape>,
) -> Option<LoweredExpr> {
    let source = stream_source_range_projection(lowered_source, plan.source().range(), ctx);
    let retained_parent = ctx.env.zip(ctx.facts).and_then(|(env, facts)| {
        let parent = infer_expr_shape_with_context(
            plan.source().expr(),
            env,
            ExprShapeContext::new(ctx.signatures, ctx.representations, Some(facts)),
        );
        crate::call_plan::retained_native_iteration_element_type(&parent, facts).map(|_| parent)
    });
    let checked_source = retained_parent.is_some();
    let mut expr = if let Some(parent) = &retained_parent {
        let stream = crate::coercion::retained_native_stream_projection(
            source,
            parent,
            ctx.propagate_java_exceptions,
        );
        stream_result_boundary(
            receiver_method_call_projection_exact(stream, "into_iter_once_result", Vec::new()),
            ctx,
        )
    } else {
        sequence_iteration_source_projection_with_result_context(
            source,
            source_optional_sequence,
            sequence_iteration_projection(plan.source().iteration()),
            ctx.propagate_java_exceptions,
        )
    };
    let (next_expr, item_is_borrowed, item_shape, item_is_result) = apply_stream_ops_structured(
        expr,
        plan.ops(),
        ctx,
        StreamRenderOptions {
            input_is_result: checked_source,
            ..StreamRenderOptions::default()
        },
        !checked_source && matches!(plan.source().iteration(), StreamSourceIteration::Borrowed),
        effective_stream_item_shape(
            plan.source_element().shape(),
            !checked_source && matches!(plan.source().iteration(), StreamSourceIteration::Borrowed),
        ),
    )?;
    if let Some(collected) = render_retained_collection_terminal(
        plan,
        next_expr.clone(),
        &item_shape,
        item_is_result,
        ctx,
        materialization_target,
    ) {
        return Some(collected);
    }
    expr = render_stream_terminal_expr(
        next_expr,
        plan,
        item_is_borrowed,
        &item_shape,
        item_is_result,
        ctx,
        materialization_target,
    )?;
    let actual = stream_terminal_actual_after_rendered_ops(
        plan,
        item_is_borrowed,
        &item_shape,
        materialization_target,
    );
    Some(LoweredExpr::new(expr, actual))
}

fn render_retained_collection_terminal(
    plan: &StreamPlan<'_>,
    input: RustExpr,
    item_shape: &RustSlotShape,
    item_is_result: bool,
    ctx: StreamRenderContext<'_>,
    target: Option<&RustSlotShape>,
) -> Option<LoweredExpr> {
    let target = target.filter(|target| target.native_allocation.shared())?;
    let facts = ctx.facts?;
    let StreamTerminal::Collect(kind) = plan.terminal() else {
        return None;
    };
    let target_kind = crate::effective_facts::EffectiveFacts::new(facts)
        .retained_native_collection_kind(&target.java_ty)?;
    if !matches!(
        kind,
        crate::stdlib::JavaCollectionKind::List
            | crate::stdlib::JavaCollectionKind::Set
            | crate::stdlib::JavaCollectionKind::InsertionOrderedSet
    ) || !(*kind == target_kind || (kind.is_set_like() && target_kind.is_set_like()))
    {
        return None;
    }
    let child_ty = RustSlotShape::native_child_type_at_path(&target.java_ty, &[0])?;
    let mut child = ctx
        .representations
        .map(|reps| crate::shape_projection::storage_shape_for_java_type(reps, child_ty.clone()))
        .unwrap_or_else(|| storage_shape_for_java_type(child_ty));
    child.inherit_native_collection_child(target, 0);
    child.ownership = OwnershipShape::Owned;
    let mut owned = item_shape.clone();
    owned.ownership = OwnershipShape::Owned;
    let value = stream_identity_value_projection("__java_collected_item", item_shape);
    let conversion = plan_lowered_slot_conversion(
        &LoweredType::Slot(Box::new(owned)),
        Slot::Assignment(child),
        facts,
    );
    let value = apply_lowered_slot_conversion_to_expr(value, &conversion);
    let list = *kind == crate::stdlib::JavaCollectionKind::List;
    let empty = function_call_projection(RustExpr::path(["Vec", "new"]), Vec::new());
    let initial = if list {
        function_call_projection(
            RustExpr::path(["JavaNativeCollection", "list_with_empty_add_policy"]),
            vec![
                empty,
                RustExpr::Literal("true".into()),
                RustExpr::Literal(plan.collection_empty_add_changes().to_string()),
            ],
        )
    } else {
        function_call_projection(
            RustExpr::path(["JavaNativeCollection", "set"]),
            vec![empty, RustExpr::path(["JavaMapKey", "java_map_key_eq"])],
        )
    };
    let insert = receiver_method_call_projection_exact(
        RustExpr::Var("__java_collected".into()),
        if list { "list_add_result" } else { "insert" },
        vec![value],
    );
    let item = RustExpr::Var("__java_collected_input".into());
    let block = RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: true,
                name: "__java_collected".into(),
                ty: None,
                value: Some(initial),
            },
            RustStmt::For {
                label: None,
                pattern: "__java_collected_input".into(),
                iterable: input,
                body: vec![
                    RustStmt::Let {
                        mutable: false,
                        name: "__java_collected_item".into(),
                        ty: None,
                        value: Some(if item_is_result {
                            stream_result_boundary(item, ctx)
                        } else {
                            item
                        }),
                    },
                    RustStmt::Expr(if list {
                        stream_result_boundary(insert, ctx)
                    } else {
                        insert
                    }),
                ],
            },
        ],
        tail: Some(Box::new(RustExpr::Var("__java_collected".into()))),
    };
    let allocated =
        crate::slot_render::storage_placement_construction_expr(target.native_allocation, block);
    let mut produced = target.clone();
    produced.java_ty = target
        .optional_element_type()
        .unwrap_or_else(|| target.java_ty.clone());
    produced.ownership = OwnershipShape::Owned;
    Some(LoweredExpr::new(
        allocated,
        LoweredType::Slot(Box::new(produced)),
    ))
}

fn apply_stream_ops_structured(
    mut expr: RustExpr,
    ops: &[StreamOp<'_>],
    ctx: StreamRenderContext<'_>,
    options: StreamRenderOptions,
    mut item_is_borrowed: bool,
    mut item_shape: RustSlotShape,
) -> Option<(RustExpr, bool, RustSlotShape, bool)> {
    let mut item_is_result = options.input_is_result;
    for op in ops {
        // Flat-map still needs a nested fallible traversal contract. Ordinary
        // maps and filters below lift over Result without evaluating errors.
        if item_is_result && matches!(op, StreamOp::FlatMap(_)) {
            return None;
        }
        let callback = match op {
            StreamOp::Filter(FilterPlan::Predicate(lambda))
            | StreamOp::Map { lambda, .. }
            | StreamOp::MapToPrimitive { lambda, .. } => Some(lambda),
            _ => None,
        };
        let capture = callback.and_then(|lambda| stream_bound_receiver_capture(lambda, ctx));
        let callback_ctx = capture
            .as_ref()
            .map(|capture| ctx.with_env(&capture.env))
            .unwrap_or(ctx);
        let input = capture.as_ref().map(|_| expr.clone());
        if capture.is_some() {
            expr = RustExpr::Var("__java_bound_stream".to_string());
        }
        match op {
            StreamOp::Filter(FilterPlan::Predicate(lambda)) => {
                let lambda = capture
                    .as_ref()
                    .map(|capture| &capture.lambda)
                    .unwrap_or(lambda);
                // A filter loans its element even when the upstream cursor
                // transfers owned values. Keep that loan in the callback's
                // slot contract without changing the element it forwards.
                let predicate_shape = effective_stream_item_shape(&item_shape, true);
                let body = emit_stream_lambda_closure_body_expr(
                    lambda,
                    if item_is_borrowed {
                        StreamLambdaValue::FilterBorrowed
                    } else {
                        StreamLambdaValue::BorrowedItem
                    },
                    callback_ctx,
                    Some(&predicate_shape),
                )?;
                if item_is_result || body.has_unbound_try() {
                    expr = crate::coercion::fallible_stream_filter_op_projection(
                        expr,
                        lambda.rust_param_name(),
                        body,
                        item_is_result,
                        options.move_filter_closures,
                    );
                    item_is_result = true;
                } else {
                    expr = stream_filter_op_projection(
                        expr,
                        lambda.rust_param_name(),
                        body,
                        options.move_filter_closures,
                    );
                }
            }
            StreamOp::Filter(FilterPlan::NonNull { .. }) => {
                // A solved callback return can carry null even when the
                // provisional Java pipeline type did not record it. Refine
                // the actual payload, using the same projection as planning.
                if item_shape.optional_element_type().is_some() {
                    expr = if item_is_result {
                        crate::coercion::fallible_stream_non_null_filter_projection(
                            expr,
                            item_is_borrowed,
                        )
                    } else {
                        optional_non_null_filter_map_projection(expr, item_is_borrowed)
                    };
                    item_shape = crate::stream::non_null_filter_output_shape(&item_shape);
                }
            }
            StreamOp::Map {
                lambda,
                output_shape,
            }
            | StreamOp::MapToPrimitive {
                lambda,
                output_shape,
            } => {
                let lambda = capture
                    .as_ref()
                    .map(|capture| &capture.lambda)
                    .unwrap_or(lambda);
                let ctx = callback_ctx;
                let value = if item_is_borrowed {
                    StreamLambdaValue::BorrowedItem
                } else {
                    StreamLambdaValue::OwnedItem
                };
                let param_shape = effective_stream_item_shape(&item_shape, item_is_borrowed);
                let mut effective_output_shape = if matches!(op, StreamOp::Map { .. }) {
                    render_context_lambda_output_shape(lambda, &param_shape, ctx)
                        .unwrap_or_else(|| output_shape.clone())
                } else {
                    output_shape.clone()
                };
                let param =
                    render_stream_lambda_param(&lambda.rust_param_name(), &param_shape, ctx);
                let mut body =
                    emit_stream_lambda_closure_body_expr(lambda, value, ctx, Some(&param_shape))?;
                let input_is_result = item_is_result;
                item_is_result |= body.has_unbound_try();
                if matches!(op, StreamOp::MapToPrimitive { .. })
                    && let Some(actual) =
                        render_context_lambda_output_shape(lambda, &param_shape, ctx)
                    && (actual.java_ty != effective_output_shape.java_ty
                        || actual.ownership == OwnershipShape::SharedBorrow)
                {
                    let default_facts = FactSet::default();
                    let conversion = plan_lowered_slot_conversion(
                        &LoweredType::Slot(Box::new(actual)),
                        Slot::Return(effective_output_shape.clone()),
                        ctx.facts.unwrap_or(&default_facts),
                    );
                    body = apply_lowered_slot_conversion_to_expr(body, &conversion);
                }
                if effective_output_shape.ownership == OwnershipShape::SharedBorrow {
                    // A map closure owns its input item. A source helper may
                    // expose a borrowed view into that item, but the view
                    // cannot escape the closure. Materialize through the same
                    // slot conversion used at ordinary owned boundaries.
                    let actual = effective_output_shape.clone();
                    effective_output_shape.ownership = OwnershipShape::Owned;
                    effective_output_shape.view = crate::slot_shape::ViewShape::Native;
                    let default_facts = FactSet::default();
                    let conversion = plan_lowered_slot_conversion(
                        &LoweredType::Slot(Box::new(actual)),
                        Slot::Return(effective_output_shape.clone()),
                        ctx.facts.unwrap_or(&default_facts),
                    );
                    body = apply_lowered_slot_conversion_to_expr(body, &conversion);
                }
                if item_is_result {
                    body = stream_callback_result_expr(body);
                }
                expr = if input_is_result {
                    crate::coercion::stream_result_map_op_projection(expr, param, body)
                } else {
                    stream_map_op_projection_with_param(expr, param, body)
                };
                item_is_borrowed = false;
                item_shape = effective_output_shape;
            }
            StreamOp::FlatMap(FlatMapPlan::CollectionStreamMethodReference { param_name }) => {
                expr = borrowed_collection_flat_map_projection(expr, param_name);
                item_is_borrowed = true;
                item_shape = collection_flat_map_element_shape(&item_shape, ctx)?;
            }
            StreamOp::FlatMap(FlatMapPlan::LambdaCollectionStream(plan)) => {
                let param_shape = effective_stream_item_shape(&item_shape, item_is_borrowed);
                let lambda = plan.lambda();
                let (prefix_stmts, _, _) = emit_stream_lambda_prefix_stmts_and_locals(
                    lambda.body(),
                    lambda.param_name(),
                    lambda.rust_param_name(),
                    ctx,
                    StreamLambdaValue::BorrowedItem,
                    Some(&param_shape),
                )?;
                let source = emit_stream_lambda_receiver_expr(
                    plan.source(),
                    lambda.param_name(),
                    lambda.rust_param_name(),
                    StreamLambdaValue::BorrowedItem,
                    ctx,
                )?;
                let iter_source = sequence_iteration_source_projection(
                    source,
                    stream_source_optional_sequence_for_render(
                        plan.source(),
                        plan.optional_sequence(),
                        ctx,
                    ),
                    sequence_iteration_projection(plan.iteration()),
                );
                expr = if prefix_stmts.is_empty() {
                    stream_flat_map_op_projection(expr, lambda.rust_param_name(), iter_source)
                } else {
                    stream_prefixed_flat_map_op_projection(
                        expr,
                        lambda.rust_param_name(),
                        prefix_stmts,
                        iter_source,
                    )
                };
                item_is_borrowed = matches!(plan.iteration(), StreamSourceIteration::Borrowed);
                item_shape = effective_stream_item_shape(
                    &lambda_collection_flat_map_element_shape(plan, &param_shape, ctx)?,
                    item_is_borrowed,
                );
            }
            StreamOp::FlatMap(FlatMapPlan::LambdaStreamChain { lambda, pipeline }) => {
                let param_shape = effective_stream_item_shape(&item_shape, item_is_borrowed);
                let (prefix_stmts, _, scoped_env) = emit_stream_lambda_prefix_stmts_and_locals(
                    lambda.body(),
                    &lambda.param_name(),
                    &lambda.rust_param_name(),
                    ctx,
                    StreamLambdaValue::BorrowedItem,
                    Some(&param_shape),
                )?;
                let nested_ctx = scoped_env
                    .as_ref()
                    .map(|env| ctx.with_env(env))
                    .unwrap_or(ctx);
                let nested = render_stream_sub_pipeline_expr(pipeline, lambda, nested_ctx)?;
                expr = if prefix_stmts.is_empty() {
                    stream_flat_map_op_projection(expr, &lambda.rust_param_name(), nested)
                } else {
                    stream_prefixed_flat_map_op_projection(
                        expr,
                        &lambda.rust_param_name(),
                        prefix_stmts,
                        nested,
                    )
                };
                item_is_borrowed = false;
                item_shape = pipeline.output_shape().clone();
            }
        }
        if let Some(capture) = capture {
            expr = crate::coercion::stream_bound_receiver_capture_projection(
                input.unwrap(),
                capture.value,
                expr,
            );
        }
    }
    Some((expr, item_is_borrowed, item_shape, item_is_result))
}

struct StreamBoundReceiverCapture<'a> {
    value: RustExpr,
    env: ShapeEnv,
    lambda: LambdaPlan<'a>,
}

fn stream_bound_receiver_capture<'a>(
    lambda: &LambdaPlan<'a>,
    ctx: StreamRenderContext<'_>,
) -> Option<StreamBoundReceiverCapture<'a>> {
    let StreamLambdaBody::MethodReference {
        target,
        receiver: source,
        ..
    } = lambda.body()
    else {
        return None;
    };
    let env = ctx.env?;
    let receiver =
        crate::method_reference_plan::bound_method_reference_source(target, source, env)?;
    let shape = infer_expr_shape_with_context(
        &receiver,
        env,
        ExprShapeContext::new(ctx.signatures, None, ctx.facts),
    );
    let element_ty = shape.optional_element_type()?;
    let lowered = emit_stream_lambda_body_with_locals_prec(
        &receiver,
        "__java_unused_bound_receiver",
        "__java_unused_bound_receiver",
        StreamLambdaValue::OwnedItem,
        &[],
        ctx,
        None,
        0,
        false,
    )?
    .expr;
    // Evaluating a bound reference checks its receiver before traversal, even
    // for an empty stream. Reuse nullable-source exception projection rather
    // than deferring the check to each invocation of the callback.
    let mut value = crate::coercion::checked_optional_borrowed_receiver_projection(
        lowered,
        crate::coercion::OptionalSequenceReceiverProjection::Shared,
        ctx.propagate_java_exceptions,
    );
    let default_facts = FactSet::default();
    let mut captured_shape = optional_element_slot_shape_with_facts(
        &shape,
        element_ty,
        ctx.facts.unwrap_or(&default_facts),
    );
    captured_shape.ownership = OwnershipShape::SharedBorrow;
    if captured_shape.java_ty == JavaType::String {
        // Retain the canonical Java String handle, rather than a borrow of
        // the Option local. Its Clone preserves Java identity and UTF-16.
        // Native equality then receives its ordinary owned String carrier.
        value = apply_clone_projection(crate::coercion::deref_borrowed_projection(value));
        captured_shape = crate::shape_projection::owned_string_value_shape();
    }
    let mut captured_env = env.clone();
    captured_env.insert_local_shape("__java_bound_receiver".to_string(), captured_shape);
    Some(StreamBoundReceiverCapture {
        value,
        env: captured_env,
        lambda: lambda.with_method_reference_target("__java_bound_receiver")?,
    })
}

fn lambda_collection_flat_map_element_shape(
    plan: &crate::stream::LambdaCollectionStreamPlan<'_>,
    param_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> Option<RustSlotShape> {
    if matches!(plan.source(), Expr::Var(name) if name == plan.lambda().param_name()) {
        return collection_flat_map_element_shape(param_shape, ctx);
    }
    let default_facts;
    let facts = if let Some(facts) = ctx.facts {
        facts
    } else {
        default_facts = FactSet::default();
        &default_facts
    };
    let mut env = TypeEnv::new();
    env.insert_local(
        plan.lambda().param_name().to_string(),
        param_shape.java_ty.clone(),
    );
    infer_stream_source_element_shape_with_facts(plan.source(), &env, facts)
}

fn stream_source_optional_sequence_for_render(
    source: &Expr,
    planned_optional_sequence: bool,
    ctx: StreamRenderContext<'_>,
) -> bool {
    if let Some(env) = ctx.env {
        let shape = infer_expr_shape_with_context(
            source,
            env,
            ExprShapeContext::new(ctx.signatures, None, ctx.facts),
        );
        if optional_shape_is_stream_capable(&shape, ctx.facts) {
            return true;
        }
        if shape_is_stream_capable(&shape, ctx.facts) {
            return false;
        }
    }
    planned_optional_sequence
}

fn optional_shape_is_stream_capable(shape: &RustSlotShape, facts: Option<&FactSet>) -> bool {
    let Some(element_ty) = shape.optional_element_type() else {
        return false;
    };
    java_type_is_stream_capable(&element_ty, facts)
}

fn shape_is_stream_capable(shape: &RustSlotShape, facts: Option<&FactSet>) -> bool {
    java_type_is_stream_capable(&shape.java_ty(), facts)
}

fn java_type_is_stream_capable(java_ty: &JavaType, facts: Option<&FactSet>) -> bool {
    crate::types::stream_element_type(java_ty).is_some()
        || facts.is_some_and(|facts| {
            crate::stream_shape::stream_source_capability_policy(java_ty, facts)
                .capability_element()
                .is_some()
        })
}

fn collection_flat_map_element_shape(
    collection_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> Option<RustSlotShape> {
    let element_ty = collection_shape.sequence_element_type()?;
    let default_facts;
    let facts = if let Some(facts) = ctx.facts {
        facts
    } else {
        default_facts = FactSet::default();
        &default_facts
    };
    Some(crate::shape_projection::iteration_element_shape(
        element_ty, facts,
    ))
}

fn render_stream_terminal_expr(
    receiver: RustExpr,
    plan: &StreamPlan<'_>,
    item_is_borrowed: bool,
    item_shape: &RustSlotShape,
    item_is_result: bool,
    ctx: StreamRenderContext<'_>,
    materialization_target: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    if item_is_result
        && matches!(
            plan.terminal(),
            StreamTerminal::Iterator | StreamTerminal::CollectToMap { .. }
        )
    {
        // These boundaries still need a fallible callback/traversal contract;
        // never silently consume Result as the Java payload type.
        return None;
    }
    match &plan.terminal() {
        StreamTerminal::Sum => {
            let JavaType::Primitive(primitive) = item_shape.java_ty else {
                return None;
            };
            let receiver =
                primitive_terminal_receiver(receiver, item_is_borrowed, item_is_result, ctx)?;
            Some(crate::coercion::stream_sum_terminal_projection(
                receiver, primitive,
            ))
        }
        StreamTerminal::PrimitiveExtremum { maximum } => {
            let JavaType::Primitive(primitive) = item_shape.java_ty else {
                return None;
            };
            crate::stdlib::primitive_optional_name(primitive)?;
            let receiver =
                primitive_terminal_receiver(receiver, item_is_borrowed, item_is_result, ctx)?;
            Some(crate::coercion::stream_extremum_terminal_projection(
                receiver, primitive, *maximum,
            ))
        }
        StreamTerminal::Count if item_is_result => {
            let body = RustExpr::Block {
                stmts: vec![RustStmt::Expr(propagate_result_projection(RustExpr::Var(
                    "__java_item".to_string(),
                )))],
                tail: Some(Box::new(stream_callback_result_expr(RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("__java_count".to_string())),
                    right: Box::new(RustExpr::Literal("1_i64".to_string())),
                }))),
            };
            Some(stream_result_boundary(
                receiver_method_call_projection_exact(
                    receiver,
                    "try_fold",
                    vec![
                        RustExpr::Literal("0_i64".to_string()),
                        RustExpr::Closure {
                            is_move: false,
                            params: ["__java_count", "__java_item"]
                                .into_iter()
                                .map(|name| RustClosureParam {
                                    pattern: name.to_string(),
                                    ty: None,
                                })
                                .collect(),
                            return_type: None,
                            body: RustClosureBody::Expr(Box::new(body)),
                        },
                    ],
                ),
                ctx,
            ))
        }
        StreamTerminal::Count => Some(RustExpr::Cast {
            expr: Box::new(receiver_method_call_projection_exact(
                receiver,
                "count",
                Vec::new(),
            )),
            ty: "i64".to_string(),
        }),
        StreamTerminal::Iterator => {
            let items = if item_is_borrowed {
                receiver_method_call_projection_exact(receiver, "cloned", Vec::new())
            } else {
                receiver
            };
            Some(function_call_projection(
                RustExpr::path(["JavaIterator", "new"]),
                vec![items],
            ))
        }
        StreamTerminal::ForEach(lambda) => {
            let capture = stream_bound_receiver_capture(lambda, ctx);
            let callback_ctx = capture
                .as_ref()
                .map(|capture| ctx.with_env(&capture.env))
                .unwrap_or(ctx);
            let callback = capture
                .as_ref()
                .map(|capture| &capture.lambda)
                .unwrap_or(lambda);
            let value = if item_is_borrowed {
                StreamLambdaValue::BorrowedItem
            } else {
                StreamLambdaValue::OwnedItem
            };
            let body = emit_stream_consumer_body(callback, value, callback_ctx, item_shape)?;
            let fallible = item_is_result || body.has_unbound_try();
            let mut stmts = Vec::new();
            if item_is_result {
                stmts.push(RustStmt::Let {
                    mutable: false,
                    name: callback.rust_param_name().to_string(),
                    ty: None,
                    value: Some(propagate_result_projection(RustExpr::Var(
                        callback.rust_param_name().to_string(),
                    ))),
                });
            }
            stmts.push(RustStmt::Expr(body));
            let input = if capture.is_some() {
                RustExpr::Var("__java_bound_stream".to_string())
            } else {
                receiver.clone()
            };
            let result = receiver_method_call_projection_exact(
                input,
                if fallible { "try_for_each" } else { "for_each" },
                vec![RustExpr::Closure {
                    is_move: false,
                    params: vec![RustClosureParam {
                        pattern: callback.rust_param_name().to_string(),
                        ty: None,
                    }],
                    return_type: None,
                    body: RustClosureBody::Expr(Box::new(RustExpr::Block {
                        stmts,
                        tail: fallible.then(|| {
                            Box::new(stream_callback_result_expr(RustExpr::Tuple(Vec::new())))
                        }),
                    })),
                }],
            );
            let result = if fallible {
                stream_result_boundary(result, ctx)
            } else {
                result
            };
            Some(match capture {
                Some(capture) => crate::coercion::stream_bound_receiver_capture_projection(
                    receiver,
                    capture.value,
                    result,
                ),
                None => result,
            })
        }
        StreamTerminal::Reduce(reduce) => {
            let mut identity = emit_stream_lambda_body_with_locals_prec(
                reduce.identity(),
                "__java_unused_reduce_item",
                "__java_unused_reduce_item",
                StreamLambdaValue::OwnedItem,
                &[],
                ctx,
                None,
                0,
                false,
            )?
            .expr;
            let identity_type = crate::types::infer_expr_type(
                reduce.identity(),
                &ctx.env
                    .map(|env| env.type_env().clone())
                    .unwrap_or_default(),
            );
            if let JavaType::Primitive(primitive) = identity_type
                && crate::types::is_numeric_primitive_type(primitive)
            {
                // Rust method-call inference does not constrain a wrapping
                // accumulator from an unsuffixed identity literal. Java has
                // already fixed that literal/expression's primitive type.
                identity = RustExpr::Cast {
                    expr: Box::new(identity),
                    ty: crate::emit::emit_type(
                        &crate::operator_lowering::rust_type_for_java_primitive(primitive),
                    ),
                };
            }
            let accumulator = reduce.accumulator();
            let left = accumulator.left_param_name();
            let right = accumulator.right_param_name();
            let left_rust = accumulator.left_rust_param_name();
            let right_rust = accumulator.right_rust_param_name();
            let locals = [left, right];
            let mut reducer_env = ctx.env.cloned().unwrap_or_default();
            let mut reducer_shape = item_shape.clone();
            if let JavaType::Named { name, args } = &reducer_shape.java_ty
                && args.is_empty()
                && let Some(primitive) = crate::stdlib::boxed_primitive_type(name)
            {
                reducer_shape.java_ty = JavaType::Primitive(primitive);
            }
            reducer_env.insert_local_shape(left.to_string(), reducer_shape.clone());
            reducer_env.insert_local_shape(right.to_string(), reducer_shape);
            let reducer_ctx = StreamRenderContext {
                env: Some(&reducer_env),
                ..ctx
            };
            let body = emit_stream_lambda_body_with_locals_prec(
                accumulator.body().return_expr()?,
                left,
                left_rust,
                StreamLambdaValue::OwnedItem,
                &locals,
                reducer_ctx,
                Some(item_shape),
                0,
                false,
            )?
            .expr;
            if item_is_result || body.has_unbound_try() {
                let mut statements = Vec::new();
                if item_is_result {
                    statements.push(RustStmt::Let {
                        mutable: false,
                        name: right_rust.to_string(),
                        ty: None,
                        value: Some(propagate_result_projection(RustExpr::Var(
                            right_rust.to_string(),
                        ))),
                    });
                }
                let fold = receiver_method_call_projection_exact(
                    receiver,
                    "try_fold",
                    vec![
                        identity,
                        RustExpr::Closure {
                            is_move: false,
                            params: vec![
                                RustClosureParam {
                                    pattern: left_rust.to_string(),
                                    ty: None,
                                },
                                RustClosureParam {
                                    pattern: right_rust.to_string(),
                                    ty: None,
                                },
                            ],
                            return_type: None,
                            body: RustClosureBody::Expr(Box::new(RustExpr::Block {
                                stmts: statements,
                                tail: Some(Box::new(stream_callback_result_expr(body))),
                            })),
                        },
                    ],
                );
                Some(stream_result_boundary(fold, ctx))
            } else {
                Some(stream_reduce_terminal_projection(
                    receiver, identity, left_rust, right_rust, body,
                ))
            }
        }
        StreamTerminal::CollectToMap { key, value } => {
            let lambda_value = if item_is_borrowed {
                StreamLambdaValue::BorrowedItem
            } else {
                StreamLambdaValue::OwnedItem
            };
            Some(stream_collect_to_map_projection(
                receiver,
                key.rust_param_name(),
                emit_stream_collect_to_map_lambda_body_expr(
                    key,
                    plan,
                    lambda_value,
                    ctx,
                    Some(plan.source_element().shape()),
                )?,
                emit_stream_collect_to_map_lambda_body_expr(
                    value,
                    plan,
                    lambda_value,
                    ctx,
                    Some(plan.source_element().shape()),
                )?,
            ))
        }
        StreamTerminal::Join { delimiter, affixes } => {
            let eager_arguments = affixes.is_some()
                || delimiter.is_some_and(|expr| !matches!(expr, Expr::Literal(_)));
            let delimiter = stream_join_delimiter_projection(*delimiter, ctx)?;
            let item_display = stream_join_item_display_projection(item_shape);
            if eager_arguments {
                let affixes = match affixes {
                    Some((prefix, suffix)) => Some((
                        stream_join_delimiter_projection(Some(prefix), ctx)?,
                        stream_join_delimiter_projection(Some(suffix), ctx)?,
                    )),
                    None => None,
                };
                return Some(if item_is_result {
                    crate::coercion::staged_stream_join_terminal_projection_with_result_context(
                        receiver,
                        item_display,
                        delimiter,
                        affixes,
                        true,
                        ctx.propagate_java_exceptions,
                    )
                } else {
                    crate::coercion::staged_stream_join_terminal_projection(
                        receiver,
                        item_display,
                        delimiter,
                        affixes,
                    )
                });
            }
            Some(if item_is_result {
                crate::coercion::stream_join_terminal_projection_with_result_context(
                    receiver,
                    item_display,
                    delimiter,
                    true,
                    ctx.propagate_java_exceptions,
                )
            } else {
                stream_join_terminal_projection(receiver, item_display, delimiter)
            })
        }
        StreamTerminal::AnyMatch(lambda) | StreamTerminal::AllMatch(lambda) => {
            let capture = stream_bound_receiver_capture(lambda, ctx);
            let callback_ctx = capture
                .as_ref()
                .map(|capture| ctx.with_env(&capture.env))
                .unwrap_or(ctx);
            let callback = capture
                .as_ref()
                .map(|capture| &capture.lambda)
                .unwrap_or(lambda);
            let terminal_method = match &plan.terminal() {
                StreamTerminal::AnyMatch(_) => "any",
                StreamTerminal::AllMatch(_) => "all",
                _ => unreachable!("terminal was matched as any/all"),
            };
            let value = if item_is_borrowed {
                StreamLambdaValue::BorrowedItem
            } else {
                StreamLambdaValue::OwnedItem
            };
            let input = if capture.is_some() {
                RustExpr::Var("__java_bound_stream".to_string())
            } else {
                receiver.clone()
            };
            let body = emit_stream_lambda_closure_body_expr(
                callback,
                value,
                callback_ctx,
                Some(item_shape),
            )?;
            let result = if body.has_unbound_try() || item_is_result {
                let result = crate::coercion::fallible_stream_match_terminal_projection(
                    input,
                    terminal_method == "all",
                    &callback.rust_param_name(),
                    body,
                    item_is_result,
                );
                stream_result_boundary(result, ctx)
            } else {
                stream_match_terminal_projection(
                    input,
                    terminal_method,
                    &callback.rust_param_name(),
                    body,
                )
            };
            Some(match capture {
                Some(capture) => crate::coercion::stream_bound_receiver_capture_projection(
                    receiver,
                    capture.value,
                    result,
                ),
                None => result,
            })
        }
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::List)
        | StreamTerminal::ToArray
            if item_is_result && ctx.propagate_java_exceptions =>
        {
            Some(propagate_result_projection(typed_collect_projection(
                receiver,
                "Result<Vec<_>, JavaException>",
            )))
        }
        StreamTerminal::Collect(kind)
            if kind.is_set_like() && item_is_result && ctx.propagate_java_exceptions =>
        {
            Some(propagate_result_projection(typed_collect_projection(
                receiver,
                &format!(
                    "Result<{}, JavaException>",
                    stream_set_collect_type(*kind, materialization_target)?
                ),
            )))
        }
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::Map) => None,
        StreamTerminal::FindFirst if item_is_result => {
            let next = receiver_method_call_projection_exact(receiver, "next", Vec::new());
            let result = stream_result_boundary(
                receiver_method_call_projection_exact(next, "transpose", Vec::new()),
                ctx,
            );
            Some(if item_is_borrowed {
                receiver_method_call_projection_exact(result, "cloned", Vec::new())
            } else {
                result
            })
        }
        StreamTerminal::FindFirst if item_is_borrowed => {
            let next = receiver_method_call_projection_exact(receiver, "next", Vec::new());
            Some(receiver_method_call_projection_exact(
                next,
                "cloned",
                Vec::new(),
            ))
        }
        _ if item_is_result => None,
        _ => no_op_stream_terminal_method(plan.terminal(), materialization_target)
            .map(|method| receiver_method_call_projection_exact(receiver, &method, Vec::new())),
    }
}

fn stream_result_boundary(result: RustExpr, ctx: StreamRenderContext<'_>) -> RustExpr {
    if ctx.propagate_java_exceptions {
        propagate_result_projection(result)
    } else {
        crate::coercion::unwrap_result_or_panic_uncaught_java_exception(result)
    }
}

fn emit_stream_consumer_body(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    input: &RustSlotShape,
) -> Option<RustExpr> {
    if let StreamLambdaBody::Block(_) = lambda.body() {
        let (stmts, _, _) = emit_stream_lambda_prefix_stmts_and_locals(
            lambda.body(),
            lambda.param_name(),
            lambda.rust_param_name(),
            ctx,
            value,
            Some(input),
        )?;
        Some(RustExpr::Block { stmts, tail: None })
    } else {
        emit_stream_lambda_closure_body_expr(lambda, value, ctx, Some(input))
    }
}

fn primitive_terminal_receiver(
    receiver: RustExpr,
    borrowed: bool,
    fallible: bool,
    ctx: StreamRenderContext<'_>,
) -> Option<RustExpr> {
    let mut receiver = if fallible {
        if !ctx.propagate_java_exceptions {
            return None;
        }
        receiver_method_call_projection_exact(
            propagate_result_projection(typed_collect_projection(
                receiver,
                "Result<Vec<_>, JavaException>",
            )),
            "into_iter",
            Vec::new(),
        )
    } else {
        receiver
    };
    if borrowed {
        receiver = receiver_method_call_projection_exact(receiver, "copied", Vec::new());
    }
    Some(receiver)
}

fn render_no_op_stream_plan_structured(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    source_optional_sequence: bool,
    materialization_target: Option<&RustSlotShape>,
) -> Option<LoweredExpr> {
    if !plan.ops().is_empty() || plan.source().range().is_some() || source_optional_sequence {
        return None;
    }
    let terminal_method = no_op_stream_terminal_method(plan.terminal(), materialization_target)?;
    let iter = sequence_iteration_source_projection(
        lowered_source,
        false,
        sequence_iteration_projection(plan.source().iteration()),
    );
    let item_is_borrowed = matches!(plan.source().iteration(), StreamSourceIteration::Borrowed);
    let item_shape = effective_stream_item_shape(plan.source_element().shape(), item_is_borrowed);
    let actual = stream_terminal_actual_after_rendered_ops(
        plan,
        item_is_borrowed,
        &item_shape,
        materialization_target,
    );
    Some(LoweredExpr::new(
        receiver_method_call_projection_exact(iter, &terminal_method, Vec::new()),
        actual,
    ))
}

fn stream_set_uses_semantic_carrier(
    kind: crate::stdlib::JavaCollectionKind,
    target: Option<&RustSlotShape>,
) -> bool {
    target.is_some_and(|target| {
        let java_ty = crate::types::optional_element_type(&target.java_ty)
            .unwrap_or_else(|| target.java_ty.clone());
        crate::stdlib::java_collection_kind_for_type(&java_ty) == Some(kind)
            && target.view == crate::slot_shape::ViewShape::JavaSemanticHash
    })
}

fn stream_set_collect_type(
    kind: crate::stdlib::JavaCollectionKind,
    target: Option<&RustSlotShape>,
) -> Option<&'static str> {
    crate::coercion::set_collect_type_for_kind_and_equality(
        kind,
        stream_set_uses_semantic_carrier(kind, target),
    )
}

fn stream_collected_set_actual(
    element: &RustSlotShape,
    kind: crate::stdlib::JavaCollectionKind,
    target: Option<&RustSlotShape>,
) -> LoweredType {
    let mut actual = LoweredType::collected_set_kind_from_element_shape(element.clone(), kind);
    if stream_set_uses_semantic_carrier(kind, target) {
        // Preserve element borrowing separately from the physical set carrier.
        // The ordinary slot adapter still owns/clones the elements as needed.
        match &mut actual {
            LoweredType::Slot(shape)
            | LoweredType::SetElementBorrow {
                set_shape: shape, ..
            } => {
                shape.view = crate::slot_shape::ViewShape::JavaSemanticHash;
            }
            _ => unreachable!("a collected set has a set-shaped carrier"),
        }
    }
    actual
}

fn no_op_stream_terminal_method(
    terminal: &StreamTerminal<'_>,
    target: Option<&RustSlotShape>,
) -> Option<String> {
    match terminal {
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::List)
        | StreamTerminal::ToArray => Some("collect::<Vec<_>>".to_string()),
        StreamTerminal::Collect(
            kind @ (crate::stdlib::JavaCollectionKind::Set
            | crate::stdlib::JavaCollectionKind::InsertionOrderedSet),
        ) => Some(format!(
            "collect::<{}>",
            stream_set_collect_type(*kind, target)?
        )),
        StreamTerminal::FindFirst => Some("next".to_string()),
        _ => None,
    }
}

fn stream_terminal_actual_after_rendered_ops(
    plan: &StreamPlan<'_>,
    item_is_borrowed: bool,
    item_shape: &RustSlotShape,
    materialization_target: Option<&RustSlotShape>,
) -> LoweredType {
    if item_is_borrowed {
        match plan.terminal() {
            StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::List)
            | StreamTerminal::ToArray => {
                return LoweredType::collected_list_from_element_shape(item_shape.clone());
            }
            StreamTerminal::Collect(kind) if kind.is_set_like() => {
                return stream_collected_set_actual(item_shape, *kind, materialization_target);
            }
            StreamTerminal::FindFirst => {
                if let LoweredType::OptionalElementBorrow { option_shape, .. } = &plan.actual() {
                    return LoweredType::Slot(option_shape.clone());
                }
            }
            _ => {}
        }
    } else {
        // A borrowed source followed by `map` yields owned items. Reusing the
        // parser's provisional terminal shape can add a post-map `.cloned()`
        // to `Iterator<Item = T>`, where it is invalid. The rendered final item
        // shape is authoritative for materializing collection terminals.
        match plan.terminal() {
            StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::List)
            | StreamTerminal::ToArray => {
                return LoweredType::collected_list_from_element_shape(item_shape.clone());
            }
            StreamTerminal::Collect(kind) if kind.is_set_like() => {
                return stream_collected_set_actual(item_shape, *kind, materialization_target);
            }
            _ => {}
        }
    }
    plan.actual().clone()
}

fn stream_source_range_projection(
    source: RustExpr,
    range: Option<StreamSourceRange<'_>>,
    ctx: StreamRenderContext<'_>,
) -> RustExpr {
    let Some(StreamSourceRange::JavaArrays { from, to }) = range else {
        return source;
    };
    let from = emit_stream_lambda_body_with_locals_prec(
        from,
        "__java_unused_stream_bound",
        "__java_unused_stream_bound",
        StreamLambdaValue::OwnedItem,
        &[],
        ctx,
        None,
        0,
        false,
    )
    .map(|lowered| lowered.expr)
    .unwrap_or_else(|| {
        crate::unsupported::compile_error_expr("unsupported Arrays.stream range start")
    });
    let to = emit_stream_lambda_body_with_locals_prec(
        to,
        "__java_unused_stream_bound",
        "__java_unused_stream_bound",
        StreamLambdaValue::OwnedItem,
        &[],
        ctx,
        None,
        0,
        false,
    )
    .map(|lowered| lowered.expr)
    .unwrap_or_else(|| {
        crate::unsupported::compile_error_expr("unsupported Arrays.stream range end")
    });
    arrays_stream_range_projection_with_result_context(
        source,
        from,
        to,
        ctx.propagate_java_exceptions,
    )
}

fn sequence_iteration_projection(iteration: StreamSourceIteration) -> SequenceIterationProjection {
    match iteration {
        StreamSourceIteration::Borrowed => SequenceIterationProjection::Borrowed,
        StreamSourceIteration::OwnedValue => SequenceIterationProjection::OwnedValue,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct StreamRenderOptions {
    move_filter_closures: bool,
    input_is_result: bool,
}

fn render_context_lambda_output_shape(
    lambda: &LambdaPlan<'_>,
    param_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> Option<RustSlotShape> {
    let env = ctx.env?;
    if let StreamLambdaBody::MethodReference { target, method, .. } = lambda.body() {
        if let Some(shape) = source_method_reference_output_shape(target, method, param_shape, ctx)
        {
            return Some(shape);
        }
        let facts = ctx.facts?;
        return crate::method_reference_shape::infer_single_input_method_reference_output_shape_with_context(
            target,
            method,
            param_shape,
            env.type_env(),
            facts,
        );
    }
    let return_expr = lambda.body().return_expr()?;
    if let Expr::Call {
        receiver,
        name,
        args,
        resolved,
        ..
    } = return_expr
        && let Some((_, _, signature)) = stream_source_call_signature(
            receiver.as_deref(),
            name,
            args,
            resolved.as_ref(),
            lambda.param_name(),
            Some(param_shape),
            ctx,
        )
    {
        return signature.return_shape_cloned();
    }
    let mut lambda_env = env.clone();
    lambda_env.insert_local_shape(lambda.param_name(), param_shape.clone());
    Some(infer_expr_shape_with_context(
        return_expr,
        &lambda_env,
        ExprShapeContext::new(ctx.signatures, None, ctx.facts),
    ))
}

fn source_method_reference_output_shape(
    target: &str,
    method: &str,
    param_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> Option<RustSlotShape> {
    source_method_reference_signature(target, method, param_shape, ctx)?
        .2
        .return_shape_cloned()
}

fn source_method_reference_signature<'a>(
    target: &str,
    method: &str,
    param_shape: &RustSlotShape,
    ctx: StreamRenderContext<'a>,
) -> Option<(String, String, &'a crate::signature::MethodSignaturePlan)> {
    if let Some(bound) = source_bound_method_reference_signature(target, method, param_shape, ctx) {
        return Some(bound);
    }
    if matches!(target, "this" | "self") {
        return None;
    }
    crate::method_reference_shape::source_unbound_or_static_reference_signature(
        target,
        method,
        param_shape,
        ctx.env?.type_env(),
        ctx.signatures?,
        ctx.facts,
    )
}

fn source_bound_method_reference_signature<'a>(
    target: &str,
    method: &str,
    param_shape: &RustSlotShape,
    ctx: StreamRenderContext<'a>,
) -> Option<(String, String, &'a crate::signature::MethodSignaturePlan)> {
    let signatures = ctx.signatures?;
    let env = ctx.env?;
    let owner = if matches!(target, "this" | "self") {
        ctx.current_type?.to_string()
    } else {
        let receiver = crate::method_reference_plan::bound_method_reference_receiver(target, env)?;
        let ty = crate::types::infer_expr_type(&receiver.source_expr(), env.type_env());
        let ty = crate::types::optional_element_type(&ty).unwrap_or(ty);
        let name = match &ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
            _ => return None,
        };
        let bounded = env.type_env().type_param_bound(name).unwrap_or(&ty);
        match bounded {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => env
                .type_env()
                .static_type_alias(name)
                .unwrap_or(name)
                .to_string(),
            JavaType::String => "String".to_string(),
            _ => return None,
        }
    };
    let selected = ctx
        .facts
        .and_then(|facts| {
            signatures.method_signature_by_base_name_and_arg_types_with_facts(
                &owner,
                method,
                std::slice::from_ref(&param_shape.java_ty),
                facts,
            )
        })
        .or_else(|| {
            signatures.method_signature_by_base_name_and_arg_types(
                &owner,
                method,
                std::slice::from_ref(&param_shape.java_ty),
            )
        })?;
    (!matches!(selected.1.receiver(), crate::signature::ReceiverPlan::None))
        .then(|| (owner, selected.0, selected.1))
}

fn render_stream_lambda_param(
    name: &str,
    item_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> RustClosureParam {
    if stream_lambda_param_type_is_useful(item_shape) {
        let ty = render_stream_lambda_param_type(item_shape, ctx);
        RustClosureParam {
            pattern: name.to_string(),
            ty: Some(ty),
        }
    } else {
        RustClosureParam {
            pattern: name.to_string(),
            ty: None,
        }
    }
}

fn render_stream_lambda_param_type(
    item_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> String {
    if let Some(ty) = render_tuple_like_stream_lambda_param_type(&item_shape.java_ty, ctx.facts) {
        return ty;
    }
    ctx.facts
        .map(|facts| crate::emit::emit_type(&rust_type_for_slot_with_facts(item_shape, facts)))
        .unwrap_or_else(|| {
            crate::emit::emit_type(&crate::slot_render::rust_type_for_slot(item_shape))
        })
}

fn render_tuple_like_stream_lambda_param_type(
    java_ty: &JavaType,
    facts: Option<&FactSet>,
) -> Option<String> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let simple = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = simple.rsplit('.').next().unwrap_or(&simple);
    if !simple.starts_with("Tuple") || args.is_empty() {
        return None;
    }
    if matches!(simple, "Tuple2" | "Tuple3" | "Tuple4") {
        return Some(crate::java_naming::rust_name_for_java_named_type(simple));
    }
    let rendered_args = args
        .iter()
        .map(|arg| {
            facts
                .map(|facts| {
                    crate::emit::emit_type(&crate::slot_render::rust_type_for_java_type_with_facts(
                        arg, facts,
                    ))
                })
                .unwrap_or_else(|| {
                    crate::emit::emit_type(&crate::slot_render::rust_type_for_java_type(arg))
                })
        })
        .collect::<Vec<_>>();
    Some(format!(
        "{}<{}>",
        crate::java_naming::rust_name_for_java_named_type(simple),
        rendered_args.join(", ")
    ))
}

fn stream_lambda_param_type_is_useful(item_shape: &RustSlotShape) -> bool {
    if item_shape.optional_element_type().is_some()
        && item_shape.ownership == OwnershipShape::SharedBorrow
    {
        // Iterators lend `&Option<T>`, not `Option<&T>`. The latter is a useful
        // value projection but an invalid closure parameter annotation; leave
        // the parameter inferred and project through `as_ref()` at Java
        // receiver use sites.
        return false;
    }
    !matches!(
        item_shape.java_ty,
        JavaType::Primitive(_) | JavaType::String | JavaType::Unknown(_)
    )
}

fn render_stream_sub_pipeline_expr(
    pipeline: &StreamSubPipeline<'_>,
    lambda: &LambdaPlan<'_>,
    ctx: StreamRenderContext<'_>,
) -> Option<RustExpr> {
    if pipeline.source().range().is_some() {
        return None;
    }
    let source = emit_stream_lambda_receiver_expr(
        pipeline.source().expr(),
        &lambda.param_name(),
        &lambda.rust_param_name(),
        StreamLambdaValue::BorrowedItem,
        ctx,
    )?;
    let source_is_lambda_param =
        matches!(pipeline.source().expr(), Expr::Var(name) if name == &lambda.param_name());
    let iteration = if source_is_lambda_param {
        sequence_iteration_projection(pipeline.source().iteration())
    } else {
        SequenceIterationProjection::OwnedValue
    };
    let expr = sequence_iteration_source_projection_with_result_context(
        source,
        stream_source_optional_sequence_for_render(
            pipeline.source().expr(),
            pipeline.source().optional_sequence(),
            ctx,
        ),
        iteration,
        // This expression is returned from an Iterator::flat_map closure, so
        // the closure must yield an iterator directly rather than `Result<_>`.
        false,
    );
    let item_is_borrowed = source_is_lambda_param
        && matches!(
            pipeline.source().iteration(),
            StreamSourceIteration::Borrowed
        );
    let item_shape =
        effective_stream_item_shape(pipeline.source_element().shape(), item_is_borrowed);
    apply_stream_ops_structured(
        expr,
        pipeline.ops(),
        ctx,
        StreamRenderOptions {
            move_filter_closures: true,
            input_is_result: false,
        },
        item_is_borrowed,
        item_shape,
    )
    .and_then(
        |(expr, nested_item_is_borrowed, nested_item_shape, nested_item_is_result)| {
            (!nested_item_is_result).then(|| {
                recover_temporary_sub_pipeline_item(
                    expr,
                    source_is_lambda_param,
                    nested_item_is_borrowed,
                    &nested_item_shape,
                )
            })
        },
    )
}

fn recover_temporary_sub_pipeline_item(
    expr: RustExpr,
    source_is_lambda_param: bool,
    item_is_borrowed: bool,
    item_shape: &RustSlotShape,
) -> RustExpr {
    if source_is_lambda_param {
        return expr;
    }
    if item_shape.java_ty == JavaType::String
        && (item_is_borrowed || item_shape.ownership == OwnershipShape::SharedBorrow)
    {
        return own_temporary_string_stream_item(expr);
    }
    expr
}

fn own_temporary_string_stream_item(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "map" => {
            let [
                RustExpr::Closure {
                    is_move,
                    return_type,
                    params,
                    body,
                },
            ] = args.as_slice()
            else {
                return receiver_method_call_projection_exact(*receiver, &method, args);
            };
            let RustClosureBody::Expr(body_expr) = body;
            let owned_body = receiver_method_call_projection_exact(
                body_expr.as_ref().clone(),
                "to_string",
                Vec::new(),
            );
            receiver_method_call_projection_exact(
                *receiver,
                &method,
                vec![RustExpr::Closure {
                    is_move: *is_move,
                    return_type: return_type.clone(),
                    params: params.clone(),
                    body: RustClosureBody::Expr(Box::new(owned_body)),
                }],
            )
        }
        other => receiver_method_call_projection_exact(
            other,
            "map",
            vec![RustExpr::Closure {
                is_move: false,
                return_type: None,
                params: vec![RustClosureParam {
                    pattern: "value".to_string(),
                    ty: None,
                }],
                body: RustClosureBody::Expr(Box::new(receiver_method_call_projection_exact(
                    RustExpr::Var("value".to_string()),
                    "to_string",
                    Vec::new(),
                ))),
            }],
        ),
    }
}

fn stream_join_item_display_projection(item_shape: &RustSlotShape) -> RustExpr {
    if item_shape.optional_element_type().is_some() {
        optional_stream_join_item_display_projection()
    } else {
        receiver_method_call_projection_exact(
            RustExpr::Var("item".to_string()),
            "to_string",
            Vec::new(),
        )
    }
}

fn optional_stream_join_item_display_projection() -> RustExpr {
    let as_ref = receiver_method_call_projection_exact(
        RustExpr::Var("item".to_string()),
        "as_ref",
        Vec::new(),
    );
    let mapped = receiver_method_call_projection_exact(
        as_ref,
        "map",
        vec![RustExpr::Closure {
            is_move: false,
            return_type: None,
            params: vec![RustClosureParam {
                pattern: "value".to_string(),
                ty: None,
            }],
            body: RustClosureBody::Expr(Box::new(receiver_method_call_projection_exact(
                RustExpr::Var("value".to_string()),
                "to_string",
                Vec::new(),
            ))),
        }],
    );
    receiver_method_call_projection_exact(
        mapped,
        "unwrap_or_else",
        vec![RustExpr::Closure {
            is_move: false,
            return_type: None,
            params: Vec::new(),
            body: RustClosureBody::Expr(Box::new(receiver_method_call_projection_exact(
                RustExpr::Literal("\"null\"".to_string()),
                "to_string",
                Vec::new(),
            ))),
        }],
    )
}

fn stream_join_delimiter_projection(
    delimiter: Option<&Expr>,
    ctx: StreamRenderContext<'_>,
) -> Option<RustExpr> {
    match delimiter {
        None => Some(RustExpr::Literal("\"\"".to_string())),
        Some(Expr::Literal(Literal::String(value))) => Some(rust_string_literal_projection(value)),
        Some(expr) => stream_join_expr_delimiter_projection(expr, ctx),
    }
}

fn stream_join_expr_delimiter_projection(
    expr: &Expr,
    ctx: StreamRenderContext<'_>,
) -> Option<RustExpr> {
    let delimiter = emit_stream_lambda_body_with_locals_prec(
        expr,
        "__java_unused_stream_join_param",
        "__java_unused_stream_join_param",
        StreamLambdaValue::OwnedItem,
        &["__java_unused_stream_join_param"],
        ctx,
        None,
        0,
        false,
    )?;
    let target = borrowed_string_content_shape();
    let Some(env) = ctx.env else {
        return Some(ensure_borrow_projection(
            delimiter.expr,
            None,
            Some(&target),
            BorrowProjectionUse::StringView,
        ));
    };
    let actual = infer_expr_shape_with_context(
        expr,
        env,
        ExprShapeContext::new(ctx.signatures, None, ctx.facts),
    );
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let plan = plan_lowered_slot_conversion(
        &LoweredType::Slot(Box::new(actual)),
        Slot::Argument(target),
        facts,
    );
    Some(apply_lowered_slot_conversion_to_expr(delimiter.expr, &plan))
}

fn emit_stream_collect_to_map_lambda_body_expr(
    lambda: &LambdaPlan<'_>,
    plan: &StreamPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    let return_expr = lambda.body().return_expr()?;
    if matches!(return_expr, Expr::Var(name) if name == &lambda.param_name()) {
        if matches!(value, StreamLambdaValue::OwnedItem) {
            return Some(RustExpr::Var(lambda.rust_param_name().to_string()));
        }
        return Some(stream_identity_value_projection(
            &lambda.rust_param_name(),
            plan.source_element().shape(),
        ));
    }
    emit_stream_lambda_body_with_locals_prec(
        return_expr,
        &lambda.param_name(),
        &lambda.rust_param_name(),
        value,
        &[lambda.param_name()],
        ctx,
        param_shape,
        0,
        false,
    )
    .map(|lowered| lowered.expr)
}

fn emit_stream_lambda_closure_body_expr(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    if let StreamLambdaBody::MethodReference { .. } = lambda.body() {
        return emit_stream_method_reference_body_expr(lambda, value, ctx, param_shape);
    }
    if let Some(nullable_body) =
        emit_nullable_stream_lambda_closure_body_expr(lambda, value, ctx, param_shape)
    {
        return Some(nullable_body);
    }
    let return_expr = lambda.body().return_expr()?;
    let (prefix_stmts, locals, scoped_env) = emit_stream_lambda_prefix_stmts_and_locals(
        lambda.body(),
        &lambda.param_name(),
        &lambda.rust_param_name(),
        ctx,
        value,
        param_shape,
    )?;
    let body_ctx = scoped_env
        .as_ref()
        .map(|env| ctx.with_env(env))
        .unwrap_or(ctx);
    let body = emit_stream_lambda_body_with_locals_prec(
        return_expr,
        &lambda.param_name(),
        &lambda.rust_param_name(),
        value,
        &locals,
        body_ctx,
        param_shape,
        0,
        false,
    )?
    .expr;
    if prefix_stmts.is_empty() {
        Some(body)
    } else {
        Some(RustExpr::Block {
            stmts: prefix_stmts,
            tail: Some(Box::new(body)),
        })
    }
}

fn emit_stream_method_reference_body_expr(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    let StreamLambdaBody::MethodReference { target, method, .. } = lambda.body() else {
        return None;
    };
    if method != "new"
        && let Some((owner, rust_name, signature)) = param_shape
            .and_then(|shape| source_method_reference_signature(target, method, shape, ctx))
    {
        let static_call = matches!(signature.receiver(), crate::signature::ReceiverPlan::None);
        let bound = ctx.env.and_then(|env| {
            crate::method_reference_plan::bound_method_reference_receiver(target, env)
        });
        let current_receiver = matches!(target, "this" | "self");
        let unbound = !static_call && !current_receiver && bound.is_none();
        let receiver = if static_call {
            Expr::Var(owner.clone())
        } else if let Some(bound) = bound {
            bound.source_expr()
        } else if current_receiver {
            Expr::This
        } else {
            Expr::Var(lambda.param_name().to_string())
        };
        let call = Expr::Call {
            type_args: Vec::new(),
            receiver: Some(Box::new(receiver)),
            name: method.to_string(),
            args: if unbound {
                Vec::new()
            } else {
                vec![Expr::Var(lambda.param_name().to_string())]
            },
            resolved: Some(crate::java_ir::ResolvedCall {
                type_args: Vec::new(),
                declaring_type: owner,
                rust_name,
                kind: if static_call {
                    CallKind::Static
                } else {
                    CallKind::Instance
                },
                return_type: signature
                    .return_shape()
                    .map(|shape| shape.java_ty.clone())
                    .unwrap_or(JavaType::Void),
                arg_target_types: signature
                    .params()
                    .iter()
                    .map(|param| param.shape().java_ty.clone())
                    .collect(),
                arg_coercions: Vec::new(),
            }),
        };
        return emit_stream_lambda_body_with_locals_prec(
            &call,
            lambda.param_name(),
            &lambda.rust_param_name(),
            value,
            &[lambda.param_name()],
            StreamRenderContext {
                single_use_owned_input: matches!(value, StreamLambdaValue::OwnedItem),
                ..ctx
            },
            param_shape,
            0,
            false,
        )
        .map(|lowered| lowered.expr);
    }
    if method != "new"
        && let Some(bound) = ctx.env.and_then(|env| {
            crate::method_reference_plan::bound_method_reference_receiver(target, env)
        })
    {
        let resolved = param_shape
            .and_then(|shape| source_bound_method_reference_signature(target, method, shape, ctx))
            .map(
                |(owner, rust_name, signature)| crate::java_ir::ResolvedCall {
                    type_args: Vec::new(),
                    declaring_type: owner,
                    rust_name,
                    kind: CallKind::Instance,
                    return_type: signature
                        .return_shape()
                        .map(|shape| shape.java_ty.clone())
                        .unwrap_or(JavaType::Void),
                    arg_target_types: signature
                        .params()
                        .iter()
                        .map(|param| param.shape().java_ty.clone())
                        .collect(),
                    arg_coercions: Vec::new(),
                },
            );
        let call = Expr::Call {
            type_args: Vec::new(),
            receiver: Some(Box::new(bound.source_expr())),
            name: method.to_string(),
            args: vec![Expr::Var(lambda.param_name().to_string())],
            resolved,
        };
        // A bound method reference forwards its input exactly once. A map
        // closure owns that input, unlike a filter's borrowed item; consuming
        // it needs neither a Clone bound nor a copied Java reference carrier.
        let ctx = StreamRenderContext {
            single_use_owned_input: matches!(value, StreamLambdaValue::OwnedItem),
            ..ctx
        };
        return emit_stream_lambda_body_with_locals_prec(
            &call,
            lambda.param_name(),
            &lambda.rust_param_name(),
            value,
            &[lambda.param_name()],
            ctx,
            param_shape,
            0,
            false,
        )
        .map(|lowered| lowered.expr);
    }
    if let Some(expr) = emit_stream_method_reference_body_through_call_plan(
        target,
        method,
        &lambda.param_name(),
        value,
        ctx,
        param_shape,
    ) {
        return Some(expr);
    }
    let value_expr = stream_lambda_value_expr(value, &lambda.param_name());
    let receiver =
        required_optional_stream_param_receiver(value, &lambda.param_name(), param_shape)
            .unwrap_or_else(|| stream_lambda_receiver_expr(value, &lambda.param_name()));
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    if matches!(target, "this" | "self")
        && let Some(((current_type, signatures), param_shape)) =
            ctx.current_type.zip(ctx.signatures).zip(param_shape)
        && let Some((rust_name, signature)) = signatures
            .method_signature_by_base_name_and_arg_types_with_facts(
                current_type,
                method,
                std::slice::from_ref(&param_shape.java_ty),
                facts,
            )
            .or_else(|| {
                signatures.method_signature_by_base_name_and_arg_types(
                    current_type,
                    method,
                    std::slice::from_ref(&param_shape.java_ty),
                )
            })
        && !matches!(signature.receiver(), crate::signature::ReceiverPlan::None)
        && let Some(param_slot) = signature.params().first()
    {
        let source_arg = Expr::Var(lambda.param_name().to_string());
        let lowered_arg = adapt_stream_lambda_arg_for_slot_expr(
            &source_arg,
            stream_lambda_expr(value_expr, 12),
            param_slot.shape(),
            lambda.param_name(),
            &[lambda.param_name()],
            ctx,
            Some(param_shape),
        );
        return Some(current_receiver_method_call_projection(
            &rust_name,
            vec![lowered_arg.expr],
        ));
    }
    let plan = crate::method_reference_plan::plan_method_reference(target, method);
    if let Some(param_shape) = param_shape {
        return Some(
            crate::method_reference_lowering::single_input_method_reference_body_expr_with_facts(
                &plan,
                receiver,
                value_expr,
                param_shape,
                facts,
            ),
        );
    }
    Some(
        crate::method_reference_lowering::single_input_method_reference_body_expr(
            &plan, receiver, value_expr,
        ),
    )
}

fn emit_stream_method_reference_body_through_call_plan(
    target: &str,
    method: &str,
    param: &str,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    if matches!(target, "this" | "self") {
        return None;
    }
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let mut typed_lambda_env = ctx
        .env
        .map(|env| env.type_env().clone())
        .unwrap_or_default();
    if let Some(param_shape) = param_shape {
        typed_lambda_env.insert_local(param.to_string(), param_shape.java_ty.clone());
    }
    if let Some(expr) = emit_stream_instance_method_reference_body_through_call_plan(
        target,
        method,
        param,
        value,
        param_shape,
        ctx,
        facts,
        &typed_lambda_env,
    ) {
        return Some(expr);
    }
    let owner_expr = Expr::Var(target.to_string());
    let target_plan = plan_stream_lambda_call_target(
        Some(&owner_expr),
        method,
        1,
        None,
        facts,
        ctx.external_index,
        &typed_lambda_env,
    );
    let crate::call_plan::CallTargetPlan::NativeJavaStatic {
        method: native_method,
    } = target_plan
    else {
        return None;
    };
    if matches!(
        native_method,
        crate::call_plan::NativeJavaStaticMethod::ObjectsEquals
    ) || (matches!(
        native_method,
        crate::call_plan::NativeJavaStaticMethod::ObjectsNonNull
            | crate::call_plan::NativeJavaStaticMethod::ObjectsIsNull
    ) && param_shape.is_none_or(|shape| shape.optional_element_type().is_none()))
    {
        return None;
    }
    let arg_exprs = vec![Expr::Var(param.to_string())];
    let default_signatures;
    let signatures = match ctx.signatures {
        Some(signatures) => signatures,
        None => {
            default_signatures = SignaturePlan::default();
            &default_signatures
        }
    };
    let argument_plan = crate::call_plan::plan_call_arguments_with_facts_and_args(
        Some(&owner_expr),
        method,
        None,
        &arg_exprs,
        signatures,
        facts,
        &typed_lambda_env,
    );
    let lowered_args = adapt_stream_lambda_args_for_plan_exprs(
        &arg_exprs,
        vec![stream_lambda_expr(RustExpr::Var(param.to_string()), 12)],
        argument_plan,
        param,
        &[param],
        ctx,
        param_shape,
    )
    .into_iter()
    .map(|arg| arg.expr)
    .collect();
    Some(stream_call_target_projection(
        &target_plan,
        None,
        lowered_args,
        ctx,
    ))
}

fn emit_stream_instance_method_reference_body_through_call_plan(
    target: &str,
    method: &str,
    param: &str,
    value: StreamLambdaValue,
    param_shape: Option<&RustSlotShape>,
    ctx: StreamRenderContext<'_>,
    facts: &FactSet,
    typed_lambda_env: &crate::types::TypeEnv,
) -> Option<RustExpr> {
    let receiver_ty = method_reference_receiver_type_for_call_plan_with_context(
        target,
        method,
        param_shape,
        facts,
        typed_lambda_env,
    )?;
    let mut typed_lambda_env = typed_lambda_env.clone();
    typed_lambda_env.insert_local(param.to_string(), receiver_ty);
    let receiver_expr = Expr::Var(param.to_string());
    let target_plan = plan_stream_lambda_call_target(
        Some(&receiver_expr),
        method,
        0,
        None,
        facts,
        ctx.external_index,
        &typed_lambda_env,
    );
    if matches!(
        target_plan,
        crate::call_plan::CallTargetPlan::NativeCollectionMethod { .. }
            | crate::call_plan::CallTargetPlan::NativeJavaInstance {
                method: crate::call_plan::NativeJavaInstanceMethod::VirtualObjectToString,
                ..
            }
    ) {
        return emit_stream_lambda_shared_native_call(
            &receiver_expr,
            method,
            &[],
            None,
            param,
            param,
            value,
            &[param],
            ctx,
            param_shape,
        )
        .map(|lowered| lowered.expr);
    }
    let crate::call_plan::CallTargetPlan::NativeJavaInstance { .. } = target_plan else {
        return None;
    };
    let lowered_receiver = stream_method_reference_native_instance_receiver(
        target,
        param,
        value,
        param_shape,
        &target_plan,
    );
    Some(stream_call_target_projection(
        &target_plan,
        Some(lowered_receiver),
        Vec::new(),
        ctx,
    ))
}

fn stream_method_reference_native_instance_receiver(
    target: &str,
    param: &str,
    value: StreamLambdaValue,
    param_shape: Option<&RustSlotShape>,
    target_plan: &crate::call_plan::CallTargetPlan<'_>,
) -> RustExpr {
    if let Some(receiver) = required_optional_stream_param_receiver(value, param, param_shape) {
        return receiver;
    }
    match target_plan {
        crate::call_plan::CallTargetPlan::NativeJavaInstance {
            method: crate::call_plan::NativeJavaInstanceMethod::PrimitiveNumberValue { .. },
            ..
        } => stream_lambda_value_expr(StreamLambdaValue::BorrowedItem, param),
        _ if crate::stdlib::canonical_external_receiver_type_key(target) == "Object" => {
            stream_method_reference_object_receiver_expr(value, param)
        }
        _ => RustExpr::Var(param.to_string()),
    }
}

fn required_optional_stream_param_receiver(
    value: StreamLambdaValue,
    param: &str,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    param_shape?.optional_element_type()?;
    let borrowed_carrier = match value {
        // `Iterator::filter` lends one extra reference to the already borrowed
        // stream item. Remove only that closure reference; the remaining
        // `&Option<T>` is the carrier on which Java receiver null-checking acts.
        StreamLambdaValue::FilterBorrowed => {
            stream_lambda_value_expr(StreamLambdaValue::BorrowedItem, param)
        }
        StreamLambdaValue::BorrowedItem | StreamLambdaValue::OwnedItem => {
            RustExpr::Var(param.to_string())
        }
    };
    Some(crate::coercion::fallible_optional_receiver_projection(
        borrowed_carrier,
        crate::coercion::OptionalReceiverProjection::Shared,
    ))
}

fn stream_method_reference_object_receiver_expr(value: StreamLambdaValue, param: &str) -> RustExpr {
    stream_lambda_receiver_expr(value, param)
}

fn emit_nullable_stream_lambda_closure_body_expr(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    let StreamLambdaBody::Block(block) = lambda.body() else {
        return None;
    };
    let (guard_index, condition) = first_null_return_guard(block)?;
    let final_expr = lambda.body().return_expr()?;
    let mutated = stream_lambda_mutated_locals(block, ctx);
    let mut locals = vec![lambda.param_name()];
    let source_names = block
        .statements
        .iter()
        .take(block.statements.len().saturating_sub(1))
        .filter_map(|stmt| match stmt {
            Stmt::VarDecl { source_name, .. } => Some(source_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let access_names = crate::java_naming::source_binding_rust_names(
        std::iter::once(lambda.param_name()).chain(source_names.iter().copied()),
    );
    let mut scoped_env = stream_lambda_scoped_env(
        ctx,
        &lambda.param_name(),
        &lambda.rust_param_name(),
        param_shape,
    );
    if let Some(env) = scoped_env.as_mut() {
        for source_name in &source_names {
            if let Some(access_name) = access_names.get(*source_name) {
                env.insert_binding_access_name((*source_name).to_string(), access_name.clone());
            }
        }
    }
    let mut before_guard = Vec::new();
    for stmt in &block.statements[..guard_index] {
        let stmt_ctx = scoped_env
            .as_ref()
            .map(|env| ctx.with_env(env))
            .unwrap_or(ctx);
        before_guard.push(emit_stream_lambda_stmt_structured(
            stmt,
            &lambda.param_name(),
            &lambda.rust_param_name(),
            &locals,
            stmt_ctx,
            &mutated,
            value,
            param_shape,
        )?);
        if let Stmt::VarDecl {
            ty,
            name,
            source_name,
            binding,
            ..
        } = stmt
        {
            locals.push(source_name);
            let access_name = access_names
                .get(source_name.as_str())
                .cloned()
                .unwrap_or_else(|| name.clone());
            let shape = stream_lambda_local_shape(binding.as_ref(), name, ty, stmt_ctx)
                .unwrap_or_else(|| RustSlotShape::for_storage(ty.clone()));
            if let Some(env) = scoped_env.as_mut() {
                env.insert_local_shape_with_access_name_and_java_type(
                    source_name.clone(),
                    access_name.clone(),
                    shape.clone(),
                    ty.clone(),
                );
                if name != source_name {
                    if env.binding_access_name(name).is_some() {
                        env.insert_local_shape_with_java_type(name.clone(), shape, ty.clone());
                    } else {
                        env.insert_local_shape_with_access_name_and_java_type(
                            name.clone(),
                            access_name,
                            shape,
                            ty.clone(),
                        );
                    }
                }
            }
        }
    }
    let mut else_body = Vec::new();
    for stmt in &block.statements[guard_index + 1..block.statements.len().saturating_sub(1)] {
        let stmt_ctx = scoped_env
            .as_ref()
            .map(|env| ctx.with_env(env))
            .unwrap_or(ctx);
        else_body.push(emit_stream_lambda_stmt_structured(
            stmt,
            &lambda.param_name(),
            &lambda.rust_param_name(),
            &locals,
            stmt_ctx,
            &mutated,
            value,
            param_shape,
        )?);
        if let Stmt::VarDecl {
            ty,
            name,
            source_name,
            binding,
            ..
        } = stmt
        {
            locals.push(source_name);
            let access_name = access_names
                .get(source_name.as_str())
                .cloned()
                .unwrap_or_else(|| name.clone());
            let shape = stream_lambda_local_shape(binding.as_ref(), name, ty, stmt_ctx)
                .unwrap_or_else(|| RustSlotShape::for_storage(ty.clone()));
            if let Some(env) = scoped_env.as_mut() {
                env.insert_local_shape_with_access_name_and_java_type(
                    source_name.clone(),
                    access_name.clone(),
                    shape.clone(),
                    ty.clone(),
                );
                if name != source_name {
                    if env.binding_access_name(name).is_some() {
                        env.insert_local_shape_with_java_type(name.clone(), shape, ty.clone());
                    } else {
                        env.insert_local_shape_with_access_name_and_java_type(
                            name.clone(),
                            access_name,
                            shape,
                            ty.clone(),
                        );
                    }
                }
            }
        }
    }
    let body_ctx = scoped_env
        .as_ref()
        .map(|env| ctx.with_env(env))
        .unwrap_or(ctx);
    let condition = emit_stream_lambda_body_with_locals_prec(
        condition,
        &lambda.param_name(),
        &lambda.rust_param_name(),
        value,
        &locals,
        body_ctx,
        param_shape,
        0,
        false,
    )?
    .expr;
    let final_expr = emit_stream_lambda_body_with_locals_prec(
        final_expr,
        &lambda.param_name(),
        &lambda.rust_param_name(),
        value,
        &locals,
        body_ctx,
        param_shape,
        0,
        false,
    )?
    .expr;
    Some(nullable_stream_map_body_projection(
        before_guard,
        condition,
        else_body,
        final_expr,
    ))
}

fn first_null_return_guard(block: &crate::java_ir::Block) -> Option<(usize, &Expr)> {
    block
        .statements
        .iter()
        .enumerate()
        .find_map(|(index, stmt)| {
            let Stmt::If {
                condition,
                then_block,
                else_block: None,
            } = stmt
            else {
                return None;
            };
            if matches!(
                then_block.statements.as_slice(),
                [Stmt::Return(Some(Expr::Literal(Literal::Null)))]
            ) {
                Some((index, condition))
            } else {
                None
            }
        })
}

fn stream_lambda_scoped_env(
    ctx: StreamRenderContext<'_>,
    param: &str,
    param_rust: &str,
    param_shape: Option<&RustSlotShape>,
) -> Option<ShapeEnv> {
    let mut env = ctx.env?.clone();
    if let Some(param_shape) = param_shape {
        env.insert_param_shape_with_access_name_and_java_type(
            param.to_string(),
            param_rust.to_string(),
            param_shape.clone(),
            param_shape.java_ty.clone(),
        );
    } else {
        env.insert_binding_access_name(param.to_string(), param_rust.to_string());
    }
    Some(env)
}

fn emit_stream_lambda_prefix_stmts_and_locals<'a>(
    body: StreamLambdaBody<'a>,
    param: &'a str,
    param_rust: &str,
    ctx: StreamRenderContext<'_>,
    value: StreamLambdaValue,
    param_shape: Option<&RustSlotShape>,
) -> Option<(Vec<RustStmt>, Vec<&'a str>, Option<ShapeEnv>)> {
    let mut out = Vec::new();
    let mut locals = vec![param];
    let mut scoped_env = stream_lambda_scoped_env(ctx, param, param_rust, param_shape);
    let source_names = body
        .prefix_statements()
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::VarDecl { source_name, .. } => Some(source_name.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let access_names = crate::java_naming::source_binding_rust_names(
        std::iter::once(param).chain(source_names.iter().copied()),
    );
    if let Some(env) = scoped_env.as_mut() {
        for source_name in &source_names {
            if let Some(access_name) = access_names.get(*source_name) {
                env.insert_binding_access_name((*source_name).to_string(), access_name.clone());
            }
        }
    }
    let mutated = match body {
        StreamLambdaBody::Expr(_) => HashSet::new(),
        StreamLambdaBody::Block(block) => stream_lambda_mutated_locals(block, ctx),
        StreamLambdaBody::MethodReference { .. } => HashSet::new(),
    };
    for stmt in body.prefix_statements() {
        let stmt_ctx = scoped_env
            .as_ref()
            .map(|env| ctx.with_env(env))
            .unwrap_or(ctx);
        out.push(emit_stream_lambda_stmt_structured(
            stmt,
            param,
            param_rust,
            &locals,
            stmt_ctx,
            &mutated,
            value,
            param_shape,
        )?);
        if let Stmt::VarDecl {
            ty,
            name,
            source_name,
            binding,
            ..
        } = stmt
        {
            locals.push(source_name);
            let access_name = access_names
                .get(source_name.as_str())
                .cloned()
                .unwrap_or_else(|| name.clone());
            let shape = stream_lambda_local_shape(binding.as_ref(), name, ty, stmt_ctx)
                .unwrap_or_else(|| RustSlotShape::for_storage(ty.clone()));
            if let Some(env) = scoped_env.as_mut() {
                env.insert_local_shape_with_access_name_and_java_type(
                    source_name.clone(),
                    access_name.clone(),
                    shape.clone(),
                    ty.clone(),
                );
                if name != source_name {
                    if env.binding_access_name(name).is_some() {
                        env.insert_local_shape_with_java_type(name.clone(), shape, ty.clone());
                    } else {
                        env.insert_local_shape_with_access_name_and_java_type(
                            name.clone(),
                            access_name,
                            shape,
                            ty.clone(),
                        );
                    }
                }
            }
        }
    }
    Some((out, locals, scoped_env))
}

fn emit_stream_lambda_stmt_structured(
    stmt: &Stmt,
    param: &str,
    param_rust: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    mutated: &HashSet<String>,
    input_value: StreamLambdaValue,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustStmt> {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            source_name,
            initializer,
            binding,
            ..
        } => {
            let rust_name = ctx
                .env
                .and_then(|env| env.binding_access_name(source_name))
                .map(str::to_string)
                .unwrap_or_else(|| name.clone());
            let planned_shape = stream_lambda_local_shape(binding.as_ref(), name, ty, ctx);
            let value = initializer
                .as_ref()
                .and_then(|expr| {
                    emit_stream_lambda_local_initializer_expr(
                        ty,
                        expr,
                        param,
                        param_rust,
                        locals,
                        ctx,
                        input_value,
                        param_shape,
                    )
                    .map(|lowered| {
                        let Some(target) = planned_shape.as_ref() else {
                            return lowered;
                        };
                        let Some(actual) =
                            stream_lambda_arg_actual_shape(expr, param, locals, ctx, param_shape)
                        else {
                            return lowered;
                        };
                        let default_facts;
                        let facts = match ctx.facts {
                            Some(facts) => facts,
                            None => {
                                default_facts = FactSet::default();
                                &default_facts
                            }
                        };
                        convert_stream_lambda_value_to_shape(lowered, actual, target, facts)
                    })
                })
                .unwrap_or_else(|| {
                    function_call_projection(RustExpr::path(["Default", "default"]), Vec::new())
                });
            let rust_ty = planned_shape.as_ref().and_then(|shape| {
                ctx.facts
                    .map(|facts| rust_type_for_slot_with_facts(shape, facts))
            });
            Some(RustStmt::Let {
                mutable: mutated.contains(name)
                    || mutated.contains(source_name)
                    || mutated.contains(&rust_name),
                name: rust_name.clone(),
                ty: rust_ty,
                value: Some(value),
            })
        }
        Stmt::Expr(Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        }) => Some(RustStmt::Assign {
            target: emit_stream_lambda_body_with_locals_prec(
                left,
                param,
                param_rust,
                input_value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?
            .expr,
            value: emit_stream_lambda_assignment_value(
                left,
                right,
                param,
                param_rust,
                locals,
                ctx,
                input_value,
                param_shape,
            )?,
        }),
        Stmt::Expr(expr) => Some(RustStmt::Expr(
            emit_stream_lambda_body_with_locals_prec(
                expr,
                param,
                param_rust,
                input_value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?
            .expr,
        )),
        Stmt::Assign { target, value } => Some(RustStmt::Assign {
            target: emit_stream_lambda_body_with_locals_prec(
                target,
                param,
                param_rust,
                input_value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?
            .expr,
            value: emit_stream_lambda_assignment_value(
                target,
                value,
                param,
                param_rust,
                locals,
                ctx,
                input_value,
                param_shape,
            )?,
        }),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            let condition = emit_stream_lambda_body_with_locals_prec(
                condition,
                param,
                param_rust,
                input_value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?
            .expr;
            let then_body = then_block
                .statements
                .iter()
                .map(|stmt| {
                    emit_stream_lambda_stmt_structured(
                        stmt,
                        param,
                        param_rust,
                        locals,
                        ctx,
                        mutated,
                        input_value,
                        param_shape,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            let else_body = match else_block {
                Some(block) => block
                    .statements
                    .iter()
                    .map(|stmt| {
                        emit_stream_lambda_stmt_structured(
                            stmt,
                            param,
                            param_rust,
                            locals,
                            ctx,
                            mutated,
                            input_value,
                            param_shape,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?,
                None => Vec::new(),
            };
            Some(RustStmt::If {
                condition,
                then_body,
                else_body,
            })
        }
        _ => None,
    }
}

fn emit_stream_lambda_assignment_value(
    target: &Expr,
    value: &Expr,
    param: &str,
    param_rust: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    input_value: StreamLambdaValue,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    let lowered = emit_stream_lambda_body_with_locals_prec(
        value,
        param,
        param_rust,
        input_value,
        locals,
        ctx,
        param_shape,
        0,
        false,
    )?
    .expr;
    let Expr::Var(target_name) = target else {
        return Some(lowered);
    };
    let Some(target_shape) = ctx
        .env
        .and_then(|env| env.local_shape(target_name))
        .cloned()
    else {
        return Some(lowered);
    };
    let Some(actual_shape) = stream_lambda_arg_actual_shape(value, param, locals, ctx, param_shape)
    else {
        return Some(lowered);
    };
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    Some(convert_stream_lambda_value_to_shape(
        lowered,
        actual_shape,
        &target_shape,
        facts,
    ))
}

fn convert_stream_lambda_value_to_shape(
    lowered: RustExpr,
    actual: RustSlotShape,
    target: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    if let Some(element_ty) = target.optional_element_type()
        && actual.optional_element_type().is_none()
    {
        let element_shape = optional_element_slot_shape_with_facts(target, element_ty, facts);
        let plan = plan_lowered_slot_conversion(
            &LoweredType::Slot(Box::new(actual)),
            Slot::Assignment(element_shape),
            facts,
        );
        return function_call_projection(
            RustExpr::path(["Some"]),
            vec![apply_lowered_slot_conversion_to_expr(lowered, &plan)],
        );
    }
    let plan = plan_lowered_slot_conversion(
        &LoweredType::Slot(Box::new(actual)),
        Slot::Assignment(target.clone()),
        facts,
    );
    apply_lowered_slot_conversion_to_expr(lowered, &plan)
}

fn stream_lambda_local_shape(
    binding: Option<&crate::binding_id::BindingId>,
    name: &str,
    ty: &JavaType,
    ctx: StreamRenderContext<'_>,
) -> Option<RustSlotShape> {
    let locals = ctx.current_local_shapes?;
    if let (Some(signatures), Some(binding)) = (ctx.signatures, binding)
        && let Some(local) = signatures.local_plan_by_binding(locals, binding)
    {
        return Some(local.shape().clone());
    }
    idless_compatibility_local_plan_for_declared_type(locals, name, ty)
        .map(|local| local.shape().clone())
}

fn emit_stream_lambda_local_initializer_expr(
    ty: &JavaType,
    expr: &Expr,
    param: &str,
    param_rust: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    input_value: StreamLambdaValue,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    let mut lowered = emit_stream_lambda_body_with_locals_prec(
        expr,
        param,
        param_rust,
        input_value,
        locals,
        ctx,
        param_shape,
        0,
        false,
    )?
    .expr;
    let needs_optional_unwrap = stream_lambda_initializer_needs_optional_unwrap(ty, expr, ctx);
    if matches!(ty, JavaType::String) && matches!(expr, Expr::ArrayAccess(_, _)) {
        if needs_optional_unwrap {
            lowered = unwrap_java_null_receiver_projection(receiver_method_call_projection_exact(
                lowered,
                "as_ref",
                Vec::new(),
            ));
        }
        Some(apply_clone_projection(lowered))
    } else {
        if needs_optional_unwrap {
            lowered = unwrap_java_null_receiver_projection(lowered);
        }
        Some(lowered)
    }
}

fn stream_lambda_initializer_needs_optional_unwrap(
    target_ty: &JavaType,
    expr: &Expr,
    ctx: StreamRenderContext<'_>,
) -> bool {
    let Some(env) = ctx.env else {
        return false;
    };
    let actual_shape = infer_expr_shape_with_context(
        expr,
        env,
        ExprShapeContext::new(ctx.signatures, None, ctx.facts),
    );
    crate::types::optional_element_type(&actual_shape.java_ty)
        .is_some_and(|element_ty| element_ty == *target_ty)
}

fn stream_lambda_mutated_locals(
    block: &crate::java_ir::Block,
    ctx: StreamRenderContext<'_>,
) -> HashSet<String> {
    let mut mutated = HashSet::new();
    for stmt in &block.statements {
        collect_stream_stmt_mutated_locals(stmt, &mut mutated, ctx);
    }
    mutated
}

fn collect_stream_stmt_mutated_locals(
    stmt: &Stmt,
    mutated: &mut HashSet<String>,
    ctx: StreamRenderContext<'_>,
) {
    match stmt {
        Stmt::Assign { target, value } => {
            collect_stream_assignment_target(target, mutated, ctx);
            collect_stream_expr_mutated_locals(value, mutated, ctx);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_stream_expr_mutated_locals(expr, mutated, ctx);
        }
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_stream_expr_mutated_locals(initializer, mutated, ctx);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_stream_expr_mutated_locals(condition, mutated, ctx);
            for stmt in &then_block.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
            if let Some(else_block) = else_block {
                for stmt in &else_block.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
                }
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_stream_expr_mutated_locals(condition, mutated, ctx);
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
            if let Some(condition) = condition {
                collect_stream_expr_mutated_locals(condition, mutated, ctx);
            }
            for expr in update {
                collect_stream_expr_mutated_locals(expr, mutated, ctx);
            }
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_stream_expr_mutated_locals(iterable, mutated, ctx);
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
        }
        Stmt::Switch { selector, cases } => {
            collect_stream_expr_mutated_locals(selector, mutated, ctx);
            for case in cases {
                for stmt in &case.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
                }
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for stmt in resources {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
            for catch in catches {
                for stmt in &catch.body.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
                }
            }
            if let Some(finally_block) = finally_block {
                for stmt in &finally_block.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
                }
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            for stmt in &block.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated, ctx);
            }
        }
        Stmt::Return(None)
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => {}
    }
}

fn collect_stream_expr_mutated_locals(
    expr: &Expr,
    mutated: &mut HashSet<String>,
    ctx: StreamRenderContext<'_>,
) {
    match expr {
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => collect_stream_assignment_target(expr, mutated, ctx),
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_stream_expr_mutated_locals(expr, mutated, ctx);
        }
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            collect_stream_assignment_target(left, mutated, ctx);
            collect_stream_expr_mutated_locals(right, mutated, ctx);
        }
        Expr::Binary { left, right, .. } => {
            collect_stream_expr_mutated_locals(left, mutated, ctx);
            collect_stream_expr_mutated_locals(right, mutated, ctx);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_stream_expr_mutated_locals(condition, mutated, ctx);
            collect_stream_expr_mutated_locals(then_expr, mutated, ctx);
            collect_stream_expr_mutated_locals(else_expr, mutated, ctx);
        }
        Expr::Switch { selector, cases } => {
            collect_stream_expr_mutated_locals(selector, mutated, ctx);
            for case in cases {
                for label in &case.labels {
                    if let SwitchLabel::Expr(expr) = label {
                        collect_stream_expr_mutated_locals(expr, mutated, ctx);
                    }
                }
                collect_stream_expr_mutated_locals(&case.value, mutated, ctx);
            }
        }
        Expr::FieldAccess(receiver, _) => {
            collect_stream_expr_mutated_locals(receiver, mutated, ctx)
        }
        Expr::ArrayAccess(array, index) => {
            collect_stream_expr_mutated_locals(array, mutated, ctx);
            collect_stream_expr_mutated_locals(index, mutated, ctx);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_stream_expr_mutated_locals(expr, mutated, ctx);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_stream_expr_mutated_locals(expr, mutated, ctx);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for expr in items {
                collect_stream_expr_mutated_locals(expr, mutated, ctx);
            }
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
            ..
        } => {
            if let Some(receiver) = receiver {
                if stream_call_mutates_receiver(name, resolved.as_ref(), ctx) {
                    collect_stream_assignment_target(receiver, mutated, ctx);
                } else {
                    collect_stream_expr_mutated_locals(receiver, mutated, ctx);
                }
            }
            for arg in args {
                collect_stream_expr_mutated_locals(arg, mutated, ctx);
            }
        }
        Expr::New {
            qualifier, args, ..
        } => {
            if let Some(qualifier) = qualifier {
                collect_stream_expr_mutated_locals(qualifier, mutated, ctx);
            }
            for arg in args {
                collect_stream_expr_mutated_locals(arg, mutated, ctx);
            }
        }
        Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn stream_call_mutates_receiver(
    method_name: &str,
    resolved: Option<&crate::java_ir::ResolvedCall>,
    ctx: StreamRenderContext<'_>,
) -> bool {
    let Some(resolved) = resolved else {
        return false;
    };
    if !matches!(resolved.kind, CallKind::Instance) {
        return false;
    }
    let Some(facts) = ctx.facts else {
        return false;
    };
    [resolved.rust_name.as_str(), method_name]
        .iter()
        .any(|method| {
            facts
                .method_effect_for(&resolved.declaring_type, method)
                .is_some_and(|effect| {
                    effect.receiver_mutability == ReceiverMutabilityFact::MutatesReceiver
                })
        })
}

fn collect_stream_assignment_target(
    expr: &Expr,
    mutated: &mut HashSet<String>,
    ctx: StreamRenderContext<'_>,
) {
    match expr {
        Expr::Var(name) => {
            mutated.insert(name.clone());
            mutated.insert(crate::parser::to_snake_case(name));
        }
        Expr::FieldAccess(receiver, _) | Expr::ArrayAccess(receiver, _) => {
            collect_stream_assignment_target(receiver, mutated, ctx);
        }
        _ => collect_stream_expr_mutated_locals(expr, mutated, ctx),
    }
}

fn emit_stream_lambda_args_with_locals_prec(
    args: &[Expr],
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<Vec<StreamLambdaRenderedExpr>> {
    args.iter()
        .map(|arg| {
            emit_stream_lambda_body_with_locals_prec(
                arg,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )
        })
        .collect()
}

fn emit_stream_lambda_call_args_for_receiver_exprs(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&crate::java_ir::ResolvedCall>,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<Vec<StreamLambdaRenderedExpr>> {
    let default_args = emit_stream_lambda_args_with_locals_prec(
        args,
        param,
        param_rust,
        value,
        locals,
        ctx,
        param_shape,
    )?;
    let Some(facts) = ctx.facts else {
        return Some(default_args);
    };
    let Some(env) = ctx.env else {
        return Some(default_args);
    };
    let mut typed_lambda_env = env.type_env().clone();
    if let Some(param_shape) = param_shape {
        typed_lambda_env.insert_local(param.to_string(), param_shape.java_ty.clone());
    }
    if let Some(receiver) = receiver
        && let Some(shape) =
            stream_lambda_arg_actual_shape(receiver, param, locals, ctx, param_shape)
    {
        // Native argument planning must see the same qualified collection
        // element domain as the emitted receiver, including captured fields.
        match receiver {
            Expr::Var(name) if env.local_shape(name).is_some() => {
                typed_lambda_env.insert_local(name.clone(), shape.java_ty);
            }
            Expr::Var(name) => {
                typed_lambda_env.insert_field(name.clone(), shape.java_ty);
            }
            Expr::FieldAccess(owner, field) if matches!(owner.as_ref(), Expr::This) => {
                typed_lambda_env.insert_field(field.clone(), shape.java_ty);
            }
            _ => {}
        }
    }

    let default_signatures;
    let signatures = match ctx.signatures {
        Some(signatures) => signatures,
        None => {
            default_signatures = SignaturePlan::default();
            &default_signatures
        }
    };
    let plan = crate::call_plan::plan_call_arguments_with_facts_and_args(
        receiver,
        name,
        resolved,
        args,
        signatures,
        facts,
        &typed_lambda_env,
    );
    let crate::call_plan::ArgumentLoweringPlan::Unshaped { .. } = plan else {
        return Some(adapt_stream_lambda_args_for_plan_exprs(
            args,
            default_args,
            plan,
            param,
            locals,
            ctx,
            param_shape,
        ));
    };
    if let Some((_, _, signature)) =
        stream_source_call_signature(receiver, name, args, resolved, param, param_shape, ctx)
    {
        return Some(adapt_stream_lambda_args_for_plan_exprs(
            args,
            default_args,
            crate::call_plan::ArgumentLoweringPlan::Shaped(
                signature
                    .params()
                    .iter()
                    .map(|param| param.shape().clone())
                    .collect(),
            ),
            param,
            locals,
            ctx,
            param_shape,
        ));
    }
    Some(
        args.iter()
            .zip(default_args)
            .map(|(arg, lowered)| {
                recover_stream_lambda_borrowed_string_param_move_expr(
                    arg,
                    lowered,
                    param,
                    locals,
                    ctx,
                    param_shape,
                )
            })
            .collect(),
    )
}

fn adapt_stream_lambda_args_for_plan_exprs(
    args: &[Expr],
    lowered_args: Vec<StreamLambdaRenderedExpr>,
    plan: crate::call_plan::ArgumentLoweringPlan,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Vec<StreamLambdaRenderedExpr> {
    match plan {
        crate::call_plan::ArgumentLoweringPlan::Shaped(shapes) => args
            .iter()
            .zip(lowered_args)
            .zip(shapes.iter())
            .map(|((arg, lowered), slot)| {
                adapt_stream_lambda_arg_for_slot_expr(
                    arg,
                    lowered,
                    slot,
                    param,
                    locals,
                    ctx,
                    param_shape,
                )
            })
            .collect(),
        crate::call_plan::ArgumentLoweringPlan::Slotted(slots) => args
            .iter()
            .zip(lowered_args)
            .zip(slots.iter())
            .map(|((arg, lowered), slot)| {
                adapt_stream_lambda_arg_for_explicit_slot_expr(
                    arg,
                    lowered,
                    slot,
                    param,
                    locals,
                    ctx,
                    param_shape,
                )
            })
            .collect(),
        crate::call_plan::ArgumentLoweringPlan::Unshaped { .. } => {
            lowered_args.into_iter().collect()
        }
    }
}

fn emit_stream_lambda_call_args_for_resolved_exprs(
    name: &str,
    args: &[Expr],
    resolved: Option<&crate::java_ir::ResolvedCall>,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<Vec<StreamLambdaRenderedExpr>> {
    let default_args = emit_stream_lambda_args_with_locals_prec(
        args,
        param,
        param_rust,
        value,
        locals,
        ctx,
        param_shape,
    )?;
    let source_signature =
        stream_receiverless_call_signature(name, args.len(), args, param, param_shape, ctx);
    Some(
        args.iter()
            .enumerate()
            .zip(default_args)
            .map(|((index, arg), lowered)| {
                source_signature
                    .as_ref()
                    .and_then(|(_, signature)| signature.params().get(index))
                    .map(|param_plan| {
                        adapt_stream_lambda_arg_for_slot_expr(
                            arg,
                            lowered.clone(),
                            &param_plan.shape(),
                            param,
                            locals,
                            ctx,
                            param_shape,
                        )
                    })
                    .or_else(|| {
                        resolved
                            .and_then(|resolved| resolved_call_arg_target_type(resolved, index))
                            .map(|target| {
                                let slot = storage_shape_for_java_type(target.clone());
                                adapt_stream_lambda_arg_for_slot_expr(
                                    arg,
                                    lowered.clone(),
                                    &slot,
                                    param,
                                    locals,
                                    ctx,
                                    param_shape,
                                )
                            })
                    })
                    .unwrap_or_else(|| {
                        if matches!(arg, Expr::Var(name) if name == param)
                            && stream_lambda_borrowed_item_expr(lowered.expr(), param)
                        {
                            recover_stream_lambda_owned_arg_move_expr(
                                arg,
                                lowered,
                                param,
                                locals,
                                ctx.single_use_owned_input,
                            )
                        } else {
                            lowered
                        }
                    })
            })
            .collect(),
    )
}

fn stream_receiverless_call_signature<'a>(
    name: &str,
    arg_count: usize,
    args: &[Expr],
    param: &str,
    param_shape: Option<&RustSlotShape>,
    ctx: StreamRenderContext<'a>,
) -> Option<(String, &'a crate::signature::MethodSignaturePlan)> {
    let _ = arg_count;
    stream_source_call_signature(None, name, args, None, param, param_shape, ctx)
        .map(|(_, rust_name, signature)| (rust_name, signature))
}

fn stream_source_call_signature<'a>(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&crate::java_ir::ResolvedCall>,
    param: &str,
    param_shape: Option<&RustSlotShape>,
    ctx: StreamRenderContext<'a>,
) -> Option<(String, String, &'a crate::signature::MethodSignaturePlan)> {
    let signatures = ctx.signatures?;
    let env = ctx.env?;
    let owner = if let Some(receiver) = receiver {
        let mut types = env.type_env().clone();
        if let Some(shape) = param_shape {
            types.insert_local(param.to_string(), shape.java_ty.clone());
        }
        let ty = crate::types::infer_expr_type(receiver, &types);
        let ty = crate::types::optional_element_type(&ty).unwrap_or(ty);
        match ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
            _ => resolved?.declaring_type.clone(),
        }
    } else {
        resolved
            .map(|call| call.declaring_type.as_str())
            .or(ctx.current_type)?
            .to_string()
    };
    let owner = env
        .type_env()
        .static_type_alias(&owner)
        .unwrap_or(&owner)
        .to_string();
    let arg_types = args
        .iter()
        .map(|arg| {
            if matches!(arg, Expr::Var(arg_name) if arg_name == param)
                && let Some(param_shape) = param_shape
            {
                param_shape.java_ty.clone()
            } else {
                crate::types::infer_expr_type(arg, env.type_env())
            }
        })
        .collect::<Vec<_>>();
    let selected = ctx
        .facts
        .and_then(|facts| {
            signatures.method_signature_by_base_name_and_arg_types_with_facts(
                &owner, name, &arg_types, facts,
            )
        })
        .or_else(|| {
            signatures.method_signature_by_base_name_and_arg_types(&owner, name, &arg_types)
        })
        .or_else(|| {
            resolved
                .and_then(|call| signatures.method_signature_by_rust_name(&owner, &call.rust_name))
                .map(|signature| (resolved.unwrap().rust_name.clone(), signature))
        })
        .or_else(|| signatures.method_signature_by_base_name_and_arity(&owner, name, args.len()))?;
    let owner = signatures
        .method_identity_by_rust_name(&owner, &selected.0)
        .map(|(owner, _)| owner.to_string())
        .unwrap_or(owner);
    Some((owner, selected.0, selected.1))
}

fn adapt_stream_lambda_arg_for_slot_expr(
    arg: &Expr,
    lowered: StreamLambdaRenderedExpr,
    slot: &RustSlotShape,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> StreamLambdaRenderedExpr {
    let slot_plan = stream_lambda_argument_slot_plan(slot);
    let planned = render_stream_lambda_arg_to_slot_through_plan_expr(
        arg,
        lowered.clone(),
        slot,
        param,
        locals,
        ctx,
        param_shape,
    );
    let adapted = match slot_plan {
        StreamLambdaArgumentSlotPlan::BorrowedStringView => planned.unwrap_or_else(|| {
            render_stream_lambda_borrowed_str_arg_expr(
                arg,
                lowered,
                param,
                locals,
                ctx,
                param_shape,
            )
        }),
        StreamLambdaArgumentSlotPlan::OwnedValue => recover_stream_lambda_owned_arg_move_expr(
            arg,
            planned.unwrap_or(lowered),
            param,
            locals,
            ctx.single_use_owned_input,
        ),
        StreamLambdaArgumentSlotPlan::GenericSlotConversion => planned.unwrap_or(lowered),
    };
    recover_stream_lambda_borrowed_string_param_move_expr(
        arg,
        adapted,
        param,
        locals,
        ctx,
        param_shape,
    )
}

fn render_stream_lambda_borrowed_str_arg_expr(
    arg: &Expr,
    lowered: StreamLambdaRenderedExpr,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> StreamLambdaRenderedExpr {
    let target = borrowed_string_content_shape();
    if let Some(planned) = render_stream_lambda_arg_to_slot_through_plan_expr(
        arg,
        lowered.clone(),
        &target,
        param,
        locals,
        ctx,
        param_shape,
    ) {
        return planned;
    }
    let actual = stream_lambda_arg_actual_shape(arg, param, locals, ctx, param_shape);
    let Some(actual) = actual else {
        return lowered;
    };
    stream_lambda_expr(
        ensure_borrow_projection(
            lowered.expr,
            Some(&actual),
            Some(&target),
            BorrowProjectionUse::StringView,
        ),
        11,
    )
}

fn adapt_stream_lambda_arg_for_explicit_slot_expr(
    arg: &Expr,
    lowered: StreamLambdaRenderedExpr,
    slot: &Slot,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> StreamLambdaRenderedExpr {
    match slot {
        Slot::Argument(shape) => adapt_stream_lambda_arg_for_slot_expr(
            arg,
            lowered,
            shape,
            param,
            locals,
            ctx,
            param_shape,
        ),
        Slot::Index => stream_lambda_expr(
            index_to_usize_projection(lowered.expr, IndexToUsizeProjection::StreamLambdaArgument),
            11,
        ),
        Slot::Value
        | Slot::Condition
        | Slot::Assignment(_)
        | Slot::Return(_)
        | Slot::StringConcatSegment
        | Slot::DisplaySegment => lowered,
    }
}

fn render_stream_lambda_arg_to_slot_through_plan_expr(
    arg: &Expr,
    lowered: StreamLambdaRenderedExpr,
    target: &RustSlotShape,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<StreamLambdaRenderedExpr> {
    let mut actual = lowered
        .expr
        .recorded_slot_shape()
        .cloned()
        .or_else(|| stream_lambda_arg_actual_shape(arg, param, locals, ctx, param_shape))?;
    let mut lowered = lowered;
    // The stream projector spells a borrowed Java variable as a dereferenced
    // Rust place. A borrow-shaped slot expects the reference to that place,
    // not one further dereference merely because the iterator added a layer.
    if matches!(
        target.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && matches!(
        actual.ownership,
        OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
    ) && matches!(arg, Expr::Var(name) if name == param)
        && let RustExpr::Unary { op, expr } = lowered.expr.unshaped()
        && op == "*"
    {
        lowered.expr = expr.as_ref().clone();
    }
    let mut recovered_before_conversion = false;
    if target.ownership == OwnershipShape::Owned
        && matches!(
            actual.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        )
        && !actual.capabilities().is_copy()
        && matches!(arg, Expr::Var(name) if name == param)
        && matches!(lowered.expr.unshaped(), RustExpr::Unary { op, .. } if op == "*")
    {
        // The projector produced an owned Rust place behind the cursor loan,
        // not an Option<&T> facade. Recover that place before nullable slot
        // conversion, so its borrowed-optional adapter cannot emit `.cloned()`
        // on an Option<T>. This also preserves the input for later predicates.
        lowered = recover_stream_lambda_owned_arg_move_expr(arg, lowered, param, locals, false);
        actual.ownership = OwnershipShape::Owned;
        recovered_before_conversion = true;
    }
    if target.ownership == OwnershipShape::Owned
        && actual.ownership == OwnershipShape::Owned
        && !actual.capabilities().is_copy()
        && !recovered_before_conversion
    {
        let recovered = recover_stream_lambda_owned_arg_move_expr(
            arg,
            lowered.clone(),
            param,
            locals,
            ctx.single_use_owned_input,
        );
        recovered_before_conversion = recovered.expr != lowered.expr;
        lowered = recovered;
    }
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let plan = plan_lowered_slot_conversion(
        &LoweredType::Slot(Box::new(actual.clone())),
        Slot::Argument(target.clone()),
        facts,
    );
    let converted = apply_lowered_slot_conversion_to_expr(lowered.expr, &plan);
    Some(stream_lambda_expr(
        if recovered_before_conversion {
            RustExpr::shaped(crate::rust_ir::RustExprShape::OwnedRecovery, converted)
        } else {
            converted
        },
        11,
    ))
}

fn stream_lambda_cast_plan(
    target: &JavaType,
    source: &Expr,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<crate::conversion_plan::CastConversionPlan> {
    let actual = stream_lambda_arg_actual_shape(source, param, locals, ctx, param_shape)?;
    // This renderer handles identity reference casts, not checked narrowing.
    // Keep nominal admission separate from the conversion planner's recovery
    // policies for partially modeled external types.
    let actual_ty = actual
        .optional_element_type()
        .unwrap_or_else(|| actual.java_ty.clone());
    let target_ty = crate::types::optional_element_type(target).unwrap_or_else(|| target.clone());
    if actual_ty != target_ty {
        return None;
    }
    let plan = crate::conversion_plan::plan_cast_conversion_from_actual(
        actual,
        target,
        ExprShapeContext::new(ctx.signatures, ctx.representations, ctx.facts),
    );
    matches!(
        plan.semantics(),
        crate::conversion_plan::JavaCastSemanticsPolicy::Identity
    )
    .then_some(plan)
}

fn stream_lambda_arg_actual_shape(
    arg: &Expr,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustSlotShape> {
    match arg {
        Expr::Var(name) if name == param => param_shape
            .cloned()
            .or_else(|| ctx.env.and_then(|env| env.local_shape(name)).cloned()),
        Expr::Cast { ty, expr } => {
            stream_lambda_cast_plan(ty, expr, param, locals, ctx, param_shape)
                .map(|plan| plan.target().clone())
        }
        Expr::Var(name) if locals.iter().any(|local| *local == name) => {
            ctx.env.and_then(|env| env.local_shape(name)).cloned()
        }
        Expr::Var(name) => ctx
            .env
            .and_then(|env| env.local_shape(name).or_else(|| env.field_shape(name)))
            .cloned(),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            ctx.env.and_then(|env| env.field_shape(field)).cloned()
        }
        _ => ctx.env.map(|env| {
            infer_expr_shape_with_context(
                arg,
                env,
                ExprShapeContext::new(ctx.signatures, None, ctx.facts),
            )
        }),
    }
}

fn recover_stream_lambda_owned_arg_move_expr(
    arg: &Expr,
    lowered: StreamLambdaRenderedExpr,
    param: &str,
    locals: &[&str],
    single_use_owned_input: bool,
) -> StreamLambdaRenderedExpr {
    if lowered.expr.recorded_shape() == Some(crate::rust_ir::RustExprShape::OwnedRecovery) {
        return lowered;
    }
    match arg {
        Expr::Var(name) if name == param && !single_use_owned_input => {
            stream_lambda_expr(apply_clone_projection(lowered.expr), 11)
        }
        Expr::Var(name) if locals.iter().all(|local| *local != name) => {
            stream_lambda_expr(apply_clone_projection(lowered.expr), 11)
        }
        Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This) => {
            stream_lambda_expr(apply_clone_projection(lowered.expr), 11)
        }
        _ => lowered,
    }
}

fn recover_stream_lambda_borrowed_string_param_move_expr(
    arg: &Expr,
    lowered: StreamLambdaRenderedExpr,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> StreamLambdaRenderedExpr {
    if matches!(arg, Expr::Var(name) if name == param)
        && stream_lambda_borrowed_item_expr(lowered.expr(), param)
        && stream_lambda_arg_actual_shape(arg, param, locals, ctx, param_shape)
            .is_some_and(|shape| shape.java_ty == JavaType::String)
    {
        // Clone the projected String, not the closure's reference to it.
        // Explicit Clone::clone on &(&String) only copies the reference.
        stream_lambda_expr(apply_clone_projection(lowered.expr), 11)
    } else {
        lowered
    }
}

fn parenthesize_stream_expr_if_needed(
    lowered: StreamLambdaRenderedExpr,
    parent_prec: u8,
    right_child: bool,
    binary_op: Option<BinaryOp>,
) -> Option<StreamLambdaRenderedExpr> {
    let needs_parens = lowered.precedence < parent_prec
        || (right_child
            && lowered.precedence == parent_prec
            && binary_op.is_some_and(|op| !stream_binary_op_is_associative(op)));
    Some(if needs_parens {
        stream_lambda_expr(RustExpr::Grouped(Box::new(lowered.expr)), 12)
    } else {
        lowered
    })
}

fn stream_binary_expr_projection(
    op: BinaryOp,
    left: StreamLambdaRenderedExpr,
    right: StreamLambdaRenderedExpr,
) -> RustExpr {
    RustExpr::Binary {
        op: binary_op_to_rust(op).to_string(),
        left: Box::new(left.expr),
        right: Box::new(right.expr),
    }
}

fn stream_unary_expr_projection(op: &str, expr: StreamLambdaRenderedExpr) -> RustExpr {
    RustExpr::Unary {
        op: op.to_string(),
        expr: Box::new(expr.expr),
    }
}

fn stream_binary_precedence(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Assign => 1,
        BinaryOp::Or => 2,
        BinaryOp::And => 3,
        BinaryOp::BitOr => 4,
        BinaryOp::BitXor => 5,
        BinaryOp::BitAnd => 6,
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            7
        }
        BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr => 8,
        BinaryOp::Add | BinaryOp::Sub => 9,
        BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => 10,
    }
}

fn stream_binary_op_is_associative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Add
            | BinaryOp::Mul
    )
}

fn stream_static_owner_expr_type_name(
    receiver: &Expr,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    stream_static_owner_expr_segment(receiver, locals, ctx)
}

fn stream_static_owner_expr_java_name(
    receiver: &Expr,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    match receiver {
        Expr::Var(name) => {
            if locals.iter().any(|local| *local == name) {
                return None;
            }
            if ctx
                .env
                .is_some_and(|env| env.local_type(name).is_some() || env.field_type(name).is_some())
            {
                return None;
            }
            Some(
                ctx.env
                    .and_then(|env| env.type_env().static_type_alias(name))
                    .unwrap_or(name)
                    .to_string(),
            )
        }
        Expr::FieldAccess(_, _) => crate::stdlib::qualified_static_owner_java_name(receiver),
        _ => None,
    }
}

fn stream_native_static_owner_call_expr(
    owner: &str,
    name: &str,
    source_args: &[Expr],
    lowered_args: Vec<RustExpr>,
    ctx: StreamRenderContext<'_>,
) -> Option<RustExpr> {
    let default_types;
    let types = if let Some(env) = ctx.env {
        env.type_env()
    } else {
        default_types = TypeEnv::default();
        &default_types
    };
    if stream_static_owner_alias_may_be_source(owner, types) {
        return None;
    }
    let target = plan_native_java_static_owner_call(owner, name, Some(source_args), types)?;
    Some(stream_call_target_projection(
        &target,
        None,
        lowered_args,
        ctx,
    ))
}

fn stream_static_owner_alias_may_be_source(owner: &str, types: &TypeEnv) -> bool {
    let Some(alias) = types.static_type_alias(owner) else {
        return false;
    };
    alias == owner && !owner.contains('.')
}

fn stream_static_constant_owner_name(
    receiver: &Expr,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    let Expr::Var(name) = receiver else {
        return None;
    };
    if locals.iter().any(|local| *local == name) {
        return None;
    }
    if ctx
        .env
        .is_some_and(|env| env.local_type(name).is_some() || env.field_type(name).is_some())
    {
        return None;
    }
    Some(stream_static_raw_owner_name(name, ctx))
}

fn stream_static_owner_segment_name(owner: &str) -> String {
    if owner.contains('.') {
        return crate::java_naming::rust_name_for_java_named_type(owner);
    }
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in owner.chars() {
        if ch == '_' || ch == '-' || ch == '$' {
            uppercase_next = true;
        } else if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn stream_static_owner_type_name_with_context(owner: &str, ctx: StreamRenderContext<'_>) -> String {
    if let Some((representations, facts)) = ctx.representations.zip(ctx.facts)
        && crate::effective_facts::EffectiveFacts::new(facts)
            .has_parsed_source_type_capability_for_name(owner)
    {
        return crate::emit::emit_type(&crate::slot_render::rust_source_static_owner_type(
            owner,
            &crate::effective_facts::EffectiveFacts::new(facts),
            representations,
        ));
    }
    let rendered = ctx
        .facts
        .map(|facts| crate::call_plan::rust_static_owner_type_name_with_facts(owner, facts))
        .unwrap_or_else(|| crate::call_plan::rust_static_owner_type_name(owner));
    if rendered == crate::parser::to_snake_case(owner)
        && owner
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        stream_static_owner_segment_name(owner)
    } else {
        rendered
    }
}

fn stream_static_owner_expr_segment(
    receiver: &Expr,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    match receiver {
        Expr::Var(name) => {
            if locals.iter().any(|local| *local == name) {
                return None;
            }
            if ctx
                .env
                .is_some_and(|env| env.local_type(name).is_some() || env.field_type(name).is_some())
            {
                return None;
            }
            if let Some(owner) = ctx
                .env
                .and_then(|env| env.type_env().static_type_alias(name))
            {
                return Some(stream_static_owner_type_name_with_context(owner, ctx));
            }
            if !is_static_owner_name(name) {
                return None;
            }
            Some(stream_static_owner_type_name_with_context(name, ctx))
        }
        Expr::FieldAccess(inner, field) => {
            if let Some(owner) = crate::stdlib::qualified_static_owner_java_name(receiver) {
                return Some(stream_static_owner_type_name_with_context(&owner, ctx));
            }
            let _ = stream_static_owner_expr_segment(inner, locals, ctx)?;
            Some(stream_static_owner_type_name_with_context(field, ctx))
        }
        _ => None,
    }
}

fn is_static_owner_name(name: &str) -> bool {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    if matches!(
        canonical.as_str(),
        "Integer" | "Math" | "Objects" | "Collectors" | "Constants"
    ) {
        return true;
    }
    if canonical
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        return true;
    }
    let owner = crate::call_plan::rust_static_owner_type_name(name);
    owner
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        && owner != crate::parser::to_snake_case(name)
}

fn is_static_constant_member(member: &str) -> bool {
    member
        .chars()
        .any(|ch| ch == '_' || ch.is_ascii_uppercase())
        && member
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn stream_lambda_type_name(ty: &JavaType) -> Option<String> {
    match ty {
        JavaType::Named { name, .. } => Some(
            name.rsplit('.')
                .next()
                .unwrap_or(name)
                .rsplit('$')
                .next()
                .unwrap_or(name)
                .to_string(),
        ),
        JavaType::String => Some("String".to_string()),
        _ => None,
    }
}

fn emit_stream_lambda_receiver_expr(
    expr: &Expr,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
) -> Option<RustExpr> {
    emit_stream_lambda_receiver_with_locals_expr(
        expr,
        param,
        param_rust,
        value,
        &[param],
        ctx,
        None,
    )
}

fn emit_stream_lambda_receiver_with_locals_expr(
    expr: &Expr,
    param: &str,
    param_rust: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    match expr {
        Expr::Var(name) if name == param => Some(match value {
            StreamLambdaValue::FilterBorrowed
            | StreamLambdaValue::BorrowedItem
            | StreamLambdaValue::OwnedItem => RustExpr::Var(param_rust.to_string()),
        }),
        _ => emit_stream_lambda_body_with_locals_prec(
            expr,
            param,
            param_rust,
            value,
            locals,
            ctx,
            param_shape,
            0,
            false,
        )
        .map(|lowered| lowered.expr),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_source_alias_keeps_overloaded_predicate_effect_identity() {
        let parsed = crate::parser::parse_java_source(
            "interface Predicate<T> { boolean accepts(T value); boolean accepts(T left, T right); }\n\
             class Impl<T> implements Predicate<T> {\n\
               public boolean accepts(T value) { return false; }\n\
               public boolean accepts(T left, T right) { throw new IllegalArgumentException(); }\n\
             }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let reps = crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &reps,
            &semantic.usage,
            &semantic.facts,
        );
        let graph = crate::usage::build_call_graph_with_facts(&resolved.program, &semantic.facts);
        let fallibility = crate::exception_plan::plan_program_fallibility_with_signatures(
            &resolved.program,
            &graph,
            &semantic.facts,
            Some(&signatures),
        );
        let exceptions = crate::exception_plan::plan_program_exceptions_with_fallibility(
            &resolved.program,
            &semantic.facts,
            &fallibility,
        );
        let analysis = CallableAnalysis::new(&semantic.usage, &exceptions, &fallibility);
        let mut env = ShapeEnv::new();
        env.insert_local(
            "predicate",
            JavaType::Named {
                name: "sample.Predicate".into(),
                args: vec![JavaType::String],
            },
        );
        env.insert_local("pivot", JavaType::String);
        let external_index = semantic.facts.external_lookup_index();
        let ctx = StreamRenderContext::new_with_facts_analysis_and_self_receiver(
            &env,
            &semantic.facts,
            &signatures,
            &analysis,
            &external_index,
            None,
            false,
            true,
        );
        let shape = RustSlotShape::for_storage(JavaType::String);
        let expression = parse_expr("predicate.accepts(value, pivot) && !predicate.accepts(value)");
        let rendered = emit_stream_lambda_body_with_locals(
            &expression,
            "value",
            StreamLambdaValue::OwnedItem,
            &["value"],
            ctx,
            Some(&shape),
        )
        .unwrap();
        assert!(rendered.contains("accepts_t_t"), "{rendered}");
        assert_eq!(
            rendered.matches('?').count(),
            1,
            "only the checked overload propagates: {rendered}"
        );
        let facade = emit_stream_lambda_body_with_locals(
            &expression,
            "value",
            StreamLambdaValue::OwnedItem,
            &["value"],
            StreamRenderContext {
                propagate_java_exceptions: false,
                ..ctx
            },
            Some(&shape),
        )
        .unwrap();
        assert!(
            !facade.contains(")?"),
            "an infallible facade must not acquire propagation: {facade}"
        );
        assert!(facade.contains("unwrap_or_else"), "{facade}");
    }

    #[test]
    fn native_stream_instance_calls_require_a_rendered_receiver() {
        let mut env = ShapeEnv::new();
        let set = JavaType::Named {
            name: "Set".to_string(),
            args: vec![JavaType::Primitive(PrimitiveType::Int)],
        };
        env.insert_field("selected", set);
        env.insert_local(
            "opaque",
            JavaType::Named {
                name: "Object".to_string(),
                args: Vec::new(),
            },
        );
        let facts = FactSet::default();
        let signatures = SignaturePlan::default();
        let external_index = facts.external_lookup_index();
        let ctx = StreamRenderContext::new_with_facts_and_self_receiver(
            &env,
            &facts,
            &signatures,
            &external_index,
            Some("Owner"),
            true,
            false,
        );
        let expression = parse_expr("this.selected.contains(value)");
        let rendered = emit_stream_lambda_body_with_context(
            &expression,
            "value",
            StreamLambdaValue::OwnedItem,
            ctx,
        )
        .unwrap();
        assert!(rendered.contains("self.selected.contains"), "{rendered}");
        let expression = parse_expr("((Set<Integer>) opaque).contains(value)");
        let Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } = expression
        else {
            panic!("expected call");
        };
        assert!(matches!(
            plan_stream_lambda_call_target(
                Some(&receiver),
                &name,
                args.len(),
                None,
                &facts,
                Some(&external_index),
                env.type_env()
            ),
            crate::call_plan::CallTargetPlan::NativeCollectionMethod { .. }
        ));
        let unsupported_receiver = emit_stream_lambda_shared_native_call(
            &receiver,
            &name,
            &args,
            None,
            "value",
            "value",
            StreamLambdaValue::OwnedItem,
            &["value"],
            ctx,
            None,
        );
        assert!(
            unsupported_receiver.is_none(),
            "receiver={receiver:?} shape={:?} rendered={unsupported_receiver:?}",
            stream_lambda_arg_actual_shape(&receiver, "value", &["value"], ctx, None)
        );
        let no_self = StreamRenderContext::new_with_facts(&env, &facts);
        assert!(
            emit_stream_lambda_body_with_context(
                &parse_expr("this.selected.contains(value)"),
                "value",
                StreamLambdaValue::OwnedItem,
                no_self
            )
            .is_none()
        );
    }

    #[test]
    fn primitive_mapping_does_not_invent_a_sum_for_a_numeric_slot() {
        let expr = parse_expr("Arrays.stream(values).mapToInt(String::length)");
        let mut env = TypeEnv::new();
        env.insert_local("values", JavaType::Array(Box::new(JavaType::String)));
        let facts = FactSet::default();
        assert!(crate::stream::build_stream_plan(&expr, &env, &facts).is_none());
        assert!(crate::stream::build_stream_plan_for_slot(&expr, &env, &facts).is_none());
    }

    #[test]
    fn constructor_reference_uses_type_namespace_despite_same_named_local() {
        let expr =
            parse_expr("Arrays.stream(values).map(Factory::new).collect(Collectors.toList())");
        let mut env = ShapeEnv::new();
        env.insert_local_shape(
            "values".to_string(),
            RustSlotShape::conservative(JavaType::Array(Box::new(JavaType::String))),
        );
        env.insert_local_shape(
            "Factory".to_string(),
            RustSlotShape::conservative(JavaType::Named {
                name: "Factory".to_string(),
                args: Vec::new(),
            }),
        );
        let facts = FactSet::default();
        let plan = crate::stream::build_stream_plan(&expr, env.type_env(), &facts).unwrap();
        let lowered = render_stream_plan_with_context(
            &plan,
            RustExpr::Var("values".to_string()),
            StreamRenderContext::new_with_facts(&env, &facts),
        )
        .unwrap();
        let rust = crate::emit::emit_expr(&lowered.into_expr());
        assert!(rust.contains("Factory::new"), "{rust}");
        assert!(!rust.contains("factory.new"), "{rust}");
    }
    use crate::emit::emit_expr;
    use crate::facts::{ExternalConstructorFact, FactProvenance};
    use crate::java_ir::PrimitiveType;
    use crate::parser::parse_expr;

    #[test]
    fn stream_lambda_constructor_rendering_uses_external_constructor_arity_names() {
        let mut facts = FactSet::default();
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: "SimpleInterval".to_string(),
                arg_types: vec![JavaType::Named {
                    name: "Locatable".to_string(),
                    args: Vec::new(),
                }],
                provenance: FactProvenance::ExternalLibraryPack,
            });
        facts
            .external_constructor_facts
            .push(ExternalConstructorFact {
                type_name: "SimpleInterval".to_string(),
                arg_types: vec![
                    JavaType::String,
                    JavaType::Primitive(PrimitiveType::Int),
                    JavaType::Primitive(PrimitiveType::Int),
                ],
                provenance: FactProvenance::ExternalLibraryPack,
            });
        let env = ShapeEnv::new();
        let ctx = StreamRenderContext::new_with_facts(&env, &facts);

        let rendered = emit_stream_lambda_body_with_context(
            &parse_expr("new SimpleInterval(contig, start, end)"),
            "partition",
            StreamLambdaValue::BorrowedItem,
            ctx,
        );

        assert_eq!(
            rendered,
            Some("SimpleInterval::new_3(contig, start, end)".to_string())
        );
    }

    #[test]
    fn stream_mutated_local_scan_sees_qualified_new_receiver() {
        let expr = Expr::New {
            infer_type_arguments: false,
            ty: JavaType::Unknown("Inner".to_string()),
            qualifier: Some(Box::new(Expr::Unary {
                op: UnaryOp::PostInc,
                expr: Box::new(Expr::Var("value".to_string())),
            })),
            args: Vec::new(),
            body: None,
            resolved: None,
        };
        let mut mutated = HashSet::new();

        collect_stream_expr_mutated_locals(&expr, &mut mutated, StreamRenderContext::empty());

        assert_eq!(mutated, HashSet::from(["value".to_string()]));
    }

    #[test]
    fn stream_lambda_emission_owns_java_string_method_mappings() {
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("value.trim()"),
                "value",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::empty(),
            )
            .unwrap(),
            "value.trim().to_string()"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("value.isEmpty()"),
                "value",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::empty(),
            )
            .unwrap(),
            "value.is_empty()"
        );
        assert!(matches!(
            emit_stream_lambda_body_expr(
                &parse_expr("value.isEmpty()"),
                "value",
                StreamLambdaValue::BorrowedItem
            )
            .unwrap()
            .unshaped(),
            RustExpr::MethodCall { method, .. } if method == "is_empty"
        ));
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("value.toLowerCase()"),
                "value",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::empty(),
            )
            .unwrap(),
            "value.to_lowercase()"
        );
    }

    #[test]
    fn stream_lambda_emission_uses_structured_operator_rendering() {
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("value - (other - third)"),
                "value",
                StreamLambdaValue::OwnedItem,
                StreamRenderContext::empty(),
            )
            .unwrap(),
            "value - (other - third)"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("-(value + other)"),
                "value",
                StreamLambdaValue::OwnedItem,
                StreamRenderContext::empty(),
            )
            .unwrap(),
            "-(value + other)"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("value < other && other < third"),
                "value",
                StreamLambdaValue::OwnedItem,
                StreamRenderContext::empty(),
            )
            .unwrap(),
            "value < other && other < third"
        );
        let parsed_binary = emit_stream_lambda_body_expr(
            &parse_expr("left + right"),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&parsed_binary), "left + right");
        assert!(matches!(
            parsed_binary,
            RustExpr::Binary { op, left, right }
                if op == "+"
                    && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                    && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
        ));
        let raw_binary = emit_stream_lambda_body_expr(
            &Expr::Raw("left + right".to_string()),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&raw_binary), "left + right");
        assert!(matches!(
            raw_binary,
            RustExpr::Binary { op, left, right }
                if op == "+"
                    && matches!(left.as_ref(), RustExpr::Var(name) if name == "left")
                    && matches!(right.as_ref(), RustExpr::Var(name) if name == "right")
        ));
        let opaque_raw = emit_stream_lambda_body_expr(
            &Expr::Raw("left +".to_string()),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(opaque_raw, RustExpr::Raw("left +".to_string()));
        let parsed_static_constant = emit_stream_lambda_body_expr(
            &parse_expr("Owner.VALUE"),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&parsed_static_constant), "Owner::VALUE");
        assert!(matches!(
            parsed_static_constant,
            RustExpr::Path(segments)
                if segments == vec!["Owner".to_string(), "VALUE".to_string()]
        ));
        let raw_static_constant = emit_stream_lambda_body_expr(
            &Expr::Raw("Owner.VALUE".to_string()),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&raw_static_constant), "Owner.VALUE");
        assert!(matches!(
            raw_static_constant,
            RustExpr::Field(receiver, field)
                if field == "VALUE"
                    && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "Owner")
        ));
        let qualified_static_constant = emit_stream_lambda_body_expr(
            &parse_expr("a.Item.KIND"),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&qualified_static_constant), "AItem::KIND");
        let raw_qualified_static_constant = emit_stream_lambda_body_expr(
            &Expr::Raw("a.Item.KIND".to_string()),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&raw_qualified_static_constant), "a.Item.KIND");
        let raw_b_qualified_static_constant = emit_stream_lambda_body_expr(
            &Expr::Raw("b.Item.KIND".to_string()),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&raw_b_qualified_static_constant), "b.Item.KIND");
        let raw_instance_field = emit_stream_lambda_body_expr(
            &Expr::Raw("owner.value".to_string()),
            "value",
            StreamLambdaValue::OwnedItem,
        )
        .unwrap();
        assert_eq!(emit_expr(&raw_instance_field), "owner.value");
        assert!(matches!(
            raw_instance_field,
            RustExpr::Field(receiver, field)
                if field == "value"
                    && matches!(receiver.as_ref(), RustExpr::Var(name) if name == "owner")
        ));
    }

    #[test]
    fn stream_lambda_emission_uses_shape_env_for_bare_fields_and_static_fields() {
        let mut env = ShapeEnv::new();
        env.insert_field_shape("workspace", RustSlotShape::for_storage(JavaType::String));
        env.insert_static_field_shape(
            "ARRAY_COLUMN_BOUNDS_START",
            RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
        );
        env.insert_static_type_alias("genomics_d_b_utils", "org.genomicsdb.GenomicsDBUtils");
        // Source expressions retain Java type spelling; the normalized alias
        // is still seeded separately for compatibility inputs below.
        env.insert_static_type_alias("GenomicsDBUtils", "org.genomicsdb.GenomicsDBUtils");
        env.insert_local_shape(
            "vid_map_p_b",
            RustSlotShape::for_storage(JavaType::Unknown("VidMapPB".to_string())),
        );
        let ctx = StreamRenderContext::new(&env);

        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("workspace"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                ctx,
            )
            .unwrap(),
            "self.workspace"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("ARRAY_COLUMN_BOUNDS_START"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                ctx,
            )
            .unwrap(),
            "Self::ARRAY_COLUMN_BOUNDS_START"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("GenomicsDBUtils.getArrayColumnBounds(workspace, partition)"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                ctx,
            )
            .unwrap(),
            "OrgGenomicsdbGenomicsDBUtils::get_array_column_bounds(self.workspace, *partition)"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("genomics_d_b_utils.getArrayColumnBounds(workspace, partition)"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                ctx,
            )
            .unwrap(),
            "OrgGenomicsdbGenomicsDBUtils::get_array_column_bounds(self.workspace, *partition)"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("vidMapPB.getContigsList()"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                ctx,
            )
            .unwrap(),
            "vid_map_p_b.get_contigs_list()"
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("Coordinates.GenomicsDBColumn.newBuilder()"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                ctx,
            )
            .unwrap(),
            "GenomicsDBColumn::new_builder()"
        );
    }

    #[test]
    fn stream_raw_expr_recovery_preserves_binary_precedence() {
        let expr = Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Raw("left + right".to_string())),
            right: Box::new(Expr::Literal(Literal::Int(2))),
        };

        let lowered = emit_stream_lambda_body_expr(&expr, "value", StreamLambdaValue::BorrowedItem)
            .expect("synthetic raw stream lambda expression should lower");

        assert_eq!(emit_expr(&lowered), "(left + right) * 2");
        assert!(matches!(
            lowered,
            RustExpr::Binary { left, .. }
                if matches!(left.as_ref(), RustExpr::Grouped(grouped)
                    if matches!(grouped.as_ref(), RustExpr::Binary { op, .. } if op == "+"))
        ));
    }

    #[test]
    fn stream_lambda_helper_uses_context_for_member_and_static_access() {
        let mut env = ShapeEnv::new();
        env.insert_local_shape(
            "vid_map_p_b",
            RustSlotShape::conservative(JavaType::Named {
                name: "VidMapPB".to_string(),
                args: Vec::new(),
            }),
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("vid_map_p_b.getContigsList()"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::new(&env),
            ),
            Some("vid_map_p_b.get_contigs_list()".to_string())
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("constants.DELIMITER"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::empty(),
            ),
            Some("Constants::DELIMITER".to_string())
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &parse_expr("constants.DELIMITER"),
                "partition",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::new(&env),
            ),
            Some("Constants::DELIMITER".to_string())
        );
        let mut alias_env = ShapeEnv::new();
        alias_env.insert_static_type_alias("Cfg", "AB.C");
        alias_env.insert_static_type_alias("cfg", "AB.C");
        assert_eq!(alias_env.type_env().static_type_alias("Cfg"), Some("AB.C"));
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &Expr::FieldAccess(Box::new(Expr::Var("Cfg".to_string())), "N".to_string()),
                "partition",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::new(&alias_env),
            ),
            Some("AB_C::N".to_string())
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &Expr::Raw("Cfg.N".to_string()),
                "partition",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::new(&alias_env),
            ),
            Some("AB_C::N".to_string())
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &Expr::Raw("Math.PI".to_string()),
                "partition",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::new(&ShapeEnv::new()),
            ),
            Some("Math::PI".to_string())
        );
        assert_eq!(
            emit_stream_lambda_body_with_context(
                &Expr::Raw("Item.VALUE".to_string()),
                "partition",
                StreamLambdaValue::BorrowedItem,
                StreamRenderContext::new(&ShapeEnv::new()),
            ),
            Some("Item.VALUE".to_string())
        );
    }
}
