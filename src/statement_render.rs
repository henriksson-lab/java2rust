use crate::facts::FactSet;
use crate::rust_ir::{RustExpr, RustStmt};
use crate::signature::LocalPlan;
use crate::slot_render::rust_type_for_slot_with_facts;

pub fn local_let_stmt(local: &LocalPlan, value: Option<RustExpr>) -> RustStmt {
    local_let_stmt_with_facts(local, value, &FactSet::default())
}

pub fn local_let_stmt_with_facts(
    local: &LocalPlan,
    value: Option<RustExpr>,
    facts: &FactSet,
) -> RustStmt {
    local_let_stmt_with_facts_and_type_annotation(local, value, facts, true)
}

pub fn local_let_stmt_with_facts_and_type_annotation(
    local: &LocalPlan,
    value: Option<RustExpr>,
    facts: &FactSet,
    annotate_type: bool,
) -> RustStmt {
    RustStmt::Let {
        mutable: local.binding_mutable,
        name: local.name.clone(),
        ty: annotate_type.then(|| rust_type_for_slot_with_facts(&local.shape, facts)),
        value,
    }
}

pub fn assignment_stmt(target: RustExpr, value: RustExpr) -> RustStmt {
    RustStmt::Assign { target, value }
}

pub fn expr_stmt(expr: RustExpr) -> RustStmt {
    RustStmt::Expr(expr)
}

pub fn if_stmt(
    condition: RustExpr,
    then_body: Vec<RustStmt>,
    else_body: Vec<RustStmt>,
) -> RustStmt {
    RustStmt::If {
        condition,
        then_body,
        else_body,
    }
}

pub fn while_stmt(condition: RustExpr, body: Vec<RustStmt>) -> RustStmt {
    RustStmt::While { condition, body }
}

pub fn break_stmt(label: Option<String>) -> RustStmt {
    RustStmt::Break(label)
}

pub fn continue_stmt(label: Option<String>) -> RustStmt {
    RustStmt::Continue(label)
}

pub fn raw_stmt(raw: String) -> RustStmt {
    RustStmt::Raw(raw)
}

pub fn render_enhanced_for(
    var_name: &str,
    iterable: &RustExpr,
    body: &[RustStmt],
    label: Option<&str>,
) -> RustStmt {
    let label = label.map(|label| format!("'{label}: ")).unwrap_or_default();
    RustStmt::Raw(format!(
        "{label}for {var_name} in {}.iter() {{\n{}\n}}",
        crate::emit::emit_expr(iterable),
        crate::emit::emit_statements(body, 1)
    ))
}

pub fn render_enhanced_for_iterator(
    var_name: &str,
    iterator: &RustExpr,
    body: &[RustStmt],
    label: Option<&str>,
) -> RustStmt {
    let label = label.map(|label| format!("'{label}: ")).unwrap_or_default();
    RustStmt::Raw(format!(
        "{label}for {var_name} in {} {{\n{}\n}}",
        crate::emit::emit_expr(iterator),
        crate::emit::emit_statements(body, 1)
    ))
}

pub fn render_block_stmt(body: &[RustStmt]) -> RustStmt {
    RustStmt::Raw(format!("{{\n{}\n}}", crate::emit::emit_statements(body, 1)))
}
