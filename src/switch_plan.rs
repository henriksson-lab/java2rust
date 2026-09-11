use crate::java_ir::{Block, Expr, Stmt, SwitchCase, SwitchLabel};

pub const SWITCH_LABEL: &str = "java_switch";
pub const SWITCH_VALUE: &str = "__java_switch_value";
pub const SWITCH_MATCHED: &str = "__java_switch_matched";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchPlan<'a> {
    pub selector: &'a Expr,
    pub cases: &'a [SwitchCase],
    pub break_label: &'static str,
    pub value_binding: &'static str,
    pub matched_binding: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SwitchCasePlan {
    pub(crate) labels: Vec<SwitchCaseLabelPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimpleReturnSwitchPlan<'a> {
    pub selector: &'a Expr,
    pub cases: &'a [SwitchCase],
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SimpleReturnSwitchCasePlan<'a> {
    pub labels: Vec<SwitchCaseLabelPlan>,
    pub return_expr: &'a Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SwitchCaseLabelPlan {
    Expr(Expr),
    Default,
}

pub fn plan_switch<'a>(selector: &'a Expr, cases: &'a [SwitchCase]) -> SwitchPlan<'a> {
    SwitchPlan {
        selector,
        cases,
        break_label: SWITCH_LABEL,
        value_binding: SWITCH_VALUE,
        matched_binding: SWITCH_MATCHED,
    }
}

pub(crate) fn plan_switch_case(case: &SwitchCase) -> SwitchCasePlan {
    SwitchCasePlan {
        labels: case
            .labels
            .iter()
            .map(|label| match label {
                SwitchLabel::Expr(expr) => SwitchCaseLabelPlan::Expr(expr.clone()),
                SwitchLabel::Default => SwitchCaseLabelPlan::Default,
            })
            .collect(),
    }
}

pub fn plan_simple_return_switch<'a>(
    selector: &'a Expr,
    cases: &'a [SwitchCase],
) -> Option<SimpleReturnSwitchPlan<'a>> {
    if cases.is_empty() {
        return None;
    }
    let mut pending_label_group = false;
    for case in cases {
        if plan_simple_return_switch_case(case).is_some() {
            pending_label_group = false;
            continue;
        }
        if case.statements.is_empty()
            && !case
                .labels
                .iter()
                .any(|label| matches!(label, SwitchLabel::Default))
        {
            pending_label_group = true;
            continue;
        }
        return None;
    }
    if pending_label_group {
        return None;
    }
    Some(SimpleReturnSwitchPlan { selector, cases })
}

pub(crate) fn plan_simple_return_switch_case(
    case: &SwitchCase,
) -> Option<SimpleReturnSwitchCasePlan<'_>> {
    let [Stmt::Return(Some(return_expr))] = case.statements.as_slice() else {
        return None;
    };
    Some(SimpleReturnSwitchCasePlan {
        labels: plan_switch_case(case).labels,
        return_expr,
    })
}

/// Whether `rewrite_switch_breaks` would actually change this statement.
///
/// The rewrite clones, which destroys the statement identity that per-statement
/// analyses (liveness) are keyed on. Most case statements contain no bare
/// `break` at all, so checking first lets the caller keep the original node.
/// Mirrors the traversal in `rewrite_switch_breaks` exactly: a `break` inside a
/// nested loop belongs to that loop, not to the switch.
pub(crate) fn switch_case_stmt_needs_break_rewrite(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Break(None) => true,
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_needs_switch_break_rewrite(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_needs_switch_break_rewrite)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_needs_switch_break_rewrite(block)
        }
        _ => false,
    }
}

fn block_needs_switch_break_rewrite(block: &Block) -> bool {
    block
        .statements
        .iter()
        .any(switch_case_stmt_needs_break_rewrite)
}

pub(crate) fn rewrite_switch_breaks(stmt: &Stmt, switch_label: &str) -> Stmt {
    match stmt {
        Stmt::Break(None) => Stmt::Break(Some(switch_label.to_string())),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => Stmt::If {
            condition: condition.clone(),
            then_block: rewrite_switch_breaks_in_block(then_block, switch_label),
            else_block: else_block
                .as_ref()
                .map(|block| rewrite_switch_breaks_in_block(block, switch_label)),
        },
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            Stmt::Block(rewrite_switch_breaks_in_block(block, switch_label))
        }
        _ => stmt.clone(),
    }
}

fn rewrite_switch_breaks_in_block(block: &Block, switch_label: &str) -> Block {
    Block {
        statements: block
            .statements
            .iter()
            .map(|stmt| rewrite_switch_breaks(stmt, switch_label))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::Literal;

    fn return_case(labels: Vec<SwitchLabel>, value: i32) -> SwitchCase {
        SwitchCase {
            labels,
            statements: vec![Stmt::Return(Some(Expr::Literal(Literal::Int(value))))],
        }
    }

    #[test]
    fn simple_return_switch_case_plans_return_cases_not_label_groups() {
        let label_group = SwitchCase {
            labels: vec![SwitchLabel::Expr(Expr::Literal(Literal::Int(2)))],
            statements: Vec::new(),
        };
        let return_group = return_case(vec![SwitchLabel::Expr(Expr::Literal(Literal::Int(3)))], 30);

        assert!(plan_simple_return_switch_case(&label_group).is_none());

        let planned = plan_simple_return_switch_case(&return_group).unwrap();
        assert_eq!(
            planned.labels,
            vec![SwitchCaseLabelPlan::Expr(Expr::Literal(Literal::Int(3)))]
        );
        assert_eq!(planned.return_expr, &Expr::Literal(Literal::Int(30)));
    }

    #[test]
    fn switch_case_plans_labels_and_rewrites_bare_breaks() {
        let expr_case = SwitchCase {
            labels: vec![SwitchLabel::Expr(Expr::Literal(Literal::Int(1)))],
            statements: vec![Stmt::Break(None)],
        };
        let default_case = SwitchCase {
            labels: vec![SwitchLabel::Default],
            statements: Vec::new(),
        };

        assert!(matches!(
            plan_switch_case(&expr_case).labels.as_slice(),
            [SwitchCaseLabelPlan::Expr(_)]
        ));
        assert!(matches!(
            plan_switch_case(&default_case).labels.as_slice(),
            [SwitchCaseLabelPlan::Default]
        ));
        assert!(switch_case_stmt_needs_break_rewrite(
            &expr_case.statements[0]
        ));
        assert_eq!(
            rewrite_switch_breaks(&expr_case.statements[0], SWITCH_LABEL),
            Stmt::Break(Some(SWITCH_LABEL.to_string()))
        );
    }
}
