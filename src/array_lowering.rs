use crate::array_plan::ArrayExprPlan;
use crate::emit::emit_expr;
use crate::rust_ir::RustExpr;

pub fn render_array_expr(plan: &ArrayExprPlan<'_>, children: Vec<RustExpr>) -> RustExpr {
    match plan {
        ArrayExprPlan::Creation { element_type, .. } => {
            let dims = children
                .iter()
                .map(emit_expr)
                .collect::<Vec<_>>()
                .join("][");
            RustExpr::Raw(format!(
                "/* Java array creation: {:?}[{dims}] */ Vec::new()",
                element_type
            ))
        }
        ArrayExprPlan::Initializer { .. } => RustExpr::Raw(format!(
            "vec![{}]",
            children
                .iter()
                .map(emit_expr)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}
