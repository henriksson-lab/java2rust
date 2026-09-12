use crate::coercion::{receiver_method_call_projection_exact, unwrap_optional_shared_value};
use crate::rewrite::NullTestKind;
use crate::rust_ir::{RustExpr, RustStmt};

pub fn assemble_for_loop(
    condition: Option<RustExpr>,
    body: Vec<RustStmt>,
    label: Option<String>,
) -> RustStmt {
    if let Some(label) = label {
        let mut loop_body = Vec::new();
        loop_body.push(RustStmt::If {
            condition: crate::coercion::logical_not_projection(
                condition.unwrap_or_else(default_for_condition),
            ),
            then_body: vec![RustStmt::Break(Some(label.clone()))],
            else_body: Vec::new(),
        });
        loop_body.extend(body);
        return RustStmt::Loop {
            label: Some(label),
            body: loop_body,
        };
    }
    RustStmt::While {
        condition: condition.unwrap_or_else(default_for_condition),
        body,
    }
}

pub(crate) fn continue_with_updates(update_exprs: &[RustExpr]) -> Vec<RustStmt> {
    update_exprs
        .iter()
        .cloned()
        .map(RustStmt::Expr)
        .chain(std::iter::once(RustStmt::Continue(None)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continue_with_updates_replays_updates_before_continue() {
        assert_eq!(
            continue_with_updates(&[RustExpr::Var("i += 1".to_string())]),
            vec![
                RustStmt::Expr(RustExpr::Var("i += 1".to_string())),
                RustStmt::Continue(None),
            ]
        );
    }
}

pub fn assignment_condition_loop(
    local_name: &str,
    assigned_value: RustExpr,
    null_test: NullTestKind,
    mut body: Vec<RustStmt>,
    label: Option<String>,
) -> RustStmt {
    let local = RustExpr::Var(local_name.to_string());
    let break_condition = match null_test {
        NullTestKind::NotNull => {
            receiver_method_call_projection_exact(local.clone(), "is_none", Vec::new())
        }
        NullTestKind::IsNull => {
            receiver_method_call_projection_exact(local.clone(), "is_some", Vec::new())
        }
    };
    let mut loop_body = vec![
        RustStmt::Assign {
            target: local.clone(),
            value: assigned_value,
        },
        RustStmt::If {
            condition: break_condition,
            then_body: vec![RustStmt::Break(None)],
            else_body: Vec::new(),
        },
    ];
    if null_test == NullTestKind::NotNull {
        loop_body.push(RustStmt::Let {
            mutable: false,
            name: local_name.to_string(),
            ty: None,
            value: Some(unwrap_optional_shared_value(local)),
        });
    }
    loop_body.append(&mut body);
    RustStmt::Loop {
        label,
        body: loop_body,
    }
}

fn default_for_condition() -> RustExpr {
    RustExpr::Literal("true".to_string())
}
