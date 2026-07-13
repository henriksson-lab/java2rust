use crate::coercion::LoweredExpr;
use crate::java_ir::Expr;
use crate::literal_lowering::literal_to_rust;
use crate::operator_lowering::{binary_op_to_rust, stream_unary_prefix};
use crate::rust_ir::RustExpr;
use crate::stream::{StreamOp, StreamPlan, StreamTerminal};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamLambdaValue {
    FilterBorrowed,
    BorrowedItem,
}

pub fn emit_stream_lambda_body(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
) -> Option<String> {
    match expr {
        Expr::Literal(lit) => Some(literal_to_rust(lit)),
        Expr::Var(name) if name == param => Some(match value {
            StreamLambdaValue::FilterBorrowed => format!("**{param}"),
            StreamLambdaValue::BorrowedItem => format!("*{param}"),
        }),
        Expr::Var(name) => Some(name.clone()),
        Expr::Binary { op, left, right } => Some(format!(
            "({} {} {})",
            emit_stream_lambda_body(left, param, value)?,
            binary_op_to_rust(*op),
            emit_stream_lambda_body(right, param, value)?
        )),
        Expr::Unary { op, expr } => {
            let op = stream_unary_prefix(*op)?;
            Some(format!(
                "{op}{}",
                emit_stream_lambda_body(expr, param, value)?
            ))
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if args.is_empty() => {
            let receiver = emit_stream_lambda_receiver(receiver, param, value)?;
            match name.as_str() {
                "isEmpty" => Some(format!("{receiver}.is_empty()")),
                "length" => Some(format!("{receiver}.len() as i32")),
                "trim" => Some(format!("{receiver}.trim()")),
                _ => Some(format!(
                    "{}.{}()",
                    receiver,
                    crate::parser::to_snake_case(name)
                )),
            }
        }
        _ => None,
    }
}

pub fn render_stream_plan(plan: &StreamPlan<'_>, lowered_source: RustExpr) -> Option<LoweredExpr> {
    let mut out = format!("{}.iter()", crate::emit::emit_expr(&lowered_source));
    for op in &plan.ops {
        match op {
            StreamOp::Filter(lambda) => {
                out.push_str(&format!(
                    ".filter(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_body(
                        lambda.body,
                        &lambda.param_name,
                        StreamLambdaValue::FilterBorrowed
                    )?
                ));
            }
            StreamOp::Map { lambda, .. } | StreamOp::MapToPrimitive { lambda, .. } => {
                out.push_str(&format!(
                    ".map(|{}| {})",
                    lambda.param_name,
                    emit_stream_lambda_body(
                        lambda.body,
                        &lambda.param_name,
                        StreamLambdaValue::BorrowedItem
                    )?
                ));
            }
        }
    }

    match &plan.terminal {
        StreamTerminal::Sum => out.push_str(".sum::<i32>()"),
        StreamTerminal::CollectToList => out.push_str(".collect::<Vec<_>>()"),
        StreamTerminal::FindFirst => out.push_str(".next()"),
        StreamTerminal::AnyMatch(lambda) => {
            out.push_str(&format!(
                ".any(|{}| {})",
                lambda.param_name,
                emit_stream_lambda_body(
                    lambda.body,
                    &lambda.param_name,
                    StreamLambdaValue::BorrowedItem
                )?
            ));
        }
    };
    Some(LoweredExpr {
        expr: RustExpr::Raw(out),
        actual: plan.actual.clone(),
    })
}

fn emit_stream_lambda_receiver(
    expr: &Expr,
    param: &str,
    value: StreamLambdaValue,
) -> Option<String> {
    match expr {
        Expr::Var(name) if name == param => Some(match value {
            StreamLambdaValue::FilterBorrowed | StreamLambdaValue::BorrowedItem => {
                param.to_string()
            }
        }),
        _ => emit_stream_lambda_body(expr, param, value),
    }
}
