use crate::coercion::{
    Coercion, apply_coercions, optional_string_view_projection,
    render_string_equality_operand_projection,
};
use crate::facts::FactSet;
use crate::java_ir::{BinaryOp, JavaType, PrimitiveType, UnaryOp};
use crate::rust_ir::RustExpr;
use crate::shape_projection::shape_is_borrowed_string_content;
use crate::slot_shape::{OwnershipShape, RustSlotShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EqualityComparisonPlan {
    Direct,
    StringView {
        left_view: StringEqualityOperandView,
        right_view: StringEqualityOperandView,
    },
    BothOptionalByRef,
    LeftOptionalByRef,
    RightOptionalByRef,
    LeftOptionalStringView {
        left_view: OptionalStringEqualityOperandView,
    },
    RightOptionalStringView {
        right_view: OptionalStringEqualityOperandView,
    },
    LeftSharedBorrowRightOwned,
    LeftOwnedRightSharedBorrow,
    BothOptionalCustomEquals {
        method_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringEqualityOperandView {
    NativeString,
    BorrowedStr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalStringEqualityOperandView {
    NativeString,
    BorrowedStr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullComparisonMethod {
    IsNone,
    IsSome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericBinaryPromotionPlan {
    pub promoted: PrimitiveType,
    pub left: Vec<Coercion>,
    pub right: Vec<Coercion>,
}

pub fn binary_op_to_rust(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Assign => "=",
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::UShr => ">>>",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

pub fn plan_numeric_binary_promotion(
    op: BinaryOp,
    left: &JavaType,
    right: &JavaType,
) -> Option<NumericBinaryPromotionPlan> {
    if !matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr
            | BinaryOp::UShr
            | BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
    ) {
        return None;
    }
    let (JavaType::Primitive(left), JavaType::Primitive(right)) = (left, right) else {
        return None;
    };
    if !crate::types::is_numeric_primitive_type(*left)
        || !crate::types::is_numeric_primitive_type(*right)
    {
        return None;
    }
    if matches!(op, BinaryOp::Shl | BinaryOp::Shr | BinaryOp::UShr) {
        let promoted_left = shift_left_promoted_primitive(*left);
        let promoted_right = shift_right_promoted_primitive(*right);
        return Some(NumericBinaryPromotionPlan {
            promoted: promoted_left,
            left: primitive_operand_coercions(*left, promoted_left),
            right: primitive_operand_coercions(*right, promoted_right),
        });
    }
    let promoted = promoted_numeric_primitive(op, *left, *right);
    Some(NumericBinaryPromotionPlan {
        promoted,
        left: primitive_operand_coercions(*left, promoted),
        right: primitive_operand_coercions(*right, promoted),
    })
}

pub fn lower_numeric_binary_expr(
    op: BinaryOp,
    left: RustExpr,
    right: RustExpr,
    plan: NumericBinaryPromotionPlan,
) -> RustExpr {
    if op == BinaryOp::UShr {
        return lower_unsigned_right_shift_expr(left, right, plan);
    }
    if let Some(expr) = lower_java_integer_binary_expr(op, &left, &right, &plan) {
        return expr;
    }
    crate::expr_render::binary_expr(
        op,
        apply_coercions(left, &plan.left),
        apply_coercions(right, &plan.right),
    )
}

pub fn lower_checked_java_integer_div_rem_expr(
    op: BinaryOp,
    left: RustExpr,
    right: RustExpr,
    plan: NumericBinaryPromotionPlan,
) -> Option<RustExpr> {
    if !matches!(op, BinaryOp::Div | BinaryOp::Rem)
        || !matches!(plan.promoted, PrimitiveType::Int | PrimitiveType::Long)
    {
        return None;
    }
    let method = match op {
        BinaryOp::Div => "wrapping_div",
        BinaryOp::Rem => "wrapping_rem",
        _ => unreachable!("checked above"),
    };
    let left = integer_wrapping_receiver_text(
        crate::emit::emit_expr(&apply_coercions(left, &plan.left)),
        plan.promoted,
    );
    let right = crate::emit::emit_expr(&apply_coercions(right, &plan.right));
    Some(RustExpr::Raw(format!(
        "{{ let __java_left = {left}; let __java_right = {right}; if __java_right == 0 {{ Err(JavaException::new(\"ArithmeticException\", \"/ by zero\".to_string())) }} else {{ Ok((__java_left).{method}(__java_right)) }} }}"
    )))
}

fn lower_java_integer_binary_expr(
    op: BinaryOp,
    left: &RustExpr,
    right: &RustExpr,
    plan: &NumericBinaryPromotionPlan,
) -> Option<RustExpr> {
    if !matches!(plan.promoted, PrimitiveType::Int | PrimitiveType::Long) {
        return None;
    }
    let method = match op {
        BinaryOp::Add => "wrapping_add",
        BinaryOp::Sub => "wrapping_sub",
        BinaryOp::Mul => "wrapping_mul",
        BinaryOp::Shl => "wrapping_shl",
        BinaryOp::Shr => "wrapping_shr",
        _ => return None,
    };
    let left = integer_wrapping_receiver_text(
        crate::emit::emit_expr(&apply_coercions(left.clone(), &plan.left)),
        plan.promoted,
    );
    let right = crate::emit::emit_expr(&apply_coercions(right.clone(), &plan.right));
    if matches!(op, BinaryOp::Shl | BinaryOp::Shr) {
        return Some(RustExpr::Raw(format!(
            "({left}).{method}(({right}) as u32)"
        )));
    }
    Some(RustExpr::Raw(format!("({left}).{method}({right})")))
}

fn integer_wrapping_receiver_text(rendered: String, primitive: PrimitiveType) -> String {
    if !rendered.chars().all(|ch| ch.is_ascii_digit()) {
        return rendered;
    }
    match primitive {
        PrimitiveType::Long => format!("{rendered}i64"),
        PrimitiveType::Int => format!("{rendered}i32"),
        _ => rendered,
    }
}

pub fn lower_unsigned_right_shift_expr(
    left: RustExpr,
    right: RustExpr,
    plan: NumericBinaryPromotionPlan,
) -> RustExpr {
    let (unsigned_ty, signed_ty) = match plan.promoted {
        PrimitiveType::Long => ("u64", "i64"),
        _ => ("u32", "i32"),
    };
    let left = crate::emit::emit_expr(&apply_coercions(left, &plan.left));
    let right = crate::emit::emit_expr(&apply_coercions(right, &plan.right));
    RustExpr::Raw(format!(
        "(({left} as {unsigned_ty}).wrapping_shr(({right}) as u32)) as {signed_ty}"
    ))
}

fn promoted_numeric_primitive(
    op: BinaryOp,
    left: PrimitiveType,
    right: PrimitiveType,
) -> PrimitiveType {
    use PrimitiveType::*;
    if matches!(op, BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor) {
        return if matches!(left, Long) || matches!(right, Long) {
            Long
        } else {
            Int
        };
    }
    let rank = |primitive| match primitive {
        Double => 5,
        Float => 4,
        Long => 3,
        Int | Short | Byte | Char => 2,
        Boolean => 0,
    };
    match rank(left).max(rank(right)) {
        5 => Double,
        4 => Float,
        3 => Long,
        _ => Int,
    }
}

fn shift_left_promoted_primitive(primitive: PrimitiveType) -> PrimitiveType {
    use PrimitiveType::*;
    if matches!(primitive, Long) { Long } else { Int }
}

fn shift_right_promoted_primitive(primitive: PrimitiveType) -> PrimitiveType {
    use PrimitiveType::*;
    if matches!(primitive, Byte | Short | Char) {
        Int
    } else {
        primitive
    }
}

fn primitive_operand_coercions(from: PrimitiveType, to: PrimitiveType) -> Vec<Coercion> {
    if from == to {
        Vec::new()
    } else {
        vec![Coercion::WidenPrimitive { from, to }]
    }
}

pub fn null_comparison_method(op: BinaryOp, null_on_left: bool) -> Option<NullComparisonMethod> {
    match (op, null_on_left) {
        (BinaryOp::Eq, _) => Some(NullComparisonMethod::IsNone),
        (BinaryOp::Ne, _) => Some(NullComparisonMethod::IsSome),
        _ => None,
    }
}

pub fn lower_null_comparison_expr(
    op: BinaryOp,
    value: RustExpr,
    null_on_left: bool,
) -> Option<RustExpr> {
    let method = match null_comparison_method(op, null_on_left)? {
        NullComparisonMethod::IsNone => "is_none",
        NullComparisonMethod::IsSome => "is_some",
    };
    Some(RustExpr::MethodCall {
        receiver: Box::new(value),
        method: method.to_string(),
        args: Vec::new(),
    })
}

pub fn plan_equality_comparison(
    left: &RustSlotShape,
    right: &RustSlotShape,
) -> EqualityComparisonPlan {
    plan_equality_comparison_with_facts(left, right, &FactSet::default())
}

pub fn plan_equality_comparison_with_facts(
    left: &RustSlotShape,
    right: &RustSlotShape,
    facts: &FactSet,
) -> EqualityComparisonPlan {
    if left.optional_element_type().is_none()
        && right.optional_element_type().is_none()
        && left.java_ty == JavaType::String
        && right.java_ty == JavaType::String
    {
        return EqualityComparisonPlan::StringView {
            left_view: string_equality_operand_view(left),
            right_view: string_equality_operand_view(right),
        };
    }
    match (left.optional_element_type(), right.optional_element_type()) {
        (Some(left_element), Some(right_element)) if left_element == right_element => {
            if let Some(method_name) = custom_equals_method(&left_element, facts) {
                return EqualityComparisonPlan::BothOptionalCustomEquals { method_name };
            }
            EqualityComparisonPlan::BothOptionalByRef
        }
        (Some(left_element), None)
            if left_element == JavaType::String && is_borrowed_string_view(right) =>
        {
            EqualityComparisonPlan::LeftOptionalStringView {
                left_view: optional_string_equality_operand_view(left, left_element),
            }
        }
        (None, Some(right_element))
            if right_element == JavaType::String && is_borrowed_string_view(left) =>
        {
            EqualityComparisonPlan::RightOptionalStringView {
                right_view: optional_string_equality_operand_view(right, right_element),
            }
        }
        (Some(left_element), None) if left_element == right.java_ty => {
            EqualityComparisonPlan::LeftOptionalByRef
        }
        (None, Some(right_element)) if left.java_ty == right_element => {
            EqualityComparisonPlan::RightOptionalByRef
        }
        _ if left.java_ty == right.java_ty
            && left.ownership == OwnershipShape::SharedBorrow
            && right.ownership != OwnershipShape::SharedBorrow =>
        {
            EqualityComparisonPlan::LeftSharedBorrowRightOwned
        }
        _ if left.java_ty == right.java_ty
            && left.ownership != OwnershipShape::SharedBorrow
            && right.ownership == OwnershipShape::SharedBorrow =>
        {
            EqualityComparisonPlan::LeftOwnedRightSharedBorrow
        }
        _ => EqualityComparisonPlan::Direct,
    }
}

fn string_equality_operand_view(shape: &RustSlotShape) -> StringEqualityOperandView {
    if shape_is_borrowed_string_content(shape) {
        StringEqualityOperandView::BorrowedStr
    } else {
        StringEqualityOperandView::NativeString
    }
}

fn optional_string_equality_operand_view(
    shape: &RustSlotShape,
    element_ty: JavaType,
) -> OptionalStringEqualityOperandView {
    let element_shape = crate::conversion_plan::optional_element_slot_shape(shape, element_ty);
    if shape_is_borrowed_string_content(&element_shape) {
        OptionalStringEqualityOperandView::BorrowedStr
    } else {
        OptionalStringEqualityOperandView::NativeString
    }
}

fn is_borrowed_string_view(shape: &RustSlotShape) -> bool {
    shape_is_borrowed_string_content(shape)
}

fn custom_equals_method(java_ty: &JavaType, facts: &FactSet) -> Option<String> {
    let JavaType::Named { name, .. } = java_ty else {
        return None;
    };
    facts
        .method_effect_for(name, "equals")
        .map(|_| "equals".to_string())
}

pub fn lower_equality_comparison_expr(
    plan: EqualityComparisonPlan,
    left: RustExpr,
    right: RustExpr,
) -> RustExpr {
    lower_equality_comparison_expr_for_op(BinaryOp::Eq, plan, left, right)
}

pub fn lower_equality_comparison_expr_for_op(
    op: BinaryOp,
    plan: EqualityComparisonPlan,
    left: RustExpr,
    right: RustExpr,
) -> RustExpr {
    let op = match op {
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        _ => "==",
    };
    match plan {
        EqualityComparisonPlan::Direct => RustExpr::Raw(format!(
            "({} {op} {})",
            crate::emit::emit_expr(&left),
            crate::emit::emit_expr(&right)
        )),
        EqualityComparisonPlan::StringView {
            left_view,
            right_view,
        } => RustExpr::Raw(format!(
            "({} {op} {})",
            string_equality_operand_expr(left, left_view),
            string_equality_operand_expr(right, right_view),
        )),
        EqualityComparisonPlan::BothOptionalByRef => RustExpr::Binary {
            op: op.to_string(),
            left: Box::new(option_as_ref(left)),
            right: Box::new(option_as_ref(right)),
        },
        EqualityComparisonPlan::LeftOptionalByRef => RustExpr::Raw(format!(
            "{} {op} Some(&{})",
            crate::emit::emit_expr(&option_as_ref(left)),
            crate::emit::emit_expr(&right)
        )),
        EqualityComparisonPlan::RightOptionalByRef => RustExpr::Raw(format!(
            "Some(&{}) {op} {}",
            crate::emit::emit_expr(&left),
            crate::emit::emit_expr(&option_as_ref(right))
        )),
        EqualityComparisonPlan::LeftOptionalStringView { left_view } => RustExpr::Raw(format!(
            "{} {op} Some({})",
            crate::emit::emit_expr(&option_as_string_view(left, left_view)),
            crate::emit::emit_expr(&right)
        )),
        EqualityComparisonPlan::RightOptionalStringView { right_view } => RustExpr::Raw(format!(
            "Some({}) {op} {}",
            crate::emit::emit_expr(&left),
            crate::emit::emit_expr(&option_as_string_view(right, right_view))
        )),
        EqualityComparisonPlan::LeftSharedBorrowRightOwned => RustExpr::Raw(format!(
            "({} {op} &{})",
            crate::emit::emit_expr(&left),
            crate::emit::emit_expr(&right)
        )),
        EqualityComparisonPlan::LeftOwnedRightSharedBorrow => RustExpr::Raw(format!(
            "(&{} {op} {})",
            crate::emit::emit_expr(&left),
            crate::emit::emit_expr(&right)
        )),
        EqualityComparisonPlan::BothOptionalCustomEquals { method_name } => {
            let true_expr = matches!(op, "==");
            let false_expr = !true_expr;
            RustExpr::Raw(format!(
                "match ({}, {}) {{ (Some(left), Some(right)) => left.{method_name}(right), (None, None) => {true_expr}, _ => {false_expr} }}",
                crate::emit::emit_expr(&left),
                crate::emit::emit_expr(&right),
            ))
        }
    }
}

fn string_equality_operand_expr(expr: RustExpr, view: StringEqualityOperandView) -> String {
    render_string_equality_operand_projection(
        expr,
        matches!(view, StringEqualityOperandView::BorrowedStr),
    )
}

fn option_as_ref(expr: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "as_ref".to_string(),
        args: Vec::new(),
    }
}

fn option_as_string_view(expr: RustExpr, view: OptionalStringEqualityOperandView) -> RustExpr {
    match view {
        OptionalStringEqualityOperandView::NativeString => optional_string_view_projection(expr),
        OptionalStringEqualityOperandView::BorrowedStr => expr,
    }
}

pub fn lower_unary_expr(op: UnaryOp, operand: RustExpr) -> RustExpr {
    match op {
        UnaryOp::PostInc => RustExpr::Raw(format!("{} += 1", crate::emit::emit_expr(&operand))),
        UnaryOp::PostDec => RustExpr::Raw(format!("{} -= 1", crate::emit::emit_expr(&operand))),
        UnaryOp::Not => RustExpr::Unary {
            op: "!".to_string(),
            expr: Box::new(operand),
        },
        UnaryOp::Neg => RustExpr::Unary {
            op: "-".to_string(),
            expr: Box::new(operand),
        },
        UnaryOp::Pos => operand,
        UnaryOp::PreInc | UnaryOp::PreDec => RustExpr::Raw(crate::emit::emit_expr(&operand)),
    }
}

pub fn stream_unary_prefix(op: UnaryOp) -> Option<&'static str> {
    match op {
        UnaryOp::Not => Some("!"),
        UnaryOp::Neg => Some("-"),
        UnaryOp::Pos => Some(""),
        _ => None,
    }
}
