use crate::rust_ir::RustExpr;
use crate::string_concat_plan::StringConcatPlan;

pub fn render_string_concat(plan: &StringConcatPlan<'_>, args: Vec<RustExpr>) -> RustExpr {
    let mut raw = format!("format!({:?}", plan.format_string);
    for arg in args {
        raw.push_str(", ");
        let rendered = crate::emit::emit_expr(&arg);
        raw.push_str(&strip_arithmetic_display_conversion(&rendered));
    }
    raw.push(')');
    RustExpr::Raw(raw)
}

fn strip_arithmetic_display_conversion(rendered: &str) -> String {
    if !(rendered.contains(" * ")
        || rendered.contains(" / ")
        || rendered.contains(" - ")
        || rendered.contains(" + "))
    {
        return rendered.to_string();
    }
    rendered.replace(".to_string()", "")
}
