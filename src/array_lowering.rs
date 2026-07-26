use crate::array_plan::ArrayExprPlan;
use crate::emit::emit_expr;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::{RustExpr, RustStmt};
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
                    array_dimension_len_expr(&children[0])
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
    if crate::types::ndarray_array2_element_type(&target.java_ty).is_none() {
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

pub fn array2_from_rows_projection(rows: RustExpr, ownership: OwnershipShape) -> Option<RustExpr> {
    let rows = emit_expr(&rows);
    match ownership {
        OwnershipShape::SharedBorrow => Some(RustExpr::Raw(format!(
            "&ndarray::Array2::from_rows({rows})"
        ))),
        OwnershipShape::Owned => Some(RustExpr::Raw(format!("ndarray::Array2::from_rows({rows})"))),
        OwnershipShape::MutableBorrow | OwnershipShape::CopyValue => None,
    }
}

pub fn array2_index_projection(array: RustExpr, row: RustExpr, col: RustExpr) -> RustExpr {
    RustExpr::Raw(format!(
        "{}[[{}, {}]]",
        emit_expr(&array),
        emit_expr(&row),
        emit_expr(&col)
    ))
}

pub fn array2_row_mut_assign_stmt(array: RustExpr, index: RustExpr, value: RustExpr) -> RustStmt {
    RustStmt::Raw(format!(
        "{}.row_mut({}).assign(&ndarray::ArrayView1::from({}));",
        emit_expr(&array),
        emit_expr(&index),
        emit_expr(&value)
    ))
}

pub fn ragged_array_outer_creation_projection(len: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("vec![Vec::new(); {}]", emit_expr(&len)))
}

fn array_dimension_len_expr(expr: &RustExpr) -> String {
    let rendered = emit_expr(expr);
    if rendered_needs_grouping_before_cast(&rendered) {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn rendered_needs_grouping_before_cast(rendered: &str) -> bool {
    let mut depth = 0usize;
    let mut prev_non_ws = None;
    let mut chars = rendered.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '+' | '*' | '/' | '%' if depth == 0 => return true,
            '-' if depth == 0 && prev_non_ws.is_some() => return true,
            '<' | '>' if depth == 0 => {
                if chars.peek().is_some_and(|(_, next)| *next == ch) {
                    return true;
                }
            }
            '&' | '|' if depth == 0 => {
                if chars.peek().is_some_and(|(_, next)| *next == ch) {
                    return true;
                }
            }
            _ => {}
        }
        if !ch.is_whitespace() {
            prev_non_ws = Some(ch);
        }
    }
    false
}

fn render_array_initializer(children: Vec<RustExpr>) -> RustExpr {
    let children = children
        .into_iter()
        .filter(|child| !is_comment_only_expr(child))
        .collect::<Vec<_>>();
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
    let children = children
        .iter()
        .filter(|child| !is_comment_only_expr(child))
        .collect::<Vec<_>>();
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

fn is_comment_only_expr(expr: &RustExpr) -> bool {
    emit_expr(expr).trim_start().starts_with("//")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array2_from_rows_projection_owns_static_matrix_handoff_spelling() {
        assert_eq!(
            emit_expr(
                &array2_from_rows_projection(
                    RustExpr::Raw("Self::COEF".to_string()),
                    OwnershipShape::SharedBorrow,
                )
                .unwrap()
            ),
            "&ndarray::Array2::from_rows(Self::COEF)"
        );
        assert_eq!(
            emit_expr(
                &array2_from_rows_projection(
                    RustExpr::Raw("Self::COEF".to_string()),
                    OwnershipShape::Owned,
                )
                .unwrap()
            ),
            "ndarray::Array2::from_rows(Self::COEF)"
        );
        assert!(
            array2_from_rows_projection(
                RustExpr::Raw("Self::COEF".to_string()),
                OwnershipShape::MutableBorrow,
            )
            .is_none()
        );
        assert_eq!(
            emit_expr(&array2_index_projection(
                RustExpr::Var("values".to_string()),
                RustExpr::Raw("row as usize".to_string()),
                RustExpr::Raw("col as usize".to_string()),
            )),
            "values[[row as usize, col as usize]]"
        );
        assert_eq!(
            array2_row_mut_assign_stmt(
                RustExpr::Var("values".to_string()),
                RustExpr::Raw("row as usize".to_string()),
                RustExpr::Var("replacement".to_string()),
            ),
            RustStmt::Raw(
                "values.row_mut(row as usize).assign(&ndarray::ArrayView1::from(replacement));"
                    .to_string()
            )
        );
        assert_eq!(
            emit_expr(&ragged_array_outer_creation_projection(RustExpr::Raw(
                "base_names.len() as i32 as usize".to_string()
            ))),
            "vec![Vec::new(); base_names.len() as i32 as usize]"
        );
    }
}
