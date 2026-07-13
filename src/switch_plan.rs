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
        Stmt::Block(block) => Stmt::Block(rewrite_switch_breaks_in_block(block, switch_label)),
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
