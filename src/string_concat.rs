use crate::java_ir::{BinaryOp, Expr, JavaType};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub struct StringConcatParts<'a> {
    pub parts: Vec<&'a Expr>,
}

pub fn analyze_string_concat<'a>(expr: &'a Expr, env: &TypeEnv) -> Option<StringConcatParts<'a>> {
    let mut parts = Vec::new();
    flatten_string_concat(expr, env, &mut parts);
    if parts.len() > 1 && binary_add_is_string_concat(expr, env) {
        Some(StringConcatParts { parts })
    } else {
        None
    }
}

pub fn is_string_concat(expr: &Expr, env: &TypeEnv) -> bool {
    analyze_string_concat(expr, env).is_some()
}

fn flatten_string_concat<'a>(expr: &'a Expr, env: &TypeEnv, out: &mut Vec<&'a Expr>) {
    if let Expr::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = expr
        && binary_add_operands_make_string_concat(left, right, env)
    {
        flatten_string_concat(left, env, out);
        flatten_string_concat(right, env, out);
    } else {
        out.push(expr);
    }
}

fn binary_add_is_string_concat(expr: &Expr, env: &TypeEnv) -> bool {
    if let Expr::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = expr
    {
        binary_add_operands_make_string_concat(left, right, env)
    } else {
        false
    }
}

fn binary_add_operands_make_string_concat(left: &Expr, right: &Expr, env: &TypeEnv) -> bool {
    matches!(infer_expr_type(left, env), JavaType::String)
        || matches!(infer_expr_type(right, env), JavaType::String)
}
