use crate::java_ir::{Expr, Literal};
use crate::string_concat::analyze_string_concat;
use crate::types::TypeEnv;

#[derive(Debug, Clone)]
pub(crate) struct StringConcatPlan<'a> {
    format_string: String,
    args: Vec<&'a Expr>,
}

impl<'a> StringConcatPlan<'a> {
    pub(crate) fn new(format_string: String, args: Vec<&'a Expr>) -> Self {
        Self {
            format_string,
            args,
        }
    }

    pub(crate) fn format_string(&self) -> &str {
        &self.format_string
    }

    pub(crate) fn args(&self) -> &[&'a Expr] {
        &self.args
    }
}

pub(crate) fn build_string_concat_plan<'a>(
    expr: &'a Expr,
    env: &TypeEnv,
) -> Option<StringConcatPlan<'a>> {
    let parts = analyze_string_concat(expr, env)?;
    let format_string = parts
        .parts()
        .iter()
        .map(|part| match part {
            Expr::Literal(Literal::String(s)) => s.replace('{', "{{").replace('}', "}}"),
            _ => "{}".to_string(),
        })
        .collect::<String>();
    let args = parts
        .into_parts()
        .into_iter()
        .filter(|part| !matches!(part, Expr::Literal(Literal::String(_))))
        .collect();
    Some(StringConcatPlan::new(format_string, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::{JavaType, PrimitiveType};
    use crate::parser::parse_expr;

    #[test]
    fn string_concat_plan_extracts_format_string_and_dynamic_args() {
        let mut env = TypeEnv::new();
        env.insert_local("name", JavaType::String);
        env.insert_local("count", JavaType::Primitive(PrimitiveType::Int));

        let expr = parse_expr("\"name=\" + name + count");
        let plan = build_string_concat_plan(&expr, &env).unwrap();

        assert_eq!(plan.format_string(), "name={}{}");
        assert_eq!(plan.args().len(), 2);
    }
}
