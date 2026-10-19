use crate::conversion_plan::LoweredType;
use crate::conversion_plan::effective_stream_item_shape;
use crate::expr_shape::{ExprShapeContext, ShapeEnv, infer_expr_shape_with_context};
use crate::facts::FactSet;
use crate::java_ir::{Block, Expr, JavaType, LambdaBody, PrimitiveType, Stmt};
use crate::method_reference_shape::infer_single_input_method_reference_output_shape_with_context;
use crate::shape_projection::iteration_element_shape;
use crate::slot_shape::RustSlotShape;
use crate::stdlib::JavaCollectionKind;
use crate::stream_shape::{
    infer_stream_lambda_body_shape_with_facts, infer_stream_source_element_shape_with_facts,
};
use crate::types::{TypeEnv, optional_java_type, sequence_element_type};

pub struct StreamPlan<'a> {
    source: StreamSource<'a>,
    ops: Vec<StreamOp<'a>>,
    terminal: StreamTerminal<'a>,
    collection_empty_add_changes: bool,
    source_element: StreamElement,
    actual: LoweredType,
}

impl<'a> StreamPlan<'a> {
    pub fn source(&self) -> &StreamSource<'a> {
        &self.source
    }

    pub fn ops(&self) -> &[StreamOp<'a>] {
        &self.ops
    }

    pub fn terminal(&self) -> &StreamTerminal<'a> {
        &self.terminal
    }

    pub(crate) fn collection_empty_add_changes(&self) -> bool {
        self.collection_empty_add_changes
    }

    /// A materialization/consumer may copy these reference values, but a
    /// non-null filter or a value-producing map establishes a new domain.
    pub(crate) fn forwards_source_reference_domain(&self) -> bool {
        fn forwards(expr: &Expr, param: &str) -> bool {
            match expr {
                Expr::Var(name) => name == param,
                Expr::Cast { ty, expr }
                    if !matches!(ty, JavaType::Primitive(_) | JavaType::Void) =>
                {
                    forwards(expr, param)
                }
                _ => false,
            }
        }
        self.ops.iter().all(|op| match op {
            StreamOp::Filter(FilterPlan::Predicate(_)) => true,
            StreamOp::Map { lambda, .. } => lambda
                .body()
                .return_expr()
                .is_some_and(|expr| forwards(expr, lambda.param_name())),
            _ => false,
        })
    }

    pub fn source_element(&self) -> &StreamElement {
        &self.source_element
    }

    pub fn actual(&self) -> &LoweredType {
        &self.actual
    }

    /// Callbacks reached by consuming this pipeline, with their Java input
    /// types. An iterator terminal transfers traversal to its caller and does
    /// not execute the callbacks here.
    pub(crate) fn visit_consumed_callbacks(
        &self,
        visit: &mut impl FnMut(&LambdaPlan<'a>, &RustSlotShape),
    ) {
        if matches!(self.terminal, StreamTerminal::Iterator) {
            return;
        }
        self.visit_callbacks(visit);
    }

    /// Callback input contracts apply even when execution is transferred to
    /// an iterator consumer. Keep type flow separate from invocation effects.
    pub(crate) fn visit_callbacks(&self, visit: &mut impl FnMut(&LambdaPlan<'a>, &RustSlotShape)) {
        let mut input = self.source_element.shape();
        for op in &self.ops {
            match op {
                StreamOp::Filter(FilterPlan::Predicate(lambda)) => visit(lambda, input),
                StreamOp::Filter(FilterPlan::NonNull { output_shape, .. }) => input = output_shape,
                StreamOp::Map {
                    lambda,
                    output_shape,
                }
                | StreamOp::MapToPrimitive {
                    lambda,
                    output_shape,
                } => {
                    visit(lambda, input);
                    input = output_shape;
                }
                // Flat-map callback inputs require the nested pipeline's
                // cardinality/element contract, not the preceding item type.
                StreamOp::FlatMap(_) => return,
            }
        }
        match &self.terminal {
            StreamTerminal::AnyMatch(lambda)
            | StreamTerminal::AllMatch(lambda)
            | StreamTerminal::ForEach(lambda) => visit(lambda, input),
            StreamTerminal::CollectToMap { key, value } => {
                visit(key, input);
                visit(value, input);
            }
            _ => {}
        }
    }
}

pub struct StreamSource<'a> {
    expr: &'a Expr,
    optional_sequence: bool,
    iteration: StreamSourceIteration,
    range: Option<StreamSourceRange<'a>>,
}

impl<'a> StreamSource<'a> {
    pub fn expr(&self) -> &'a Expr {
        self.expr
    }

    pub fn optional_sequence(&self) -> bool {
        self.optional_sequence
    }

    pub fn iteration(&self) -> StreamSourceIteration {
        self.iteration
    }

    pub fn range(&self) -> Option<StreamSourceRange<'a>> {
        self.range
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamSourceRange<'a> {
    JavaArrays { from: &'a Expr, to: &'a Expr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamElement {
    shape: RustSlotShape,
}

impl StreamElement {
    pub fn new(shape: RustSlotShape) -> Self {
        Self { shape }
    }

    pub fn shape(&self) -> &RustSlotShape {
        &self.shape
    }
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
    PrimitiveExtremum {
        maximum: bool,
    },
    Count,
    ForEach(LambdaPlan<'a>),
    Iterator,
    Reduce(ReducePlan<'a>),
    Collect(JavaCollectionKind),
    CollectToMap {
        key: LambdaPlan<'a>,
        value: LambdaPlan<'a>,
    },
    Join {
        delimiter: Option<&'a Expr>,
        affixes: Option<(&'a Expr, &'a Expr)>,
    },
    ToArray,
    FindFirst,
    AnyMatch(LambdaPlan<'a>),
    AllMatch(LambdaPlan<'a>),
}

pub struct ReducePlan<'a> {
    identity: &'a Expr,
    accumulator: ReduceLambdaPlan<'a>,
}

impl<'a> ReducePlan<'a> {
    pub fn identity(&self) -> &'a Expr {
        self.identity
    }

    pub fn accumulator(&self) -> &ReduceLambdaPlan<'a> {
        &self.accumulator
    }
}

#[derive(Clone)]
pub struct LambdaPlan<'a> {
    param_name: String,
    rust_param_name: String,
    body: StreamLambdaBody<'a>,
}

impl<'a> LambdaPlan<'a> {
    pub fn param_name(&self) -> &str {
        &self.param_name
    }

    pub fn rust_param_name(&self) -> &str {
        &self.rust_param_name
    }

    pub fn body(&self) -> StreamLambdaBody<'a> {
        self.body
    }

    pub(crate) fn with_method_reference_target(&self, target: &'a str) -> Option<Self> {
        let StreamLambdaBody::MethodReference { method, .. } = self.body else {
            return None;
        };
        Some(Self {
            param_name: self.param_name.clone(),
            rust_param_name: self.rust_param_name.clone(),
            body: StreamLambdaBody::MethodReference { target, method },
        })
    }
}

pub struct ReduceLambdaPlan<'a> {
    left_param_name: String,
    left_rust_param_name: String,
    right_param_name: String,
    right_rust_param_name: String,
    body: StreamLambdaBody<'a>,
}

impl<'a> ReduceLambdaPlan<'a> {
    pub fn left_param_name(&self) -> &str {
        &self.left_param_name
    }

    pub fn left_rust_param_name(&self) -> &str {
        &self.left_rust_param_name
    }

    pub fn right_param_name(&self) -> &str {
        &self.right_param_name
    }

    pub fn right_rust_param_name(&self) -> &str {
        &self.right_rust_param_name
    }

    pub fn body(&self) -> StreamLambdaBody<'a> {
        self.body
    }
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
    lambda: LambdaPlan<'a>,
    source: &'a Expr,
    optional_sequence: bool,
    iteration: StreamSourceIteration,
}

impl<'a> LambdaCollectionStreamPlan<'a> {
    pub fn lambda(&self) -> &LambdaPlan<'a> {
        &self.lambda
    }

    pub fn source(&self) -> &'a Expr {
        self.source
    }

    pub fn optional_sequence(&self) -> bool {
        self.optional_sequence
    }

    pub fn iteration(&self) -> StreamSourceIteration {
        self.iteration
    }
}

pub struct StreamSubPipeline<'a> {
    source: StreamSource<'a>,
    ops: Vec<StreamOp<'a>>,
    source_element: StreamElement,
    output_shape: RustSlotShape,
}

impl<'a> StreamSubPipeline<'a> {
    pub fn source(&self) -> &StreamSource<'a> {
        &self.source
    }

    pub fn ops(&self) -> &[StreamOp<'a>] {
        &self.ops
    }

    pub fn source_element(&self) -> &StreamElement {
        &self.source_element
    }

    pub fn output_shape(&self) -> &RustSlotShape {
        &self.output_shape
    }
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
    build_stream_plan_core(expr, env, facts, None)
}

pub fn build_stream_plan_for_slot<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<StreamPlan<'a>> {
    // Compatibility entry point: a destination slot must not invent a
    // terminal operation for an intermediate primitive stream.
    build_stream_plan_core(expr, env, facts, None)
}

pub(crate) fn build_stream_plan_with_shape_context<'a>(
    expr: &'a Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
    facts: &FactSet,
) -> Option<StreamPlan<'a>> {
    build_stream_plan_core(expr, env.type_env(), facts, Some((env, ctx)))
}

fn build_stream_plan_core<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
    facts: &FactSet,
    shape_context: Option<(&ShapeEnv, ExprShapeContext<'_>)>,
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
        "min" | "max" if args.is_empty() => StreamTerminal::PrimitiveExtremum {
            maximum: name == "max",
        },
        "count" if args.is_empty() => StreamTerminal::Count,
        "forEach" | "for_each" | "forEachOrdered" | "for_each_ordered" if args.len() == 1 => {
            StreamTerminal::ForEach(unary_lambda_plan(&args[0], true)?)
        }
        "iterator" if args.is_empty() => StreamTerminal::Iterator,
        "reduce" if args.len() == 2 => StreamTerminal::Reduce(ReducePlan {
            identity: &args[0],
            accumulator: reduce_lambda_plan(&args[1])?,
        }),
        "collect" if args.len() == 1 => stream_collect_terminal(&args[0], env, facts)?,
        "toArray" | "to_array" if args.is_empty() => StreamTerminal::ToArray,
        "findFirst" if args.is_empty() => StreamTerminal::FindFirst,
        "anyMatch" | "any_match" if args.len() == 1 => {
            StreamTerminal::AnyMatch(lambda_plan(&args[0])?)
        }
        "allMatch" | "all_match" if args.len() == 1 => {
            StreamTerminal::AllMatch(lambda_plan(&args[0])?)
        }
        _ => return None,
    };
    let chain_root = receiver;
    let source_spec = receiver_source_spec(chain_root)?;
    let source_expr = source_spec.expr;
    let source_shape = match shape_context {
        Some((shape_env, shape_ctx)) => {
            crate::stream_shape::infer_stream_source_element_shape_with_context(
                source_expr,
                shape_env,
                shape_ctx,
                facts,
            )?
        }
        None => infer_stream_source_element_shape_with_facts(source_expr, env, facts)?,
    };
    let (source, ops, current_shape) =
        collect_stream_chain(chain_root, source_shape.clone(), env, facts)?;
    let actual = stream_terminal_actual(&terminal, current_shape.clone(), env, facts);
    let collection_empty_add_changes = if matches!(terminal, StreamTerminal::Collect(_)) {
        match args.first() {
            Some(Expr::Call { args: factory, .. }) => match factory.as_slice() {
                [Expr::MethodReference { target, method }] if method == "new" => {
                    crate::stdlib::native_list_empty_add_changes(
                        env.static_type_alias(target).unwrap_or(target),
                    )
                }
                _ => true,
            },
            _ => true,
        }
    } else {
        true
    };
    Some(StreamPlan {
        source: StreamSource {
            expr: source,
            optional_sequence: stream_source_is_optional_sequence(source, env, facts),
            iteration: if shape_context.is_some_and(|(env, ctx)| {
                let shape = infer_expr_shape_with_context(source, env, ctx);
                let ty = shape
                    .optional_element_type()
                    .unwrap_or_else(|| shape.java_ty.clone());
                // Native storage lends elements; borrowing a protocol/factory
                // object alone does not imply its iterator lends its payload.
                matches!(
                    shape.ownership,
                    crate::slot_shape::OwnershipShape::SharedBorrow
                        | crate::slot_shape::OwnershipShape::MutableBorrow
                ) && !crate::effective_facts::EffectiveFacts::new(facts)
                    .parsed_source_controls_native_carrier(&ty)
                    && ((matches!(ty, JavaType::Array(_)) && !shape.native_allocation.shared())
                        || crate::stdlib::java_collection_kind_for_type(&ty).is_some())
            }) {
                StreamSourceIteration::Borrowed
            } else {
                stream_source_iteration(source)
            },
            range: source_spec.range,
        },
        ops,
        terminal,
        collection_empty_add_changes,
        source_element: StreamElement::new(source_shape),
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
            .capability_element()
            .is_some()
}

fn stream_collect_terminal<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<StreamTerminal<'a>> {
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
    let owner = match receiver.as_ref() {
        Expr::Var(owner) if env.local_type(owner).is_none() && env.field_type(owner).is_none() => {
            env.static_type_alias(owner).unwrap_or(owner).to_string()
        }
        // Qualified Java names are FieldAccess chains in the source IR.
        // Reuse their resolved declaration identity instead of reconstructing
        // a path or guessing from the final spelling.
        Expr::FieldAccess(_, _) => {
            let written_owner = crate::stdlib::qualified_static_owner_java_name(receiver)?;
            let root = written_owner.split('.').next()?;
            if env.local_type(root).is_some() || env.field_type(root).is_some() {
                return None;
            }
            resolved
                .as_ref()
                .filter(|call| call.kind == crate::java_ir::CallKind::Static)
                .map(|call| call.declaring_type.clone())
                .unwrap_or(written_owner)
        }
        _ => return None,
    };
    let root = owner.split('.').next()?;
    if env.local_type(root).is_some()
        || env.field_type(root).is_some()
        || crate::stdlib::canonical_external_receiver_type_key(&owner) != "Collectors"
        || crate::call_plan::java_type_owner_is_parsed_source(&owner, env, facts)
    {
        return None;
    }
    match (name.as_str(), args.as_slice()) {
        ("toList" | "to_list", []) => Some(StreamTerminal::Collect(JavaCollectionKind::List)),
        ("toSet" | "to_set", []) => Some(StreamTerminal::Collect(JavaCollectionKind::Set)),
        ("toCollection" | "to_collection", [Expr::MethodReference { target, method }])
            if method == "new" =>
        {
            let owner = env.static_type_alias(target).unwrap_or(target);
            if crate::call_plan::java_type_owner_is_parsed_source(owner, env, facts) {
                return None;
            }
            let kind =
                crate::stdlib::native_collection_constructor_kind_for_type(&JavaType::Named {
                    name: owner.to_string(),
                    args: Vec::new(),
                })?;
            // Collection-kind classification also includes queues. A List
            // collector must prove List operations/type tests are valid;
            // queue/sorted/custom suppliers need their own construction plan.
            if kind == JavaCollectionKind::List
                && !crate::stdlib::is_known_java_platform_subtype(owner, "java.util.List")
            {
                return None;
            }
            // Reuse the existing native collection terminals and their
            // element-carrier conversion. Ordered/set variants without a
            // modeled collector retain the ordinary unsupported path.
            matches!(
                kind,
                JavaCollectionKind::List
                    | JavaCollectionKind::Set
                    | JavaCollectionKind::InsertionOrderedSet
            )
            .then_some(StreamTerminal::Collect(kind))
        }
        ("joining", []) => Some(StreamTerminal::Join {
            delimiter: None,
            affixes: None,
        }),
        ("joining", [delimiter]) => Some(StreamTerminal::Join {
            delimiter: Some(delimiter),
            affixes: None,
        }),
        ("joining", [delimiter, prefix, suffix]) => Some(StreamTerminal::Join {
            delimiter: Some(delimiter),
            affixes: Some((prefix, suffix)),
        }),
        ("toMap" | "to_map", [key, value]) => Some(StreamTerminal::CollectToMap {
            key: lambda_plan(key)?,
            value: lambda_plan(value)?,
        }),
        _ => None,
    }
}

fn receiver_source(expr: &Expr) -> Option<&Expr> {
    receiver_source_spec(expr).map(|spec| spec.expr)
}

#[derive(Debug, Clone, Copy)]
struct StreamSourceSpec<'a> {
    expr: &'a Expr,
    range: Option<StreamSourceRange<'a>>,
}

fn receiver_source_spec(expr: &Expr) -> Option<StreamSourceSpec<'_>> {
    collect_stream_chain_source(expr)
}

pub(crate) fn stream_chain_source_expression(expr: &Expr) -> Option<&Expr> {
    let Expr::Call {
        receiver: Some(chain),
        ..
    } = expr
    else {
        return None;
    };
    collect_stream_chain_source(chain).map(|source| source.expr)
}

fn collect_stream_chain_source(expr: &Expr) -> Option<StreamSourceSpec<'_>> {
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
            .map(|expr| StreamSourceSpec { expr, range: None }),
        "stream"
            if args.len() == 1
                && matches!(receiver.as_deref(), Some(Expr::Var(owner))
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "Arrays") =>
        {
            args.first()
                .map(|expr| StreamSourceSpec { expr, range: None })
        }
        "stream"
            if args.len() == 3
                && matches!(receiver.as_deref(), Some(Expr::Var(owner))
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "Arrays") =>
        {
            Some(StreamSourceSpec {
                expr: &args[0],
                range: Some(StreamSourceRange::JavaArrays {
                    from: &args[1],
                    to: &args[2],
                }),
            })
        }
        "range" | "range_closed"
            if args.len() == 2
                && matches!(receiver.as_deref(), Some(Expr::Var(owner))
                    if crate::stdlib::canonical_external_receiver_type_key(owner) == "IntStream") =>
        {
            Some(StreamSourceSpec { expr, range: None })
        }
        "filter" | "map" | "mapToInt" | "map_to_int" | "mapToLong" | "map_to_long"
        | "mapToDouble" | "map_to_double" | "mapToObj" | "map_to_obj" | "flatMap" | "flat_map"
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
        "stream"
            if args.len() == 3
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
            let receiver_type = crate::types::infer_expr_type(receiver.as_deref()?, env);
            if let JavaType::Named { name, .. } = &receiver_type
                && let Some(primitive) = crate::stdlib::primitive_stream_payload(name)
            {
                let output_shape = RustSlotShape::for_storage(JavaType::Primitive(primitive));
                ops.push(StreamOp::MapToPrimitive {
                    lambda,
                    output_shape: output_shape.clone(),
                });
                return Some((source, ops, output_shape));
            }
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
        "mapToInt" | "map_to_int" | "mapToLong" | "map_to_long" | "mapToDouble"
        | "map_to_double"
            if args.len() == 1 =>
        {
            let lambda = lambda_plan(&args[0])?;
            let (source, mut ops, _) =
                collect_stream_chain(receiver.as_deref()?, input_shape, env, facts)?;
            let primitive = match name.as_str() {
                "mapToDouble" | "map_to_double" => PrimitiveType::Double,
                "mapToLong" | "map_to_long" => PrimitiveType::Long,
                _ => PrimitiveType::Int,
            };
            let output_shape = RustSlotShape::for_storage(JavaType::Primitive(primitive));
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
    unary_lambda_plan(lambda, false)
}

fn unary_lambda_plan(lambda: &Expr, allows_void_block: bool) -> Option<LambdaPlan<'_>> {
    match lambda {
        Expr::Lambda { params, body } if params.len() == 1 => {
            let param = params.first()?;
            let body = match body {
                LambdaBody::Expr(body) => StreamLambdaBody::Expr(body),
                LambdaBody::Block(block) if allows_void_block || block_returns_value(block) => {
                    StreamLambdaBody::Block(block)
                }
                LambdaBody::Block(_) => return None,
            };
            let access_names = crate::java_naming::source_binding_rust_names(
                stream_lambda_binding_source_names(&[param.source_name.as_str()], body),
            );
            Some(LambdaPlan {
                param_name: param.source_name.clone(),
                rust_param_name: access_names
                    .get(&param.source_name)
                    .cloned()
                    .unwrap_or_else(|| param.name.clone()),
                body,
            })
        }
        Expr::MethodReference { target, method } => {
            let occupied = std::collections::HashSet::from([crate::parser::to_snake_case(target)]);
            let param = crate::java_naming::allocate_rust_ident("value", &occupied);
            Some(LambdaPlan {
                param_name: param.clone(),
                rust_param_name: param,
                body: StreamLambdaBody::MethodReference { target, method },
            })
        }
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
            let param_sources = [
                params[0].source_name.as_str(),
                params[1].source_name.as_str(),
            ];
            let access_names = crate::java_naming::source_binding_rust_names(
                stream_lambda_binding_source_names(&param_sources, body),
            );
            Some(ReduceLambdaPlan {
                left_param_name: params[0].source_name.clone(),
                left_rust_param_name: access_names
                    .get(&params[0].source_name)
                    .cloned()
                    .unwrap_or_else(|| params[0].name.clone()),
                right_param_name: params[1].source_name.clone(),
                right_rust_param_name: access_names
                    .get(&params[1].source_name)
                    .cloned()
                    .unwrap_or_else(|| params[1].name.clone()),
                body,
            })
        }
        _ => None,
    }
}

fn stream_lambda_binding_source_names<'a>(
    params: &[&'a str],
    body: StreamLambdaBody<'a>,
) -> Vec<&'a str> {
    let mut source_names = params.to_vec();
    source_names.extend(
        body.prefix_statements()
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::VarDecl { source_name, .. } => Some(source_name.as_str()),
                _ => None,
            }),
    );
    source_names
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
        return infer_single_input_method_reference_output_shape_with_context(
            target,
            method,
            input_shape,
            env,
            facts,
        )
        .unwrap_or_else(|| input_shape.clone());
    }
    let Some(return_expr) = lambda.body.return_expr() else {
        return input_shape.clone();
    };
    let mut lambda_env = env.clone();
    lambda_env.insert_local(lambda.param_name.clone(), input_shape.java_ty.clone());
    insert_lambda_prefix_locals(lambda, &mut lambda_env, facts);
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
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => block_has_null_return(block),
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
    if matches!(
        expr,
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Ne,
            left,
            right,
        } if is_param_and_null(left, right, param_name) || is_param_and_null(right, left, param_name)
    ) {
        return true;
    }
    matches!(
        expr,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.len() == 1
            && matches!(args.first(), Some(Expr::Var(name)) if name == param_name)
            && matches!(name.as_str(), "nonNull" | "non_null")
            && matches!(receiver.as_ref(), Expr::Var(owner)
                if crate::stdlib::canonical_external_receiver_type_key(owner) == "Objects")
    )
}

fn is_param_and_null(left: &Expr, right: &Expr, param_name: &str) -> bool {
    matches!(left, Expr::Var(name) if name == param_name)
        && matches!(right, Expr::Literal(crate::java_ir::Literal::Null))
}

pub(crate) fn non_null_filter_output_shape(input_shape: &RustSlotShape) -> RustSlotShape {
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
    if let Some(plan) = lambda_collection_stream_plan(lambda.clone(), input_shape, env, facts) {
        return Some(FlatMapPlan::LambdaCollectionStream(plan));
    }
    lambda_stream_chain_plan(lambda, input_shape, env, facts)
}

fn lambda_collection_stream_plan<'a>(
    lambda: LambdaPlan<'a>,
    input_shape: &RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
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
    let mut lambda_env = env.clone();
    lambda_env.insert_local(lambda.param_name.clone(), input_shape.java_ty.clone());
    Some(LambdaCollectionStreamPlan {
        lambda,
        source,
        optional_sequence: stream_source_is_optional_sequence(source, &lambda_env, facts),
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
    insert_lambda_prefix_locals(&lambda, &mut lambda_env, facts);
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
                range: receiver_source_spec(return_expr)?.range,
            },
            ops,
            source_element: StreamElement::new(infer_stream_source_element_shape_with_facts(
                source,
                &lambda_env,
                facts,
            )?),
            output_shape,
        }),
    })
}

fn insert_lambda_prefix_locals(lambda: &LambdaPlan<'_>, env: &mut TypeEnv, facts: &FactSet) {
    let StreamLambdaBody::Block(block) = lambda.body else {
        return;
    };
    for stmt in block
        .statements
        .iter()
        .take(block.statements.len().saturating_sub(1))
    {
        if let Stmt::VarDecl {
            ty,
            name,
            initializer,
            ..
        } = stmt
        {
            env.insert_local(
                name.clone(),
                stream_prefix_local_planning_type(ty, initializer.as_ref(), facts),
            );
        }
    }
}

fn stream_prefix_local_planning_type(
    declared_ty: &JavaType,
    initializer: Option<&Expr>,
    facts: &FactSet,
) -> JavaType {
    if crate::types::optional_element_type(declared_ty).is_some()
        || !java_type_is_stream_capable(declared_ty, facts)
    {
        return declared_ty.clone();
    }
    match initializer {
        Some(Expr::Literal(crate::java_ir::Literal::Null)) => {
            optional_java_type(declared_ty.clone())
        }
        Some(Expr::Call { .. }) => optional_java_type(declared_ty.clone()),
        _ => declared_ty.clone(),
    }
}

fn java_type_is_stream_capable(java_ty: &JavaType, facts: &FactSet) -> bool {
    crate::types::stream_element_type(java_ty).is_some()
        || crate::stream_shape::stream_source_capability_policy(java_ty, facts)
            .capability_element()
            .is_some()
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
                .map(|shape| stream_iteration_effective_element_shape(shape, plan.iteration))
        }
        FlatMapPlan::LambdaStreamChain { pipeline, .. } => Some(pipeline.output_shape.clone()),
    }
}

fn stream_iteration_effective_element_shape(
    shape: RustSlotShape,
    iteration: StreamSourceIteration,
) -> RustSlotShape {
    effective_stream_item_shape(&shape, matches!(iteration, StreamSourceIteration::Borrowed))
}

pub fn stream_terminal_actual(
    terminal: &StreamTerminal<'_>,
    element_shape: RustSlotShape,
    env: &TypeEnv,
    facts: &FactSet,
) -> LoweredType {
    match terminal {
        StreamTerminal::Sum => LoweredType::from_java(element_shape.java_ty.clone()),
        StreamTerminal::PrimitiveExtremum { .. } => {
            let primitive = match element_shape.java_ty {
                JavaType::Primitive(primitive) => primitive,
                _ => {
                    return LoweredType::from_java(JavaType::Unknown(
                        "non-primitive stream extremum".to_string(),
                    ));
                }
            };
            LoweredType::from_java(JavaType::Named {
                name: crate::stdlib::primitive_optional_name(primitive)
                    .unwrap_or("Optional")
                    .to_string(),
                args: Vec::new(),
            })
        }
        StreamTerminal::Count => LoweredType::from_java(JavaType::Primitive(PrimitiveType::Long)),
        StreamTerminal::ForEach(_) => LoweredType::from_java(JavaType::Void),
        StreamTerminal::Iterator => LoweredType::from_java(JavaType::Named {
            name: "Iterator".to_string(),
            args: vec![element_shape.java_ty.clone()],
        }),
        StreamTerminal::Reduce(reduce) => {
            LoweredType::from_java(crate::types::infer_expr_type(reduce.identity, env))
        }
        StreamTerminal::Collect(JavaCollectionKind::List) => stream_collect_actual(element_shape),
        StreamTerminal::Collect(
            kind @ (JavaCollectionKind::Set
            | JavaCollectionKind::SortedSet
            | JavaCollectionKind::InsertionOrderedSet),
        ) => LoweredType::collected_set_kind_from_element_shape(element_shape, *kind),
        StreamTerminal::Collect(JavaCollectionKind::Map)
        | StreamTerminal::Collect(JavaCollectionKind::SortedMap)
        | StreamTerminal::Collect(JavaCollectionKind::InsertionOrderedMap) => {
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
        return infer_single_input_method_reference_output_shape_with_context(
            target,
            method,
            element_shape,
            _env,
            _facts,
        )
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

#[cfg(test)]
mod collector_tests {
    use super::*;

    #[test]
    fn stream_consumers_accept_void_blocks_without_weakening_value_callbacks() {
        let mut env = TypeEnv::new();
        env.insert_local(
            "values",
            JavaType::Array(Box::new(JavaType::Primitive(PrimitiveType::Int))),
        );
        let facts = FactSet::default();
        let consumer = crate::parser::parse_expr(
            "Arrays.stream(values).forEach(value -> { consume(value); })",
        );
        let plan = build_stream_plan(&consumer, &env, &facts).expect("void consumer");
        assert!(matches!(plan.terminal(), StreamTerminal::ForEach(_)));
        let mut callbacks = 0;
        plan.visit_consumed_callbacks(&mut |_, _| callbacks += 1);
        assert_eq!(callbacks, 1);
        let invalid_map = crate::parser::parse_expr(
            "Arrays.stream(values).map(value -> { consume(value); }).count()",
        );
        assert!(build_stream_plan(&invalid_map, &env, &facts).is_none());
    }

    #[test]
    fn reference_array_callback_contract_is_nullable_even_when_execution_is_deferred() {
        let call = |receiver: Expr, name: &str, args: Vec<Expr>| Expr::Call {
            type_args: Vec::new(),
            receiver: Some(Box::new(receiver)),
            name: name.to_string(),
            args,
            resolved: None,
        };
        let source = call(
            Expr::Var("Arrays".to_string()),
            "stream",
            vec![Expr::Var("values".to_string())],
        );
        let mapped = call(
            source,
            "map",
            vec![Expr::MethodReference {
                target: "String".to_string(),
                method: "trim".to_string(),
            }],
        );
        let expr = call(mapped, "iterator", Vec::new());
        let mut env = TypeEnv::new();
        env.insert_local(
            "values".to_string(),
            JavaType::Array(Box::new(JavaType::String)),
        );
        let facts = FactSet::default();
        let plan = build_stream_plan(&expr, &env, &facts).expect("deferred stream");
        assert_eq!(
            plan.source_element().shape().optional_element_type(),
            Some(JavaType::String)
        );
        let mut contracts = 0;
        plan.visit_callbacks(&mut |_, input| {
            assert_eq!(input.optional_element_type(), Some(JavaType::String));
            contracts += 1;
        });
        let mut invoked = 0;
        plan.visit_consumed_callbacks(&mut |_, _| invoked += 1);
        assert_eq!((contracts, invoked), (1, 0));
    }

    #[test]
    fn queue_suppliers_are_not_reclassified_as_list_collectors() {
        let (mut collector, facts) = collector_call(
            r#"
import java.util.*;
import java.util.stream.Collectors;
class Use {
    Collection<String> run(List<String> values) {
        return values.stream().collect(Collectors.toCollection(ArrayDeque::new));
    }
}
"#,
        );
        let env = TypeEnv::new();
        assert!(stream_collect_terminal(&collector, &env, &facts).is_none());
        let Expr::Call { args, .. } = &mut collector else {
            unreachable!()
        };
        let Expr::MethodReference { target, .. } = &mut args[0] else {
            unreachable!()
        };
        *target = "java.util.LinkedList".into();
        assert!(matches!(
            stream_collect_terminal(&collector, &env, &facts),
            Some(StreamTerminal::Collect(JavaCollectionKind::List))
        ));
    }

    fn collector_call(source: &str) -> (Expr, FactSet) {
        let parsed = crate::parser::parse_java_source(source);
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Use")
            .unwrap();
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                crate::java_ir::Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        let Stmt::Return(Some(Expr::Call { args, .. })) =
            &method.body.as_ref().unwrap().statements[0]
        else {
            panic!("expected a stream collect return");
        };
        (args[0].clone(), resolved.facts)
    }

    #[test]
    fn source_collectors_owner_is_not_a_builtin_terminal() {
        let (mut collector, facts) = collector_call(
            r#"
import java.util.List;
import java.util.stream.Collector;
class Collectors {
    static <T> Collector<T, ?, List<T>> toList() {
        return java.util.stream.Collectors.toList();
    }
}
class Use {
    List<String> run(List<String> values) {
        return values.stream().collect(Collectors.toList());
    }
}
"#,
        );
        let env = TypeEnv::new();
        assert!(stream_collect_terminal(&collector, &env, &facts).is_none());
        let Expr::Call { receiver, .. } = &mut collector else {
            unreachable!()
        };
        *receiver = Some(Box::new(Expr::Var(
            "java.util.stream.Collectors".to_string(),
        )));
        assert!(matches!(
            stream_collect_terminal(&collector, &env, &facts),
            Some(StreamTerminal::Collect(JavaCollectionKind::List))
        ));
    }

    #[test]
    fn source_collection_constructor_is_not_replaced_by_a_native_collector() {
        let (mut collector, facts) = collector_call(
            r#"
import java.util.List;
import java.util.stream.Collectors;
class ArrayList<T> extends java.util.ArrayList<T> {}
class Use {
    List<String> run(List<String> values) {
        return values.stream().collect(Collectors.toCollection(ArrayList::new));
    }
}
"#,
        );
        let env = TypeEnv::new();
        assert!(stream_collect_terminal(&collector, &env, &facts).is_none());
        let Expr::Call { args, .. } = &mut collector else {
            unreachable!()
        };
        let Expr::MethodReference { target, .. } = &mut args[0] else {
            unreachable!()
        };
        *target = "java.util.ArrayList".to_string();
        assert!(matches!(
            stream_collect_terminal(&collector, &env, &facts),
            Some(StreamTerminal::Collect(JavaCollectionKind::List))
        ));
    }
}
