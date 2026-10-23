use crate::array_plan::ArrayExprPlan;
use crate::coercion::{
    BorrowProjectionUse, ensure_borrow_projection, function_call_projection,
    postfix_receiver_projection, receiver_method_call_projection_exact,
    recover_parseable_raw_operand_projection,
};
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::{RustClosureBody, RustExpr, RustStmt};
use crate::slot_shape::{OwnershipShape, RustSlotShape};

/// `array_expr`, told whether the element type has a usable zero value.
///
/// `new T[n]` allocates `n` `null` slots in Java. Without a zero value for `T`
/// the translation used to emit `Vec::new()` — the right *type* and the wrong
/// *length*, so every index panicked
/// ([0034](../claude_audits/0034-object-array-allocation-produces-an-empty-vec.md)).
/// Reference-typed arrays use `Option<T>` elements, so their Java default
/// element is `None`, not `T::default()`.
pub(crate) fn array_expr_with_object_default(
    plan: &ArrayExprPlan<'_>,
    children: Vec<RustExpr>,
    element_has_default: bool,
) -> RustExpr {
    match plan {
        ArrayExprPlan::Creation { element_type, .. } => {
            let default = default_array_element_expr(element_type)
                .or_else(|| element_has_default.then(object_element_default_expr));
            if children.len() >= 3
                && let Some(default) = default
            {
                // `Array2` is deliberately restricted to an exactly
                // two-dimensional primitive array. Higher-rank Java arrays
                // retain their nested/ragged representation, even when every
                // dimension happens to be supplied by one creation
                // expression. Build that representation from the innermost
                // dimension out instead of returning the old empty-vector
                // fallback (which made every subsequent index fail).
                return nested_array_from_elem_projection(
                    &children,
                    default,
                    nullable_array_default_needs_repeat_with(element_type),
                );
            }
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
                    nullable_array_default_needs_repeat_with(element_type),
                );
            }
            if children.len() == 1
                && let Some(default) = default
            {
                return array_from_elem_projection_for_element_type(
                    children[0].clone(),
                    default,
                    nullable_array_default_needs_repeat_with(element_type),
                );
            }
            java_array_creation_fallback_projection(element_type, &children)
        }
        ArrayExprPlan::TypedInitializer { .. } | ArrayExprPlan::Initializer { .. } => {
            if let Some(expr) = array2_initializer_projection(&children) {
                return expr;
            }
            array_initializer_projection(children)
        }
    }
}

pub(crate) fn array2_from_elem_projection(
    rows: RustExpr,
    cols: RustExpr,
    default: RustExpr,
) -> RustExpr {
    function_call_projection(
        RustExpr::path(["ndarray", "Array2", "from_elem"]),
        vec![
            RustExpr::Tuple(vec![
                RustExpr::Cast {
                    expr: Box::new(array_dimension_cast_operand_projection(rows)),
                    ty: "usize".to_string(),
                },
                RustExpr::Cast {
                    expr: Box::new(array_dimension_cast_operand_projection(cols)),
                    ty: "usize".to_string(),
                },
            ]),
            default,
        ],
    )
}

/// `new T[rows][cols]` for a non-primitive `T`: rows of `Vec<T>`.
///
/// Matches what `new T[rows][]` followed by per-row allocation already
/// produced, and what the declared type says.
pub(crate) fn nested_array2_from_elem_projection(
    rows: RustExpr,
    cols: RustExpr,
    default: RustExpr,
    repeat_with_default: bool,
) -> RustExpr {
    let row = array_from_elem_projection_for_element_type(cols, default, repeat_with_default);
    if repeat_with_default {
        repeat_with_projection(row, rows)
    } else {
        RustExpr::VecRepeat {
            value: Box::new(row),
            len: Box::new(array_dimension_len_projection(rows)),
        }
    }
}

/// A fully-sized two-dimensional Java array stored in a ragged carrier.
///
/// A destination can require nullable rows after another assignment creates
/// the same array with an omitted inner dimension (`new T[n][]`).  A later
/// `new T[n][m]` still creates every row, so retain the destination carrier
/// while wrapping each newly-created row in `Some`.  `repeat_with` constructs
/// rows independently and therefore also works when their elements are not
/// `Clone`.
pub(crate) fn nested_array2_with_nullable_rows_projection(
    rows: RustExpr,
    cols: RustExpr,
    element_type: &JavaType,
    element_has_default: bool,
) -> Option<RustExpr> {
    let default = default_array_element_expr(element_type)
        .or_else(|| element_has_default.then(object_element_default_expr))?;
    let row = array_from_elem_projection_for_element_type(
        cols,
        default,
        nullable_array_default_needs_repeat_with(element_type),
    );
    let present_row = function_call_projection(RustExpr::path(["Some"]), vec![row]);
    Some(repeat_with_projection(present_row, rows))
}

/// A fully-sized rank-three primitive Java array stored as nullable matrices.
///
/// A field or local that is also assigned `new T[n][][]` needs the stable
/// carrier `Vec<Option<Array2<T>>>`.  A fully-sized assignment to that same
/// slot creates every matrix eagerly, so preserve the carrier by wrapping each
/// independently-created matrix in `Some`.
pub(crate) fn nested_array3_with_nullable_matrices_projection(
    planes: RustExpr,
    rows: RustExpr,
    cols: RustExpr,
    element_type: &JavaType,
    element_has_default: bool,
) -> Option<RustExpr> {
    if !matches!(element_type, JavaType::Primitive(_)) {
        return None;
    }
    let default = default_array_element_expr(element_type)
        .or_else(|| element_has_default.then(object_element_default_expr))?;
    let matrix = array2_from_elem_projection(rows, cols, default);
    let present_matrix = function_call_projection(RustExpr::path(["Some"]), vec![matrix]);
    Some(repeat_with_projection(present_matrix, planes))
}

/// A fully-sized Java array with three or more dimensions.
///
/// Rust's carrier for these arrays is a nested `Vec`, because Java arrays of
/// rank greater than two may later become ragged. Repeating from the inside
/// out preserves the dimension order. Nullable reference elements use
/// `repeat_with` at every level so allocation does not invent a `T: Clone`
/// requirement merely to repeat `None`-filled inner arrays.
fn nested_array_from_elem_projection(
    dimensions: &[RustExpr],
    default: RustExpr,
    repeat_with_default: bool,
) -> RustExpr {
    dimensions
        .iter()
        .rev()
        .cloned()
        .fold(default, |value, dimension| {
            array_from_elem_projection_for_element_type(dimension, value, repeat_with_default)
        })
}

#[cfg(test)]
fn array_from_elem_projection(len: RustExpr, default: RustExpr) -> RustExpr {
    array_from_elem_projection_for_element_type(len, default, false)
}

fn array_from_elem_projection_for_element_type(
    len: RustExpr,
    default: RustExpr,
    repeat_with_default: bool,
) -> RustExpr {
    if repeat_with_default {
        return repeat_with_projection(default, len);
    }
    RustExpr::VecRepeat {
        value: Box::new(default),
        len: Box::new(array_dimension_len_projection(len)),
    }
}

fn repeat_with_projection(default: RustExpr, len: RustExpr) -> RustExpr {
    let repeated = function_call_projection(
        RustExpr::path(["std", "iter", "repeat_with"]),
        vec![RustExpr::Closure {
            is_move: false,
            params: Vec::new(),
            return_type: None,
            body: RustClosureBody::Expr(Box::new(default)),
        }],
    );
    let taken = receiver_method_call_projection_exact(
        repeated,
        "take",
        vec![array_dimension_len_projection(len)],
    );
    receiver_method_call_projection_exact(taken, "collect::<Vec<_>>", Vec::new())
}

fn nullable_array_default_needs_repeat_with(element_type: &JavaType) -> bool {
    default_array_element_expr(element_type)
        .as_ref()
        .is_some_and(|default| matches!(default, RustExpr::Var(name) if name == "None"))
}

pub(crate) fn java_array_creation_fallback_projection(
    element_type: &JavaType,
    dimensions: &[RustExpr],
) -> RustExpr {
    // The marker text is what `count_invalid_patterns` looks for. It used to
    // read "Java array creation", which that detector does not match, so an
    // allocation that silently comes out with length 0 — every index into it
    // panics — counted as a clean translation. Both sides now read one
    // constant, so the text cannot drift again.
    RustExpr::UnsupportedJavaArrayCreation {
        element_type: format!("{element_type:?}"),
        dimensions: dimensions.to_vec(),
    }
}

pub(crate) fn borrowed_array2_temporary_for_slot(
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

pub(crate) fn array2_from_rows_projection(
    rows: RustExpr,
    ownership: OwnershipShape,
) -> Option<RustExpr> {
    let rows = recover_parseable_raw_operand_projection(rows);
    let call = function_call_projection(
        RustExpr::path(["ndarray", "Array2", "from_rows"]),
        vec![rows],
    );
    match ownership {
        OwnershipShape::SharedBorrow => Some(RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(call),
        }),
        OwnershipShape::Owned => Some(call),
        OwnershipShape::MutableBorrow | OwnershipShape::CopyValue => None,
    }
}

/// Materialize Java's nested-array carrier as the rectangular primitive
/// carrier required by a destination slot.
///
/// `new T[n][]` must initially retain nullable rows, even when every row is
/// subsequently filled.  Crossing into a method declared with an ordinary
/// rectangular `T[][]` ABI is the point where those row references can be
/// checked and collected into `Array2<T>`.
pub(crate) fn array2_from_nested_rows_projection(
    rows: RustExpr,
    rows_are_nullable: bool,
    ownership: OwnershipShape,
) -> Option<RustExpr> {
    let iter = receiver_method_call_projection_exact(rows, "iter", Vec::new());
    let row = RustExpr::Var("__java_row".to_string());
    let projected_row = if rows_are_nullable {
        let borrowed = receiver_method_call_projection_exact(row, "as_deref", Vec::new());
        receiver_method_call_projection_exact(
            borrowed,
            "ok_or_else",
            vec![RustExpr::Closure {
                is_move: false,
                return_type: None,
                params: Vec::new(),
                body: RustClosureBody::Expr(Box::new(
                    crate::exception_lowering::new_exception_expr(
                        "NullPointerException",
                        crate::coercion::rust_string_literal_projection("Java null array row"),
                    ),
                )),
            }],
        )
    } else {
        receiver_method_call_projection_exact(row, "as_slice", Vec::new())
    };
    let mapped = receiver_method_call_projection_exact(
        iter,
        "map",
        vec![RustExpr::Closure {
            is_move: false,
            return_type: None,
            params: vec![crate::rust_ir::RustClosureParam {
                pattern: "__java_row".to_string(),
                ty: None,
            }],
            body: RustClosureBody::Expr(Box::new(projected_row)),
        }],
    );
    let row_refs = if rows_are_nullable {
        RustExpr::Try(Box::new(receiver_method_call_projection_exact(
            mapped,
            "collect::<Result<Vec<_>, JavaException>>",
            Vec::new(),
        )))
    } else {
        receiver_method_call_projection_exact(mapped, "collect::<Vec<_>>", Vec::new())
    };
    array2_from_rows_projection(
        RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(row_refs),
        },
        ownership,
    )
}

pub(crate) fn array2_index_projection(array: RustExpr, row: RustExpr, col: RustExpr) -> RustExpr {
    RustExpr::Index(
        Box::new(postfix_receiver_projection(array)),
        Box::new(RustExpr::ArrayLiteral(vec![
            recover_parseable_raw_operand_projection(row),
            recover_parseable_raw_operand_projection(col),
        ])),
    )
}

pub(crate) fn array2_row_mut_assign_stmt(
    array: RustExpr,
    index: RustExpr,
    value: RustExpr,
) -> RustStmt {
    let row = receiver_method_call_projection_exact(
        postfix_receiver_projection(array),
        "row_mut",
        vec![recover_parseable_raw_operand_projection(index)],
    );
    let view = function_call_projection(
        RustExpr::path(["ndarray", "ArrayView1", "from"]),
        vec![recover_parseable_raw_operand_projection(value)],
    );
    RustStmt::Expr(receiver_method_call_projection_exact(
        row,
        "assign",
        vec![RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(view),
        }],
    ))
}

pub(crate) fn ragged_array_outer_creation_projection(len: RustExpr) -> RustExpr {
    RustExpr::VecRepeat {
        // A trailing, unspecified Java array dimension is not an empty inner
        // array. Each allocated outer slot contains null until assigned.
        value: Box::new(RustExpr::path(["None"])),
        len: Box::new(recover_parseable_raw_operand_projection(len)),
    }
}

fn array_dimension_len_projection(expr: RustExpr) -> RustExpr {
    RustExpr::Cast {
        expr: Box::new(array_dimension_cast_operand_projection(expr)),
        ty: "usize".to_string(),
    }
}

fn array_dimension_cast_operand_projection(expr: RustExpr) -> RustExpr {
    postfix_receiver_projection(expr)
}

fn array_initializer_projection(children: Vec<RustExpr>) -> RustExpr {
    RustExpr::VecLiteral(children)
}

fn array2_initializer_projection(children: &[RustExpr]) -> Option<RustExpr> {
    let rows = children
        .iter()
        .map(array2_initializer_row_items)
        .collect::<Option<Vec<_>>>()?;
    if rows.is_empty() {
        return None;
    }
    let width = rows.first()?.len();
    if width == 0 || rows.iter().any(|row| row.len() != width) {
        return None;
    }
    Some(function_call_projection(
        RustExpr::path(["ndarray", "arr2"]),
        vec![RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(RustExpr::ArrayLiteral(
                rows.into_iter().map(RustExpr::ArrayLiteral).collect(),
            )),
        }],
    ))
}

fn array2_initializer_row_items(row: &RustExpr) -> Option<Vec<RustExpr>> {
    match row {
        RustExpr::VecLiteral(items) | RustExpr::ArrayLiteral(items) => Some(items.clone()),
        RustExpr::Shaped { expr, .. } => array2_initializer_row_items(expr),
        // Slot-aware array lowering gives each nested row an explicit local
        // type so empty and generic Java initializers do not depend on Rust
        // inference.  That annotation is representation-neutral, but it used
        // to hide an otherwise rectangular primitive literal from the Array2
        // recognizer.  Recover only the structured block we emit ourselves;
        // arbitrary blocks must remain opaque because they may have effects.
        RustExpr::Block { stmts, tail }
            if matches!(
                (stmts.as_slice(), tail.as_deref()),
                ([RustStmt::Let {
                    mutable: false,
                    name,
                    value: Some(_),
                    ..
                }], Some(RustExpr::Var(tail_name)))
                    if name == "__java_typed_array" && tail_name == name
            ) =>
        {
            let [
                RustStmt::Let {
                    value: Some(value), ..
                },
            ] = stmts.as_slice()
            else {
                unreachable!("matched one typed-array let statement")
            };
            array2_initializer_row_items(value)
        }
        _ => None,
    }
}

pub(crate) fn default_array_element_expr(element_type: &JavaType) -> Option<RustExpr> {
    match element_type {
        JavaType::Primitive(PrimitiveType::Boolean) => Some(RustExpr::Literal("false".to_string())),
        JavaType::Primitive(PrimitiveType::Byte)
        | JavaType::Primitive(PrimitiveType::Short)
        | JavaType::Primitive(PrimitiveType::Int)
        | JavaType::Primitive(PrimitiveType::Long) => Some(RustExpr::Literal("0".to_string())),
        JavaType::Primitive(PrimitiveType::Float) | JavaType::Primitive(PrimitiveType::Double) => {
            Some(RustExpr::Literal("0.0".to_string()))
        }
        JavaType::Primitive(PrimitiveType::Char) => {
            Some(RustExpr::Literal("JavaCodeUnit(0u16)".to_string()))
        }
        ty if reference_array_element_is_nullable(ty) => Some(RustExpr::Var("None".to_string())),
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
fn object_element_default_expr() -> RustExpr {
    function_call_projection(RustExpr::path(["Default", "default"]), Vec::new())
}

fn reference_array_element_is_nullable(element_type: &JavaType) -> bool {
    !matches!(
        element_type,
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_)
    ) && crate::types::optional_element_type(element_type).is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::emit_expr;
    use crate::java_ir::{BinaryOp, Expr, Literal};

    fn array_expr(plan: &ArrayExprPlan<'_>, children: Vec<RustExpr>) -> RustExpr {
        array_expr_with_object_default(plan, children, false)
    }

    #[test]
    fn array_expr_builds_structured_array_values() {
        let sized_dimensions = [Expr::Var("len".to_string())];
        let sized_primitive_plan = ArrayExprPlan::Creation {
            element_type: &JavaType::Primitive(PrimitiveType::Byte),
            dimensions: &sized_dimensions,
        };
        let sized_primitive_array = array_expr(
            &sized_primitive_plan,
            vec![RustExpr::Var("len".to_string())],
        );
        assert_eq!(emit_expr(&sized_primitive_array), "vec![0; len as usize]");
        assert!(matches!(sized_primitive_array, RustExpr::VecRepeat { .. }));

        let typed_elements = [Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Var("prefix".to_string())),
            right: Box::new(Expr::Var("comment".to_string())),
        }];
        let typed_initializer_plan = ArrayExprPlan::TypedInitializer {
            element_type: &JavaType::String,
            elements: &typed_elements,
        };
        let typed_initializer_array = array_expr(
            &typed_initializer_plan,
            vec![RustExpr::Raw(
                "format!(\"{}{}\", prefix, comment)".to_string(),
            )],
        );
        assert_eq!(
            emit_expr(&typed_initializer_array),
            "vec![format!(\"{}{}\", prefix, comment)]"
        );
        assert!(matches!(typed_initializer_array, RustExpr::VecLiteral(_)));

        let dimensions = [Expr::Var("rows".to_string()), Expr::Var("cols".to_string())];
        let array2_creation = array_expr(
            &ArrayExprPlan::Creation {
                element_type: &JavaType::Primitive(PrimitiveType::Int),
                dimensions: &dimensions,
            },
            vec![
                RustExpr::Var("rows".to_string()),
                RustExpr::Var("cols".to_string()),
            ],
        );
        assert_eq!(
            emit_expr(&array2_creation),
            "ndarray::Array2::from_elem((rows as usize, cols as usize), 0)"
        );
        assert!(matches!(array2_creation, RustExpr::Call { .. }));

        let array3_creation = array_expr(
            &ArrayExprPlan::Creation {
                element_type: &JavaType::Primitive(PrimitiveType::Double),
                dimensions: &[
                    Expr::Var("planes".to_string()),
                    Expr::Var("rows".to_string()),
                    Expr::Var("cols".to_string()),
                ],
            },
            vec![
                RustExpr::Var("planes".to_string()),
                RustExpr::Var("rows".to_string()),
                RustExpr::Var("cols".to_string()),
            ],
        );
        assert_eq!(
            emit_expr(&array3_creation),
            "vec![vec![vec![0.0; cols as usize]; rows as usize]; planes as usize]"
        );
        assert!(matches!(array3_creation, RustExpr::VecRepeat { .. }));

        let initializer_elements = [
            Expr::Literal(Literal::Int(1)),
            Expr::Literal(Literal::Int(2)),
        ];
        let initializer_plan = ArrayExprPlan::Initializer {
            elements: &initializer_elements,
        };
        let initializer_array = array_expr(
            &initializer_plan,
            vec![
                RustExpr::Literal("1".to_string()),
                RustExpr::Literal("2".to_string()),
            ],
        );
        assert_eq!(emit_expr(&initializer_array), "vec![1, 2]");
        assert!(matches!(initializer_array, RustExpr::VecLiteral(_)));
    }

    #[test]
    fn array2_from_rows_projection_owns_static_matrix_handoff_spelling() {
        fn is_usize_cast_of_var(expr: &RustExpr, expected: &str) -> bool {
            matches!(
                expr,
                RustExpr::Cast { expr, ty }
                    if ty == "usize"
                        && matches!(expr.as_ref(), RustExpr::Var(name) if name == expected)
            )
        }

        let structured_from_elem = array2_from_elem_projection(
            RustExpr::Var("rows".to_string()),
            RustExpr::Var("cols".to_string()),
            RustExpr::Literal("0".to_string()),
        );
        assert_eq!(
            emit_expr(&structured_from_elem),
            "ndarray::Array2::from_elem((rows as usize, cols as usize), 0)"
        );
        assert!(matches!(structured_from_elem, RustExpr::Call { .. }));
        let structured_from_elem_with_raw_dimension = array2_from_elem_projection(
            RustExpr::Raw("rows".to_string()),
            RustExpr::Var("cols".to_string()),
            RustExpr::Literal("0".to_string()),
        );
        assert_eq!(
            emit_expr(&structured_from_elem_with_raw_dimension),
            "ndarray::Array2::from_elem((rows as usize, cols as usize), 0)"
        );
        assert!(matches!(
            structured_from_elem_with_raw_dimension,
            RustExpr::Call { .. }
        ));
        let structured_from_elem_with_raw_binary_dimension = array2_from_elem_projection(
            RustExpr::Raw("rows + extra".to_string()),
            RustExpr::Raw("cols + extra".to_string()),
            RustExpr::Literal("0".to_string()),
        );
        assert_eq!(
            emit_expr(&structured_from_elem_with_raw_binary_dimension),
            "ndarray::Array2::from_elem(((rows + extra) as usize, (cols + extra) as usize), 0)"
        );
        let structured_vec_repeat = array_from_elem_projection(
            RustExpr::Var("len".to_string()),
            RustExpr::Literal("0".to_string()),
        );
        assert_eq!(emit_expr(&structured_vec_repeat), "vec![0; len as usize]");
        assert!(matches!(
            structured_vec_repeat,
            RustExpr::VecRepeat { value, .. } if matches!(*value, RustExpr::Literal(_))
        ));
        let structured_vec_repeat_with_raw_binary_dimension = array_from_elem_projection(
            RustExpr::Raw("len + extra".to_string()),
            RustExpr::Literal("0".to_string()),
        );
        assert_eq!(
            emit_expr(&structured_vec_repeat_with_raw_binary_dimension),
            "vec![0; (len + extra) as usize]"
        );
        let fallback = java_array_creation_fallback_projection(
            &JavaType::Unknown("Node".to_string()),
            &[
                RustExpr::Var("len".to_string()),
                RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Var("left".to_string())),
                    right: Box::new(RustExpr::Var("right".to_string())),
                },
            ],
        );
        let rendered_fallback = emit_expr(&fallback);
        assert!(rendered_fallback.contains(crate::output_markers::UNSUPPORTED_JAVA_ARRAY_CREATION));
        assert!(rendered_fallback.contains("Vec::new()"));
        assert!(matches!(
            fallback,
            RustExpr::UnsupportedJavaArrayCreation { dimensions, .. }
                if matches!(
                    dimensions.as_slice(),
                    [RustExpr::Var(name), RustExpr::Binary { op, .. }]
                        if name == "len" && op == "+"
                )
        ));
        let structured_nested_repeat = nested_array2_from_elem_projection(
            RustExpr::Var("rows".to_string()),
            RustExpr::Var("cols".to_string()),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["String", "new"])),
                args: Vec::new(),
            },
            false,
        );
        assert_eq!(
            emit_expr(&structured_nested_repeat),
            "vec![vec![String::new(); cols as usize]; rows as usize]"
        );
        assert_eq!(
            emit_expr(&nested_array2_from_elem_projection(
                RustExpr::Raw("rows + extra".to_string()),
                RustExpr::Raw("cols + extra".to_string()),
                RustExpr::Call {
                    function: Box::new(RustExpr::path(["String", "new"])),
                    args: Vec::new(),
                },
                false,
            )),
            "vec![vec![String::new(); (cols + extra) as usize]; (rows + extra) as usize]"
        );
        assert_eq!(
            emit_expr(&array_from_elem_projection_for_element_type(
                RustExpr::Var("len".to_string()),
                RustExpr::Var("None".to_string()),
                true,
            )),
            "std::iter::repeat_with(|| None).take(len as usize).collect::<Vec<_>>()"
        );
        let RustExpr::VecRepeat { value, .. } = structured_nested_repeat else {
            panic!("nested array default should be an outer VecRepeat");
        };
        let RustExpr::VecRepeat { value, .. } = *value else {
            panic!("nested array default should be an inner VecRepeat");
        };
        assert!(matches!(*value, RustExpr::Call { .. }));

        let array2_initializer = array2_initializer_projection(&[
            RustExpr::VecLiteral(vec![
                RustExpr::Literal("1f64".to_string()),
                RustExpr::Binary {
                    op: "+".to_string(),
                    left: Box::new(RustExpr::Literal("1f64".to_string())),
                    right: Box::new(RustExpr::Literal("1f64".to_string())),
                },
            ]),
            RustExpr::ArrayLiteral(vec![
                RustExpr::Call {
                    function: Box::new(RustExpr::Var("third".to_string())),
                    args: Vec::new(),
                },
                RustExpr::Literal("4f64".to_string()),
            ]),
        ])
        .expect("rectangular structural rows should lower to ndarray::arr2");
        assert_eq!(
            emit_expr(&array2_initializer),
            "ndarray::arr2(&[[1f64, 1f64 + 1f64], [third(), 4f64]])"
        );
        assert!(matches!(array2_initializer, RustExpr::Call { .. }));
        assert!(
            array2_initializer_projection(&[
                RustExpr::VecLiteral(vec![RustExpr::Literal("1".to_string())]),
                RustExpr::VecLiteral(vec![
                    RustExpr::Literal("2".to_string()),
                    RustExpr::Literal("3".to_string()),
                ]),
            ])
            .is_none(),
            "ragged structural rows should not lower to ndarray::arr2"
        );
        assert!(
            array2_initializer_projection(&[
                RustExpr::Raw("// skipped".to_string()),
                RustExpr::VecLiteral(vec![RustExpr::Literal("1".to_string())]),
            ])
            .is_none(),
            "comment-only raw expressions should not be treated as nonsemantic array rows"
        );
        assert!(
            array2_initializer_projection(&[RustExpr::Raw("vec![1, 2]".to_string())]).is_none(),
            "raw vec spelling should not be parsed back into ndarray::arr2"
        );

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
            } if op == "&"
                && matches!(
                    expr.as_ref(),
                    RustExpr::Call { args, .. }
                        if matches!(args.as_slice(), [RustExpr::Path(path)]
                            if path == &vec!["Self".to_string(), "COEF".to_string()])
                )
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
        assert!(matches!(
            raw_rows_owned,
            RustExpr::Call { args, .. }
                if matches!(args.as_slice(), [RustExpr::Path(path)]
                    if path == &vec!["Self".to_string(), "COEF".to_string()])
        ));
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
        let nullable_rows = array2_from_nested_rows_projection(
            RustExpr::Var("rows".to_string()),
            true,
            OwnershipShape::Owned,
        )
        .expect("owned rectangular handoff");
        let nullable_rows = emit_expr(&nullable_rows);
        assert!(nullable_rows.starts_with("ndarray::Array2::from_rows(&rows.iter().map("));
        assert!(nullable_rows.contains("__java_row.as_deref().ok_or_else("));
        assert!(nullable_rows.contains("collect::<Result<Vec<_>, JavaException>>()?"));
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
            RustExpr::Index(_, ref index)
                if matches!(
                    index.as_ref(),
                    RustExpr::ArrayLiteral(items)
                        if matches!(items.as_slice(), [row, col]
                            if is_usize_cast_of_var(row, "row")
                                && is_usize_cast_of_var(col, "col"))
                )
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
        assert!(matches!(
            raw_receiver_index,
            RustExpr::Index(receiver, index)
                if matches!(receiver.as_ref(), RustExpr::Var(name) if name == "values")
                    && matches!(
                        index.as_ref(),
                        RustExpr::ArrayLiteral(items)
                            if matches!(items.as_slice(), [row, col]
                                if is_usize_cast_of_var(row, "row")
                                    && is_usize_cast_of_var(col, "col"))
                    )
        ));
        let compound_raw_receiver_index = array2_index_projection(
            RustExpr::Raw("left + right".to_string()),
            RustExpr::Raw("row as usize".to_string()),
            RustExpr::Raw("col as usize".to_string()),
        );
        assert_eq!(
            emit_expr(&compound_raw_receiver_index),
            "(left + right)[[row as usize, col as usize]]"
        );
        assert!(matches!(
            compound_raw_receiver_index,
            RustExpr::Index(receiver, index)
                if matches!(receiver.as_ref(), RustExpr::Grouped(grouped)
                    if matches!(grouped.as_ref(), RustExpr::Binary { op, .. } if op == "+"))
                    && matches!(
                        index.as_ref(),
                        RustExpr::ArrayLiteral(items)
                            if matches!(items.as_slice(), [row, col]
                                if is_usize_cast_of_var(row, "row")
                                    && is_usize_cast_of_var(col, "col"))
                    )
        ));
        let raw_cast_receiver_index = array2_index_projection(
            RustExpr::Raw("values as ndarray::Array2<i32>".to_string()),
            RustExpr::Var("row".to_string()),
            RustExpr::Var("col".to_string()),
        );
        assert_eq!(
            emit_expr(&raw_cast_receiver_index),
            "(values as ndarray::Array2<i32>)[[row, col]]"
        );
        assert!(matches!(
            raw_cast_receiver_index,
            RustExpr::Index(receiver, _)
                if matches!(receiver.as_ref(), RustExpr::Grouped(grouped)
                    if matches!(
                        grouped.as_ref(),
                        RustExpr::Cast { expr, ty }
                            if ty == "ndarray::Array2<i32>"
                                && matches!(expr.as_ref(), RustExpr::Var(name) if name == "values")
                    ))
        ));
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
            RustExpr::MethodCall { receiver, .. }
                if matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { args, .. }
                        if matches!(args.as_slice(), [index] if is_usize_cast_of_var(index, "row"))
                )
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
        assert!(matches!(
            structured_row_assign_with_raw_value_expr,
            RustExpr::MethodCall { receiver, args, .. }
                if matches!(
                    receiver.as_ref(),
                    RustExpr::MethodCall { args, .. }
                        if matches!(args.as_slice(), [index] if !matches!(index, RustExpr::Raw(_)))
                )
                    && matches!(
                        args.as_slice(),
                        [RustExpr::Unary { op, expr }]
                            if op == "&"
                                && matches!(expr.as_ref(), RustExpr::Call { args, .. }
                                    if matches!(args.as_slice(), [value]
                                        if !matches!(value, RustExpr::Raw(_))))
                    )
        ));
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
        let structured_row_assign_with_raw_receiver = array2_row_mut_assign_stmt(
            RustExpr::Raw("left + right".to_string()),
            RustExpr::Var("row".to_string()),
            RustExpr::Var("replacement".to_string()),
        );
        let RustStmt::Expr(structured_row_assign_with_raw_receiver_expr) =
            structured_row_assign_with_raw_receiver
        else {
            panic!("expected structured row assignment expression for raw receiver");
        };
        assert_eq!(
            emit_expr(&structured_row_assign_with_raw_receiver_expr),
            "(left + right).row_mut(row).assign(&ndarray::ArrayView1::from(replacement))"
        );
        assert!(matches!(
            structured_row_assign_with_raw_receiver_expr,
            RustExpr::MethodCall { receiver, .. }
                if matches!(receiver.as_ref(), RustExpr::MethodCall { receiver, .. }
                    if matches!(receiver.as_ref(), RustExpr::Grouped(grouped)
                        if matches!(grouped.as_ref(), RustExpr::Binary { op, .. } if op == "+")))
        ));
        assert_eq!(
            emit_expr(&ragged_array_outer_creation_projection(RustExpr::Raw(
                "base_names.len() as i32 as usize".to_string()
            ))),
            "vec![None; base_names.len() as i32 as usize]"
        );
        let recovered_ragged_len =
            ragged_array_outer_creation_projection(RustExpr::Raw("left + right".to_string()));
        assert_eq!(emit_expr(&recovered_ragged_len), "vec![None; left + right]");
        assert!(matches!(
            recovered_ragged_len,
            RustExpr::VecRepeat { len, .. }
                if matches!(len.as_ref(), RustExpr::Binary { op, .. } if op == "+")
        ));
        assert!(matches!(
            ragged_array_outer_creation_projection(RustExpr::Var("len".to_string())),
            RustExpr::VecRepeat { .. }
        ));
    }
}
