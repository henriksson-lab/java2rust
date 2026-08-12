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
    /// Java reference identity on a type stored behind a shared handle
    /// ([0028](../claude_audits/done/0028-reference-equality-lowered-as-structural-equality.md)).
    ///
    /// `==` on `Rc<T>` is *structural* — `Rc`'s `PartialEq` delegates to `T` —
    /// so two distinct objects with equal fields compared equal where Java says
    /// they differ. `ptr_eq` asks the question Java asks.
    ///
    /// `handle_path` rather than a hardcoded `Rc`: a type reachable from a Java
    /// `static` is stored in an `Arc`, and the placement already decides which.
    /// The `*_borrowed` flags say whether an operand is already a reference, so
    /// `ptr_eq` gets `&Rc<T>` either way; swapping them is what makes this arm
    /// its own mirror.
    SharedHandleIdentity {
        handle_path: String,
        left_borrowed: bool,
        right_borrowed: bool,
    },
    /// Identity where one side may be null. Java's `null == null` is `true`, and
    /// a null never equals a live reference.
    LeftOptionalSharedHandleIdentity {
        handle_path: String,
        right_borrowed: bool,
    },
    RightOptionalSharedHandleIdentity {
        handle_path: String,
        left_borrowed: bool,
    },
    BothOptionalSharedHandleIdentity {
        handle_path: String,
    },
    CustomEquals {
        method_name: String,
        right_borrowed: bool,
        erased_arg_handle_path: Option<String>,
    },
    BothOptionalCustomEquals {
        method_name: String,
        erased_arg_handle_path: Option<String>,
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
    let temp_ty = match plan.promoted {
        PrimitiveType::Long => "i64",
        _ => "i32",
    };
    Some(RustExpr::Raw(format!(
        "{{ let __java_left: {temp_ty} = {left}; let __java_right: {temp_ty} = {right}; if __java_right == 0 {{ Err(JavaException::new(\"ArithmeticException\", \"/ by zero\".to_string())) }} else {{ Ok((__java_left).{method}(__java_right)) }} }}"
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

/// Pin an integer literal receiver to a concrete type.
///
/// `(-1).wrapping_shr(..)` does not compile: the literal stays `{integer}` and
/// the method is ambiguous. A leading `-` has to be accepted, not just digits,
/// or every negative literal keeps that ambiguity.
fn integer_wrapping_receiver_text(rendered: String, primitive: PrimitiveType) -> String {
    let digits = rendered.strip_prefix('-').unwrap_or(&rendered);
    if digits.is_empty() || !digits.chars().all(|ch| ch.is_ascii_digit()) {
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
    // Pin the operand's type before the cast. `(-1) as u32` infers the literal
    // as `u32` and then negates it; `(-1i32) as u32` is the reinterpretation
    // Java's `>>>` actually means.
    let left = integer_wrapping_receiver_text(
        crate::emit::emit_expr(&apply_coercions(left, &plan.left)),
        plan.promoted,
    );
    // `as` binds tighter than unary minus, so `-x as u32` is `-(x as u32)`. Only
    // a negated operand needs the grouping; adding it unconditionally would just
    // make every ordinary shift noisier.
    let left = if left.starts_with('-') {
        format!("({left})")
    } else {
        left
    };
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

impl EqualityComparisonPlan {
    /// The plan for the same comparison with the operands swapped.
    ///
    /// Java `==` and `.equals()` are symmetric, so this is an **involution**:
    /// `p.mirrored().mirrored() == p`. Three of the variants come in `Left`/
    /// `Right` pairs that exist only to record which side needed the adapter,
    /// and each pair was written out by hand — a table maintaining a law
    /// instead of deriving from it.
    ///
    /// Having the involution lets the law be *tested* rather than trusted, and
    /// lets the planner canonicalise: decide once for an ordered pair, mirror
    /// the answer for the other order.
    pub fn mirrored(self) -> Self {
        match self {
            EqualityComparisonPlan::StringView {
                left_view,
                right_view,
            } => EqualityComparisonPlan::StringView {
                left_view: right_view,
                right_view: left_view,
            },
            EqualityComparisonPlan::LeftOptionalByRef => EqualityComparisonPlan::RightOptionalByRef,
            EqualityComparisonPlan::RightOptionalByRef => EqualityComparisonPlan::LeftOptionalByRef,
            EqualityComparisonPlan::LeftOptionalStringView { left_view } => {
                EqualityComparisonPlan::RightOptionalStringView {
                    right_view: left_view,
                }
            }
            EqualityComparisonPlan::RightOptionalStringView { right_view } => {
                EqualityComparisonPlan::LeftOptionalStringView {
                    left_view: right_view,
                }
            }
            EqualityComparisonPlan::SharedHandleIdentity {
                handle_path,
                left_borrowed,
                right_borrowed,
            } => EqualityComparisonPlan::SharedHandleIdentity {
                handle_path,
                left_borrowed: right_borrowed,
                right_borrowed: left_borrowed,
            },
            EqualityComparisonPlan::LeftOptionalSharedHandleIdentity {
                handle_path,
                right_borrowed,
            } => EqualityComparisonPlan::RightOptionalSharedHandleIdentity {
                handle_path,
                left_borrowed: right_borrowed,
            },
            EqualityComparisonPlan::RightOptionalSharedHandleIdentity {
                handle_path,
                left_borrowed,
            } => EqualityComparisonPlan::LeftOptionalSharedHandleIdentity {
                handle_path,
                right_borrowed: left_borrowed,
            },
            EqualityComparisonPlan::BothOptionalSharedHandleIdentity { handle_path } => {
                EqualityComparisonPlan::BothOptionalSharedHandleIdentity { handle_path }
            }
            EqualityComparisonPlan::LeftSharedBorrowRightOwned => {
                EqualityComparisonPlan::LeftOwnedRightSharedBorrow
            }
            EqualityComparisonPlan::LeftOwnedRightSharedBorrow => {
                EqualityComparisonPlan::LeftSharedBorrowRightOwned
            }
            // Symmetric in themselves.
            same @ (EqualityComparisonPlan::Direct
            | EqualityComparisonPlan::BothOptionalByRef
            | EqualityComparisonPlan::CustomEquals { .. }
            | EqualityComparisonPlan::BothOptionalCustomEquals { .. }) => same,
        }
    }
}

/// Which Java comparison is being lowered.
///
/// Java's two equality operators disagree on a source object, and the
/// disagreement is exactly what 0028 is about:
///
///   - `==` is **reference identity**, whatever `equals` says;
///   - `.equals()` is the override if there is one, and identity if there is not.
///
/// Most overridden `.equals()` calls never reach this planner — the call plan
/// routes them to the source method. Optional receivers do arrive here, because
/// they need the null-safe wrapper, and there the two must not be conflated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqualityComparisonSource {
    /// `a == b` / `a != b`.
    ReferenceOperator,
    /// `a.equals(b)` or `Objects.equals(a, b)`.
    EqualsCall,
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
    plan_equality_comparison_for_source(
        left,
        right,
        facts,
        EqualityComparisonSource::ReferenceOperator,
    )
}

pub fn plan_equality_comparison_for_source(
    left: &RustSlotShape,
    right: &RustSlotShape,
    facts: &FactSet,
    source: EqualityComparisonSource,
) -> EqualityComparisonPlan {
    // Java `==` is symmetric, so decide for one operand order and mirror the
    // answer for the other. Before this the planner carried a `Right*` arm for
    // every `Left*` arm — three hand-written mirror pairs, each of which had to
    // be remembered. Canonicalising makes the symmetry *structural*: there is no
    // longer a place to write an asymmetric arm.
    //
    // `equality_planning_is_symmetric_under_operand_swap` still asserts the law,
    // because canonicalisation only guarantees it if the ordering is total and
    // `mirrored` is a true involution.
    if equality_operand_rank(right) < equality_operand_rank(left) {
        return plan_canonical_equality_comparison(right, left, facts, source).mirrored();
    }
    plan_canonical_equality_comparison(left, right, facts, source)
}

/// A total order that puts the operand needing an adapter on the left.
///
/// Optional-ness first, then shared-borrowness: the optional arms return before
/// the borrow arms are reached, so the two criteria never compete.
fn equality_operand_rank(shape: &RustSlotShape) -> u8 {
    let optional = u8::from(shape.optional_element_type().is_none());
    let borrowed = u8::from(shape.ownership != OwnershipShape::SharedBorrow);
    optional * 2 + borrowed
}

fn plan_canonical_equality_comparison(
    left: &RustSlotShape,
    right: &RustSlotShape,
    facts: &FactSet,
    source: EqualityComparisonSource,
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
    if let Some(handle_path) = shared_handle_identity_path(left, right, facts, source) {
        return match (
            left.optional_element_type().is_some(),
            right.optional_element_type().is_some(),
        ) {
            (true, true) => {
                EqualityComparisonPlan::BothOptionalSharedHandleIdentity { handle_path }
            }
            (true, false) => EqualityComparisonPlan::LeftOptionalSharedHandleIdentity {
                handle_path,
                right_borrowed: right.ownership == OwnershipShape::SharedBorrow,
            },
            (false, true) => EqualityComparisonPlan::RightOptionalSharedHandleIdentity {
                handle_path,
                left_borrowed: left.ownership == OwnershipShape::SharedBorrow,
            },
            (false, false) => EqualityComparisonPlan::SharedHandleIdentity {
                handle_path,
                left_borrowed: left.ownership == OwnershipShape::SharedBorrow,
                right_borrowed: right.ownership == OwnershipShape::SharedBorrow,
            },
        };
    }
    if source == EqualityComparisonSource::EqualsCall
        && left.optional_element_type().is_none()
        && right.optional_element_type().is_none()
        && left.java_ty == right.java_ty
        && let Some(method_name) = custom_equals_method(&left.java_ty, facts)
    {
        return EqualityComparisonPlan::CustomEquals {
            method_name,
            right_borrowed: right.ownership == OwnershipShape::SharedBorrow,
            erased_arg_handle_path: custom_equals_erased_arg_handle_path(&left.java_ty, facts),
        };
    }
    match (left.optional_element_type(), right.optional_element_type()) {
        (Some(left_element), Some(right_element)) if left_element == right_element => {
            if source == EqualityComparisonSource::EqualsCall
                && let Some(method_name) = custom_equals_method(&left_element, facts)
            {
                return EqualityComparisonPlan::BothOptionalCustomEquals {
                    erased_arg_handle_path: custom_equals_erased_arg_handle_path(
                        &left_element,
                        facts,
                    ),
                    method_name,
                };
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
        (Some(left_element), None) if left_element == right.java_ty => {
            EqualityComparisonPlan::LeftOptionalByRef
        }
        _ if left.java_ty == right.java_ty
            && left.ownership == OwnershipShape::SharedBorrow
            && right.ownership != OwnershipShape::SharedBorrow =>
        {
            EqualityComparisonPlan::LeftSharedBorrowRightOwned
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

/// The handle path to compare with `ptr_eq`, when both operands are the same
/// source type stored behind one.
///
/// For `==`, this is deliberately not conditioned on whether the type overrides
/// `equals`: Java reference equality ignores the override. For `.equals()` and
/// `Objects.equals`, a custom override suppresses identity and the planner emits
/// a call instead.
fn shared_handle_identity_path(
    left: &RustSlotShape,
    right: &RustSlotShape,
    facts: &FactSet,
    source: EqualityComparisonSource,
) -> Option<String> {
    // An optional operand is a nullable Java reference; identity still applies,
    // so compare the *element* types on each side.
    let left_ty = left
        .optional_element_type()
        .unwrap_or_else(|| left.java_ty.clone());
    let right_ty = right
        .optional_element_type()
        .unwrap_or_else(|| right.java_ty.clone());
    if left_ty != right_ty {
        return None;
    }
    let JavaType::Named { name, .. } = &left_ty else {
        return None;
    };
    // `.equals()` on a type that overrides it is the override, not identity.
    // `==` ignores the override entirely, which is why this is asked only for
    // the call.
    if source == EqualityComparisonSource::EqualsCall
        && custom_equals_method(&left_ty, facts).is_some()
    {
        return None;
    }
    let placement = facts.storage_placement_for(name);
    if !placement.shared || !placement.is_implemented() {
        return None;
    }
    placement.handle_path().map(str::to_string)
}

/// Renders an operand as `&Rc<T>`, which is what `ptr_eq` takes.
fn borrowed_handle_operand(expr: &RustExpr, already_borrowed: bool) -> String {
    let rendered = crate::emit::emit_expr(expr);
    if already_borrowed {
        rendered
    } else {
        format!("&{rendered}")
    }
}

fn erased_custom_equals_arg(expr: &RustExpr, borrowed: bool, handle_path: Option<&str>) -> String {
    let rendered = crate::emit::emit_expr(expr);
    if let Some(handle_path) = handle_path {
        if borrowed {
            return format!("Box::new({handle_path}::new((*({rendered})).clone()))");
        }
        return format!("Box::new(({rendered}).clone())");
    }
    if borrowed {
        format!("Box::new((*({rendered})).clone())")
    } else {
        format!("Box::new(({rendered}).clone())")
    }
}

fn erased_optional_custom_equals_arg(handle_path: Option<&str>) -> String {
    if let Some(handle_path) = handle_path {
        format!("Box::new({handle_path}::new((*right).clone()))")
    } else {
        "Box::new((*right).clone())".to_string()
    }
}

fn custom_equals_method(java_ty: &JavaType, facts: &FactSet) -> Option<String> {
    let JavaType::Named { name, .. } = java_ty else {
        return None;
    };
    facts
        .source_equals_override_for(name)
        .map(|_| "equals".to_string())
}

fn custom_equals_erased_arg_handle_path(java_ty: &JavaType, facts: &FactSet) -> Option<String> {
    let JavaType::Named { name, .. } = java_ty else {
        return None;
    };
    let placement = facts.storage_placement_for(name);
    if placement.shared && placement.is_implemented() {
        placement.handle_path().map(str::to_string)
    } else {
        None
    }
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
        EqualityComparisonPlan::SharedHandleIdentity {
            handle_path,
            left_borrowed,
            right_borrowed,
        } => {
            let call = format!(
                "{handle_path}::ptr_eq({}, {})",
                borrowed_handle_operand(&left, left_borrowed),
                borrowed_handle_operand(&right, right_borrowed)
            );
            RustExpr::Raw(if op == "!=" { format!("!{call}") } else { call })
        }
        EqualityComparisonPlan::LeftOptionalSharedHandleIdentity {
            handle_path,
            right_borrowed,
        } => {
            let other = borrowed_handle_operand(&right, right_borrowed);
            let call = format!(
                "{}.as_ref().is_some_and(|value| {handle_path}::ptr_eq(value, {other}))",
                crate::emit::emit_expr(&left)
            );
            RustExpr::Raw(if op == "!=" {
                format!("!({call})")
            } else {
                call
            })
        }
        EqualityComparisonPlan::RightOptionalSharedHandleIdentity {
            handle_path,
            left_borrowed,
        } => {
            let other = borrowed_handle_operand(&left, left_borrowed);
            let call = format!(
                "{}.as_ref().is_some_and(|value| {handle_path}::ptr_eq(value, {other}))",
                crate::emit::emit_expr(&right)
            );
            RustExpr::Raw(if op == "!=" {
                format!("!({call})")
            } else {
                call
            })
        }
        EqualityComparisonPlan::BothOptionalSharedHandleIdentity { handle_path } => {
            // `null == null` is `true` in Java, and a null never equals a live
            // reference -- so this is not `Option::eq`, which would compare the
            // pointees structurally.
            let call = format!(
                "match ({}.as_ref(), {}.as_ref()) {{ (Some(left), Some(right)) => {handle_path}::ptr_eq(left, right), (None, None) => true, _ => false }}",
                crate::emit::emit_expr(&left),
                crate::emit::emit_expr(&right)
            );
            RustExpr::Raw(if op == "!=" {
                format!("!({call})")
            } else {
                call
            })
        }
        EqualityComparisonPlan::CustomEquals {
            method_name,
            right_borrowed,
            erased_arg_handle_path,
        } => {
            let call = format!(
                "{}.{method_name}({})",
                crate::emit::emit_expr(&left),
                erased_custom_equals_arg(&right, right_borrowed, erased_arg_handle_path.as_deref())
            );
            RustExpr::Raw(if op == "!=" { format!("!{call}") } else { call })
        }
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
        EqualityComparisonPlan::BothOptionalCustomEquals {
            method_name,
            erased_arg_handle_path,
        } => {
            let true_expr = matches!(op, "==");
            let false_expr = !true_expr;
            let right_arg = erased_optional_custom_equals_arg(erased_arg_handle_path.as_deref());
            let some_expr = if true_expr {
                format!("left.{method_name}({right_arg})")
            } else {
                format!("!left.{method_name}({right_arg})")
            };
            RustExpr::Raw(format!(
                "match ({}, {}) {{ (Some(left), Some(right)) => {some_expr}, (None, None) => {true_expr}, _ => {false_expr} }}",
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

#[cfg(test)]
mod equality_symmetry_tests {
    use super::*;
    use crate::facts::{FactProvenance, SourceEqualsOverrideFact};
    use crate::java_ir::JavaType;
    use crate::slot_shape::{DispatchShape, MutabilityShape, TypeIdentity, ViewShape};

    fn shape(java_ty: JavaType, ownership: OwnershipShape, view: ViewShape) -> RustSlotShape {
        RustSlotShape {
            type_identity: TypeIdentity::from_java_type(java_ty.clone()),
            java_ty,
            ownership,
            view,
            dispatch: DispatchShape::Concrete,
            mutability: MutabilityShape::Immutable,
        }
    }

    fn named(name: &str) -> JavaType {
        JavaType::Named {
            name: name.to_string(),
            args: Vec::new(),
        }
    }

    fn optional(inner: JavaType) -> JavaType {
        JavaType::Named {
            name: "Optional".to_string(),
            args: vec![inner],
        }
    }

    /// Every shape worth crossing with every other.
    fn representative_shapes() -> Vec<RustSlotShape> {
        vec![
            shape(JavaType::String, OwnershipShape::Owned, ViewShape::Native),
            shape(
                JavaType::String,
                OwnershipShape::SharedBorrow,
                ViewShape::Str,
            ),
            shape(
                optional(JavaType::String),
                OwnershipShape::Owned,
                ViewShape::Native,
            ),
            shape(named("Node"), OwnershipShape::Owned, ViewShape::Native),
            shape(
                named("Node"),
                OwnershipShape::SharedBorrow,
                ViewShape::Native,
            ),
            shape(
                optional(named("Node")),
                OwnershipShape::Owned,
                ViewShape::Native,
            ),
            shape(
                JavaType::Primitive(crate::java_ir::PrimitiveType::Int),
                OwnershipShape::CopyValue,
                ViewShape::Native,
            ),
        ]
    }

    /// Java `==` is symmetric, so the plan for `(a, b)` must be the mirror of
    /// the plan for `(b, a)`.
    ///
    /// The planner writes its `Left*`/`Right*` arms out by hand, which means it
    /// maintains this law rather than deriving from it. Asserting the law is
    /// what turns "someone remembered to add the mirror arm" into something
    /// checked — the same reason `src/output_markers.rs` exists one level up.
    #[test]
    fn equality_planning_is_symmetric_under_operand_swap() {
        let shapes = representative_shapes();
        let mut asymmetric = Vec::new();
        for left in &shapes {
            for right in &shapes {
                let forward = plan_equality_comparison(left, right);
                let backward = plan_equality_comparison(right, left);
                if forward != backward.clone().mirrored() {
                    asymmetric.push(format!(
                        "({:?} {:?}, {:?} {:?}): forward {forward:?} but mirrored backward {:?}",
                        left.java_ty,
                        left.ownership,
                        right.java_ty,
                        right.ownership,
                        backward.mirrored(),
                    ));
                }
            }
        }
        assert!(
            asymmetric.is_empty(),
            "equality planning is not symmetric:\n{}",
            asymmetric.join("\n")
        );
    }

    /// Mirroring twice is the identity — the property that makes it an
    /// involution and lets a planner canonicalise on operand order.
    #[test]
    fn mirroring_an_equality_plan_twice_is_the_identity() {
        let shapes = representative_shapes();
        for left in &shapes {
            for right in &shapes {
                let plan = plan_equality_comparison(left, right);
                assert_eq!(plan.clone().mirrored().mirrored(), plan);
            }
        }
    }

    #[test]
    fn custom_optional_not_equals_negates_the_some_case() {
        let mut facts = FactSet::default();
        facts
            .source_equals_override_facts
            .push(SourceEqualsOverrideFact {
                type_name: "Node".to_string(),
                provenance: FactProvenance::ParsedSource,
            });
        let optional_node = shape(
            optional(named("Node")),
            OwnershipShape::Owned,
            ViewShape::Native,
        );
        assert_eq!(
            plan_equality_comparison_for_source(
                &optional_node,
                &optional_node,
                &facts,
                EqualityComparisonSource::ReferenceOperator,
            ),
            EqualityComparisonPlan::BothOptionalByRef,
            "Java == must not consult an optional element's custom equals override"
        );
        assert_eq!(
            plan_equality_comparison_for_source(
                &optional_node,
                &optional_node,
                &facts,
                EqualityComparisonSource::EqualsCall,
            ),
            EqualityComparisonPlan::BothOptionalCustomEquals {
                erased_arg_handle_path: None,
                method_name: "equals".to_string(),
            },
            "optional .equals() still needs the null-safe custom-equals wrapper"
        );

        let rendered = lower_equality_comparison_expr_for_op(
            BinaryOp::Ne,
            EqualityComparisonPlan::BothOptionalCustomEquals {
                erased_arg_handle_path: None,
                method_name: "equals".to_string(),
            },
            RustExpr::Var("left_value".to_string()),
            RustExpr::Var("right_value".to_string()),
        );

        assert_eq!(
            crate::emit::emit_expr(&rendered),
            "match (left_value, right_value) { (Some(left), Some(right)) => !left.equals(Box::new((*right).clone())), (None, None) => false, _ => true }"
        );
    }
}

#[cfg(test)]
mod shared_handle_identity_tests {
    use super::*;
    use crate::facts::{FactProvenance, SourceEqualsOverrideFact};
    use crate::java_ir::JavaType;
    use crate::slot_shape::{DispatchShape, MutabilityShape, OwnershipShape, ViewShape};
    use crate::storage_strategy::{Escape, StoragePlan, TypeStorageFacts};

    fn shared_node_facts() -> FactSet {
        shared_node_facts_with_static_reachability(false)
    }

    fn shared_node_facts_with_static_reachability(static_reachable: bool) -> FactSet {
        let mut plan = StoragePlan::default();
        plan.insert(
            "Node".to_string(),
            TypeStorageFacts {
                escape: Escape::Stored,
                locally_aliased: true,
                mutated: false,
                mutation_provably_absent: true,
                threaded: false,
                static_reachable,
                surface_forwardable: true,
            },
        );
        FactSet {
            storage_strategies: plan,
            ..FactSet::default()
        }
    }

    fn shared_node_facts_with_custom_equals() -> FactSet {
        let mut facts = shared_node_facts();
        facts
            .source_equals_override_facts
            .push(SourceEqualsOverrideFact {
                type_name: "Node".to_string(),
                provenance: FactProvenance::ParsedSource,
            });
        facts
    }

    fn node(ownership: OwnershipShape, optional: bool) -> RustSlotShape {
        let java_ty = if optional {
            JavaType::Named {
                name: "Optional".to_string(),
                args: vec![JavaType::Named {
                    name: "Node".to_string(),
                    args: Vec::new(),
                }],
            }
        } else {
            JavaType::Named {
                name: "Node".to_string(),
                args: Vec::new(),
            }
        };
        RustSlotShape {
            type_identity: crate::slot_shape::TypeIdentity::from_java_type(java_ty.clone()),
            java_ty,
            ownership,
            view: ViewShape::Native,
            dispatch: DispatchShape::Concrete,
            mutability: MutabilityShape::Immutable,
        }
    }

    fn render(plan: EqualityComparisonPlan, op: BinaryOp) -> String {
        crate::emit::emit_expr(&lower_equality_comparison_expr_for_op(
            op,
            plan,
            RustExpr::Var("a".to_string()),
            RustExpr::Var("b".to_string()),
        ))
    }

    /// `==` on `Rc<T>` is structural -- `Rc`'s `PartialEq` delegates to `T` --
    /// so two distinct objects with equal fields compared equal where Java says
    /// they differ. See claude_audits/0028.
    #[test]
    fn reference_equality_on_a_shared_type_is_pointer_identity() {
        let facts = shared_node_facts();
        let owned = node(OwnershipShape::Owned, false);
        let plan = plan_equality_comparison_with_facts(&owned, &owned, &facts);
        assert_eq!(render(plan, BinaryOp::Eq), "std::rc::Rc::ptr_eq(&a, &b)");
    }

    #[test]
    fn reference_inequality_negates_the_identity_test() {
        let facts = shared_node_facts();
        let owned = node(OwnershipShape::Owned, false);
        let plan = plan_equality_comparison_with_facts(&owned, &owned, &facts);
        assert_eq!(render(plan, BinaryOp::Ne), "!std::rc::Rc::ptr_eq(&a, &b)");
    }

    #[test]
    fn static_reachable_shared_identity_uses_arc_ptr_eq() {
        let facts = shared_node_facts_with_static_reachability(true);
        let owned = node(OwnershipShape::Owned, false);
        let plan = plan_equality_comparison_with_facts(&owned, &owned, &facts);
        assert_eq!(render(plan, BinaryOp::Eq), "std::sync::Arc::ptr_eq(&a, &b)");
    }

    #[test]
    fn equals_call_on_custom_equals_type_calls_the_override() {
        let facts = shared_node_facts_with_custom_equals();
        let owned = node(OwnershipShape::Owned, false);
        let plan = plan_equality_comparison_for_source(
            &owned,
            &owned,
            &facts,
            EqualityComparisonSource::EqualsCall,
        );
        assert_eq!(
            plan,
            EqualityComparisonPlan::CustomEquals {
                method_name: "equals".to_string(),
                right_borrowed: false,
                erased_arg_handle_path: Some("std::rc::Rc".to_string()),
            }
        );
        assert_eq!(
            render(plan, BinaryOp::Eq),
            "a.equals(Box::new((b).clone()))"
        );
    }

    #[test]
    fn reference_operator_ignores_custom_equals_override() {
        let facts = shared_node_facts_with_custom_equals();
        let owned = node(OwnershipShape::Owned, false);
        let plan = plan_equality_comparison_with_facts(&owned, &owned, &facts);
        assert_eq!(render(plan, BinaryOp::Eq), "std::rc::Rc::ptr_eq(&a, &b)");
    }

    /// An operand that is already a reference must not be borrowed twice.
    #[test]
    fn a_borrowed_operand_is_not_borrowed_again() {
        let facts = shared_node_facts();
        let plan = plan_equality_comparison_with_facts(
            &node(OwnershipShape::SharedBorrow, false),
            &node(OwnershipShape::Owned, false),
            &facts,
        );
        assert_eq!(render(plan, BinaryOp::Eq), "std::rc::Rc::ptr_eq(a, &b)");
    }

    /// Java's `null == null` is `true`, and a null never equals a live
    /// reference -- so this cannot be `Option::eq`, which compares the pointees.
    #[test]
    fn both_nullable_compares_identity_and_treats_two_nulls_as_equal() {
        let facts = shared_node_facts();
        let optional = node(OwnershipShape::Owned, true);
        let plan = plan_equality_comparison_with_facts(&optional, &optional, &facts);
        assert_eq!(
            render(plan, BinaryOp::Eq),
            "match (a.as_ref(), b.as_ref()) { (Some(left), Some(right)) => std::rc::Rc::ptr_eq(left, right), (None, None) => true, _ => false }"
        );
    }

    #[test]
    fn one_nullable_side_compares_identity_through_the_option() {
        let facts = shared_node_facts();
        let plan = plan_equality_comparison_with_facts(
            &node(OwnershipShape::Owned, true),
            &node(OwnershipShape::Owned, false),
            &facts,
        );
        assert_eq!(
            render(plan, BinaryOp::Eq),
            "a.as_ref().is_some_and(|value| std::rc::Rc::ptr_eq(value, &b))"
        );
    }

    /// A type that is *not* shared has no pointer to compare, so it keeps
    /// structural equality -- the scope line 0002b draws.
    #[test]
    fn an_unshared_type_keeps_structural_equality() {
        let owned = node(OwnershipShape::Owned, false);
        let plan = plan_equality_comparison_with_facts(&owned, &owned, &FactSet::default());
        assert_eq!(plan, EqualityComparisonPlan::Direct);
    }

    /// The involution still holds with the identity arms in play: swapping the
    /// operands and mirroring must be the identity function.
    #[test]
    fn identity_planning_is_symmetric_under_operand_swap() {
        let facts = shared_node_facts();
        let shapes = [
            node(OwnershipShape::Owned, false),
            node(OwnershipShape::SharedBorrow, false),
            node(OwnershipShape::Owned, true),
        ];
        for left in &shapes {
            for right in &shapes {
                let direct = plan_equality_comparison_with_facts(left, right, &facts);
                let swapped = plan_equality_comparison_with_facts(right, left, &facts).mirrored();
                assert_eq!(direct, swapped, "{left:?} vs {right:?}");
                assert_eq!(direct.clone().mirrored().mirrored(), direct);
            }
        }
    }
}
