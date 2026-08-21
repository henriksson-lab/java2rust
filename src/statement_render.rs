use crate::coercion::receiver_method_call_projection_exact;
use crate::facts::FactSet;
use crate::representation::RepresentationPlan;
use crate::rust_ir::{RustExpr, RustStmt};
use crate::signature::LocalPlan;
use crate::slot_render::{
    rust_type_for_slot_with_facts, rust_type_for_slot_with_facts_and_representations,
};

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

pub fn local_let_stmt_with_facts_representations_and_type_annotation(
    local: &LocalPlan,
    value: Option<RustExpr>,
    facts: &FactSet,
    representations: &RepresentationPlan,
    annotate_type: bool,
) -> RustStmt {
    RustStmt::Let {
        mutable: local.binding_mutable,
        name: local.name.clone(),
        ty: annotate_type.then(|| {
            rust_type_for_slot_with_facts_and_representations(&local.shape, facts, representations)
        }),
        value,
    }
}

pub fn assignment_stmt(target: RustExpr, value: RustExpr) -> RustStmt {
    RustStmt::Assign { target, value }
}

pub fn assignment_op_stmt(target: RustExpr, op: impl Into<String>, value: RustExpr) -> RustStmt {
    RustStmt::AssignOp {
        target,
        op: op.into(),
        value,
    }
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

pub fn if_let_stmt(
    pattern: impl Into<String>,
    expr: RustExpr,
    then_body: Vec<RustStmt>,
    else_body: Vec<RustStmt>,
) -> RustStmt {
    RustStmt::IfLet {
        pattern: pattern.into(),
        expr,
        then_body,
        else_body,
    }
}

pub fn while_stmt(condition: RustExpr, body: Vec<RustStmt>) -> RustStmt {
    RustStmt::While { condition, body }
}

pub fn while_let_stmt(
    label: Option<String>,
    pattern: impl Into<String>,
    expr: RustExpr,
    body: Vec<RustStmt>,
) -> RustStmt {
    RustStmt::WhileLet {
        label,
        pattern: pattern.into(),
        expr,
        body,
    }
}

pub fn break_stmt(label: Option<String>) -> RustStmt {
    RustStmt::Break(label)
}

pub fn break_value_stmt(label: Option<String>, value: RustExpr) -> RustStmt {
    RustStmt::BreakValue { label, value }
}

pub fn continue_stmt(label: Option<String>) -> RustStmt {
    RustStmt::Continue(label)
}

pub fn raw_stmt(raw: String) -> RustStmt {
    RustStmt::raw_leaf(raw)
}

pub fn render_enhanced_for(
    var_name: &str,
    iterable: &RustExpr,
    body: &[RustStmt],
    label: Option<&str>,
) -> RustStmt {
    let label = label.map(|label| label.to_string());
    RustStmt::For {
        label,
        pattern: var_name.to_string(),
        iterable: receiver_method_call_projection_exact(iterable.clone(), "iter", Vec::new()),
        body: body.to_vec(),
    }
}

pub fn render_enhanced_for_iterator(
    var_name: &str,
    iterator: &RustExpr,
    body: &[RustStmt],
    label: Option<&str>,
) -> RustStmt {
    let label = label.map(|label| label.to_string());
    RustStmt::For {
        label,
        pattern: var_name.to_string(),
        iterable: iterator.clone(),
        body: body.to_vec(),
    }
}

pub fn render_block_stmt(body: &[RustStmt]) -> RustStmt {
    RustStmt::Expr(RustExpr::Block {
        stmts: body.to_vec(),
        tail: None,
    })
}
