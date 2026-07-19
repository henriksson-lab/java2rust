use crate::array_plan::ArrayExprPlan;
use crate::emit::emit_expr;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::RustExpr;

pub fn render_array_expr(plan: &ArrayExprPlan<'_>, children: Vec<RustExpr>) -> RustExpr {
    match plan {
        ArrayExprPlan::Creation { element_type, .. } => {
            if children.len() == 1
                && let Some(default) = default_array_element_expr(element_type)
            {
                return RustExpr::Raw(format!(
                    "vec![{}; {} as usize]",
                    default,
                    emit_expr(&children[0])
                ));
            }
            let dims = children
                .iter()
                .map(emit_expr)
                .collect::<Vec<_>>()
                .join("][");
            RustExpr::Raw(format!(
                "/* Java array creation: {:?}[{dims}] */ Vec::new()",
                element_type
            ))
        }
        ArrayExprPlan::TypedInitializer { .. } | ArrayExprPlan::Initializer { .. } => {
            render_array_initializer(children)
        }
    }
}

fn render_array_initializer(children: Vec<RustExpr>) -> RustExpr {
    RustExpr::Raw(format!(
        "vec![{}]",
        children
            .iter()
            .map(emit_expr)
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn default_array_element_expr(element_type: &JavaType) -> Option<&'static str> {
    match element_type {
        JavaType::Primitive(PrimitiveType::Boolean) => Some("false"),
        JavaType::Primitive(PrimitiveType::Byte)
        | JavaType::Primitive(PrimitiveType::Short)
        | JavaType::Primitive(PrimitiveType::Int)
        | JavaType::Primitive(PrimitiveType::Long) => Some("0"),
        JavaType::Primitive(PrimitiveType::Float) | JavaType::Primitive(PrimitiveType::Double) => {
            Some("0.0")
        }
        JavaType::Primitive(PrimitiveType::Char) => Some("'\\0'"),
        JavaType::String => Some("String::new()"),
        _ => None,
    }
}
