use crate::rust_ir::RustExpr;
use crate::string_concat_plan::StringConcatPlan;

pub fn render_string_concat(plan: &StringConcatPlan<'_>, args: Vec<RustExpr>) -> RustExpr {
    let mut raw = format!("format!({:?}", plan.format_string);
    for arg in args {
        raw.push_str(", ");
        raw.push_str(&crate::emit::emit_expr(&arg));
    }
    raw.push(')');
    RustExpr::Raw(raw)
}
