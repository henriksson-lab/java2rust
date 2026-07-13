use crate::rust_ir::{RustExpr, RustStmt};

pub fn assemble_for_loop(
    condition: Option<RustExpr>,
    body: Vec<RustStmt>,
    label: Option<String>,
) -> RustStmt {
    RustStmt::While {
        condition: condition.unwrap_or_else(default_for_condition),
        body: wrap_labeled_for_body(body, label),
    }
}

pub fn continue_with_updates(update_exprs: &[RustExpr]) -> Vec<RustStmt> {
    update_exprs
        .iter()
        .cloned()
        .map(RustStmt::Expr)
        .chain(std::iter::once(RustStmt::Continue(None)))
        .collect()
}

fn default_for_condition() -> RustExpr {
    RustExpr::Literal("true".to_string())
}

fn wrap_labeled_for_body(body: Vec<RustStmt>, label: Option<String>) -> Vec<RustStmt> {
    if label.is_some() {
        vec![RustStmt::Loop { label, body }]
    } else {
        body
    }
}
