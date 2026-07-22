use crate::conversion_plan::LoweredType;
use crate::expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape_with_context};
use crate::facts::FactSet;
use crate::java_ir::{Block, Expr, JavaType, LambdaBody, PrimitiveType, Stmt};
use crate::shape_projection::iteration_element_shape;
use crate::slot_shape::RustSlotShape;
use crate::stdlib::JavaCollectionKind;
use crate::stream_shape::{
    infer_stream_lambda_body_shape_with_facts, infer_stream_map_to_int_shape,
    infer_stream_source_element_shape_with_facts,
};
use crate::types::{TypeEnv, optional_java_type, sequence_element_type};

pub struct StreamPlan<'a> {
    pub source: StreamSource<'a>,
    pub ops: Vec<StreamOp<'a>>,
    pub terminal: StreamTerminal<'a>,
    pub source_element: StreamElement,
    pub actual: LoweredType,
}

pub struct StreamSource<'a> {
    pub expr: &'a Expr,
    pub optional_sequence: bool,
    pub iteration: StreamSourceIteration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamElement {
    pub shape: RustSlotShape,
    pub value_category: StreamValueCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamValueCategory {
    Borrowed,
}

pub enum StreamOp<'a> {
    Filter(FilterPlan<'a>),
    Map {
        lambda: LambdaPlan<'a>,
        output_shape: RustSlotShape,
    },
    MapToPrimitive {
        lambda: LambdaPlan<'a>,
        output_shape: RustSlotShape,
    },
    FlatMap(FlatMapPlan<'a>),
}

pub enum FilterPlan<'a> {
    Predicate(LambdaPlan<'a>),
    NonNull {
        input_shape: RustSlotShape,
        output_shape: RustSlotShape,
    },
}

pub enum StreamTerminal<'a> {
    Sum,
    Reduce(ReducePlan<'a>),
    Collect(JavaCollectionKind),
    CollectToMap {
        key: LambdaPlan<'a>,
        value: LambdaPlan<'a>,
    },
    Join {
        delimiter: Option<&'a Expr>,
    },
    ToArray,
    FindFirst,
    AnyMatch(LambdaPlan<'a>),
    AllMatch(LambdaPlan<'a>),
}

pub struct ReducePlan<'a> {
    pub identity: &'a Expr,
    pub accumulator: ReduceLambdaPlan<'a>,
}

#[derive(Clone)]
pub struct LambdaPlan<'a> {
    pub param_name: String,
    pub body: StreamLambdaBody<'a>,
}

pub struct ReduceLambdaPlan<'a> {
    pub left_param_name: String,
    pub right_param_name: String,
    pub body: StreamLambdaBody<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamLambdaBody<'a> {
    Expr(&'a Expr),
    Block(&'a Block),
    MethodReference { target: &'a str, method: &'a str },
}

impl<'a> StreamLambdaBody<'a> {
    pub fn return_expr(self) -> Option<&'a Expr> {
        match self {
            StreamLambdaBody::Expr(expr) => Some(expr),
            StreamLambdaBody::Block(block) => block.statements.last().and_then(|stmt| match stmt {
                Stmt::Return(Some(expr)) => Some(expr),
                _ => None,
            }),
            StreamLambdaBody::MethodReference { .. } => None,
        }
    }

    pub fn prefix_statements(self) -> &'a [Stmt] {
        match self {
            StreamLambdaBody::Expr(_) => &[],
            StreamLambdaBody::Block(block) => {
                let len = block.statements.len();
                if matches!(block.statements.last(), Some(Stmt::Return(Some(_)))) {
                    &block.statements[..len - 1]
                } else {
                    &block.statements
                }
            }
            StreamLambdaBody::MethodReference { .. } => &[],
        }
    }
}

pub enum FlatMapPlan<'a> {
    CollectionStreamMethodReference {
        param_name: String,
    },
    LambdaCollectionStream(LambdaCollectionStreamPlan<'a>),
    LambdaStreamChain {
        lambda: LambdaPlan<'a>,
        pipeline: Box<StreamSubPipeline<'a>>,
    },
}

pub struct LambdaCollectionStreamPlan<'a> {
    pub lambda: LambdaPlan<'a>,
    pub source: &'a Expr,
    pub iteration: StreamSourceIteration,
}

pub struct StreamSubPipeline<'a> {
    pub source: StreamSource<'a>,
    pub ops: Vec<StreamOp<'a>>,
    pub source_element: StreamElement,
    pub output_shape: RustSlotShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSourceIteration {
    Borrowed,
    OwnedValue,
}

pub fn build_stream_plan<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<StreamPlan<'a>> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };

    let terminal = match name.as_str() {
        "sum" if args.is_empty() => StreamTerminal::Sum,
        "reduce" if args.len() == 2 => StreamTerminal::Reduce(ReducePlan {
            identity: &args[0],
            accumulator: reduce_lambda_plan(&args[1])?,
        }),
        "collect" if args.len() == 1 => stream_collect_terminal(&args[0])?,
        "toArray" | "to_array" if args.is_empty() => StreamTerminal::ToArray,
        "findFirst" if args.is_empty() => StreamTerminal::FindFirst,
        "anyMatch" if args.len() == 1 => StreamTerminal::AnyMatch(lambda_plan(&args[0])?),
        "allMatch" | "all_match" if args.len() == 1 => {
            StreamTerminal::AllMatch(lambda_plan(&args[0])?)
        }
        _ => return None,
    };
    let source_expr = receiver_source(receiver)?;
    let source_shape = infer_stream_source_element_shape_with_facts(source_expr, env, facts)?;
    let (source, ops, current_shape) = collect_stream_chain(receiver, source_shape, env, facts)?;
    let actual = stream_terminal_actual(&terminal, current_shape.clone(), env, facts);
    Some(StreamPlan {
        source: StreamSource {
            expr: source,
            optional_sequence: stream_source_is_optional_sequence(source, env, facts),
            iteration: stream_source_iteration(source),
        },
        ops,
        terminal,
        source_element: StreamElement {
            shape: infer_stream_source_element_shape_with_facts(source, env, facts)?,
            value_category: StreamValueCategory::Borrowed,
        },
        actual,
    })
}

fn stream_source_is_optional_sequence(source: &Expr, env: &TypeEnv, facts: &FactSet) -> bool {
    let source_ty = crate::types::infer_expr_type(source, env);
    let Some(element_ty) = crate::types::optional_element_type(&source_ty) else {
        return false;
    };
    crate::types::stream_element_type(&element_ty).is_some()
        || crate::stream_shape::stream_source_capability_policy(&element_ty, facts)
            .capability_element
            .is_some()
}

fn is_collectors_to_list(expr: &Expr) -> bool {
    is_collectors_terminal(expr, "toList")
}

fn is_collectors_to_set(expr: &Expr) -> bool {
    is_collectors_terminal(expr, "toSet")
}

fn stream_collect_terminal(expr: &Expr) -> Option<StreamTerminal<'_>> {
    if is_collectors_to_list(expr) {
        return Some(StreamTerminal::Collect(JavaCollectionKind::List));
    }
    if is_collectors_to_set(expr) {
        return Some(StreamTerminal::Collect(JavaCollectionKind::Set));
    }
    if let Some(delimiter) = collectors_joining_delimiter(expr) {
        return Some(StreamTerminal::Join { delimiter });
    }
    let (key, value) = collectors_to_map_lambdas(expr)?;
    Some(StreamTerminal::CollectToMap { key, value })
}

fn collectors_joining_delimiter(expr: &Expr) -> Option<Option<&Expr>> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if name != "joining" {
        return None;
    }
    if !matches!(receiver.as_ref(), Expr::Var(owner) if owner == "Collectors" || owner == "collectors")
    {
        return None;
    }
    match args.as_slice() {
        [] => Some(None),
        [delimiter] => Some(Some(delimiter)),
        _ => None,
    }
}

fn collectors_to_map_lambdas<'a>(expr: &'a Expr) -> Option<(LambdaPlan<'a>, LambdaPlan<'a>)> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if name != "toMap" || args.len() != 2 {
        return None;
    }
    if !matches!(receiver.as_ref(), Expr::Var(owner) if owner == "Collectors" || owner == "collectors")
    {
        return None;
    }
    Some((lambda_plan(&args[0])?, lambda_plan(&args[1])?))
}

fn is_collectors_terminal(expr: &Expr, terminal_name: &str) -> bool {
    matches!(
        expr,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == terminal_name
            && args.is_empty()
            && matches!(receiver.as_ref(), Expr::Var(owner) if owner == "Collectors" || owner == "collectors")
    )
}

fn receiver_source(expr: &Expr) -> Option<&Expr> {
    collect_stream_chain_source(expr)
}

fn collect_stream_chain_source(expr: &Expr) -> Option<&Expr> {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };

    match name.as_str() {
        "stream" | "parallelStream" | "parallel_stream" if args.is_empty() => receiver.as_deref(),
        "stream"
            if args.len() == 1
                && matches!(receiver.as_deref(), Some(Expr::Var(owner))
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "Arrays") =>
        {
            args.first()
        }
        "range" | "range_closed"
            if args.len() == 2
                && matches!(receiver.as_deref(), Some(Expr::Var(owner))
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "IntStream") =>
        {
            Some(expr)
        }
        "filter" | "map" | "mapToInt" | "map_to_int" | "mapToDouble" | "map_to_double"
        | "mapToObj" | "map_to_obj" | "flatMap" | "flat_map"
            if args.len() == 1 =>
        {
            collect_stream_chain_source(receiver.as_deref()?)
        }
        "reduce" if args.len() == 2 => collect_stream_chain_source(receiver.as_deref()?),
        _ => None,
    }
}

fn collect_stream_chain<'a>(
    expr: &'a Expr,
    input_shape: RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<(&'a Expr, Vec<StreamOp<'a>>, RustSlotShape)> {
    let Expr::Call {
        receiver,
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };

    match name.as_str() {
        "stream" | "parallelStream" | "parallel_stream" if args.is_empty() => receiver
            .as_deref()
            .map(|source| (source, Vec::new(), input_shape)),
        "stream"
            if args.len() == 1
                && matches!(receiver.as_deref(), Some(Expr::Var(owner))
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "Arrays") =>
        {
            args.first().map(|source| (source, Vec::new(), input_shape))
        }
        "range" | "range_closed"
            if args.len() == 2
                && matches!(receiver.as_deref(), Some(Expr::Var(owner))
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "IntStream") =>
        {
            Some((expr, Vec::new(), input_shape))
        }
        "filter" if args.len() == 1 => {
            let (source, mut ops, current_shape) =
                collect_stream_chain(receiver.as_deref()?, input_shape, env, facts)?;
            if is_non_null_filter_arg(&args[0]) {
                let output_shape = non_null_filter_output_shape(&current_shape);
                ops.push(StreamOp::Filter(FilterPlan::NonNull {
                    input_shape: current_shape,
                    output_shape: output_shape.clone(),
                }));
                Some((source, ops, output_shape))
            } else {
                ops.push(StreamOp::Filter(FilterPlan::Predicate(lambda_plan(
                    &args[0],
                )?)));
                Some((source, ops, current_shape))
            }
        }
        "map" if args.len() == 1 => {
            let lambda = lambda_plan(&args[0])?;
            let (source, mut ops, current_shape) =
                collect_stream_chain(receiver.as_deref()?, input_shape, env, facts)?;
            let output_shape =
                infer_stream_lambda_output_shape(&lambda, &current_shape, env, facts);
            ops.push(StreamOp::Map {
                lambda,
                output_shape: output_shape.clone(),
            });
            Some((source, ops, output_shape))
        }
        "mapToObj" | "map_to_obj" if args.len() == 1 => {
            let lambda = lambda_plan(&args[0])?;
            let (source, mut ops, current_shape) =
                collect_stream_chain(receiver.as_deref()?, input_shape, env, facts)?;
            let output_shape =
                infer_stream_lambda_output_shape(&lambda, &current_shape, env, facts);
            ops.push(StreamOp::Map {
                lambda,
                output_shape: output_shape.clone(),
            });
            Some((source, ops, output_shape))
        }
        "mapToInt" | "map_to_int" | "mapToDouble" | "map_to_double" if args.len() == 1 => {
            let lambda = lambda_plan(&args[0])?;
            let (source, mut ops, _) =
                collect_stream_chain(receiver.as_deref()?, input_shape, env, facts)?;
            let output_shape = if matches!(name.as_str(), "mapToDouble" | "map_to_double") {
                RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Double))
            } else if matches!(lambda.body, StreamLambdaBody::MethodReference { .. }) {
                infer_stream_lambda_output_shape(
                    &lambda,
                    &RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
                    env,
                    facts,
                )
            } else {
                infer_stream_map_to_int_shape(lambda.body.return_expr()?)
            };
            ops.push(StreamOp::MapToPrimitive {
                lambda,
                output_shape: output_shape.clone(),
            });
            Some((source, ops, output_shape))
        }
        "flatMap" | "flat_map" if args.len() == 1 => {
            let (source, mut ops, current_shape) =
                collect_stream_chain(receiver.as_deref()?, input_shape, env, facts)?;
            let flat_map = flat_map_plan(&args[0], &current_shape, env, facts)?;
            let output_shape = infer_flat_map_output_shape(&flat_map, &current_shape, env, facts)?;
            ops.push(StreamOp::FlatMap(flat_map));
            Some((source, ops, output_shape))
        }
        _ => None,
    }
}

fn stream_source_iteration(source: &Expr) -> StreamSourceIteration {
    match source {
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.len() == 2
            && matches!(receiver.as_ref(), Expr::Var(owner)
                if crate::stdlib::canonical_external_receiver_type_key(owner) == "IntStream")
            && matches!(name.as_str(), "range" | "range_closed") =>
        {
            StreamSourceIteration::OwnedValue
        }
        Expr::Var(_) | Expr::FieldAccess(_, _) | Expr::ArrayAccess(_, _) => {
            StreamSourceIteration::Borrowed
        }
        _ => StreamSourceIteration::OwnedValue,
    }
}

fn lambda_plan(lambda: &Expr) -> Option<LambdaPlan<'_>> {
    match lambda {
        Expr::Lambda { params, body } => {
            let param = params.first()?.name.clone();
            let body = match body {
                LambdaBody::Expr(body) => StreamLambdaBody::Expr(body),
                LambdaBody::Block(block) if block_returns_value(block) => {
                    StreamLambdaBody::Block(block)
                }
                LambdaBody::Block(_) => return None,
            };
            Some(LambdaPlan {
                param_name: param,
                body,
            })
        }
        Expr::MethodReference { target, method } => Some(LambdaPlan {
            param_name: "value".to_string(),
            body: StreamLambdaBody::MethodReference { target, method },
        }),
        _ => None,
    }
}

fn reduce_lambda_plan(lambda: &Expr) -> Option<ReduceLambdaPlan<'_>> {
    match lambda {
        Expr::Lambda { params, body } if params.len() == 2 => {
            let body = match body {
                LambdaBody::Expr(body) => StreamLambdaBody::Expr(body),
                LambdaBody::Block(block) if block_returns_value(block) => {
                    StreamLambdaBody::Block(block)
                }
                LambdaBody::Block(_) => return None,
            };
            Some(ReduceLambdaPlan {
                left_param_name: params[0].name.clone(),
                right_param_name: params[1].name.clone(),
                body,
            })
        }
        _ => None,
    }
}

fn block_returns_value(block: &Block) -> bool {
    matches!(block.statements.last(), Some(Stmt::Return(Some(_))))
}

fn infer_stream_lambda_output_shape(
    lambda: &LambdaPlan<'_>,
    input_shape: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> RustSlotShape {
    if let StreamLambdaBody::MethodReference { target, method } = lambda.body {
        return infer_stream_method_reference_output_shape(target, method, input_shape)
            .unwrap_or_else(|| input_shape.clone());
    }
    let Some(return_expr) = lambda.body.return_expr() else {
        return input_shape.clone();
    };
    let mut lambda_env = env.clone();
    lambda_env.insert_local(lambda.param_name.clone(), input_shape.java_ty.clone());
    insert_lambda_prefix_locals(lambda, &mut lambda_env);
    let output_shape =
        infer_stream_lambda_body_shape_with_facts(return_expr, input_shape, &lambda_env, facts);
    if matches!(lambda.body, StreamLambdaBody::Block(block) if block_has_null_return(block)) {
        let mut nullable = output_shape.clone();
        nullable.java_ty = optional_java_type(output_shape.java_ty);
        nullable
    } else {
        output_shape
    }
}

fn infer_stream_method_reference_output_shape(
    target: &str,
    method: &str,
    input_shape: &RustSlotShape,
) -> Option<RustSlotShape> {
    let target = crate::stdlib::canonical_external_receiver_type_key(target);
    match (target.as_str(), method) {
        ("Integer", "intValue" | "int_value") => Some(RustSlotShape::for_storage(
            JavaType::Primitive(PrimitiveType::Int),
        )),
        ("String", "valueOf" | "value_of") | ("Object", "toString" | "to_string") => {
            Some(crate::shape_projection::owned_string_value_shape())
        }
        (_, method) if method.starts_with("get") || method.starts_with("get_") => {
            let mut shape = input_shape.clone();
            shape.java_ty = JavaType::Unknown(crate::parser::to_snake_case(method));
            Some(shape)
        }
        _ => None,
    }
}

fn block_has_null_return(block: &Block) -> bool {
    block.statements.iter().any(stmt_has_null_return)
}

fn stmt_has_null_return(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(Some(Expr::Literal(crate::java_ir::Literal::Null))) => true,
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_has_null_return(then_block)
                || else_block.as_ref().is_some_and(block_has_null_return)
        }
        Stmt::Block(block) => block_has_null_return(block),
        _ => false,
    }
}

fn is_non_null_filter_arg(expr: &Expr) -> bool {
    match expr {
        Expr::MethodReference { target, method } => {
            crate::stdlib::canonical_external_receiver_type_key(target) == "Objects"
                && matches!(method.as_str(), "nonNull" | "non_null")
        }
        Expr::Lambda { params, body } if params.len() == 1 => {
            let param_name = &params[0].name;
            let LambdaBody::Expr(expr) = body else {
                return false;
            };
            is_non_null_predicate_expr(expr, param_name)
        }
        _ => false,
    }
}

fn is_non_null_predicate_expr(expr: &Expr, param_name: &str) -> bool {
    matches!(
        expr,
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Ne,
            left,
            right,
        } if is_param_and_null(left, right, param_name) || is_param_and_null(right, left, param_name)
    )
}

fn is_param_and_null(left: &Expr, right: &Expr, param_name: &str) -> bool {
    matches!(left, Expr::Var(name) if name == param_name)
        && matches!(right, Expr::Literal(crate::java_ir::Literal::Null))
}

fn non_null_filter_output_shape(input_shape: &RustSlotShape) -> RustSlotShape {
    if let Some(element_ty) = input_shape.optional_element_type() {
        let mut output = input_shape.clone();
        output.java_ty = element_ty;
        output
    } else {
        input_shape.clone()
    }
}

fn flat_map_plan<'a>(
    expr: &'a Expr,
    input_shape: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<FlatMapPlan<'a>> {
    if let Expr::MethodReference { target, method } = expr {
        if method != "stream" || !matches!(target.as_str(), "Collection" | "Iterable") {
            return None;
        }
        return Some(FlatMapPlan::CollectionStreamMethodReference {
            param_name: "item".to_string(),
        });
    }

    let lambda = lambda_plan(expr)?;
    if let Some(plan) = lambda_collection_stream_plan(lambda.clone()) {
        return Some(FlatMapPlan::LambdaCollectionStream(plan));
    }
    lambda_stream_chain_plan(lambda, input_shape, env, facts)
}

fn lambda_collection_stream_plan<'a>(
    lambda: LambdaPlan<'a>,
) -> Option<LambdaCollectionStreamPlan<'a>> {
    let Expr::Call {
        receiver: Some(source),
        name,
        args,
        ..
    } = lambda.body.return_expr()?
    else {
        return None;
    };
    if name != "stream" || !args.is_empty() {
        return None;
    }
    let iteration = if matches!(source.as_ref(), Expr::Var(name) if name == &lambda.param_name) {
        StreamSourceIteration::Borrowed
    } else {
        StreamSourceIteration::OwnedValue
    };
    Some(LambdaCollectionStreamPlan {
        lambda,
        source,
        iteration,
    })
}

fn lambda_stream_chain_plan<'a>(
    lambda: LambdaPlan<'a>,
    input_shape: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<FlatMapPlan<'a>> {
    let return_expr = lambda.body.return_expr()?;
    let source_expr = receiver_source(return_expr)?;
    let mut lambda_env = env.clone();
    lambda_env.insert_local(lambda.param_name.clone(), input_shape.java_ty.clone());
    insert_lambda_prefix_locals(&lambda, &mut lambda_env);
    let source_shape =
        infer_stream_source_element_shape_with_facts(source_expr, &lambda_env, facts)?;
    let (source, ops, output_shape) =
        collect_stream_chain(return_expr, source_shape, &lambda_env, facts)?;
    Some(FlatMapPlan::LambdaStreamChain {
        lambda,
        pipeline: Box::new(StreamSubPipeline {
            source: StreamSource {
                expr: source,
                optional_sequence: stream_source_is_optional_sequence(source, &lambda_env, facts),
                iteration: stream_source_iteration(source),
            },
            ops,
            source_element: StreamElement {
                shape: infer_stream_source_element_shape_with_facts(source, &lambda_env, facts)?,
                value_category: StreamValueCategory::Borrowed,
            },
            output_shape,
        }),
    })
}

fn insert_lambda_prefix_locals(lambda: &LambdaPlan<'_>, env: &mut TypeEnv) {
    let StreamLambdaBody::Block(block) = lambda.body else {
        return;
    };
    for stmt in block
        .statements
        .iter()
        .take(block.statements.len().saturating_sub(1))
    {
        if let Stmt::VarDecl { ty, name, .. } = stmt {
            env.insert_local(name.clone(), ty.clone());
        }
    }
}

fn infer_flat_map_output_shape(
    flat_map: &FlatMapPlan<'_>,
    input_shape: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    match flat_map {
        FlatMapPlan::CollectionStreamMethodReference { .. } => {
            sequence_element_type(&input_shape.java_ty)
                .map(|element_ty| iteration_element_shape(element_ty, facts))
        }
        FlatMapPlan::LambdaCollectionStream(plan) => {
            let mut lambda_env = env.clone();
            lambda_env.insert_local(plan.lambda.param_name.clone(), input_shape.java_ty.clone());
            infer_stream_source_element_shape_with_facts(plan.source, &lambda_env, facts)
        }
        FlatMapPlan::LambdaStreamChain { pipeline, .. } => Some(pipeline.output_shape.clone()),
    }
}

pub fn stream_terminal_actual(
    terminal: &StreamTerminal<'_>,
    element_shape: RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> LoweredType {
    match terminal {
        StreamTerminal::Sum => LoweredType::from_java(JavaType::Primitive(PrimitiveType::Int)),
        StreamTerminal::Reduce(reduce) => {
            LoweredType::from_java(crate::types::infer_expr_type(reduce.identity, env))
        }
        StreamTerminal::Collect(JavaCollectionKind::List) => stream_collect_actual(element_shape),
        StreamTerminal::Collect(JavaCollectionKind::Set) => {
            LoweredType::collected_set_from_element_shape(element_shape)
        }
        StreamTerminal::Collect(JavaCollectionKind::Map) => {
            LoweredType::from_java(JavaType::Unknown("stream_collect_map".to_string()))
        }
        StreamTerminal::CollectToMap { key, value } => {
            let key_shape = infer_terminal_lambda_output_shape(key, &element_shape, env, facts);
            let value_shape = infer_terminal_lambda_output_shape(value, &element_shape, env, facts);
            LoweredType::from_java(JavaType::Named {
                name: "Map".to_string(),
                args: vec![key_shape.java_ty, value_shape.java_ty],
            })
        }
        StreamTerminal::Join { .. } => LoweredType::from_java(JavaType::String),
        StreamTerminal::ToArray => stream_collect_actual(element_shape),
        StreamTerminal::FindFirst => LoweredType::optional_element_borrow_shape(element_shape),
        StreamTerminal::AnyMatch(_) | StreamTerminal::AllMatch(_) => {
            LoweredType::from_java(JavaType::Primitive(PrimitiveType::Boolean))
        }
    }
}

fn infer_terminal_lambda_output_shape(
    lambda: &LambdaPlan<'_>,
    element_shape: &RustSlotShape,
    _env: &TypeEnv,
    _facts: &FactSet,
) -> RustSlotShape {
    if let StreamLambdaBody::MethodReference { target, method } = lambda.body {
        return infer_stream_method_reference_output_shape(target, method, element_shape)
            .unwrap_or_else(|| element_shape.clone());
    }
    let Some(return_expr) = lambda.body.return_expr() else {
        return element_shape.clone();
    };
    let mut lambda_env = ShapeEnv::new();
    lambda_env.insert_local_shape(lambda.param_name.clone(), element_shape.clone());
    infer_expr_shape_with_context(return_expr, &lambda_env, ExprShapeContext::default())
}

pub fn stream_collect_actual(element_shape: RustSlotShape) -> LoweredType {
    LoweredType::collected_sequence_from_element_shape(element_shape)
}

pub fn stream_identity_value_expr(param_name: &str, element_shape: &RustSlotShape) -> String {
    if element_shape.is_shared_borrowed_value() {
        if element_shape.capabilities().copy {
            format!("*{param_name}")
        } else {
            format!("(*{param_name}).clone()")
        }
    } else {
        param_name.to_string()
    }
}
