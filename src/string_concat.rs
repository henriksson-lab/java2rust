use crate::java_ir::{BinaryOp, Expr, JavaType};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub struct StringConcatParts<'a> {
    pub parts: Vec<&'a Expr>,
}

pub fn analyze_string_concat<'a>(expr: &'a Expr, env: &TypeEnv) -> Option<StringConcatParts<'a>> {
    let mut parts = Vec::new();
    flatten_add(expr, &mut parts);
    if parts.len() > 1
        && parts
            .iter()
            .any(|part| matches!(infer_expr_type(part, env), JavaType::String))
    {
        Some(StringConcatParts { parts })
    } else {
        None
    }
}

pub fn is_string_concat(expr: &Expr, env: &TypeEnv) -> bool {
    analyze_string_concat(expr, env).is_some()
}

fn flatten_add<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = expr
    {
        flatten_add(left, out);
        flatten_add(right, out);
    } else {
        out.push(expr);
    }
}
