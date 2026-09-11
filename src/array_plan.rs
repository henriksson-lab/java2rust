use crate::java_ir::{Expr, JavaType};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ArrayExprPlan<'a> {
    Creation {
        element_type: &'a JavaType,
        dimensions: &'a [Expr],
    },
    TypedInitializer {
        element_type: &'a JavaType,
        elements: &'a [Expr],
    },
    Initializer {
        elements: &'a [Expr],
    },
}

impl<'a> ArrayExprPlan<'a> {
    pub(crate) fn child_exprs(&self) -> &'a [Expr] {
        match self {
            ArrayExprPlan::Creation { dimensions, .. } => dimensions,
            ArrayExprPlan::TypedInitializer { elements, .. } => elements,
            ArrayExprPlan::Initializer { elements } => elements,
        }
    }
}

pub(crate) fn plan_array_expr(expr: &Expr) -> Option<ArrayExprPlan<'_>> {
    match expr {
        Expr::ArrayCreation {
            element_type,
            dimensions,
            initializer,
        } => {
            if let Some(elements) = initializer {
                Some(ArrayExprPlan::TypedInitializer {
                    element_type,
                    elements,
                })
            } else {
                Some(ArrayExprPlan::Creation {
                    element_type,
                    dimensions,
                })
            }
        }
        Expr::ArrayInitializer(elements) => Some(ArrayExprPlan::Initializer { elements }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::{Literal, PrimitiveType};

    #[test]
    fn plans_array_creation_and_initializer_children_without_rendering() {
        let creation_dimensions =
            vec![Expr::Var("rows".to_string()), Expr::Var("cols".to_string())];
        let creation_expr = Expr::ArrayCreation {
            element_type: JavaType::Primitive(PrimitiveType::Int),
            dimensions: creation_dimensions,
            initializer: None,
        };
        let Some(ArrayExprPlan::Creation {
            element_type,
            dimensions,
        }) = plan_array_expr(&creation_expr)
        else {
            panic!("expected array creation plan");
        };
        assert_eq!(element_type, &JavaType::Primitive(PrimitiveType::Int));
        assert_eq!(dimensions.len(), 2);

        let sized_primitive_dimensions = vec![Expr::Var("len".to_string())];
        let sized_primitive_expr = Expr::ArrayCreation {
            element_type: JavaType::Primitive(PrimitiveType::Byte),
            dimensions: sized_primitive_dimensions,
            initializer: None,
        };
        let Some(ArrayExprPlan::Creation {
            element_type,
            dimensions,
        }) = plan_array_expr(&sized_primitive_expr)
        else {
            panic!("expected sized primitive array creation plan");
        };
        assert_eq!(element_type, &JavaType::Primitive(PrimitiveType::Byte));
        assert_eq!(dimensions.len(), 1);

        let typed_initializer_elements = vec![Expr::Literal(Literal::String("name".to_string()))];
        let typed_initializer_expr = Expr::ArrayCreation {
            element_type: JavaType::String,
            dimensions: Vec::new(),
            initializer: Some(typed_initializer_elements),
        };
        let Some(
            typed_initializer_plan @ ArrayExprPlan::TypedInitializer {
                element_type,
                elements,
            },
        ) = plan_array_expr(&typed_initializer_expr)
        else {
            panic!("expected typed array initializer plan");
        };
        assert_eq!(element_type, &JavaType::String);
        assert_eq!(elements.len(), 1);
        assert_eq!(typed_initializer_plan.child_exprs(), elements);

        let initializer_expr = Expr::ArrayInitializer(vec![
            Expr::Literal(Literal::Int(1)),
            Expr::Literal(Literal::Int(2)),
        ]);
        let Some(initializer_plan @ ArrayExprPlan::Initializer { elements }) =
            plan_array_expr(&initializer_expr)
        else {
            panic!("expected array initializer plan");
        };
        assert_eq!(elements.len(), 2);
        assert_eq!(
            initializer_plan.child_exprs(),
            elements,
            "array plan exposes child expressions without rendering them"
        );
        assert_eq!(plan_array_expr(&Expr::Var("value".to_string())), None);
    }
}
