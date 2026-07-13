use crate::java_ir::{Block, Expr, Stmt};

#[derive(Debug, Clone, PartialEq)]
pub enum ForBodyStmtPlan<'a> {
    ContinueWithUpdates {
        updates: &'a [Expr],
    },
    If {
        condition: &'a Expr,
        then_block: &'a Block,
        else_block: Option<&'a Block>,
    },
    While {
        condition: &'a Expr,
        body: &'a Block,
    },
    Default {
        stmt: &'a Stmt,
    },
}

pub fn plan_for_body_stmt<'a>(stmt: &'a Stmt, updates: &'a [Expr]) -> ForBodyStmtPlan<'a> {
    match stmt {
        Stmt::Continue(None) => ForBodyStmtPlan::ContinueWithUpdates { updates },
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => ForBodyStmtPlan::If {
            condition,
            then_block,
            else_block: else_block.as_ref(),
        },
        Stmt::While { condition, body } => ForBodyStmtPlan::While { condition, body },
        _ => ForBodyStmtPlan::Default { stmt },
    }
}
