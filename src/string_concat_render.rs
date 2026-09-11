use crate::rust_ir::RustExpr;
use crate::string_concat_plan::StringConcatPlan;

pub(crate) fn string_concat_expr(plan: &StringConcatPlan<'_>, args: Vec<RustExpr>) -> RustExpr {
    let mut format_args = Vec::with_capacity(args.len() + 1);
    format_args.push(crate::coercion::rust_string_literal_projection(
        &plan.format_string,
    ));
    format_args.extend(
        args.into_iter()
            .map(crate::coercion::java_display_operand_projection),
    );
    crate::coercion::macro_call_projection("format", format_args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_concat_render_builds_format_macro_with_display_args() {
        let plan = StringConcatPlan {
            format_string: "name={}{}".to_string(),
            args: Vec::new(),
        };

        assert_eq!(
            string_concat_expr(
                &plan,
                vec![
                    RustExpr::Var("name".to_string()),
                    RustExpr::Var("count".to_string()),
                ],
            ),
            RustExpr::MacroCall {
                macro_name: "format".to_string(),
                args: vec![
                    RustExpr::Literal("\"name={}{}\"".to_string()),
                    RustExpr::Var("name".to_string()),
                    RustExpr::Var("count".to_string()),
                ],
            }
        );
    }
}
