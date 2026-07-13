use crate::java_ir::{Expr, JavaType};

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayExprPlan<'a> {
    Creation {
        element_type: &'a JavaType,
        dimensions: &'a [Expr],
    },
    Initializer {
        elements: &'a [Expr],
    },
}

impl<'a> ArrayExprPlan<'a> {
    pub fn child_exprs(&self) -> &'a [Expr] {
        match self {
            ArrayExprPlan::Creation { dimensions, .. } => dimensions,
            ArrayExprPlan::Initializer { elements } => elements,
        }
    }
}

pub fn plan_array_expr(expr: &Expr) -> Option<ArrayExprPlan<'_>> {
    match expr {
        Expr::ArrayCreation {
            element_type,
            dimensions,
        } => Some(ArrayExprPlan::Creation {
            element_type,
            dimensions,
        }),
        Expr::ArrayInitializer(elements) => Some(ArrayExprPlan::Initializer { elements }),
        _ => None,
    }
}
