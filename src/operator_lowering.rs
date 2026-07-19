use crate::coercion::{Coercion, apply_coercions};
use crate::facts::FactSet;
use crate::java_ir::{BinaryOp, JavaType, PrimitiveType, UnaryOp};
use crate::rust_ir::RustExpr;
use crate::slot_shape::RustSlotShape;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EqualityComparisonPlan {
    Direct,
    BothOptionalByRef,
    LeftOptionalByRef,
    RightOptionalByRef,
    LeftOptionalStringView,
    RightOptionalStringView,
    BothOptionalCustomEquals { method_name: String },
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
    crate::expr_render::binary_expr(
        op,
        apply_coercions(left, &plan.left),
        apply_coercions(right, &plan.right),
    )
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
            EqualityComparisonPlan::LeftOptionalStringView
        }
        (None, Some(right_element))
            if right_element == JavaType::String && is_borrowed_string_view(left) =>
        {
            EqualityComparisonPlan::RightOptionalStringView
        }
        (Some(left_element), None) if left_element == right.java_ty => {
            EqualityComparisonPlan::LeftOptionalByRef
        }
        (None, Some(right_element)) if left.java_ty == right_element => {
            EqualityComparisonPlan::RightOptionalByRef
        }
        _ => EqualityComparisonPlan::Direct,
    }
}

fn is_borrowed_string_view(shape: &RustSlotShape) -> bool {
    shape.java_ty == JavaType::String
        && matches!(
            shape.ownership,
            crate::slot_shape::OwnershipShape::SharedBorrow
        )
        && matches!(shape.view, crate::slot_shape::ViewShape::Str)
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
        EqualityComparisonPlan::Direct => RustExpr::Binary {
            op: op.to_string(),
            left: Box::new(left),
            right: Box::new(right),
        },
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
        EqualityComparisonPlan::LeftOptionalStringView => RustExpr::Raw(format!(
            "{} {op} Some({})",
            crate::emit::emit_expr(&option_as_deref(left)),
            crate::emit::emit_expr(&right)
        )),
        EqualityComparisonPlan::RightOptionalStringView => RustExpr::Raw(format!(
            "Some({}) {op} {}",
            crate::emit::emit_expr(&left),
            crate::emit::emit_expr(&option_as_deref(right))
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

fn option_as_ref(expr: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "as_ref".to_string(),
        args: Vec::new(),
    }
}

fn option_as_deref(expr: RustExpr) -> RustExpr {
    RustExpr::MethodCall {
        receiver: Box::new(expr),
        method: "as_deref".to_string(),
        args: Vec::new(),
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
