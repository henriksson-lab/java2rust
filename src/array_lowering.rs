use crate::array_plan::ArrayExprPlan;
use crate::coercion::{BorrowProjectionUse, ensure_borrow_projection, rendered_vec_macro_body};
use crate::emit::emit_expr;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::{RustExpr, RustStmt};
use crate::slot_shape::{OwnershipShape, RustSlotShape};

pub fn render_array_expr(plan: &ArrayExprPlan<'_>, children: Vec<RustExpr>) -> RustExpr {
    render_array_expr_with_object_default(plan, children, false)
}

/// `render_array_expr`, told whether the element type has a usable zero value.
///
/// `new T[n]` allocates `n` `null` slots in Java. Without a zero value for `T`
/// the translation used to emit `Vec::new()` — the right *type* and the wrong
/// *length*, so every index panicked
/// ([0034](../claude_audits/0034-object-array-allocation-produces-an-empty-vec.md)).
/// `element_has_default` says whether `T` derives `Default`, which lowering
/// decides from the same capability check that adds the derive, so the two
/// cannot disagree.
pub fn render_array_expr_with_object_default(
    plan: &ArrayExprPlan<'_>,
    children: Vec<RustExpr>,
    element_has_default: bool,
) -> RustExpr {
    match plan {
        ArrayExprPlan::Creation { element_type, .. } => {
            let default = default_array_element_expr(element_type)
                .or(element_has_default.then_some(OBJECT_ELEMENT_DEFAULT));
            if children.len() == 2
                && let Some(default) = default
            {
                // The `ndarray` carrier is for **primitive** elements only —
                // `types::ndarray_array2_element_type` has always said so, and
                // the write path has always consulted it. Construction did not,
                // so `new String[2][2]` was built as an `Array2` while its
                // declared type said `Vec<Vec<String>>` and `new String[2][]`
                // built a nested `Vec`: one Java type, three carriers, and
                // output `rustc` rejects
                // ([0038](../claude_audits/done/0038-two-dimensional-object-array-carrier-disagrees.md)).
                if matches!(element_type, JavaType::Primitive(_)) {
                    return array2_from_elem_projection(
                        children[0].clone(),
                        children[1].clone(),
                        default,
                    );
                }
                return nested_array2_from_elem_projection(
                    children[0].clone(),
                    children[1].clone(),
                    default,
                );
            }
            if children.len() == 1
                && let Some(default) = default
            {
                return array_from_elem_projection(children[0].clone(), default);
            }
            java_array_creation_fallback_projection(element_type, &children)
        }
        ArrayExprPlan::TypedInitializer { .. } | ArrayExprPlan::Initializer { .. } => {
            if let Some(expr) = render_array2_initializer(&children) {
                return expr;
            }
            render_array_initializer(children)
        }
    }
}

pub fn array2_from_elem_projection(rows: RustExpr, cols: RustExpr, default: &str) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["ndarray", "Array2", "from_elem"])),
        args: vec![
            RustExpr::Tuple(vec![
                RustExpr::Cast {
                    expr: Box::new(rows),
                    ty: "usize".to_string(),
                },
                RustExpr::Cast {
                    expr: Box::new(cols),
                    ty: "usize".to_string(),
                },
            ]),
            RustExpr::Raw(default.to_string()),
        ],
    }
}

/// `new T[rows][cols]` for a non-primitive `T`: rows of `Vec<T>`.
///
/// Matches what `new T[rows][]` followed by per-row allocation already
/// produced, and what the declared type says.
pub fn nested_array2_from_elem_projection(
    rows: RustExpr,
    cols: RustExpr,
    default: &str,
) -> RustExpr {
    RustExpr::VecRepeat {
        value: Box::new(RustExpr::VecRepeat {
            value: Box::new(RustExpr::Raw(default.to_string())),
            len: Box::new(array_dimension_len_projection(cols)),
        }),
        len: Box::new(array_dimension_len_projection(rows)),
    }
}

pub fn array_from_elem_projection(len: RustExpr, default: &str) -> RustExpr {
    RustExpr::VecRepeat {
        value: Box::new(RustExpr::Raw(default.to_string())),
        len: Box::new(array_dimension_len_projection(len)),
    }
}

pub fn java_array_creation_fallback_projection(
    element_type: &JavaType,
    dimensions: &[RustExpr],
) -> RustExpr {
    let dims = dimensions
        .iter()
        .map(emit_expr)
        .collect::<Vec<_>>()
        .join("][");
    // The marker text is what `count_invalid_patterns` looks for. It used to
    // read "Java array creation", which that detector does not match, so an
    // allocation that silently comes out with length 0 — every index into it
    // panics — counted as a clean translation. Both sides now read one
    // constant, so the text cannot drift again.
    RustExpr::Raw(format!(
        "/* {}: {:?}[{dims}] */ Vec::new()",
        crate::output_markers::UNSUPPORTED_JAVA_ARRAY_CREATION,
        element_type
    ))
}

pub fn borrowed_array2_temporary_for_slot(
    lowered: &RustExpr,
    target: &RustSlotShape,
) -> Option<RustExpr> {
    if crate::types::ndarray_array2_element_type(&target.java_ty).is_none() {
        return None;
    }
    match target.ownership {
        OwnershipShape::SharedBorrow => Some(ensure_borrow_projection(
            lowered.clone(),
            None,
            Some(target),
            BorrowProjectionUse::Argument,
        )),
        OwnershipShape::MutableBorrow => Some(ensure_borrow_projection(
            lowered.clone(),
            None,
            Some(target),
            BorrowProjectionUse::MutableSliceView,
        )),
        OwnershipShape::Owned | OwnershipShape::CopyValue => None,
    }
}

pub fn array2_from_rows_projection(rows: RustExpr, ownership: OwnershipShape) -> Option<RustExpr> {
    let call = RustExpr::Call {
        function: Box::new(RustExpr::path(["ndarray", "Array2", "from_rows"])),
        args: vec![rows],
    };
    match ownership {
        OwnershipShape::SharedBorrow => Some(RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(call),
        }),
        OwnershipShape::Owned => Some(call),
        OwnershipShape::MutableBorrow | OwnershipShape::CopyValue => None,
    }
}

pub fn array2_index_projection(array: RustExpr, row: RustExpr, col: RustExpr) -> RustExpr {
    RustExpr::Index(
        Box::new(parenthesize_raw_postfix_receiver(array)),
        Box::new(RustExpr::ArrayLiteral(vec![row, col])),
    )
}

fn parenthesize_raw_postfix_receiver(expr: RustExpr) -> RustExpr {
    let RustExpr::Raw(raw) = expr else {
        return expr;
    };
    let trimmed = raw.trim();
    if raw_needs_postfix_receiver_parentheses(trimmed) {
        RustExpr::Raw(format!("({trimmed})"))
    } else {
        RustExpr::Raw(raw)
    }
}

fn raw_needs_postfix_receiver_parentheses(raw: &str) -> bool {
    if raw.starts_with('(') && raw.ends_with(')') {
        return false;
    }
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '"' | '\'' => {
                let quote = ch;
                let mut escaped = false;
                for (_, inner) in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == quote {
                        break;
                    }
                }
            }
            '+' | '-' | '*' | '/' | '%' | '^' | '&' | '|' | '=' | '<' | '>'
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
            {
                return true;
            }
            '!' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && (idx == 0 || raw[idx + 1..].starts_with('=')) =>
            {
                return true;
            }
            'a' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && raw[idx..].starts_with("as")
                && raw[..idx].ends_with(char::is_whitespace)
                && raw[idx + 2..].starts_with(char::is_whitespace) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

pub fn array2_row_mut_assign_stmt(array: RustExpr, index: RustExpr, value: RustExpr) -> RustStmt {
    if !matches!(array.unshaped(), RustExpr::Raw(_)) {
        return RustStmt::Expr(RustExpr::MethodCall {
            receiver: Box::new(RustExpr::MethodCall {
                receiver: Box::new(array),
                method: "row_mut".to_string(),
                args: vec![index],
            }),
            method: "assign".to_string(),
            args: vec![RustExpr::Unary {
                op: "&".to_string(),
                expr: Box::new(RustExpr::Call {
                    function: Box::new(RustExpr::path(["ndarray", "ArrayView1", "from"])),
                    args: vec![value],
                }),
            }],
        });
    }
    RustStmt::Raw(format!(
        "{}.row_mut({}).assign(&ndarray::ArrayView1::from({}));",
        emit_expr(&array),
        emit_expr(&index),
        emit_expr(&value)
    ))
}

pub fn ragged_array_outer_creation_projection(len: RustExpr) -> RustExpr {
    RustExpr::VecRepeat {
        value: Box::new(RustExpr::Call {
            function: Box::new(RustExpr::path(["Vec", "new"])),
            args: Vec::new(),
        }),
        len: Box::new(len),
    }
}

fn array_dimension_len_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(expr),
        ty: "usize".to_string(),
    }
}

fn render_array_initializer(children: Vec<RustExpr>) -> RustExpr {
    let children = children
        .into_iter()
        .filter(|child| !is_comment_only_expr(child))
        .collect::<Vec<_>>();
    RustExpr::VecLiteral(children)
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
        .map(|child| rendered_vec_macro_body(&emit_expr(child)))
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

pub fn default_array_element_expr(element_type: &JavaType) -> Option<&'static str> {
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

/// The element expression for an object array whose element type derives
/// `Default`.
///
/// Spelled `Default::default()` rather than `T::default()` so the element type
/// is inferred from the annotated `Vec<T>` binding, which keeps the projection
/// independent of how `T`'s Rust name is rendered.
///
/// This is the audit's option 2: length-correct, with one divergence left —
/// reading an element that was never assigned yields a zero value where Java
/// throws `NullPointerException`. Option 1 (`Vec<Option<T>>`) is the faithful
/// end state and is a slot-shape change; this is strictly better than an
/// allocation of the wrong length.
const OBJECT_ELEMENT_DEFAULT: &str = "Default::default()";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array2_from_rows_projection_owns_static_matrix_handoff_spelling() {
        let structured_from_elem = array2_from_elem_projection(
            RustExpr::Var("rows".to_string()),
            RustExpr::Var("cols".to_string()),
            "0",
        );
        assert_eq!(
            emit_expr(&structured_from_elem),
            "ndarray::Array2::from_elem((rows as usize, cols as usize), 0)"
        );
        assert!(matches!(structured_from_elem, RustExpr::Call { .. }));
        let structured_from_elem_with_raw_dimension = array2_from_elem_projection(
            RustExpr::Raw("rows".to_string()),
            RustExpr::Var("cols".to_string()),
            "0",
        );
        assert_eq!(
            emit_expr(&structured_from_elem_with_raw_dimension),
            "ndarray::Array2::from_elem((rows as usize, cols as usize), 0)"
        );
        assert!(matches!(
            structured_from_elem_with_raw_dimension,
            RustExpr::Call { .. }
        ));
        let structured_vec_repeat =
            array_from_elem_projection(RustExpr::Var("len".to_string()), "0");
        assert_eq!(emit_expr(&structured_vec_repeat), "vec![0; len as usize]");
        assert!(matches!(structured_vec_repeat, RustExpr::VecRepeat { .. }));
        let structured_nested_repeat = nested_array2_from_elem_projection(
            RustExpr::Var("rows".to_string()),
            RustExpr::Var("cols".to_string()),
            "String::new()",
        );
        assert_eq!(
            emit_expr(&structured_nested_repeat),
            "vec![vec![String::new(); cols as usize]; rows as usize]"
        );
        assert!(matches!(
            structured_nested_repeat,
            RustExpr::VecRepeat { .. }
        ));

        let raw_rows_borrow = array2_from_rows_projection(
            RustExpr::Raw("Self::COEF".to_string()),
            OwnershipShape::SharedBorrow,
        )
        .unwrap();
        assert_eq!(
            emit_expr(&raw_rows_borrow),
            "&ndarray::Array2::from_rows(Self::COEF)"
        );
        assert!(matches!(
            &raw_rows_borrow,
            RustExpr::Unary {
                op,
                expr,
            } if op == "&" && matches!(expr.as_ref(), RustExpr::Call { .. })
        ));
        let raw_rows_owned = array2_from_rows_projection(
            RustExpr::Raw("Self::COEF".to_string()),
            OwnershipShape::Owned,
        )
        .unwrap();
        assert_eq!(
            emit_expr(&raw_rows_owned),
            "ndarray::Array2::from_rows(Self::COEF)"
        );
        assert!(matches!(raw_rows_owned, RustExpr::Call { .. }));
        let structured_borrow = array2_from_rows_projection(
            RustExpr::Var("rows".to_string()),
            OwnershipShape::SharedBorrow,
        )
        .unwrap();
        assert!(matches!(
            &structured_borrow,
            RustExpr::Unary {
                op,
                expr,
            } if op == "&" && matches!(expr.as_ref(), RustExpr::Call { .. })
        ));
        assert_eq!(
            emit_expr(&structured_borrow),
            "&ndarray::Array2::from_rows(rows)"
        );
        assert!(matches!(
            array2_from_rows_projection(RustExpr::Var("rows".to_string()), OwnershipShape::Owned)
                .unwrap(),
            RustExpr::Call { .. }
        ));
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
        let index_expr = array2_index_projection(
            RustExpr::Var("values".to_string()),
            RustExpr::Raw("row as usize".to_string()),
            RustExpr::Raw("col as usize".to_string()),
        );
        assert!(matches!(
            index_expr,
            RustExpr::Index(_, ref index) if matches!(index.as_ref(), RustExpr::ArrayLiteral(_))
        ));
        let raw_receiver_index = array2_index_projection(
            RustExpr::Raw("values".to_string()),
            RustExpr::Raw("row as usize".to_string()),
            RustExpr::Raw("col as usize".to_string()),
        );
        assert_eq!(
            emit_expr(&raw_receiver_index),
            "values[[row as usize, col as usize]]"
        );
        assert!(matches!(raw_receiver_index, RustExpr::Index(_, _)));
        let compound_raw_receiver_index = array2_index_projection(
            RustExpr::Raw("left + right".to_string()),
            RustExpr::Raw("row as usize".to_string()),
            RustExpr::Raw("col as usize".to_string()),
        );
        assert_eq!(
            emit_expr(&compound_raw_receiver_index),
            "(left + right)[[row as usize, col as usize]]"
        );
        assert!(matches!(compound_raw_receiver_index, RustExpr::Index(_, _)));
        let structured_row_assign_with_raw_index = array2_row_mut_assign_stmt(
            RustExpr::Var("values".to_string()),
            RustExpr::Raw("row as usize".to_string()),
            RustExpr::Var("replacement".to_string()),
        );
        let RustStmt::Expr(structured_row_assign_with_raw_index_expr) =
            structured_row_assign_with_raw_index
        else {
            panic!("expected structured row assignment expression for raw index");
        };
        assert_eq!(
            emit_expr(&structured_row_assign_with_raw_index_expr),
            "values.row_mut(row as usize).assign(&ndarray::ArrayView1::from(replacement))"
        );
        assert!(matches!(
            structured_row_assign_with_raw_index_expr,
            RustExpr::MethodCall { .. }
        ));
        let structured_row_assign_with_raw_value = array2_row_mut_assign_stmt(
            RustExpr::Var("values".to_string()),
            RustExpr::Raw("0 as usize".to_string()),
            RustExpr::Raw("&vec![2, 3, 6, 4]".to_string()),
        );
        let RustStmt::Expr(structured_row_assign_with_raw_value_expr) =
            structured_row_assign_with_raw_value
        else {
            panic!("expected structured row assignment expression for raw value");
        };
        assert_eq!(
            emit_expr(&structured_row_assign_with_raw_value_expr),
            "values.row_mut(0 as usize).assign(&ndarray::ArrayView1::from(&vec![2, 3, 6, 4]))"
        );
        let structured_row_assign = array2_row_mut_assign_stmt(
            RustExpr::Var("values".to_string()),
            RustExpr::Var("row".to_string()),
            RustExpr::Var("replacement".to_string()),
        );
        let RustStmt::Expr(structured_row_assign_expr) = structured_row_assign else {
            panic!("expected structured row assignment expression");
        };
        assert_eq!(
            emit_expr(&structured_row_assign_expr),
            "values.row_mut(row).assign(&ndarray::ArrayView1::from(replacement))"
        );
        assert!(matches!(
            structured_row_assign_expr,
            RustExpr::MethodCall { .. }
        ));
        assert_eq!(
            emit_expr(&ragged_array_outer_creation_projection(RustExpr::Raw(
                "base_names.len() as i32 as usize".to_string()
            ))),
            "vec![Vec::new(); base_names.len() as i32 as usize]"
        );
        assert!(matches!(
            ragged_array_outer_creation_projection(RustExpr::Var("len".to_string())),
            RustExpr::VecRepeat { .. }
        ));
    }
}
