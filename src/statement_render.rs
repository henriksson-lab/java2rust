use crate::coercion::receiver_method_call_projection_exact;
use crate::facts::FactSet;
use crate::representation::RepresentationPlan;
use crate::rust_ir::{RustExpr, RustStmt};
use crate::signature::LocalPlan;
#[cfg(test)]
use crate::slot_render::rust_type_for_slot_with_facts;
use crate::slot_render::rust_type_for_slot_with_facts_and_representations;

#[cfg(test)]
pub(crate) fn local_let_stmt_with_facts(
    local: &LocalPlan,
    value: Option<RustExpr>,
    facts: &FactSet,
) -> RustStmt {
    local_let_stmt_with_facts_and_type_annotation(local, value, facts, true)
}

#[cfg(test)]
fn local_let_stmt_with_facts_and_type_annotation(
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

pub(crate) fn local_let_stmt_with_facts_representations_and_type_annotation(
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

pub(crate) fn assignment_stmt(target: RustExpr, value: RustExpr) -> RustStmt {
    RustStmt::Assign { target, value }
}

pub(crate) fn assignment_op_stmt(
    target: RustExpr,
    op: impl Into<String>,
    value: RustExpr,
) -> RustStmt {
    RustStmt::AssignOp {
        target,
        op: op.into(),
        value,
    }
}

pub(crate) fn expr_stmt(expr: RustExpr) -> RustStmt {
    RustStmt::Expr(expr)
}

pub(crate) fn if_stmt(
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

pub(crate) fn if_let_stmt(
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

pub(crate) fn while_stmt(condition: RustExpr, body: Vec<RustStmt>) -> RustStmt {
    RustStmt::While { condition, body }
}

pub(crate) fn while_let_stmt(
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

pub(crate) fn break_stmt(label: Option<String>) -> RustStmt {
    RustStmt::Break(label)
}

pub(crate) fn continue_stmt(label: Option<String>) -> RustStmt {
    RustStmt::Continue(label)
}

pub(crate) fn render_enhanced_for(
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

pub(crate) fn render_enhanced_for_iterator(
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

pub(crate) fn render_block_stmt(body: &[RustStmt]) -> RustStmt {
    RustStmt::Expr(RustExpr::Block {
        stmts: body.to_vec(),
        tail: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{FactProvenance, TypeCapabilityOverride};
    use crate::java_ir::{JavaType, PrimitiveType};
    use crate::rust_ir::RustType;
    use crate::slot_shape::RustSlotShape;

    #[test]
    fn local_let_helper_uses_fact_aware_slot_types() {
        let local = LocalPlan {
            binding: crate::binding_id::BindingId::compatibility_local("count"),
            name: "count".to_string(),
            java_ty: JavaType::Primitive(PrimitiveType::Int),
            shape: RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int)),
            binding_mutable: true,
        };

        assert_eq!(
            local_let_stmt_with_facts(
                &local,
                Some(RustExpr::Literal("1".to_string())),
                &FactSet::default(),
            ),
            RustStmt::Let {
                mutable: true,
                name: "count".to_string(),
                ty: Some(RustType::I32),
                value: Some(RustExpr::Literal("1".to_string())),
            }
        );

        let external_ids = JavaType::Named {
            name: "ExternalIds".to_string(),
            args: Vec::new(),
        };
        let external_id = JavaType::Named {
            name: "ExternalId".to_string(),
            args: Vec::new(),
        };
        let external_local = LocalPlan {
            binding: crate::binding_id::BindingId::compatibility_local("ids"),
            name: "ids".to_string(),
            java_ty: external_ids.clone(),
            shape: RustSlotShape::conservative(external_ids),
            binding_mutable: false,
        };
        let mut facts = FactSet::default();
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: "ExternalIds".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: Some(external_id),
                implements: Vec::new(),
                extends: None,
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::UserOverride,
            });

        assert_eq!(
            local_let_stmt_with_facts(&external_local, None, &facts),
            RustStmt::Let {
                mutable: false,
                name: "ids".to_string(),
                ty: Some(RustType::Vec(Box::new(RustType::Named(
                    "ExternalId".to_string()
                )))),
                value: None,
            }
        );
    }

    #[test]
    fn simple_statement_constructors_stay_structured() {
        let body = vec![RustStmt::Expr(RustExpr::Var("body".to_string()))];

        assert_eq!(
            assignment_stmt(
                RustExpr::Var("count".to_string()),
                RustExpr::Literal("2".to_string())
            ),
            RustStmt::Assign {
                target: RustExpr::Var("count".to_string()),
                value: RustExpr::Literal("2".to_string()),
            }
        );
        assert_eq!(
            assignment_op_stmt(
                RustExpr::Var("flags".to_string()),
                "&",
                RustExpr::Literal("1".to_string())
            ),
            RustStmt::AssignOp {
                target: RustExpr::Var("flags".to_string()),
                op: "&".to_string(),
                value: RustExpr::Literal("1".to_string()),
            }
        );
        assert_eq!(
            expr_stmt(RustExpr::Var("tick".to_string())),
            RustStmt::Expr(RustExpr::Var("tick".to_string()))
        );
        assert_eq!(
            if_stmt(RustExpr::Var("ready".to_string()), body.clone(), Vec::new()),
            RustStmt::If {
                condition: RustExpr::Var("ready".to_string()),
                then_body: body.clone(),
                else_body: Vec::new(),
            }
        );
        assert_eq!(
            if_let_stmt(
                "Some(item)",
                RustExpr::Var("maybe_item".to_string()),
                body.clone(),
                Vec::new()
            ),
            RustStmt::IfLet {
                pattern: "Some(item)".to_string(),
                expr: RustExpr::Var("maybe_item".to_string()),
                then_body: body.clone(),
                else_body: Vec::new(),
            }
        );
        assert_eq!(
            while_stmt(RustExpr::Var("ready".to_string()), body.clone()),
            RustStmt::While {
                condition: RustExpr::Var("ready".to_string()),
                body: body.clone(),
            }
        );
        assert_eq!(
            while_let_stmt(
                Some("scan".to_string()),
                "Some(item)",
                RustExpr::Var("cursor".to_string()),
                body,
            ),
            RustStmt::WhileLet {
                label: Some("scan".to_string()),
                pattern: "Some(item)".to_string(),
                expr: RustExpr::Var("cursor".to_string()),
                body: vec![RustStmt::Expr(RustExpr::Var("body".to_string()))],
            }
        );
        assert_eq!(
            break_stmt(Some("loop_label".to_string())),
            RustStmt::Break(Some("loop_label".to_string()))
        );
        assert_eq!(continue_stmt(None), RustStmt::Continue(None));
    }

    #[test]
    fn enhanced_for_and_block_render_helpers_stay_structured() {
        let body = vec![RustStmt::Expr(RustExpr::Var("body".to_string()))];

        assert_eq!(
            render_enhanced_for("item", &RustExpr::Var("items".to_string()), &body, None),
            RustStmt::For {
                label: None,
                pattern: "item".to_string(),
                iterable: RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("items".to_string())),
                    method: "iter".to_string(),
                    args: Vec::new(),
                },
                body: body.clone(),
            }
        );
        assert_eq!(
            render_enhanced_for_iterator(
                "item",
                &RustExpr::Var("items_iter".to_string()),
                &body,
                Some("outer"),
            ),
            RustStmt::For {
                label: Some("outer".to_string()),
                pattern: "item".to_string(),
                iterable: RustExpr::Var("items_iter".to_string()),
                body: body.clone(),
            }
        );
        assert_eq!(
            render_block_stmt(&body),
            RustStmt::Expr(RustExpr::Block {
                stmts: body,
                tail: None,
            })
        );
    }
}
