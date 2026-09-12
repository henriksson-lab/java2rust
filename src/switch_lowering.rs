use crate::rust_ir::{RustExpr, RustStmt};
use crate::switch_plan::{SwitchCaseLabelPlan, SwitchCasePlan, SwitchPlan};

#[derive(Debug, Clone, PartialEq)]
pub struct SwitchRenderCase {
    label_condition: RustExpr,
    statements: Vec<RustStmt>,
}

impl SwitchRenderCase {
    pub fn new(label_condition: RustExpr, statements: Vec<RustStmt>) -> Self {
        Self {
            label_condition,
            statements,
        }
    }

    fn into_parts(self) -> (RustExpr, Vec<RustStmt>) {
        (self.label_condition, self.statements)
    }
}

pub(crate) fn switch_case_condition(
    plan: &SwitchPlan<'_>,
    case_plan: &SwitchCasePlan,
    label_exprs: Vec<RustExpr>,
) -> RustExpr {
    if case_plan
        .labels()
        .iter()
        .any(|label| matches!(label, SwitchCaseLabelPlan::Default))
    {
        return switch_unmatched_expr(plan);
    }
    let comparisons = label_exprs
        .into_iter()
        .map(|expr| switch_value_equals_label_expr(plan, expr))
        .collect::<Vec<_>>();
    let Some(any_label_matches) = or_exprs(comparisons) else {
        return RustExpr::Literal("false".to_string());
    };
    RustExpr::Binary {
        op: "&&".to_string(),
        left: Box::new(switch_unmatched_expr(plan)),
        right: Box::new(RustExpr::Grouped(Box::new(any_label_matches))),
    }
}

fn switch_unmatched_expr(plan: &SwitchPlan<'_>) -> RustExpr {
    RustExpr::Unary {
        op: "!".to_string(),
        expr: Box::new(RustExpr::Var(plan.matched_binding().to_string())),
    }
}

fn switch_value_equals_label_expr(plan: &SwitchPlan<'_>, label: RustExpr) -> RustExpr {
    RustExpr::Binary {
        op: "==".to_string(),
        left: Box::new(RustExpr::Var(plan.value_binding().to_string())),
        right: Box::new(label),
    }
}

fn or_exprs(exprs: Vec<RustExpr>) -> Option<RustExpr> {
    let mut exprs = exprs.into_iter();
    let first = exprs.next()?;
    Some(exprs.fold(first, |left, right| RustExpr::Binary {
        op: "||".to_string(),
        left: Box::new(left),
        right: Box::new(right),
    }))
}

pub fn lower_switch_block_stmts(
    plan: &SwitchPlan<'_>,
    selector: RustExpr,
    cases: Vec<SwitchRenderCase>,
    append_unreachable: bool,
) -> Vec<RustStmt> {
    let mut loop_body = Vec::new();
    for case in cases {
        let (label_condition, statements) = case.into_parts();
        loop_body.push(RustStmt::If {
            condition: label_condition,
            then_body: vec![RustStmt::Assign {
                target: RustExpr::Var(plan.matched_binding().to_string()),
                value: RustExpr::Literal("true".to_string()),
            }],
            else_body: Vec::new(),
        });
        loop_body.push(RustStmt::If {
            condition: RustExpr::Var(plan.matched_binding().to_string()),
            then_body: statements,
            else_body: Vec::new(),
        });
    }
    loop_body.push(RustStmt::BreakValue {
        label: Some(plan.break_label().to_string()),
        value: RustExpr::Tuple(Vec::new()),
    });
    let mut out = vec![RustStmt::Expr(RustExpr::Block {
        stmts: vec![
            RustStmt::Let {
                mutable: false,
                name: plan.value_binding().to_string(),
                ty: None,
                value: Some(selector),
            },
            RustStmt::Let {
                mutable: true,
                name: plan.matched_binding().to_string(),
                ty: None,
                value: Some(RustExpr::Literal("false".to_string())),
            },
            RustStmt::Loop {
                label: Some(plan.break_label().to_string()),
                body: loop_body,
            },
        ],
        tail: None,
    })];
    if append_unreachable {
        out.push(RustStmt::Expr(RustExpr::MacroCall {
            macro_name: "unreachable".to_string(),
            args: vec![RustExpr::Literal(
                "\"Java switch completed without returning\"".to_string(),
            )],
        }));
    }
    out
}

pub(crate) fn switch_match_pattern_expr(expr: RustExpr) -> String {
    crate::emit::emit_expr(&expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit;
    use crate::java_ir::{Expr, Literal, SwitchCase, SwitchLabel};
    use crate::switch_plan::{SWITCH_LABEL, plan_switch, plan_switch_case};

    #[test]
    fn switch_case_condition_builds_structured_exprs() {
        let selector = Expr::Var("value".to_string());
        let expr_case = SwitchCase {
            labels: vec![SwitchLabel::Expr(Expr::Literal(Literal::Int(1)))],
            statements: Vec::new(),
        };
        let default_case = SwitchCase {
            labels: vec![SwitchLabel::Default],
            statements: Vec::new(),
        };
        let cases = vec![expr_case, default_case];
        let plan = plan_switch(&selector, &cases);

        assert_eq!(plan.break_label(), SWITCH_LABEL);
        assert_eq!(
            emit::emit_expr(&switch_case_condition(
                &plan,
                &plan_switch_case(&cases[0]),
                vec![RustExpr::Literal("1".to_string())],
            )),
            "!__java_switch_matched && (__java_switch_value == 1)"
        );
        assert!(matches!(
            switch_case_condition(
                &plan,
                &plan_switch_case(&cases[0]),
                vec![RustExpr::Literal("1".to_string())],
            ),
            RustExpr::Binary { op, .. } if op == "&&"
        ));
        assert_eq!(
            emit::emit_expr(&switch_case_condition(
                &plan,
                &plan_switch_case(&cases[1]),
                Vec::new(),
            )),
            "!__java_switch_matched"
        );
    }
}
