use crate::rust_ir::RustExpr;
use crate::string_concat_plan::StringConcatPlan;

pub fn string_concat_expr(plan: &StringConcatPlan<'_>, args: Vec<RustExpr>) -> RustExpr {
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
