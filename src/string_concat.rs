use crate::java_ir::{BinaryOp, Expr, JavaType};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub(crate) struct StringConcatParts<'a> {
    pub(crate) parts: Vec<&'a Expr>,
}

pub(crate) fn analyze_string_concat<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
) -> Option<StringConcatParts<'a>> {
    let mut parts = Vec::new();
    flatten_string_concat(expr, env, &mut parts);
    if parts.len() > 1 && binary_add_is_string_concat(expr, env) {
        Some(StringConcatParts { parts })
    } else {
        None
    }
}

pub(crate) fn is_string_concat(expr: &Expr, env: &TypeEnv) -> bool {
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

/// Whether a `+` whose operands are `left` and `right` is a string concatenation.
///
/// Callers that already hold the operands can ask directly instead of rebuilding
/// the `Expr::Binary` node to pass to [`is_string_concat`].
/// `infer_binary_type` used to do exactly that — deep-cloning both subtrees to
/// reconstruct a node it was already looking at — which made type inference
/// **exponential** in the length of a `+` chain, because the reconstructed node
/// was then re-analysed down the same path. An eleven-segment concatenation
/// took over two minutes to translate.
///
/// For a `Binary { Add, left, right }` node this is equivalent to
/// `is_string_concat` on that node: when the operand check passes, flattening
/// descends into both sides and so always yields more than one part.
pub(crate) fn binary_add_operands_make_string_concat(
    left: &Expr,
    right: &Expr,
    env: &TypeEnv,
) -> bool {
    matches!(infer_expr_type(left, env), JavaType::String)
        || matches!(infer_expr_type(right, env), JavaType::String)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::PrimitiveType;
    use crate::parser::parse_expr;

    #[test]
    fn string_concat_detection_flattens_string_add_chains() {
        let mut env = TypeEnv::new();
        env.insert_local("name", JavaType::String);
        env.insert_local("count", JavaType::Primitive(PrimitiveType::Int));

        let expr = parse_expr("\"name=\" + name + count");
        let parts = analyze_string_concat(&expr, &env).unwrap();

        assert!(is_string_concat(&expr, &env));
        assert_eq!(parts.parts.len(), 3);
        assert!(!is_string_concat(&parse_expr("count + 1"), &env));
    }
}
