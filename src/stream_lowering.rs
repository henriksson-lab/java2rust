use crate::coercion::LoweredExpr;
use crate::expr_shape::ShapeEnv;
use crate::facts::{FactSet, ReceiverMutabilityFact};
use crate::java_ir::{BinaryOp, CallKind, Expr, JavaType, Literal, Stmt, UnaryOp};
use crate::literal_lowering::literal_to_rust;
use crate::operator_lowering::{binary_op_to_rust, stream_unary_prefix};
use crate::rust_ir::RustExpr;
use crate::shape_projection::{shape_is_borrowed_string_content, shape_is_owned};
use crate::slot_shape::RustSlotShape;
use crate::stream::{
    FilterPlan, FlatMapPlan, LambdaPlan, StreamLambdaBody, StreamOp, StreamPlan, StreamSource,
    StreamSourceIteration, StreamSubPipeline, StreamTerminal,
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLambdaValue {
    FilterBorrowed,
    BorrowedItem,
    OwnedItem,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamRenderContext<'a> {
    env: Option<&'a ShapeEnv>,
    facts: Option<&'a FactSet>,
}

impl<'a> StreamRenderContext<'a> {
    pub fn new(env: &'a ShapeEnv) -> Self {
        Self {
            env: Some(env),
            facts: None,
        }
    }

    pub fn new_with_facts(env: &'a ShapeEnv, facts: &'a FactSet) -> Self {
        Self {
            env: Some(env),
            facts: Some(facts),
        }
    }

    pub fn empty() -> Self {
        Self {
            env: None,
            facts: None,
        }
    }

    fn render_bare_name(self, name: &str, locals: &[&str]) -> String {
        if locals.iter().any(|local| *local == name) {
            return crate::parser::to_snake_case(name);
        }
        self.env
            .map(|env| crate::access_lowering::render_bare_name(name, env))
            .unwrap_or_else(|| crate::parser::to_snake_case(name))
    }
}

impl Default for StreamRenderContext<'_> {
    fn default() -> Self {
        Self::empty()
    }
}

fn emit_stream_lambda_shared_native_call(
    receiver: &Expr,
    name: &str,
    args: &[Expr],
    resolved: Option<&crate::java_ir::ResolvedCall>,
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<(String, u8)> {
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
            } else if stream_native_string_method_requires_string_receiver(name) {
                typed_lambda_env.insert_local(param.to_string(), JavaType::String);
                &typed_lambda_env
            } else {
                &typed_lambda_env
            }
        } else if locals.iter().any(|local| *local == receiver_name)
            && stream_native_string_method_requires_string_receiver(name)
        {
            typed_lambda_env.insert_local(receiver_name.clone(), JavaType::String);
            &typed_lambda_env
        } else {
            &typed_lambda_env
        }
    } else {
        &typed_lambda_env
    };
    let target = crate::call_plan::plan_call_target_with_facts_and_arity(
        Some(receiver),
        name,
        args.len(),
        resolved,
        facts,
        type_env,
    );
    let precedence = match target {
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
    let lowered_receiver =
        emit_stream_lambda_receiver_with_locals(receiver, param, value, locals, ctx, param_shape)
            .map(RustExpr::Raw);
    let lowered_args = emit_stream_lambda_call_args_for_receiver(
        Some(receiver),
        name,
        args,
        param,
        value,
        locals,
        ctx,
        param_shape,
    )?
    .into_iter()
    .map(RustExpr::Raw)
    .collect();
    let rendered = crate::emit::emit_expr(&crate::call_lowering::apply_call_target(
        &target,
        lowered_receiver,
        lowered_args,
    ));
    Some((rendered, precedence))
}

fn stream_native_string_method_requires_string_receiver(name: &str) -> bool {
    matches!(
        name,
        "length"
            | "isEmpty"
            | "trim"
            | "toLowerCase"
            | "toString"
            | "contains"
            | "startsWith"
            | "endsWith"
            | "split"
    )
}

pub fn emit_stream_lambda_body(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
) -> Option<String> {
    emit_stream_lambda_body_with_context(expr, param, value, StreamRenderContext::empty())
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
    emit_stream_lambda_body_with_locals_prec(expr, param, value, locals, ctx, param_shape, 0, false)
}

fn emit_stream_lambda_body_with_locals_prec(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
    parent_prec: u8,
    right_child: bool,
) -> Option<String> {
    let (rendered, own_prec) = match expr {
        Expr::Literal(lit) => (literal_to_rust(lit), 12),
        Expr::Var(name) if name == param => (
            match value {
                StreamLambdaValue::FilterBorrowed => format!("**{param}"),
                StreamLambdaValue::BorrowedItem => format!("*{param}"),
                StreamLambdaValue::OwnedItem => param.to_string(),
            },
            12,
        ),
        Expr::Var(name) => (ctx.render_bare_name(name, locals), 12),
        Expr::Binary { op, left, right } => {
            let prec = stream_binary_precedence(*op);
            (
                format!(
                    "{} {} {}",
                    emit_stream_lambda_body_with_locals_prec(
                        left,
                        param,
                        value,
                        locals,
                        ctx,
                        param_shape,
                        prec,
                        false
                    )?,
                    binary_op_to_rust(*op),
                    emit_stream_lambda_body_with_locals_prec(
                        right,
                        param,
                        value,
                        locals,
                        ctx,
                        param_shape,
                        prec,
                        true
                    )?
                ),
                prec,
            )
        }
        Expr::Unary { op, expr } => {
            let op = stream_unary_prefix(*op)?;
            (
                format!(
                    "{op}{}",
                    emit_stream_lambda_body_with_locals_prec(
                        expr,
                        param,
                        value,
                        locals,
                        ctx,
                        param_shape,
                        10,
                        false
                    )?
                ),
                10,
            )
        }
        Expr::FieldAccess(receiver, field) => {
            if field == "length" {
                let receiver = emit_stream_lambda_receiver_with_locals(
                    receiver,
                    param,
                    value,
                    locals,
                    ctx,
                    param_shape,
                )?;
                return parenthesize_stream_expr_if_needed(
                    format!("{receiver}.len() as i32"),
                    6,
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if is_static_constant_member(field)
                && let Some(owner) = stream_static_owner_expr_type_name(receiver, locals, ctx)
            {
                return parenthesize_stream_expr_if_needed(
                    format!("{owner}::{field}"),
                    12,
                    parent_prec,
                    right_child,
                    None,
                );
            }
            (
                format!(
                    "{}.{}",
                    emit_stream_lambda_receiver_with_locals(
                        receiver,
                        param,
                        value,
                        locals,
                        ctx,
                        param_shape,
                    )?,
                    crate::parser::to_snake_case(field)
                ),
                11,
            )
        }
        Expr::ArrayAccess(array, index) => (
            format!(
                "{}[{} as usize]",
                emit_stream_lambda_receiver_with_locals(
                    array,
                    param,
                    value,
                    locals,
                    ctx,
                    param_shape
                )?,
                emit_stream_lambda_body_with_locals(index, param, value, locals, ctx, param_shape)?
            ),
            11,
        ),
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
                value,
                locals,
                ctx,
                param_shape,
            ) {
                return parenthesize_stream_expr_if_needed(
                    rendered.0,
                    rendered.1,
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if let Some(resolved) = resolved
                && matches!(resolved.kind, CallKind::Static)
            {
                let args = emit_stream_lambda_call_args_for_receiver(
                    Some(receiver),
                    name,
                    args,
                    param,
                    value,
                    locals,
                    ctx,
                    param_shape,
                )?;
                return parenthesize_stream_expr_if_needed(
                    format!(
                        "{}::{}({})",
                        stream_static_owner_type_name_with_context(&resolved.declaring_type, ctx),
                        resolved.rust_name,
                        args.join(", ")
                    ),
                    11,
                    parent_prec,
                    right_child,
                    None,
                );
            }
            if let Some(owner) = stream_static_owner_expr_type_name(receiver, locals, ctx) {
                let args = emit_stream_lambda_call_args_for_receiver(
                    Some(receiver),
                    name,
                    args,
                    param,
                    value,
                    locals,
                    ctx,
                    param_shape,
                )?;
                return parenthesize_stream_expr_if_needed(
                    format!(
                        "{}::{}({})",
                        owner,
                        crate::parser::to_snake_case(name),
                        args.join(", ")
                    ),
                    11,
                    parent_prec,
                    right_child,
                    None,
                );
            }
            let args =
                emit_stream_lambda_args_with_locals(args, param, value, locals, ctx, param_shape)?;
            let receiver = emit_stream_lambda_receiver_with_locals(
                receiver,
                param,
                value,
                locals,
                ctx,
                param_shape,
            )?;
            let rendered = format!(
                "{}.{}({})",
                receiver,
                crate::parser::to_snake_case(name),
                args.join(", ")
            );
            (rendered, 11)
        }
        Expr::Call {
            receiver: None,
            name,
            args,
            resolved,
        } => {
            let args =
                emit_stream_lambda_args_with_locals(args, param, value, locals, ctx, param_shape)?
                    .join(", ");
            let method = resolved
                .as_ref()
                .map(|resolved| resolved.rust_name.as_str())
                .unwrap_or(name);
            match resolved.as_ref().map(|resolved| resolved.kind) {
                Some(CallKind::Instance) => (
                    format!("self.{}({args})", crate::parser::to_snake_case(method)),
                    11,
                ),
                Some(CallKind::Static) => (
                    format!("Self::{}({args})", crate::parser::to_snake_case(method)),
                    11,
                ),
                None => (
                    format!("{}({args})", crate::parser::to_snake_case(method)),
                    11,
                ),
            }
        }
        Expr::New { ty, args, .. } => {
            let rust_name = ctx
                .facts
                .map(|facts| {
                    crate::call_plan::unresolved_constructor_rust_name_for_facts(
                        ty,
                        facts,
                        args.len(),
                    )
                })
                .unwrap_or_else(|| "new".to_string());
            (
                format!(
                    "{}::{}({})",
                    stream_lambda_type_name(ty)?,
                    rust_name,
                    emit_stream_lambda_args_with_locals(
                        args,
                        param,
                        value,
                        locals,
                        ctx,
                        param_shape
                    )?
                    .join(", ")
                ),
                11,
            )
        }
        _ => return None,
    };
    parenthesize_stream_expr_if_needed(
        rendered,
        own_prec,
        parent_prec,
        right_child,
        match expr {
            Expr::Binary { op, .. } => Some(*op),
            _ => None,
        },
    )
}

pub fn render_stream_plan(plan: &StreamPlan<'_>, lowered_source: RustExpr) -> Option<LoweredExpr> {
    render_stream_plan_with_context(plan, lowered_source, StreamRenderContext::empty())
}

pub fn render_stream_plan_with_context(
    plan: &StreamPlan<'_>,
    lowered_source: RustExpr,
    ctx: StreamRenderContext<'_>,
) -> Option<LoweredExpr> {
    let source = crate::emit::emit_expr(&lowered_source);
    let mut out = render_stream_source_iter(&source, &plan.source);
    let (item_is_borrowed, item_shape) = render_stream_ops(&mut out, plan, ctx)?;
    render_stream_terminal(&mut out, plan, item_is_borrowed, &item_shape, ctx)?;
    Some(LoweredExpr {
        expr: RustExpr::Raw(out),
        actual: plan.actual.clone(),
    })
}

fn render_stream_source_iter(source: &str, stream_source: &StreamSource<'_>) -> String {
    let source = parenthesize_stream_source_for_method_call(source);
    match (stream_source.optional_sequence, stream_source.iteration) {
        (true, StreamSourceIteration::Borrowed) => format!("{source}.as_ref().unwrap().iter()"),
        (true, StreamSourceIteration::OwnedValue) => format!("{source}.unwrap().into_iter()"),
        (false, StreamSourceIteration::Borrowed) => format!("{source}.iter()"),
        (false, StreamSourceIteration::OwnedValue) => format!("{source}.into_iter()"),
    }
}

fn parenthesize_stream_source_for_method_call(source: &str) -> String {
    if source.contains("..") && !(source.starts_with('(') && source.ends_with(')')) {
        format!("({source})")
    } else {
        source.to_string()
    }
}

fn render_stream_ops(
    out: &mut String,
    plan: &StreamPlan<'_>,
    ctx: StreamRenderContext<'_>,
) -> Option<(bool, RustSlotShape)> {
    render_stream_ops_with_options(
        out,
        &plan.ops,
        ctx,
        StreamRenderOptions::default(),
        matches!(plan.source.iteration, StreamSourceIteration::Borrowed),
        plan.source_element.shape.clone(),
    )
}

#[derive(Debug, Clone, Copy, Default)]
struct StreamRenderOptions {
    move_filter_closures: bool,
}

fn render_stream_ops_with_options(
    out: &mut String,
    ops: &[StreamOp<'_>],
    ctx: StreamRenderContext<'_>,
    options: StreamRenderOptions,
    mut item_is_borrowed: bool,
    mut item_shape: RustSlotShape,
) -> Option<(bool, RustSlotShape)> {
    for op in ops {
        match op {
            StreamOp::Filter(filter) => match filter {
                FilterPlan::Predicate(lambda) => {
                    let closure_prefix = if options.move_filter_closures {
                        "move "
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        ".filter({closure_prefix}|{}| {})",
                        lambda.param_name,
                        emit_stream_lambda_closure_body(
                            lambda,
                            StreamLambdaValue::FilterBorrowed,
                            ctx,
                            Some(&item_shape),
                        )?
                    ));
                }
                FilterPlan::NonNull {
                    input_shape,
                    output_shape,
                } => {
                    if input_shape.optional_element_type().is_some() {
                        if item_is_borrowed {
                            out.push_str(".filter_map(|item| item.as_ref())");
                        } else {
                            out.push_str(".filter_map(|item| item)");
                        }
                        item_shape = output_shape.clone();
                    }
                }
            },
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
                out.push_str(&format!(
                    ".map(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_closure_body(lambda, value, ctx, Some(&item_shape))?
                ));
                item_is_borrowed = false;
                item_shape = output_shape.clone();
            }
            StreamOp::FlatMap(flat_map) => match flat_map {
                FlatMapPlan::CollectionStreamMethodReference { param_name } => {
                    out.push_str(&format!(".flat_map(|{param_name}| {param_name}.iter())"));
                    item_is_borrowed = true;
                }
                FlatMapPlan::LambdaCollectionStream(plan) => {
                    let prefix =
                        emit_stream_lambda_prefix(plan.lambda.body, &plan.lambda.param_name, ctx)?;
                    let source = emit_stream_lambda_receiver(
                        plan.source,
                        &plan.lambda.param_name,
                        StreamLambdaValue::BorrowedItem,
                        ctx,
                    )?;
                    let iterator = match plan.iteration {
                        StreamSourceIteration::Borrowed => "iter",
                        StreamSourceIteration::OwnedValue => "into_iter",
                    };
                    if prefix.is_empty() {
                        out.push_str(&format!(
                            ".flat_map(|{}| {source}.{iterator}())",
                            plan.lambda.param_name
                        ));
                    } else {
                        out.push_str(&format!(
                            ".flat_map(|{}| {{ {prefix}{source}.{iterator}() }})",
                            plan.lambda.param_name
                        ));
                    }
                    item_is_borrowed = matches!(plan.iteration, StreamSourceIteration::Borrowed);
                }
                FlatMapPlan::LambdaStreamChain { lambda, pipeline } => {
                    let prefix = emit_stream_lambda_prefix(lambda.body, &lambda.param_name, ctx)?;
                    let nested = render_stream_sub_pipeline(pipeline, lambda, ctx)?;
                    if prefix.is_empty() {
                        out.push_str(&format!(".flat_map(|{}| {nested})", lambda.param_name));
                    } else {
                        out.push_str(&format!(
                            ".flat_map(|{}| {{ {prefix}{nested} }})",
                            lambda.param_name
                        ));
                    }
                    item_is_borrowed = false;
                    item_shape = pipeline.output_shape.clone();
                }
            },
        }
    }
    Some((item_is_borrowed, item_shape))
}

fn render_stream_sub_pipeline(
    pipeline: &StreamSubPipeline<'_>,
    lambda: &LambdaPlan<'_>,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    let source = emit_stream_lambda_receiver(
        pipeline.source.expr,
        &lambda.param_name,
        StreamLambdaValue::BorrowedItem,
        ctx,
    )?;
    let source_is_lambda_param =
        matches!(pipeline.source.expr, Expr::Var(name) if name == &lambda.param_name);
    let mut out = if source_is_lambda_param {
        render_stream_source_iter(&source, &pipeline.source)
    } else {
        render_stream_source_owned_iter(&source, &pipeline.source)
    };
    render_stream_ops_with_options(
        &mut out,
        &pipeline.ops,
        ctx,
        StreamRenderOptions {
            move_filter_closures: true,
        },
        matches!(pipeline.source.iteration, StreamSourceIteration::Borrowed),
        pipeline.source_element.shape.clone(),
    )?;
    Some(out)
}

fn render_stream_source_owned_iter(source: &str, stream_source: &StreamSource<'_>) -> String {
    if stream_source.optional_sequence {
        format!("{source}.unwrap().into_iter()")
    } else {
        format!("{source}.into_iter()")
    }
}

fn render_stream_terminal(
    out: &mut String,
    plan: &StreamPlan<'_>,
    item_is_borrowed: bool,
    item_shape: &RustSlotShape,
    ctx: StreamRenderContext<'_>,
) -> Option<()> {
    match &plan.terminal {
        StreamTerminal::Sum => out.push_str(".sum::<i32>()"),
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::List) => {
            out.push_str(".collect::<Vec<_>>()");
        }
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::Set) => {
            out.push_str(".collect::<std::collections::HashSet<_>>()");
        }
        StreamTerminal::Collect(crate::stdlib::JavaCollectionKind::Map) => return None,
        StreamTerminal::CollectToMap { key, value } => {
            out.push_str(&format!(
                ".map(|{}| ({}, {})).collect::<std::collections::HashMap<_, _>>()",
                key.param_name,
                emit_stream_collect_to_map_lambda_body(
                    key,
                    plan,
                    StreamLambdaValue::BorrowedItem,
                    ctx,
                    Some(&plan.source_element.shape),
                )?,
                emit_stream_collect_to_map_lambda_body(
                    value,
                    plan,
                    StreamLambdaValue::BorrowedItem,
                    ctx,
                    Some(&plan.source_element.shape),
                )?
            ));
        }
        StreamTerminal::Join { delimiter } => {
            let delimiter = render_stream_join_delimiter(*delimiter, ctx)?;
            out.push_str(&format!(
                ".map(|item| item.to_string()).collect::<Vec<_>>().join({delimiter})"
            ));
        }
        StreamTerminal::ToArray => out.push_str(".collect::<Vec<_>>()"),
        StreamTerminal::FindFirst => out.push_str(".next()"),
        StreamTerminal::AnyMatch(lambda) => {
            let value = if item_is_borrowed {
                StreamLambdaValue::BorrowedItem
            } else {
                StreamLambdaValue::OwnedItem
            };
            out.push_str(&format!(
                ".any(|{}| {})",
                lambda.param_name,
                emit_stream_lambda_closure_body(lambda, value, ctx, Some(item_shape))?
            ));
        }
        StreamTerminal::AllMatch(lambda) => {
            let value = if item_is_borrowed {
                StreamLambdaValue::BorrowedItem
            } else {
                StreamLambdaValue::OwnedItem
            };
            out.push_str(&format!(
                ".all(|{}| {})",
                lambda.param_name,
                emit_stream_lambda_closure_body(lambda, value, ctx, Some(item_shape))?
            ));
        }
    };
    Some(())
}

fn render_stream_join_delimiter(
    delimiter: Option<&Expr>,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    match delimiter {
        None => Some("\"\"".to_string()),
        Some(Expr::Literal(Literal::String(value))) => Some(format!("{value:?}")),
        Some(expr) => emit_stream_lambda_body_with_context(
            expr,
            "__java_unused_stream_join_param",
            StreamLambdaValue::OwnedItem,
            ctx,
        )
        .map(|expr| format!("{expr}.as_str()")),
    }
}

fn emit_stream_collect_to_map_lambda_body(
    lambda: &LambdaPlan<'_>,
    plan: &StreamPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<String> {
    let return_expr = lambda.body.return_expr()?;
    if matches!(return_expr, Expr::Var(name) if name == &lambda.param_name) {
        return Some(crate::stream::stream_identity_value_expr(
            &lambda.param_name,
            &plan.source_element.shape,
        ));
    }
    emit_stream_lambda_body_with_locals(
        return_expr,
        &lambda.param_name,
        value,
        &[&lambda.param_name],
        ctx,
        param_shape,
    )
}

fn emit_stream_lambda_closure_body(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<String> {
    if let StreamLambdaBody::MethodReference { target, method } = lambda.body {
        return emit_stream_method_reference_body(target, method, &lambda.param_name, value, ctx);
    }
    if let Some(nullable_body) =
        emit_nullable_stream_lambda_closure_body(lambda, value, ctx, param_shape)
    {
        return Some(nullable_body);
    }
    let return_expr = lambda.body.return_expr()?;
    let (prefix, locals) =
        emit_stream_lambda_prefix_and_locals(lambda.body, &lambda.param_name, ctx)?;
    let return_expr = emit_stream_lambda_body_with_locals(
        return_expr,
        &lambda.param_name,
        value,
        &locals,
        ctx,
        param_shape,
    )?;
    if prefix.is_empty() {
        Some(return_expr)
    } else {
        Some(format!("{{ {prefix}{return_expr} }}"))
    }
}

fn emit_stream_method_reference_body(
    target: &str,
    method: &str,
    param: &str,
    value: StreamLambdaValue,
    _ctx: StreamRenderContext<'_>,
) -> Option<String> {
    let target = crate::stdlib::canonical_external_receiver_type_key(target);
    let value_expr = match value {
        StreamLambdaValue::FilterBorrowed => format!("**{param}"),
        StreamLambdaValue::BorrowedItem => format!("*{param}"),
        StreamLambdaValue::OwnedItem => param.to_string(),
    };
    let receiver = match value {
        StreamLambdaValue::FilterBorrowed => format!("(**{param})"),
        StreamLambdaValue::BorrowedItem => format!("(*{param})"),
        StreamLambdaValue::OwnedItem => param.to_string(),
    };
    match (target.as_str(), method) {
        ("Integer", "intValue" | "int_value") => Some(value_expr),
        ("Object", "toString" | "to_string") => Some(format!("{receiver}.to_string()")),
        ("String", "valueOf" | "value_of") => Some(format!("{receiver}.to_string()")),
        (_, method) if is_scala_tuple_accessor(method) => Some(format!("{receiver}.{method}()")),
        (_, method) if method.starts_with("get") || method.starts_with("get_") => Some(format!(
            "{receiver}.{}()",
            crate::parser::to_snake_case(method)
        )),
        _ if target == "this" || target == "self" => Some(format!(
            "self.{}({receiver})",
            crate::parser::to_snake_case(method)
        )),
        _ => Some(format!(
            "{}::{}({receiver})",
            target,
            crate::parser::to_snake_case(method)
        )),
    }
}

fn is_scala_tuple_accessor(method: &str) -> bool {
    method
        .strip_prefix('_')
        .is_some_and(|index| !index.is_empty() && index.chars().all(|ch| ch.is_ascii_digit()))
}

fn emit_nullable_stream_lambda_closure_body(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<String> {
    let StreamLambdaBody::Block(block) = lambda.body else {
        return None;
    };
    let (guard_index, condition) = first_null_return_guard(block)?;
    let final_expr = lambda.body.return_expr()?;
    let mutated = stream_lambda_mutated_locals(block, ctx);
    let mut locals = vec![lambda.param_name.as_str()];
    let mut before_guard = String::new();
    for stmt in &block.statements[..guard_index] {
        before_guard.push_str(&emit_stream_lambda_stmt(
            stmt,
            &lambda.param_name,
            &locals,
            ctx,
            &mutated,
            param_shape,
        )?);
        before_guard.push(' ');
        if let Stmt::VarDecl { name, .. } = stmt {
            locals.push(name);
        }
    }
    let mut else_body = String::new();
    for stmt in &block.statements[guard_index + 1..block.statements.len().saturating_sub(1)] {
        else_body.push_str(&emit_stream_lambda_stmt(
            stmt,
            &lambda.param_name,
            &locals,
            ctx,
            &mutated,
            param_shape,
        )?);
        else_body.push(' ');
        if let Stmt::VarDecl { name, .. } = stmt {
            locals.push(name);
        }
    }
    let condition = emit_stream_lambda_body_with_locals(
        condition,
        &lambda.param_name,
        value,
        &locals,
        ctx,
        param_shape,
    )?;
    let final_expr = emit_stream_lambda_body_with_locals(
        final_expr,
        &lambda.param_name,
        value,
        &locals,
        ctx,
        param_shape,
    )?;
    Some(format!(
        "{{ {before_guard}if {condition} {{ None }} else {{ {else_body}Some({final_expr}) }} }}"
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

fn emit_stream_lambda_prefix(
    body: StreamLambdaBody<'_>,
    param: &str,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    emit_stream_lambda_prefix_and_locals(body, param, ctx).map(|(prefix, _)| prefix)
}

fn emit_stream_lambda_prefix_and_locals<'a>(
    body: StreamLambdaBody<'a>,
    param: &'a str,
    ctx: StreamRenderContext<'_>,
) -> Option<(String, Vec<&'a str>)> {
    let mut out = String::new();
    let mut locals = vec![param];
    let mutated = match body {
        StreamLambdaBody::Expr(_) => HashSet::new(),
        StreamLambdaBody::Block(block) => stream_lambda_mutated_locals(block, ctx),
        StreamLambdaBody::MethodReference { .. } => HashSet::new(),
    };
    for stmt in body.prefix_statements() {
        out.push_str(&emit_stream_lambda_stmt(
            stmt, param, &locals, ctx, &mutated, None,
        )?);
        out.push(' ');
        if let Stmt::VarDecl { name, .. } = stmt {
            locals.push(name);
        }
    }
    Some((out, locals))
}

fn emit_stream_lambda_stmt(
    stmt: &Stmt,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    mutated: &HashSet<String>,
    param_shape: Option<&RustSlotShape>,
) -> Option<String> {
    match stmt {
        Stmt::VarDecl {
            name, initializer, ..
        } => {
            let rust_name = crate::parser::to_snake_case(name);
            let mutability = if mutated.contains(name) || mutated.contains(&rust_name) {
                "mut "
            } else {
                ""
            };
            Some(format!(
                "let {mutability}{rust_name} = {};",
                initializer
                    .as_ref()
                    .and_then(|expr| {
                        emit_stream_lambda_body_with_locals(
                            expr,
                            param,
                            StreamLambdaValue::BorrowedItem,
                            locals,
                            ctx,
                            param_shape,
                        )
                    })
                    .unwrap_or_else(|| "Default::default()".to_string())
            ))
        }
        Stmt::Expr(expr) => Some(format!(
            "{};",
            emit_stream_lambda_body_with_locals(
                expr,
                param,
                StreamLambdaValue::BorrowedItem,
                locals,
                ctx,
                param_shape,
            )?
        )),
        Stmt::Assign { target, value } => Some(format!(
            "{} = {};",
            emit_stream_lambda_body_with_locals(
                target,
                param,
                StreamLambdaValue::BorrowedItem,
                locals,
                ctx,
                param_shape,
            )?,
            emit_stream_lambda_body_with_locals(
                value,
                param,
                StreamLambdaValue::BorrowedItem,
                locals,
                ctx,
                param_shape,
            )?
        )),
        _ => None,
    }
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
        Stmt::While { condition, body } => {
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
        Stmt::Block(block) => {
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
        | Expr::Raw(_) => {}
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

fn emit_stream_lambda_args_with_locals(
    args: &[Expr],
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<Vec<String>> {
    args.iter()
        .map(|arg| emit_stream_lambda_body_with_locals(arg, param, value, locals, ctx, param_shape))
        .collect()
}

fn emit_stream_lambda_call_args_for_receiver(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<Vec<String>> {
    let default_args =
        emit_stream_lambda_args_with_locals(args, param, value, locals, ctx, param_shape)?;
    let Some(facts) = ctx.facts else {
        return Some(default_args);
    };
    let Some(env) = ctx.env else {
        return Some(default_args);
    };
    let Some(slots) = crate::call_plan::external_call_arg_shapes(
        receiver,
        name,
        args.len(),
        facts,
        env.type_env(),
    ) else {
        return Some(default_args);
    };
    Some(
        args.iter()
            .zip(default_args)
            .zip(slots.iter())
            .map(|((arg, rendered), slot)| {
                adapt_stream_lambda_arg_for_slot(arg, rendered, slot, param, locals, ctx)
            })
            .collect(),
    )
}

fn adapt_stream_lambda_arg_for_slot(
    arg: &Expr,
    rendered: String,
    slot: &RustSlotShape,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> String {
    if shape_is_borrowed_string_content(slot) {
        return render_stream_lambda_borrowed_str_arg(arg, rendered, param, locals, ctx);
    }
    if shape_is_owned(slot) {
        return render_stream_lambda_owned_arg(arg, rendered, param);
    }
    rendered
}

fn render_stream_lambda_borrowed_str_arg(
    arg: &Expr,
    rendered: String,
    param: &str,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> String {
    match arg {
        Expr::Var(name) if name == param => format!("{param}.as_str()"),
        Expr::Var(name) if locals.iter().any(|local| *local == name) => {
            format!("{}.as_str()", crate::parser::to_snake_case(name))
        }
        Expr::Var(name) => ctx
            .env
            .filter(|env| env.local_shape(name).is_none())
            .and_then(|env| env.field_shape(name))
            .and_then(|shape| {
                shape
                    .optional_element_type()
                    .is_some_and(|element| element == JavaType::String)
                    .then(|| {
                        format!(
                            "self.{}.as_deref().expect(\"Java null receiver\")",
                            crate::parser::to_snake_case(name)
                        )
                    })
            })
            .unwrap_or_else(|| format!("{rendered}.as_str()")),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => ctx
            .env
            .and_then(|env| env.field_shape(field))
            .and_then(|shape| {
                shape
                    .optional_element_type()
                    .is_some_and(|element| element == JavaType::String)
                    .then(|| {
                        format!(
                            "self.{}.as_deref().expect(\"Java null receiver\")",
                            crate::parser::to_snake_case(field)
                        )
                    })
            })
            .unwrap_or_else(|| format!("{rendered}.as_str()")),
        _ => format!("{rendered}.as_str()"),
    }
}

fn render_stream_lambda_owned_arg(arg: &Expr, rendered: String, param: &str) -> String {
    match arg {
        Expr::Var(name) if name == param => format!("{param}.clone()"),
        Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This) => {
            format!("{rendered}.clone()")
        }
        _ => rendered,
    }
}

fn parenthesize_stream_expr_if_needed(
    rendered: String,
    own_prec: u8,
    parent_prec: u8,
    right_child: bool,
    binary_op: Option<BinaryOp>,
) -> Option<String> {
    let needs_parens = own_prec < parent_prec
        || (right_child
            && own_prec == parent_prec
            && binary_op.is_some_and(|op| !stream_binary_op_is_associative(op)));
    Some(if needs_parens {
        format!("({rendered})")
    } else {
        rendered
    })
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
        BinaryOp::Shl | BinaryOp::Shr => 8,
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

fn stream_static_owner_type_name_with_context(owner: &str, ctx: StreamRenderContext<'_>) -> String {
    ctx.facts
        .map(|facts| crate::call_plan::rust_static_owner_type_name_with_facts(owner, facts))
        .unwrap_or_else(|| crate::call_plan::rust_static_owner_type_name(owner))
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
            if !is_static_owner_name(name) {
                return None;
            }
            Some(stream_static_owner_type_name_with_context(name, ctx))
        }
        Expr::FieldAccess(inner, field) => {
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

fn emit_stream_lambda_receiver(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    emit_stream_lambda_receiver_with_locals(expr, param, value, &[param], ctx, None)
}

fn emit_stream_lambda_receiver_with_locals(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    param_shape: Option<&RustSlotShape>,
) -> Option<String> {
    match expr {
        Expr::Var(name) if name == param => Some(match value {
            StreamLambdaValue::FilterBorrowed
            | StreamLambdaValue::BorrowedItem
            | StreamLambdaValue::OwnedItem => param.to_string(),
        }),
        _ => emit_stream_lambda_body_with_locals(expr, param, value, locals, ctx, param_shape),
    }
}
