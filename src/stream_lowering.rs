use crate::coercion::LoweredExpr;
use crate::expr_shape::ShapeEnv;
use crate::facts::FactSet;
use crate::java_ir::{BinaryOp, CallKind, Expr, JavaType, Literal, Stmt, UnaryOp};
use crate::literal_lowering::literal_to_rust;
use crate::operator_lowering::{binary_op_to_rust, stream_unary_prefix};
use crate::rust_ir::RustExpr;
use crate::stream::{
    FilterPlan, FlatMapPlan, LambdaPlan, StreamLambdaBody, StreamOp, StreamPlan, StreamSource,
    StreamSourceIteration, StreamSubPipeline, StreamTerminal,
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLambdaValue {
    FilterBorrowed,
    BorrowedItem,
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
    emit_stream_lambda_body_with_locals(expr, param, value, &[param], ctx)
}

fn emit_stream_lambda_body_with_locals(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    emit_stream_lambda_body_with_locals_prec(expr, param, value, locals, ctx, 0, false)
}

fn emit_stream_lambda_body_with_locals_prec(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
    parent_prec: u8,
    right_child: bool,
) -> Option<String> {
    let (rendered, own_prec) = match expr {
        Expr::Literal(lit) => (literal_to_rust(lit), 12),
        Expr::Var(name) if name == param => (
            match value {
                StreamLambdaValue::FilterBorrowed => format!("**{param}"),
                StreamLambdaValue::BorrowedItem => format!("*{param}"),
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
                        left, param, value, locals, ctx, prec, false
                    )?,
                    binary_op_to_rust(*op),
                    emit_stream_lambda_body_with_locals_prec(
                        right, param, value, locals, ctx, prec, true
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
                        expr, param, value, locals, ctx, 10, false
                    )?
                ),
                10,
            )
        }
        Expr::FieldAccess(receiver, field) => {
            if field == "length" {
                let receiver =
                    emit_stream_lambda_receiver_with_locals(receiver, param, value, locals, ctx)?;
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
                    emit_stream_lambda_receiver_with_locals(receiver, param, value, locals, ctx)?,
                    crate::parser::to_snake_case(field)
                ),
                11,
            )
        }
        Expr::ArrayAccess(array, index) => (
            format!(
                "{}[{} as usize]",
                emit_stream_lambda_receiver_with_locals(array, param, value, locals, ctx)?,
                emit_stream_lambda_body_with_locals(index, param, value, locals, ctx)?
            ),
            11,
        ),
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            resolved,
        } => {
            let args = emit_stream_lambda_args_with_locals(args, param, value, locals, ctx)?;
            if let Some(resolved) = resolved
                && matches!(resolved.kind, CallKind::Static)
            {
                return parenthesize_stream_expr_if_needed(
                    format!(
                        "{}::{}({})",
                        crate::call_plan::rust_static_owner_type_name(&resolved.declaring_type),
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
                if owner == "Integer" && name == "parseInt" && args.len() == 1 {
                    return parenthesize_stream_expr_if_needed(
                        format!("{}.parse::<i32>().unwrap()", args[0]),
                        11,
                        parent_prec,
                        right_child,
                        None,
                    );
                }
                if owner == "Math" && name == "toIntExact" && args.len() == 1 {
                    return parenthesize_stream_expr_if_needed(
                        format!("{} as i32", args[0]),
                        6,
                        parent_prec,
                        right_child,
                        None,
                    );
                }
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
            let receiver =
                emit_stream_lambda_receiver_with_locals(receiver, param, value, locals, ctx)?;
            let rendered = match name.as_str() {
                "isEmpty" if args.is_empty() => format!("{receiver}.is_empty()"),
                "length" if args.is_empty() => format!("{receiver}.len() as i32"),
                "split" if args.len() == 1 => format!(
                    "{receiver}.split({}).collect::<Vec<_>>()",
                    stream_split_pattern_arg(&args[0])
                ),
                "trim" if args.is_empty() => format!("{receiver}.trim()"),
                "toLowerCase" if args.is_empty() => format!("{receiver}.to_lowercase()"),
                "toString" if args.is_empty() => format!("{receiver}.to_string()"),
                _ => format!(
                    "{}.{}({})",
                    receiver,
                    crate::parser::to_snake_case(name),
                    args.join(", ")
                ),
            };
            (
                rendered,
                if name == "length" && args.is_empty() {
                    6
                } else {
                    11
                },
            )
        }
        Expr::Call {
            receiver: None,
            name,
            args,
            resolved,
        } => {
            let args =
                emit_stream_lambda_args_with_locals(args, param, value, locals, ctx)?.join(", ");
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
                    emit_stream_lambda_args_with_locals(args, param, value, locals, ctx)?
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
    render_stream_ops(&mut out, &plan.ops, ctx)?;
    render_stream_terminal(&mut out, plan, ctx)?;
    Some(LoweredExpr {
        expr: RustExpr::Raw(out),
        actual: plan.actual.clone(),
    })
}

fn render_stream_source_iter(source: &str, stream_source: &StreamSource<'_>) -> String {
    if stream_source.optional_sequence {
        format!("{source}.as_ref().unwrap().iter()")
    } else {
        format!("{source}.iter()")
    }
}

fn render_stream_ops(
    out: &mut String,
    ops: &[StreamOp<'_>],
    ctx: StreamRenderContext<'_>,
) -> Option<()> {
    let mut item_is_borrowed = true;
    for op in ops {
        match op {
            StreamOp::Filter(filter) => match filter {
                FilterPlan::Predicate(lambda) => {
                    out.push_str(&format!(
                        ".filter(|{}| {})",
                        lambda.param_name,
                        emit_stream_lambda_closure_body(
                            lambda,
                            StreamLambdaValue::FilterBorrowed,
                            ctx
                        )?
                    ));
                }
                FilterPlan::NonNull { input_shape, .. } => {
                    if input_shape.optional_element_type().is_some() {
                        if item_is_borrowed {
                            out.push_str(".filter_map(|item| item.as_ref())");
                        } else {
                            out.push_str(".filter_map(|item| item)");
                        }
                    }
                }
            },
            StreamOp::Map { lambda, .. } | StreamOp::MapToPrimitive { lambda, .. } => {
                out.push_str(&format!(
                    ".map(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_closure_body(lambda, StreamLambdaValue::BorrowedItem, ctx)?
                ));
                item_is_borrowed = false;
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
                }
            },
        }
    }
    Some(())
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
    let mut out = render_stream_source_iter(&source, &pipeline.source);
    render_stream_ops(&mut out, &pipeline.ops, ctx)?;
    Some(out)
}

fn render_stream_terminal(
    out: &mut String,
    plan: &StreamPlan<'_>,
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
                )?,
                emit_stream_collect_to_map_lambda_body(
                    value,
                    plan,
                    StreamLambdaValue::BorrowedItem,
                    ctx,
                )?
            ));
        }
        StreamTerminal::FindFirst => out.push_str(".next()"),
        StreamTerminal::AnyMatch(lambda) => {
            out.push_str(&format!(
                ".any(|{}| {})",
                lambda.param_name,
                emit_stream_lambda_closure_body(lambda, StreamLambdaValue::BorrowedItem, ctx)?
            ));
        }
    };
    Some(())
}

fn emit_stream_collect_to_map_lambda_body(
    lambda: &LambdaPlan<'_>,
    plan: &StreamPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    let return_expr = lambda.body.return_expr()?;
    if matches!(return_expr, Expr::Var(name) if name == &lambda.param_name) {
        return Some(crate::stream::stream_identity_value_expr(
            &lambda.param_name,
            &plan.source_element.shape,
        ));
    }
    emit_stream_lambda_body_with_context(return_expr, &lambda.param_name, value, ctx)
}

fn emit_stream_lambda_closure_body(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    if let Some(nullable_body) = emit_nullable_stream_lambda_closure_body(lambda, value, ctx) {
        return Some(nullable_body);
    }
    let return_expr = lambda.body.return_expr()?;
    let (prefix, locals) =
        emit_stream_lambda_prefix_and_locals(lambda.body, &lambda.param_name, ctx)?;
    let return_expr =
        emit_stream_lambda_body_with_locals(return_expr, &lambda.param_name, value, &locals, ctx)?;
    if prefix.is_empty() {
        Some(return_expr)
    } else {
        Some(format!("{{ {prefix}{return_expr} }}"))
    }
}

fn emit_nullable_stream_lambda_closure_body(
    lambda: &LambdaPlan<'_>,
    value: StreamLambdaValue,
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    let StreamLambdaBody::Block(block) = lambda.body else {
        return None;
    };
    let (guard_index, condition) = first_null_return_guard(block)?;
    let final_expr = lambda.body.return_expr()?;
    let mutated = stream_lambda_mutated_locals(block);
    let mut locals = vec![lambda.param_name.as_str()];
    let mut before_guard = String::new();
    for stmt in &block.statements[..guard_index] {
        before_guard.push_str(&emit_stream_lambda_stmt(
            stmt,
            &lambda.param_name,
            &locals,
            ctx,
            &mutated,
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
        )?);
        else_body.push(' ');
        if let Stmt::VarDecl { name, .. } = stmt {
            locals.push(name);
        }
    }
    let condition =
        emit_stream_lambda_body_with_locals(condition, &lambda.param_name, value, &locals, ctx)?;
    let final_expr =
        emit_stream_lambda_body_with_locals(final_expr, &lambda.param_name, value, &locals, ctx)?;
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
        StreamLambdaBody::Block(block) => stream_lambda_mutated_locals(block),
    };
    for stmt in body.prefix_statements() {
        out.push_str(&emit_stream_lambda_stmt(
            stmt, param, &locals, ctx, &mutated,
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
            )?,
            emit_stream_lambda_body_with_locals(
                value,
                param,
                StreamLambdaValue::BorrowedItem,
                locals,
                ctx,
            )?
        )),
        _ => None,
    }
}

fn stream_lambda_mutated_locals(block: &crate::java_ir::Block) -> HashSet<String> {
    let mut mutated = HashSet::new();
    for stmt in &block.statements {
        collect_stream_stmt_mutated_locals(stmt, &mut mutated);
    }
    mutated
}

fn collect_stream_stmt_mutated_locals(stmt: &Stmt, mutated: &mut HashSet<String>) {
    match stmt {
        Stmt::Assign { target, value } => {
            collect_stream_assignment_target(target, mutated);
            collect_stream_expr_mutated_locals(value, mutated);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_stream_expr_mutated_locals(expr, mutated);
        }
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_stream_expr_mutated_locals(initializer, mutated);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_stream_expr_mutated_locals(condition, mutated);
            for stmt in &then_block.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated);
            }
            if let Some(else_block) = else_block {
                for stmt in &else_block.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated);
                }
            }
        }
        Stmt::While { condition, body } => {
            collect_stream_expr_mutated_locals(condition, mutated);
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated);
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
                collect_stream_stmt_mutated_locals(stmt, mutated);
            }
            if let Some(condition) = condition {
                collect_stream_expr_mutated_locals(condition, mutated);
            }
            for expr in update {
                collect_stream_expr_mutated_locals(expr, mutated);
            }
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated);
            }
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_stream_expr_mutated_locals(iterable, mutated);
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated);
            }
        }
        Stmt::Switch { selector, cases } => {
            collect_stream_expr_mutated_locals(selector, mutated);
            for case in cases {
                for stmt in &case.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated);
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
                collect_stream_stmt_mutated_locals(stmt, mutated);
            }
            for stmt in &body.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated);
            }
            for catch in catches {
                for stmt in &catch.body.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated);
                }
            }
            if let Some(finally_block) = finally_block {
                for stmt in &finally_block.statements {
                    collect_stream_stmt_mutated_locals(stmt, mutated);
                }
            }
        }
        Stmt::Block(block) => {
            for stmt in &block.statements {
                collect_stream_stmt_mutated_locals(stmt, mutated);
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

fn collect_stream_expr_mutated_locals(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => collect_stream_assignment_target(expr, mutated),
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_stream_expr_mutated_locals(expr, mutated);
        }
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => {
            collect_stream_assignment_target(left, mutated);
            collect_stream_expr_mutated_locals(right, mutated);
        }
        Expr::Binary { left, right, .. } => {
            collect_stream_expr_mutated_locals(left, mutated);
            collect_stream_expr_mutated_locals(right, mutated);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_stream_expr_mutated_locals(condition, mutated);
            collect_stream_expr_mutated_locals(then_expr, mutated);
            collect_stream_expr_mutated_locals(else_expr, mutated);
        }
        Expr::FieldAccess(receiver, _) => collect_stream_expr_mutated_locals(receiver, mutated),
        Expr::ArrayAccess(array, index) => {
            collect_stream_expr_mutated_locals(array, mutated);
            collect_stream_expr_mutated_locals(index, mutated);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                collect_stream_expr_mutated_locals(expr, mutated);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    collect_stream_expr_mutated_locals(expr, mutated);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for expr in items {
                collect_stream_expr_mutated_locals(expr, mutated);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_stream_expr_mutated_locals(receiver, mutated);
            }
            for arg in args {
                collect_stream_expr_mutated_locals(arg, mutated);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_stream_expr_mutated_locals(arg, mutated);
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

fn collect_stream_assignment_target(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            mutated.insert(name.clone());
            mutated.insert(crate::parser::to_snake_case(name));
        }
        Expr::FieldAccess(receiver, _) | Expr::ArrayAccess(receiver, _) => {
            collect_stream_assignment_target(receiver, mutated);
        }
        _ => collect_stream_expr_mutated_locals(expr, mutated),
    }
}

fn emit_stream_lambda_args_with_locals(
    args: &[Expr],
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> Option<Vec<String>> {
    args.iter()
        .map(|arg| emit_stream_lambda_body_with_locals(arg, param, value, locals, ctx))
        .collect()
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
            Some(crate::call_plan::rust_static_owner_type_name(name))
        }
        Expr::FieldAccess(inner, field) => {
            let _ = stream_static_owner_expr_segment(inner, locals, ctx)?;
            Some(crate::call_plan::rust_static_owner_type_name(field))
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

fn stream_split_pattern_arg(arg: &str) -> String {
    arg.strip_suffix(".to_string()").unwrap_or(arg).to_string()
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
    emit_stream_lambda_receiver_with_locals(expr, param, value, &[param], ctx)
}

fn emit_stream_lambda_receiver_with_locals(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
    locals: &[&str],
    ctx: StreamRenderContext<'_>,
) -> Option<String> {
    match expr {
        Expr::Var(name) if name == param => Some(match value {
            StreamLambdaValue::FilterBorrowed | StreamLambdaValue::BorrowedItem => {
                param.to_string()
            }
        }),
        _ => emit_stream_lambda_body_with_locals(expr, param, value, locals, ctx),
    }
}
