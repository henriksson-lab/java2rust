use crate::rust_ir::RustExpr;
use crate::string_concat_plan::StringConcatPlan;

pub fn render_string_concat(plan: &StringConcatPlan<'_>, args: Vec<RustExpr>) -> RustExpr {
    let mut format_args = Vec::with_capacity(args.len() + 1);
    format_args.push(RustExpr::Literal(format!("{:?}", plan.format_string)));
    format_args.extend(
        args.into_iter()
            .map(crate::coercion::java_display_operand_projection),
    );
    crate::coercion::macro_call_projection("format", format_args)
}
