use crate::conversion_plan::LoweredType;
use crate::facts::{CaptureOwnerKind, FactSet};
use crate::java_ir::{Expr, JavaType, LambdaBody, PrimitiveType};
use crate::slot_shape::RustSlotShape;
use crate::stream_shape::{
    infer_stream_lambda_body_shape, infer_stream_map_to_int_shape,
    infer_stream_source_element_shape_with_facts,
};
use crate::types::TypeEnv;

pub struct StreamPlan<'a> {
    pub source: StreamSource<'a>,
    pub ops: Vec<StreamOp<'a>>,
    pub terminal: StreamTerminal<'a>,
    pub source_element: StreamElement,
    pub actual: LoweredType,
}

pub struct StreamSource<'a> {
    pub expr: &'a Expr,
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
    Filter(LambdaPlan<'a>),
    Map {
        lambda: LambdaPlan<'a>,
        output_shape: RustSlotShape,
    },
    MapToPrimitive {
        lambda: LambdaPlan<'a>,
        output_shape: RustSlotShape,
    },
}

pub enum StreamTerminal<'a> {
    Sum,
    CollectToList,
    FindFirst,
    AnyMatch(LambdaPlan<'a>),
}

pub struct LambdaPlan<'a> {
    pub param_name: String,
    pub body: &'a Expr,
}

pub fn build_stream_plan<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
    facts: &FactSet,
) -> Option<StreamPlan<'a>> {
    if !stream_lambdas_are_capture_free(facts) {
        return None;
    }
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
        "collect" if args.len() == 1 && is_collectors_to_list(&args[0]) => {
            StreamTerminal::CollectToList
        }
        "findFirst" if args.is_empty() => StreamTerminal::FindFirst,
        "anyMatch" if args.len() == 1 => StreamTerminal::AnyMatch(lambda_plan(&args[0])?),
        _ => return None,
    };
    let source_shape =
        infer_stream_source_element_shape_with_facts(receiver_source(receiver)?, env, facts)?;
    let (source, ops, current_shape) = collect_stream_chain(receiver, source_shape)?;
    let actual = stream_terminal_actual(&terminal, current_shape.clone());
    Some(StreamPlan {
        source: StreamSource { expr: source },
        ops,
        terminal,
        source_element: StreamElement {
            shape: infer_stream_source_element_shape_with_facts(source, env, facts)?,
            value_category: StreamValueCategory::Borrowed,
        },
        actual,
    })
}

fn stream_lambdas_are_capture_free(facts: &FactSet) -> bool {
    facts
        .captures
        .owners
        .iter()
        .filter(|owner| owner.kind == CaptureOwnerKind::Lambda)
        .all(|owner| owner.captures.is_empty())
}

fn is_collectors_to_list(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == "toList"
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
        "stream" if args.is_empty() => receiver.as_deref(),
        "filter" | "map" | "mapToInt" if args.len() == 1 => {
            collect_stream_chain_source(receiver.as_deref()?)
        }
        _ => None,
    }
}

fn collect_stream_chain<'a>(
    expr: &'a Expr,
    input_shape: RustSlotShape,
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
        "stream" if args.is_empty() => receiver
            .as_deref()
            .map(|source| (source, Vec::new(), input_shape)),
        "filter" if args.len() == 1 => {
            let (source, mut ops, current_shape) =
                collect_stream_chain(receiver.as_deref()?, input_shape)?;
            ops.push(StreamOp::Filter(lambda_plan(&args[0])?));
            Some((source, ops, current_shape))
        }
        "map" if args.len() == 1 => {
            let lambda = lambda_plan(&args[0])?;
            let (source, mut ops, current_shape) =
                collect_stream_chain(receiver.as_deref()?, input_shape)?;
            let output_shape = infer_stream_lambda_body_shape(lambda.body, &current_shape);
            ops.push(StreamOp::Map {
                lambda,
                output_shape: output_shape.clone(),
            });
            Some((source, ops, output_shape))
        }
        "mapToInt" if args.len() == 1 => {
            let lambda = lambda_plan(&args[0])?;
            let (source, mut ops, _) = collect_stream_chain(receiver.as_deref()?, input_shape)?;
            let output_shape = infer_stream_map_to_int_shape(lambda.body);
            ops.push(StreamOp::MapToPrimitive {
                lambda,
                output_shape: output_shape.clone(),
            });
            Some((source, ops, output_shape))
        }
        _ => None,
    }
}

fn lambda_plan(lambda: &Expr) -> Option<LambdaPlan<'_>> {
    let Expr::Lambda { params, body } = lambda else {
        return None;
    };
    let param = params.first()?.name.clone();
    let LambdaBody::Expr(body) = body else {
        return None;
    };
    Some(LambdaPlan {
        param_name: param,
        body,
    })
}

pub fn stream_terminal_actual(
    terminal: &StreamTerminal<'_>,
    element_shape: RustSlotShape,
) -> LoweredType {
    match terminal {
        StreamTerminal::Sum => LoweredType::from_java(JavaType::Primitive(PrimitiveType::Int)),
        StreamTerminal::CollectToList => stream_collect_actual(element_shape),
        StreamTerminal::FindFirst => LoweredType::optional_element_borrow_shape(element_shape),
        StreamTerminal::AnyMatch(_) => {
            LoweredType::from_java(JavaType::Primitive(PrimitiveType::Boolean))
        }
    }
}

pub fn stream_collect_actual(element_shape: RustSlotShape) -> LoweredType {
    LoweredType::collected_sequence_from_element_shape(element_shape)
}
