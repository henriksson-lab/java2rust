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
pub struct SwitchCasePlan {
    pub labels: Vec<SwitchCaseLabelPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimpleReturnSwitchPlan<'a> {
    pub selector: &'a Expr,
    pub cases: &'a [SwitchCase],
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimpleReturnSwitchCasePlan<'a> {
    pub labels: Vec<SwitchCaseLabelPlan>,
    pub return_expr: &'a Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwitchCaseLabelPlan {
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

pub fn plan_switch_case(case: &SwitchCase) -> SwitchCasePlan {
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

pub fn plan_simple_return_switch_case(case: &SwitchCase) -> Option<SimpleReturnSwitchCasePlan<'_>> {
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
pub fn switch_case_stmt_needs_break_rewrite(stmt: &Stmt) -> bool {
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

pub fn rewrite_switch_breaks(stmt: &Stmt, switch_label: &str) -> Stmt {
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
