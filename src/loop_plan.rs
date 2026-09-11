use crate::java_ir::{BinaryOp, Block, Expr, Literal, Stmt};
use crate::rewrite::NullTestKind;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ForBodyStmtPlan<'a> {
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
pub(crate) enum WhileStmtPlan<'a> {
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
pub(crate) enum PostLoopRefinementPlan<'a> {
    LocalNonNull { local_name: &'a str },
    None,
}

pub(crate) fn plan_for_body_stmt<'a>(stmt: &'a Stmt, updates: &'a [Expr]) -> ForBodyStmtPlan<'a> {
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
        Stmt::While {
            condition, body, ..
        } => ForBodyStmtPlan::While { condition, body },
        _ => ForBodyStmtPlan::Default { stmt },
    }
}

pub(crate) fn plan_while_stmt<'a>(condition: &'a Expr, body: &'a Block) -> WhileStmtPlan<'a> {
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

pub(crate) fn post_loop_refinement_for_while<'a>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_expr;

    #[test]
    fn for_body_stmt_plans_continue_if_and_nested_while() {
        let update_exprs = vec![parse_expr("i++")];
        let then_block = Block {
            statements: vec![Stmt::Break(None)],
        };
        let else_block = Block {
            statements: vec![Stmt::Continue(Some("outer".to_string()))],
        };

        assert_eq!(
            plan_for_body_stmt(&Stmt::Continue(None), &update_exprs),
            ForBodyStmtPlan::ContinueWithUpdates {
                updates: update_exprs.as_slice()
            }
        );

        let if_stmt = Stmt::If {
            condition: parse_expr("ready"),
            then_block: then_block.clone(),
            else_block: Some(else_block.clone()),
        };
        assert_eq!(
            plan_for_body_stmt(&if_stmt, &update_exprs),
            ForBodyStmtPlan::If {
                condition: &parse_expr("ready"),
                then_block: &then_block,
                else_block: Some(&else_block),
            }
        );

        let while_stmt = Stmt::While {
            condition: parse_expr("ready"),
            body: then_block.clone(),
            label: None,
        };
        assert_eq!(
            plan_for_body_stmt(&while_stmt, &update_exprs),
            ForBodyStmtPlan::While {
                condition: &parse_expr("ready"),
                body: &then_block,
            }
        );
    }

    #[test]
    fn while_stmt_plans_assignment_in_condition_infinite_and_refinement() {
        let then_block = Block {
            statements: vec![Stmt::Break(None)],
        };
        let no_break_block = Block {
            statements: vec![Stmt::Expr(parse_expr("cursor.advance()"))],
        };
        let assignment_condition = parse_expr("(row = cursor.next()) != null");

        assert!(matches!(
            plan_while_stmt(&assignment_condition, &then_block),
            WhileStmtPlan::AssignmentInCondition {
                local_name: "row",
                value,
                null_test: NullTestKind::NotNull,
                body: _
            } if matches!(value, Expr::Call { name, .. } if name == "next")
        ));
        assert_eq!(
            plan_while_stmt(&parse_expr("true"), &then_block),
            WhileStmtPlan::Infinite { body: &then_block }
        );

        let null_assignment_condition = parse_expr("(row = cursor.next()) == null");
        let null_assignment_plan = plan_while_stmt(&null_assignment_condition, &no_break_block);
        assert_eq!(
            post_loop_refinement_for_while(&null_assignment_plan),
            PostLoopRefinementPlan::LocalNonNull { local_name: "row" }
        );
        assert_eq!(
            post_loop_refinement_for_while(&plan_while_stmt(&assignment_condition, &then_block)),
            PostLoopRefinementPlan::None
        );
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
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => block_contains_unlabeled_break(body),
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
