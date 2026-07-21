use crate::array_plan::ArrayExprPlan;
use crate::emit::emit_expr;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::RustExpr;
use crate::slot_shape::{OwnershipShape, RustSlotShape};

pub fn render_array_expr(plan: &ArrayExprPlan<'_>, children: Vec<RustExpr>) -> RustExpr {
    match plan {
        ArrayExprPlan::Creation { element_type, .. } => {
            if children.len() == 2
                && let Some(default) = default_array_element_expr(element_type)
            {
                return RustExpr::Raw(format!(
                    "ndarray::Array2::from_elem(({} as usize, {} as usize), {})",
                    emit_expr(&children[0]),
                    emit_expr(&children[1]),
                    default,
                ));
            }
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
            if let Some(expr) = render_array2_initializer(&children) {
                return expr;
            }
            render_array_initializer(children)
        }
    }
}

pub fn borrowed_array2_temporary_for_slot(
    lowered: &RustExpr,
    target: &RustSlotShape,
) -> Option<RustExpr> {
    if crate::types::two_dimensional_array_element_type(&target.java_ty).is_none() {
        return None;
    }
    match target.ownership {
        OwnershipShape::SharedBorrow => Some(RustExpr::Raw(format!("&{}", emit_expr(lowered)))),
        OwnershipShape::MutableBorrow => {
            Some(RustExpr::Raw(format!("&mut {}", emit_expr(lowered))))
        }
        OwnershipShape::Owned | OwnershipShape::CopyValue => None,
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

fn render_array2_initializer(children: &[RustExpr]) -> Option<RustExpr> {
    if children.is_empty() {
        return None;
    }
    let rows = children
        .iter()
        .map(|child| vec_macro_body(&emit_expr(child)))
        .collect::<Option<Vec<_>>>()?;
    let width = split_top_level_commas(rows.first()?).len();
    if width == 0
        || rows
            .iter()
            .any(|row| split_top_level_commas(row).len() != width)
    {
        return None;
    }
    let rows = rows
        .iter()
        .map(|row| format!("[{}]", row.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    Some(RustExpr::Raw(format!("ndarray::arr2(&[{rows}])")))
}

fn vec_macro_body(rendered: &str) -> Option<String> {
    let trimmed = rendered.trim();
    trimmed
        .strip_prefix("vec![")
        .and_then(|body| body.strip_suffix(']'))
        .map(str::to_string)
}

fn split_top_level_commas(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in input.char_indices() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(input[start..index].trim());
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(input[start..].trim());
    parts.into_iter().filter(|part| !part.is_empty()).collect()
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
