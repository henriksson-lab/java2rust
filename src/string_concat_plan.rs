use crate::java_ir::{Expr, Literal};
use crate::string_concat::analyze_string_concat;
use crate::types::TypeEnv;

#[derive(Debug, Clone)]
pub struct StringConcatPlan<'a> {
    pub format_string: String,
    pub args: Vec<&'a Expr>,
}

pub fn build_string_concat_plan<'a>(expr: &'a Expr, env: &TypeEnv) -> Option<StringConcatPlan<'a>> {
    let parts = analyze_string_concat(expr, env)?;
    let format_string = parts
        .parts
        .iter()
        .map(|part| match part {
            Expr::Literal(Literal::String(s)) => s.replace('{', "{{").replace('}', "}}"),
            _ => "{}".to_string(),
        })
        .collect::<String>();
    let args = parts
        .parts
        .into_iter()
        .filter(|part| !matches!(part, Expr::Literal(Literal::String(_))))
        .collect();
    Some(StringConcatPlan {
        format_string,
        args,
    })
}
