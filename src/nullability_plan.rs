use crate::java_ir::{BinaryOp, Expr, Literal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TernaryNullRefinementPlan<'a> {
    LocalNullTest {
        local_name: &'a str,
        then_non_null: bool,
        else_non_null: bool,
    },
    None,
}

pub(crate) fn plan_ternary_null_refinement(condition: &Expr) -> TernaryNullRefinementPlan<'_> {
    let Some((local_name, op)) = local_null_binary_test(condition) else {
        return TernaryNullRefinementPlan::None;
    };
    match op {
        BinaryOp::Eq => TernaryNullRefinementPlan::LocalNullTest {
            local_name,
            then_non_null: false,
            else_non_null: true,
        },
        BinaryOp::Ne => TernaryNullRefinementPlan::LocalNullTest {
            local_name,
            then_non_null: true,
            else_non_null: false,
        },
        _ => TernaryNullRefinementPlan::None,
    }
}

fn local_null_binary_test(condition: &Expr) -> Option<(&str, BinaryOp)> {
    let Expr::Binary { op, left, right } = condition else {
        return None;
    };
    if !matches!(op, BinaryOp::Eq | BinaryOp::Ne) {
        return None;
    }
    local_compared_to_null(left, right, *op).or_else(|| local_compared_to_null(right, left, *op))
}

fn local_compared_to_null<'a>(
    maybe_local: &'a Expr,
    maybe_null: &Expr,
    op: BinaryOp,
) -> Option<(&'a str, BinaryOp)> {
    if !matches!(maybe_null, Expr::Literal(Literal::Null)) {
        return None;
    }
    let Expr::Var(name) = maybe_local else {
        return None;
    };
    Some((name.as_str(), op))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn var_null_test(name: &str, op: BinaryOp) -> Expr {
        Expr::Binary {
            op,
            left: Box::new(Expr::Var(name.to_string())),
            right: Box::new(Expr::Literal(Literal::Null)),
        }
    }

    #[test]
    fn ternary_null_refinement_tracks_local_null_tests() {
        assert_eq!(
            plan_ternary_null_refinement(&var_null_test("old", BinaryOp::Eq)),
            TernaryNullRefinementPlan::LocalNullTest {
                local_name: "old",
                then_non_null: false,
                else_non_null: true,
            }
        );
        assert_eq!(
            plan_ternary_null_refinement(&var_null_test("old", BinaryOp::Ne)),
            TernaryNullRefinementPlan::LocalNullTest {
                local_name: "old",
                then_non_null: true,
                else_non_null: false,
            }
        );
        assert_eq!(
            plan_ternary_null_refinement(&Expr::Var("ready".to_string())),
            TernaryNullRefinementPlan::None
        );
    }
}
