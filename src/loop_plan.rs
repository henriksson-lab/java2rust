use crate::java_ir::{BinaryOp, Block, Expr, Literal, Stmt};
use crate::rewrite::NullTestKind;

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

#[derive(Debug, Clone, PartialEq)]
pub enum WhileStmtPlan<'a> {
    AssignmentInCondition {
        local_name: &'a str,
        value: &'a Expr,
        null_test: NullTestKind,
        body: &'a Block,
    },
    Infinite {
        body: &'a Block,
    },
    Plain {
        condition: &'a Expr,
        body: &'a Block,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PostLoopRefinementPlan<'a> {
    LocalNonNull { local_name: &'a str },
    None,
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

pub fn plan_while_stmt<'a>(condition: &'a Expr, body: &'a Block) -> WhileStmtPlan<'a> {
    if let Some((local_name, value, null_test)) = assignment_in_null_test(condition) {
        WhileStmtPlan::AssignmentInCondition {
            local_name,
            value,
            null_test,
            body,
        }
    } else if matches!(condition, Expr::Literal(Literal::Bool(true))) {
        WhileStmtPlan::Infinite { body }
    } else {
        WhileStmtPlan::Plain { condition, body }
    }
}

pub fn post_loop_refinement_for_while<'a>(
    plan: &'a WhileStmtPlan<'a>,
) -> PostLoopRefinementPlan<'a> {
    match plan {
        WhileStmtPlan::AssignmentInCondition {
            local_name,
            null_test: NullTestKind::IsNull,
            body,
            ..
        } if !block_contains_unlabeled_break(body) => {
            PostLoopRefinementPlan::LocalNonNull { local_name }
        }
        _ => PostLoopRefinementPlan::None,
    }
}

fn assignment_in_null_test(condition: &Expr) -> Option<(&str, &Expr, NullTestKind)> {
    let Expr::Binary { op, left, right } = condition else {
        return None;
    };
    let null_test = match op {
        BinaryOp::Ne => NullTestKind::NotNull,
        BinaryOp::Eq => NullTestKind::IsNull,
        _ => return None,
    };
    assignment_compared_to_null(left, right, null_test)
        .or_else(|| assignment_compared_to_null(right, left, null_test))
}

fn assignment_compared_to_null<'a>(
    maybe_assignment: &'a Expr,
    maybe_null: &Expr,
    null_test: NullTestKind,
) -> Option<(&'a str, &'a Expr, NullTestKind)> {
    if !matches!(maybe_null, Expr::Literal(crate::java_ir::Literal::Null)) {
        return None;
    }
    let Expr::Binary {
        op: BinaryOp::Assign,
        left,
        right,
    } = maybe_assignment
    else {
        return None;
    };
    let Expr::Var(local_name) = left.as_ref() else {
        return None;
    };
    Some((local_name.as_str(), right.as_ref(), null_test))
}

fn block_contains_unlabeled_break(block: &Block) -> bool {
    block.statements.iter().any(stmt_contains_unlabeled_break)
}

fn stmt_contains_unlabeled_break(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Break(None) => true,
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_contains_unlabeled_break(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_contains_unlabeled_break)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body) => block_contains_unlabeled_break(body),
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_contains_unlabeled_break)),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_contains_unlabeled_break)
                || block_contains_unlabeled_break(body)
                || catches
                    .iter()
                    .any(|catch| block_contains_unlabeled_break(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_contains_unlabeled_break)
        }
        _ => false,
    }
}
