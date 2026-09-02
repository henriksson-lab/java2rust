use crate::call_plan::{native_java_instance_receiver_type_hint, resolved_call_arg_target_type};
use crate::coercion::{
    BorrowProjectionUse, IndexToUsizeProjection, LoweredExpr, SequenceIterationProjection,
    StreamLambdaArgumentSlotPlan, StreamLambdaParamProjection, apply_clone_projection,
    arrays_stream_range_projection_with_result_context, borrowed_collection_flat_map_projection,
    current_receiver_method_call_projection, ensure_borrow_projection, field_projection,
    function_call_projection, index_to_usize_projection, indexed_access_projection,
    len_i32_projection, nullable_stream_map_body_projection,
    optional_non_null_filter_map_projection, owner_associated_path_projection,
    receiver_method_call_projection, receiver_method_call_projection_exact,
    rust_string_literal_projection, self_associated_path_projection,
    sequence_iteration_source_projection, sequence_iteration_source_projection_with_result_context,
    stream_collect_to_map_projection, stream_filter_op_projection, stream_flat_map_op_projection,
    stream_join_terminal_projection, stream_lambda_argument_slot_plan, stream_lambda_param_expr,
    stream_map_op_projection_with_param, stream_match_terminal_projection,
    stream_prefixed_flat_map_op_projection, stream_reduce_terminal_projection,
    unwrap_java_null_receiver_projection,
};
use crate::conversion_lowering::apply_lowered_slot_conversion_to_expr;
use crate::conversion_plan::{
    LoweredType, Slot, effective_stream_item_shape, plan_lowered_slot_conversion,
    storage_shape_for_java_type,
};
use crate::expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape_with_context};
use crate::facts::{ExternalFactLookupIndex, FactSet, ReceiverMutabilityFact};
use crate::java_ir::{BinaryOp, CallKind, Expr, JavaType, Literal, Stmt, SwitchLabel, UnaryOp};
use crate::literal_lowering::literal_to_rust;
use crate::method_reference_shape::method_reference_receiver_type_for_call_plan_with_facts;
use crate::operator_lowering::{binary_op_to_rust, stream_unary_prefix};
use crate::rust_ir::{RustClosureBody, RustClosureParam, RustExpr, RustExprShape, RustStmt};
use crate::shape_projection::borrowed_string_content_shape;
use crate::signature::SignaturePlan;
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
pub enum StreamLambdaValue {
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
        if element_shape.capabilities().copy {
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
    current_type: Option<&'a str>,
    self_receiver_available: bool,
    propagate_java_exceptions: bool,
}

impl<'a> StreamRenderContext<'a> {
    pub fn new(env: &'a ShapeEnv) -> Self {
        Self {
            env: Some(env),
            facts: None,
            external_index: None,
            signatures: None,
            current_type: None,
            self_receiver_available: false,
            propagate_java_exceptions: false,
        }
    }

    pub fn new_with_facts(env: &'a ShapeEnv, facts: &'a FactSet) -> Self {
        Self {
            env: Some(env),
            facts: Some(facts),
            external_index: None,
            signatures: None,
            current_type: None,
            self_receiver_available: false,
            propagate_java_exceptions: false,
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
            current_type,
            self_receiver_available,
            propagate_java_exceptions,
        }
    }

    pub fn empty() -> Self {
        Self {
            env: None,
            facts: None,
            external_index: None,
            signatures: None,
            current_type: None,
            self_receiver_available: false,
            propagate_java_exceptions: false,
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
            current_type: self.current_type,
            self_receiver_available: self.self_receiver_available,
            propagate_java_exceptions: self.propagate_java_exceptions,
        }
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
        | crate::call_plan::CallTargetPlan::NativeJavaInstance { .. } => 11,
        _ => return None,
    };
    let lowered_receiver = emit_stream_lambda_receiver_with_locals_expr(
        receiver,
        param,
        param_rust,
        value,
        locals,
        ctx,
        param_shape,
    );
    let lowered_args = emit_stream_lambda_call_args_for_receiver_exprs(
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
    .map(|arg| arg.expr)
    .collect();
    let expr = stream_call_target_projection(&target, lowered_receiver, lowered_args);
    Some(stream_lambda_expr(expr, precedence))
}

fn stream_call_target_projection(
    target: &crate::call_plan::CallTargetPlan<'_>,
    receiver: Option<RustExpr>,
    args: Vec<RustExpr>,
) -> RustExpr {
    crate::call_lowering::apply_call_target(target, receiver, args)
}

#[derive(Debug, Clone)]
struct StreamLambdaRenderedExpr {
    expr: RustExpr,
    precedence: u8,
}

impl StreamLambdaRenderedExpr {
    fn rendered(&self) -> String {
        crate::emit::emit_expr(&self.expr)
    }
}

fn stream_lambda_expr(expr: RustExpr, precedence: u8) -> StreamLambdaRenderedExpr {
    StreamLambdaRenderedExpr { expr, precedence }
}

pub fn emit_stream_lambda_body_expr(
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

pub fn emit_stream_lambda_body_with_context(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    emit_stream_lambda_body_with_locals(expr, param, value, &[param], ctx, None)
}

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
    let lowered = match expr {
        Expr::Literal(lit) => stream_lambda_expr(RustExpr::Literal(literal_to_rust(lit)), 12),
        Expr::Raw(raw) => {
            let expr = stream_raw_expr(raw, ctx);
            let precedence = crate::emit::expr_precedence(&expr);
            stream_lambda_expr(expr, precedence)
        }
        Expr::Var(name) if name == param => {
            stream_lambda_expr(stream_lambda_value_expr(value, param_rust), 12)
        }
        Expr::Var(name) => stream_lambda_expr(ctx.bare_name_expr(name, locals), 12),
        Expr::Binary {
            op: BinaryOp::UShr, ..
        } => stream_lambda_expr(
            stream_compile_error_expr("unsupported Java unsigned right shift in stream lambda"),
            12,
        ),
        Expr::Binary { op, left, right } => {
            let prec = stream_binary_precedence(*op);
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
            stream_lambda_expr(stream_binary_expr_projection(*op, left, right), prec)
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
        } => {
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
                let args = emit_stream_lambda_call_args_for_receiver_exprs(
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
                let expr = function_call_projection(
                    owner_associated_path_projection(
                        &stream_static_owner_type_name_with_context(&resolved.declaring_type, ctx),
                        &resolved.rust_name,
                    ),
                    args,
                );
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(expr, 11),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if let Some(owner) = stream_static_owner_expr_type_name(receiver, locals, ctx) {
                let args = emit_stream_lambda_call_args_for_receiver_exprs(
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
                let expr = function_call_projection(
                    owner_associated_path_projection(&owner, &crate::parser::to_snake_case(name)),
                    args,
                );
                return parenthesize_stream_expr_if_needed(
                    stream_lambda_expr(expr, 11),
                    parent_prec,
                    right_child,
                    None,
                );
            }
            let args = emit_stream_lambda_call_args_for_receiver_exprs(
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
            let receiver = emit_stream_lambda_receiver_with_locals_expr(
                receiver,
                param,
                param_rust,
                value,
                locals,
                ctx,
                param_shape,
            )?;
            stream_lambda_expr(receiver_method_call_projection(receiver, name, args), 11)
        }
        Expr::Call {
            receiver: None,
            name,
            args,
            resolved,
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
            let method_name =
                stream_receiverless_call_rust_name(name, args.len(), args, param, param_shape, ctx)
                    .or_else(|| resolved.as_ref().map(|resolved| resolved.rust_name.clone()))
                    .unwrap_or_else(|| name.clone());
            let method = crate::parser::to_snake_case(method_name.as_str());
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
    let own_prec = match plan.op {
        crate::member_access_plan::MemberAccessOp::LenAsI32
        | crate::member_access_plan::MemberAccessOp::Array2RowsAsI32
        | crate::member_access_plan::MemberAccessOp::Array2ColsAsI32 => 6,
        crate::member_access_plan::MemberAccessOp::StaticConstant { .. } => 12,
        crate::member_access_plan::MemberAccessOp::RustField { .. } => 11,
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
    crate::coercion::rendered_plus_expr_projection_or_raw_leaf(raw)
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
        plan.source.optional_sequence,
    )
}

pub fn render_stream_plan_with_optional_sequence_context(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    ctx: StreamRenderContext<'_>,
    source_optional_sequence: bool,
) -> Option<LoweredExpr> {
    if let Some(lowered) =
        render_stream_plan_structured(plan, lowered_source.clone(), ctx, source_optional_sequence)
    {
        return Some(lowered);
    }
    if let Some(lowered) =
        render_no_op_stream_plan_structured(plan, lowered_source.clone(), source_optional_sequence)
    {
        return Some(lowered);
    }
    None
}

fn render_stream_plan_structured(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    ctx: StreamRenderContext<'_>,
    source_optional_sequence: bool,
) -> Option<LoweredExpr> {
    let source = stream_source_range_projection(lowered_source, plan.source.range, ctx);
    let mut expr = sequence_iteration_source_projection_with_result_context(
        source,
        source_optional_sequence,
        sequence_iteration_projection(plan.source.iteration),
        ctx.propagate_java_exceptions,
    );
    let (next_expr, item_is_borrowed, item_shape) = apply_stream_ops_structured(
        expr,
        &plan.ops,
        ctx,
        StreamRenderOptions::default(),
        matches!(plan.source.iteration, StreamSourceIteration::Borrowed),
        plan.source_element.shape.clone(),
    )?;
    expr = render_stream_terminal_expr(next_expr, plan, item_is_borrowed, &item_shape, ctx)?;
    let actual = stream_terminal_actual_after_rendered_ops(plan, item_is_borrowed, &item_shape);
    Some(LoweredExpr { expr, actual })
}

fn apply_stream_ops_structured(
    mut expr: RustExpr,
    ops: &[StreamOp<'_>],
    ctx: StreamRenderContext<'_>,
    options: StreamRenderOptions,
    mut item_is_borrowed: bool,
    mut item_shape: RustSlotShape,
) -> Option<(RustExpr, bool, RustSlotShape)> {
    for op in ops {
        match op {
            StreamOp::Filter(FilterPlan::Predicate(lambda)) => {
                expr = stream_filter_op_projection(
                    expr,
                    &lambda.param_name,
                    emit_stream_lambda_closure_body_expr(
                        lambda,
                        StreamLambdaValue::FilterBorrowed,
                        ctx,
                        Some(&item_shape),
                    )?,
                    options.move_filter_closures,
                );
            }
            StreamOp::Filter(FilterPlan::NonNull {
                input_shape,
                output_shape,
            }) => {
                if input_shape.optional_element_type().is_some() {
                    expr = optional_non_null_filter_map_projection(expr, item_is_borrowed);
                    item_shape = output_shape.clone();
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
                let value = if item_is_borrowed {
                    StreamLambdaValue::BorrowedItem
                } else {
                    StreamLambdaValue::OwnedItem
                };
                let param_shape = effective_stream_item_shape(&item_shape, item_is_borrowed);
                let effective_output_shape = if matches!(op, StreamOp::Map { .. }) {
                    render_context_lambda_output_shape(lambda, &param_shape, ctx)
                        .unwrap_or_else(|| output_shape.clone())
                } else {
                    output_shape.clone()
                };
                let param = render_stream_lambda_param(&lambda.rust_param_name, &param_shape, ctx);
                expr = stream_map_op_projection_with_param(
                    expr,
                    param,
                    emit_stream_lambda_closure_body_expr(lambda, value, ctx, Some(&param_shape))?,
                );
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
                let (prefix_stmts, _, _) = emit_stream_lambda_prefix_stmts_and_locals(
                    plan.lambda.body,
                    &plan.lambda.param_name,
                    &plan.lambda.rust_param_name,
                    ctx,
                    Some(&param_shape),
                )?;
                let source = emit_stream_lambda_receiver_expr(
                    plan.source,
                    &plan.lambda.param_name,
                    &plan.lambda.rust_param_name,
                    StreamLambdaValue::BorrowedItem,
                    ctx,
                )?;
                let iter_source = sequence_iteration_source_projection(
                    source,
                    stream_source_optional_sequence_for_render(
                        plan.source,
                        plan.optional_sequence,
                        ctx,
                    ),
                    sequence_iteration_projection(plan.iteration),
                );
                expr = if prefix_stmts.is_empty() {
                    stream_flat_map_op_projection(expr, &plan.lambda.rust_param_name, iter_source)
                } else {
                    stream_prefixed_flat_map_op_projection(
                        expr,
                        &plan.lambda.rust_param_name,
                        prefix_stmts,
                        iter_source,
                    )
                };
                item_is_borrowed = matches!(plan.iteration, StreamSourceIteration::Borrowed);
                item_shape = effective_stream_item_shape(
                    &lambda_collection_flat_map_element_shape(plan, &param_shape, ctx)?,
                    item_is_borrowed,
                );
            }
            StreamOp::FlatMap(FlatMapPlan::LambdaStreamChain { lambda, pipeline }) => {
                let param_shape = effective_stream_item_shape(&item_shape, item_is_borrowed);
                let (prefix_stmts, _, scoped_env) = emit_stream_lambda_prefix_stmts_and_locals(
                    lambda.body,
                    &lambda.param_name,
                    &lambda.rust_param_name,
                    ctx,
                    Some(&param_shape),
                )?;
                let nested_ctx = scoped_env
                    .as_ref()
                    .map(|env| ctx.with_env(env))
                    .unwrap_or(ctx);
                let nested = render_stream_sub_pipeline_expr(pipeline, lambda, nested_ctx)?;
                expr = if prefix_stmts.is_empty() {
                    stream_flat_map_op_projection(expr, &lambda.rust_param_name, nested)
                } else {
                    stream_prefixed_flat_map_op_projection(
                        expr,
                        &lambda.rust_param_name,
                        prefix_stmts,
                        nested,
                    )
                };
                item_is_borrowed = false;
                item_shape = pipeline.output_shape.clone();
            }
        }
    }
    Some((expr, item_is_borrowed, item_shape))
}

fn lambda_collection_flat_map_element_shape(
    plan: &crate::stream::LambdaCollectionStreamPlan<'_>,
    param_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> Option<RustSlotShape> {
    if matches!(plan.source, Expr::Var(name) if name == &plan.lambda.param_name) {
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
    env.insert_local(plan.lambda.param_name.clone(), param_shape.java_ty.clone());
    infer_stream_source_element_shape_with_facts(plan.source, &env, facts)
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
            ExprShapeContext {
                signatures: ctx.signatures,
                representations: None,
                facts: ctx.facts,
            },
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
    java_type_is_stream_capable(&shape.java_ty, facts)
}

fn java_type_is_stream_capable(java_ty: &JavaType, facts: Option<&FactSet>) -> bool {
    crate::types::stream_element_type(java_ty).is_some()
        || facts.is_some_and(|facts| {
            crate::stream_shape::stream_source_capability_policy(java_ty, facts)
                .capability_element
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
    ctx: StreamRenderContext<'_>,
) -> Option<RustExpr> {
    match &plan.terminal {
        StreamTerminal::Reduce(reduce) => {
            let identity = emit_stream_lambda_body_with_locals_prec(
                reduce.identity,
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
            let left = reduce.accumulator.left_param_name.as_str();
            let right = reduce.accumulator.right_param_name.as_str();
            let left_rust = reduce.accumulator.left_rust_param_name.as_str();
            let right_rust = reduce.accumulator.right_rust_param_name.as_str();
            let locals = [left, right];
            let body = emit_stream_lambda_body_with_locals_prec(
                reduce.accumulator.body.return_expr()?,
                left,
                left_rust,
                StreamLambdaValue::OwnedItem,
                &locals,
                ctx,
                Some(item_shape),
                0,
                false,
            )?
            .expr;
            Some(stream_reduce_terminal_projection(
                receiver, identity, left_rust, right_rust, body,
            ))
        }
        StreamTerminal::CollectToMap { key, value } => {
            let lambda_value = if item_is_borrowed {
                StreamLambdaValue::BorrowedItem
            } else {
                StreamLambdaValue::OwnedItem
            };
            Some(stream_collect_to_map_projection(
                receiver,
                &key.rust_param_name,
                emit_stream_collect_to_map_lambda_body_expr(
                    key,
                    plan,
                    lambda_value,
                    ctx,
                    Some(&plan.source_element.shape),
                )?,
                emit_stream_collect_to_map_lambda_body_expr(
                    value,
                    plan,
                    lambda_value,
                    ctx,
                    Some(&plan.source_element.shape),
                )?,
            ))
        }
        StreamTerminal::Join { delimiter } => {
            let delimiter = stream_join_delimiter_projection(*delimiter, ctx)?;
            let item_display = stream_join_item_display_projection(item_shape);
            Some(stream_join_terminal_projection(
                receiver,
                item_display,
                delimiter,
            ))
        }
        StreamTerminal::AnyMatch(lambda) | StreamTerminal::AllMatch(lambda) => {
            let terminal_method = match &plan.terminal {
                StreamTerminal::AnyMatch(_) => "any",
                StreamTerminal::AllMatch(_) => "all",
                _ => unreachable!("terminal was matched as any/all"),
            };
            let value = if item_is_borrowed {
                StreamLambdaValue::BorrowedItem
            } else {
                StreamLambdaValue::OwnedItem
            };
            Some(stream_match_terminal_projection(
                receiver,
                terminal_method,
                &lambda.rust_param_name,
                emit_stream_lambda_closure_body_expr(lambda, value, ctx, Some(item_shape))?,
            ))
        }
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::Map) => None,
        StreamTerminal::FindFirst if item_is_borrowed => {
            let next = receiver_method_call_projection_exact(receiver, "next", Vec::new());
            let target = match &plan.actual {
                LoweredType::OptionalElementBorrow { option_shape, .. } => {
                    Some((**option_shape).clone())
                }
                _ => None,
            };
            Some(RustExpr::shaped_as(
                RustExprShape::OptionalProjected,
                target,
                next,
            ))
        }
        _ => no_op_stream_terminal_method(&plan.terminal)
            .map(|method| receiver_method_call_projection_exact(receiver, method, Vec::new())),
    }
}

fn render_no_op_stream_plan_structured(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    source_optional_sequence: bool,
) -> Option<LoweredExpr> {
    if !plan.ops.is_empty() || plan.source.range.is_some() || source_optional_sequence {
        return None;
    }
    let terminal_method = no_op_stream_terminal_method(&plan.terminal)?;
    let iter = sequence_iteration_source_projection(
        lowered_source,
        false,
        sequence_iteration_projection(plan.source.iteration),
    );
    let item_is_borrowed = matches!(plan.source.iteration, StreamSourceIteration::Borrowed);
    let item_shape = plan.source_element.shape.clone();
    let actual = stream_terminal_actual_after_rendered_ops(plan, item_is_borrowed, &item_shape);
    Some(LoweredExpr {
        expr: receiver_method_call_projection_exact(iter, terminal_method, Vec::new()),
        actual,
    })
}

fn no_op_stream_terminal_method(terminal: &StreamTerminal<'_>) -> Option<&'static str> {
    match terminal {
        StreamTerminal::Sum => Some("sum::<i32>"),
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::List)
        | StreamTerminal::ToArray => Some("collect::<Vec<_>>"),
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::Set) => {
            Some("collect::<std::collections::HashSet<_>>")
        }
        StreamTerminal::FindFirst => Some("next"),
        _ => None,
    }
}

fn stream_terminal_actual_after_rendered_ops(
    plan: &StreamPlan<'_>,
    item_is_borrowed: bool,
    item_shape: &RustSlotShape,
) -> LoweredType {
    if item_is_borrowed {
        match plan.terminal {
            StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::List)
            | StreamTerminal::ToArray => {
                return LoweredType::collected_list_from_element_shape(item_shape.clone());
            }
            StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::Set) => {
                return LoweredType::collected_set_from_element_shape(item_shape.clone());
            }
            _ => {}
        }
    }
    plan.actual.clone()
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
}

fn render_context_lambda_output_shape(
    lambda: &LambdaPlan<'_>,
    param_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> Option<RustSlotShape> {
    let env = ctx.env?;
    let return_expr = lambda.body.return_expr()?;
    let mut lambda_env = env.clone();
    lambda_env.insert_local_shape(lambda.param_name.clone(), param_shape.clone());
    Some(infer_expr_shape_with_context(
        return_expr,
        &lambda_env,
        ExprShapeContext {
            signatures: ctx.signatures,
            representations: None,
            facts: ctx.facts,
        },
    ))
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
        crate::slot_render::rust_name_for_java_named_type(simple),
        rendered_args.join(", ")
    ))
}

fn stream_lambda_param_type_is_useful(item_shape: &RustSlotShape) -> bool {
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
    if pipeline.source.range.is_some() {
        return None;
    }
    let source = emit_stream_lambda_receiver_expr(
        pipeline.source.expr,
        &lambda.param_name,
        &lambda.rust_param_name,
        StreamLambdaValue::BorrowedItem,
        ctx,
    )?;
    let source_is_lambda_param =
        matches!(pipeline.source.expr, Expr::Var(name) if name == &lambda.param_name);
    let iteration = if source_is_lambda_param {
        sequence_iteration_projection(pipeline.source.iteration)
    } else {
        SequenceIterationProjection::OwnedValue
    };
    let expr = sequence_iteration_source_projection_with_result_context(
        source,
        stream_source_optional_sequence_for_render(
            pipeline.source.expr,
            pipeline.source.optional_sequence,
            ctx,
        ),
        iteration,
        ctx.propagate_java_exceptions,
    );
    let item_is_borrowed = source_is_lambda_param
        && matches!(pipeline.source.iteration, StreamSourceIteration::Borrowed);
    let item_shape = effective_stream_item_shape(&pipeline.source_element.shape, item_is_borrowed);
    apply_stream_ops_structured(
        expr,
        &pipeline.ops,
        ctx,
        StreamRenderOptions {
            move_filter_closures: true,
        },
        item_is_borrowed,
        item_shape,
    )
    .map(|(expr, nested_item_is_borrowed, nested_item_shape)| {
        recover_temporary_sub_pipeline_item(
            expr,
            source_is_lambda_param,
            nested_item_is_borrowed,
            &nested_item_shape,
        )
    })
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
    let actual = crate::expr_shape::infer_expr_shape(expr, env);
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
    let return_expr = lambda.body.return_expr()?;
    if matches!(return_expr, Expr::Var(name) if name == &lambda.param_name) {
        if matches!(value, StreamLambdaValue::OwnedItem) {
            return Some(RustExpr::Var(lambda.rust_param_name.to_string()));
        }
        return Some(stream_identity_value_projection(
            &lambda.rust_param_name,
            &plan.source_element.shape,
        ));
    }
    emit_stream_lambda_body_with_locals_prec(
        return_expr,
        &lambda.param_name,
        &lambda.rust_param_name,
        value,
        &[lambda.param_name.as_str()],
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
    if let StreamLambdaBody::MethodReference { .. } = lambda.body {
        return emit_stream_method_reference_body_expr(lambda, value, ctx, param_shape);
    }
    if let Some(nullable_body) =
        emit_nullable_stream_lambda_closure_body_expr(lambda, value, ctx, param_shape)
    {
        return Some(nullable_body);
    }
    let return_expr = lambda.body.return_expr()?;
    let (prefix_stmts, locals, scoped_env) = emit_stream_lambda_prefix_stmts_and_locals(
        lambda.body,
        &lambda.param_name,
        &lambda.rust_param_name,
        ctx,
        param_shape,
    )?;
    let body_ctx = scoped_env
        .as_ref()
        .map(|env| ctx.with_env(env))
        .unwrap_or(ctx);
    let body = emit_stream_lambda_body_with_locals_prec(
        return_expr,
        &lambda.param_name,
        &lambda.rust_param_name,
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
    let StreamLambdaBody::MethodReference { target, method } = lambda.body else {
        return None;
    };
    if let Some(expr) = emit_stream_method_reference_body_through_call_plan(
        target,
        method,
        &lambda.param_name,
        value,
        ctx,
        param_shape,
    ) {
        return Some(expr);
    }
    let plan = crate::method_reference_plan::plan_method_reference(target, method);
    let value_expr = stream_lambda_value_expr(value, &lambda.param_name);
    let receiver = stream_lambda_receiver_expr(value, &lambda.param_name);
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
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
    let receiver_ty = method_reference_receiver_type_for_call_plan_with_facts(
        target,
        method,
        param_shape,
        facts,
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
    let crate::call_plan::CallTargetPlan::NativeJavaInstance { .. } = target_plan else {
        return None;
    };
    let lowered_receiver =
        stream_method_reference_native_instance_receiver(target, param, value, &target_plan);
    Some(stream_call_target_projection(
        &target_plan,
        Some(lowered_receiver),
        Vec::new(),
    ))
}

fn stream_method_reference_native_instance_receiver(
    target: &str,
    param: &str,
    value: StreamLambdaValue,
    target_plan: &crate::call_plan::CallTargetPlan<'_>,
) -> RustExpr {
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

fn stream_method_reference_object_receiver_expr(value: StreamLambdaValue, param: &str) -> RustExpr {
    stream_lambda_receiver_expr(value, param)
}

fn emit_nullable_stream_lambda_closure_body_expr(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    let StreamLambdaBody::Block(block) = lambda.body else {
        return None;
    };
    let (guard_index, condition) = first_null_return_guard(block)?;
    let final_expr = lambda.body.return_expr()?;
    let mutated = stream_lambda_mutated_locals(block, ctx);
    let mut locals = vec![lambda.param_name.as_str()];
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
        std::iter::once(lambda.param_name.as_str()).chain(source_names.iter().copied()),
    );
    let mut scoped_env = stream_lambda_scoped_env(
        ctx,
        &lambda.param_name,
        &lambda.rust_param_name,
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
            &lambda.param_name,
            &lambda.rust_param_name,
            &locals,
            stmt_ctx,
            &mutated,
            param_shape,
        )?);
        if let Stmt::VarDecl {
            ty,
            name,
            source_name,
            ..
        } = stmt
        {
            locals.push(source_name);
            let access_name = access_names
                .get(source_name.as_str())
                .cloned()
                .unwrap_or_else(|| name.clone());
            if let Some(env) = scoped_env.as_mut() {
                let shape = RustSlotShape::for_storage(ty.clone());
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
            &lambda.param_name,
            &lambda.rust_param_name,
            &locals,
            stmt_ctx,
            &mutated,
            param_shape,
        )?);
        if let Stmt::VarDecl {
            ty,
            name,
            source_name,
            ..
        } = stmt
        {
            locals.push(source_name);
            let access_name = access_names
                .get(source_name.as_str())
                .cloned()
                .unwrap_or_else(|| name.clone());
            if let Some(env) = scoped_env.as_mut() {
                let shape = RustSlotShape::for_storage(ty.clone());
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
        &lambda.param_name,
        &lambda.rust_param_name,
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
        &lambda.param_name,
        &lambda.rust_param_name,
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
            param_shape,
        )?);
        if let Stmt::VarDecl {
            ty,
            name,
            source_name,
            ..
        } = stmt
        {
            locals.push(source_name);
            let access_name = access_names
                .get(source_name.as_str())
                .cloned()
                .unwrap_or_else(|| name.clone());
            if let Some(env) = scoped_env.as_mut() {
                let shape = RustSlotShape::for_storage(ty.clone());
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
    param_shape: Option<&RustSlotShape>,
) -> Option<RustStmt> {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            source_name,
            initializer,
            ..
        } => {
            let rust_name = ctx
                .env
                .and_then(|env| env.binding_access_name(source_name))
                .map(str::to_string)
                .unwrap_or_else(|| name.clone());
            Some(RustStmt::Let {
                mutable: mutated.contains(name)
                    || mutated.contains(source_name)
                    || mutated.contains(&rust_name),
                name: rust_name.clone(),
                ty: None,
                value: Some(
                    initializer
                        .as_ref()
                        .and_then(|expr| {
                            emit_stream_lambda_local_initializer_expr(
                                ty,
                                expr,
                                param,
                                param_rust,
                                locals,
                                ctx,
                                param_shape,
                            )
                        })
                        .unwrap_or_else(|| {
                            function_call_projection(
                                RustExpr::path(["Default", "default"]),
                                Vec::new(),
                            )
                        }),
                ),
            })
        }
        Stmt::Expr(expr) => Some(RustStmt::Expr(
            emit_stream_lambda_body_with_locals_prec(
                expr,
                param,
                param_rust,
                StreamLambdaValue::BorrowedItem,
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
                StreamLambdaValue::BorrowedItem,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?
            .expr,
            value: emit_stream_lambda_body_with_locals_prec(
                value,
                param,
                param_rust,
                StreamLambdaValue::BorrowedItem,
                locals,
                ctx,
                param_shape,
                0,
                false,
            )?
            .expr,
        }),
        _ => None,
    }
}

fn emit_stream_lambda_local_initializer_expr(
    ty: &JavaType,
    expr: &Expr,
    param: &str,
    param_rust: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustExpr> {
    let mut lowered = emit_stream_lambda_body_with_locals_prec(
        expr,
        param,
        param_rust,
        StreamLambdaValue::BorrowedItem,
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
        ExprShapeContext {
            signatures: ctx.signatures,
            representations: None,
            facts: ctx.facts,
        },
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
        Expr::New { args, .. } => {
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
                    .and_then(|(_, signature)| signature.params.get(index))
                    .map(|param_plan| {
                        adapt_stream_lambda_arg_for_slot_expr(
                            arg,
                            lowered.clone(),
                            &param_plan.shape,
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
                            && stream_lambda_borrowed_item_expr(&lowered.expr, param)
                        {
                            recover_stream_lambda_owned_arg_move_expr(arg, lowered, param, locals)
                        } else {
                            lowered
                        }
                    })
            })
            .collect(),
    )
}

fn stream_receiverless_call_rust_name(
    name: &str,
    arg_count: usize,
    args: &[Expr],
    param: &str,
    param_shape: Option<&RustSlotShape>,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    stream_receiverless_call_signature(name, arg_count, args, param, param_shape, ctx)
        .map(|(rust_name, _)| rust_name)
}

fn stream_receiverless_call_signature<'a>(
    name: &str,
    arg_count: usize,
    args: &[Expr],
    param: &str,
    param_shape: Option<&RustSlotShape>,
    ctx: StreamRenderContext<'a>,
) -> Option<(String, &'a crate::signature::MethodSignaturePlan)> {
    let current_type = ctx.current_type?;
    let signatures = ctx.signatures?;
    let env = ctx.env?;
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
    signatures
        .method_signature_by_base_name_and_arg_types(current_type, name, &arg_types)
        .or_else(|| {
            signatures.method_signature_by_base_name_and_arity(current_type, name, arg_count)
        })
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
    let actual = stream_lambda_arg_actual_shape(arg, param, locals, ctx, param_shape)?;
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
    Some(stream_lambda_expr(
        apply_lowered_slot_conversion_to_expr(lowered.expr, &plan),
        11,
    ))
}

fn stream_lambda_arg_actual_shape(
    arg: &Expr,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<RustSlotShape> {
    match arg {
        Expr::Var(name) if name == param => ctx
            .env
            .and_then(|env| env.local_shape(name))
            .cloned()
            .or_else(|| param_shape.cloned()),
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
                ExprShapeContext {
                    signatures: ctx.signatures,
                    representations: None,
                    facts: ctx.facts,
                },
            )
        }),
    }
}

fn recover_stream_lambda_owned_arg_move_expr(
    arg: &Expr,
    lowered: StreamLambdaRenderedExpr,
    param: &str,
    locals: &[&str],
) -> StreamLambdaRenderedExpr {
    match arg {
        Expr::Var(name) if name == param => {
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
        && stream_lambda_borrowed_item_expr(&lowered.expr, param)
        && stream_lambda_arg_actual_shape(arg, param, locals, ctx, param_shape)
            .is_some_and(|shape| shape.java_ty == JavaType::String)
    {
        stream_lambda_expr(apply_clone_projection(RustExpr::Var(param.to_string())), 11)
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
        return crate::slot_render::rust_name_for_java_named_type(owner);
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
