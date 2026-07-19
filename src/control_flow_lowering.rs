use crate::rust_ir::{RustExpr, RustStmt};
use crate::statement_render::raw_stmt;

#[derive(Debug, Clone, PartialEq)]
pub struct TailRecursiveLoopRender {
    pub params: Vec<String>,
    pub condition: RustExpr,
    pub base_return: RustExpr,
    pub next_args: Vec<RustExpr>,
}

pub fn render_tail_recursive_loop_body(plan: TailRecursiveLoopRender) -> RustStmt {
    let mut out = String::new();
    for param in &plan.params {
        out.push_str(&format!("let mut {param} = {param};\n"));
    }
    out.push_str("loop {\n");
    out.push_str(&format!(
        "    if {} {{\n        return {};\n    }}\n",
        crate::emit::emit_expr(&plan.condition),
        crate::emit::emit_expr(&plan.base_return)
    ));
    for (index, arg) in plan.next_args.iter().enumerate() {
        out.push_str(&format!(
            "    let __java_tail_arg_{index} = {};\n",
            crate::emit::emit_expr(arg)
        ));
    }
    for (index, param) in plan.params.iter().enumerate() {
        out.push_str(&format!("    {param} = __java_tail_arg_{index};\n"));
    }
    out.push_str("    continue;\n");
    out.push('}');
    raw_stmt(out)
}
