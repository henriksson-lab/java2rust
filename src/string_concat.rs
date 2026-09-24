use crate::java_ir::{BinaryOp, Expr, JavaType};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub(crate) struct StringConcatParts<'a> {
    parts: Vec<&'a Expr>,
}

impl<'a> StringConcatParts<'a> {
    pub(crate) fn new(parts: Vec<&'a Expr>) -> Self {
        Self { parts }
    }

    pub(crate) fn parts(&self) -> &[&'a Expr] {
        &self.parts
    }

    pub(crate) fn into_parts(self) -> Vec<&'a Expr> {
        self.parts
    }
}

pub(crate) fn analyze_string_concat<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
) -> Option<StringConcatParts<'a>> {
    let mut parts = Vec::new();
    flatten_string_concat(expr, env, &mut parts);
    if parts.len() > 1 && binary_add_is_string_concat(expr, env) {
        Some(StringConcatParts::new(parts))
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
    operand_has_java_string_type(left, env) || operand_has_java_string_type(right, env)
}

fn operand_has_java_string_type(expr: &Expr, env: &TypeEnv) -> bool {
    let ty = infer_expr_type(expr, env);
    matches!(ty, JavaType::String)
        || crate::types::optional_element_type(&ty)
            .is_some_and(|element| matches!(element, JavaType::String))
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
        assert_eq!(parts.parts().len(), 3);
        assert!(!is_string_concat(&parse_expr("count + 1"), &env));
    }

    #[test]
    fn nullable_string_storage_still_selects_java_string_concat() {
        let mut env = TypeEnv::new();
        env.insert_local("value", crate::types::optional_java_type(JavaType::String));

        assert!(is_string_concat(&parse_expr("value + value"), &env));
    }
}
