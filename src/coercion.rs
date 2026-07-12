use crate::emit;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::RustExpr;
pub use crate::slot_shape::{
    DispatchShape, LoweredExpr, LoweredType, OwnershipShape, RustSlotShape, TypeCapabilities,
    ViewShape,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    Value,
    Condition,
    Index,
    Assignment(RustSlotShape),
    Return(RustSlotShape),
    Argument(RustSlotShape),
    StringConcatSegment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coercion {
    Identity,
    WidenPrimitive {
        from: PrimitiveType,
        to: PrimitiveType,
    },
    IndexToUsize,
    ToStringForConcat,
    AsStr,
    ToOwnedString,
    WrapClosedEnum {
        enum_name: String,
        variant_name: String,
    },
    BorrowDynTrait {
        trait_name: String,
    },
    BoxDynTrait {
        trait_name: String,
    },
    EraseObject,
    DowncastClosedEnum {
        enum_name: String,
        variant_name: String,
    },
    DowncastDynTrait {
        target_type: String,
    },
    WrapOption,
    UnwrapOption,
    Borrow,
    CloneAudit,
}

pub fn plan_coercion(actual: &JavaType, slot: &Slot) -> Vec<Coercion> {
    match slot {
        Slot::Index => vec![Coercion::IndexToUsize],
        Slot::Condition => vec![Coercion::Identity],
        Slot::StringConcatSegment if !TypeCapabilities::conservative(actual).string_like => {
            vec![Coercion::ToStringForConcat]
        }
        _ => vec![Coercion::Identity],
    }
}

pub fn plan_shape_coercion(actual: &RustSlotShape, target: &RustSlotShape) -> Vec<Coercion> {
    let mut plan = Vec::new();
    if let Some(dispatch) = dispatch_coercion(actual, target) {
        plan.push(dispatch);
    }
    if actual.capabilities().string_like
        && target.capabilities().string_like
        && actual.ownership == OwnershipShape::Owned
        && actual.view == ViewShape::Native
        && target.ownership == OwnershipShape::SharedBorrow
        && target.view == ViewShape::Str
    {
        plan.push(Coercion::AsStr);
        return plan;
    }
    if actual.capabilities().string_like
        && target.capabilities().string_like
        && actual.ownership == OwnershipShape::SharedBorrow
        && actual.view == ViewShape::Str
        && target.ownership == OwnershipShape::Owned
        && target.view == ViewShape::Native
    {
        plan.push(Coercion::ToOwnedString);
        return plan;
    }
    if actual.ownership == OwnershipShape::Owned
        && target.ownership == OwnershipShape::SharedBorrow
        && actual.view == target.view
    {
        plan.push(Coercion::Borrow);
    }
    if let Some((from, to)) = primitive_widening(&actual.java_ty, &target.java_ty) {
        plan.push(Coercion::WidenPrimitive { from, to });
    }
    if plan.is_empty() {
        plan.push(Coercion::Identity);
    }
    plan
}

fn dispatch_coercion(actual: &RustSlotShape, target: &RustSlotShape) -> Option<Coercion> {
    if actual.dispatch == target.dispatch && actual.java_ty == target.java_ty {
        return None;
    }
    match (&actual.dispatch, &target.dispatch) {
        (DispatchShape::Concrete, DispatchShape::ClosedEnum { enum_name }) => {
            rust_nominal_type_name(&actual.java_ty).map(|variant_name| Coercion::WrapClosedEnum {
                enum_name: enum_name.clone(),
                variant_name,
            })
        }
        (DispatchShape::Concrete, DispatchShape::DynTrait { trait_name }) => {
            Some(Coercion::BorrowDynTrait {
                trait_name: trait_name.clone(),
            })
        }
        (DispatchShape::Concrete, DispatchShape::BoxedDynTrait { trait_name }) => {
            Some(Coercion::BoxDynTrait {
                trait_name: trait_name.clone(),
            })
        }
        (DispatchShape::Concrete, DispatchShape::ErasedObject) => Some(Coercion::EraseObject),
        (DispatchShape::ClosedEnum { enum_name }, DispatchShape::Concrete) => {
            rust_nominal_type_name(&target.java_ty).map(|variant_name| {
                Coercion::DowncastClosedEnum {
                    enum_name: enum_name.clone(),
                    variant_name,
                }
            })
        }
        (
            DispatchShape::DynTrait { .. } | DispatchShape::BoxedDynTrait { .. },
            DispatchShape::Concrete,
        ) => rust_nominal_type_name(&target.java_ty)
            .map(|target_type| Coercion::DowncastDynTrait { target_type }),
        _ => None,
    }
}

pub fn apply_lowered_expr_to_slot(lowered: LoweredExpr, slot: &Slot) -> RustExpr {
    match (&lowered.actual, slot) {
        (LoweredType::Slot(actual), Slot::Assignment(target))
        | (LoweredType::Slot(actual), Slot::Return(target))
        | (LoweredType::Slot(actual), Slot::Argument(target)) => {
            let plan = plan_shape_coercion(actual, target);
            apply_coercions(lowered.expr, &plan)
        }
        (LoweredType::Slot(shape), slot) => {
            let plan = plan_coercion(&shape.java_ty, slot);
            apply_coercions(lowered.expr, &plan)
        }
        (
            LoweredType::OptionalElementBorrow {
                option_shape,
                element_shape,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) if option_target_matches(target, option_shape, element_shape) => {
            apply_borrowed_option_element_adapter(lowered.expr, element_shape)
        }
        (
            LoweredType::VecElementBorrow {
                vec_shape,
                element_shape,
            },
            Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target),
        ) if vec_target_matches(target, vec_shape, element_shape) => {
            apply_borrowed_vec_element_adapter(lowered.expr, element_shape)
        }
        _ => lowered.expr,
    }
}

pub fn apply_coercions(mut expr: RustExpr, coercions: &[Coercion]) -> RustExpr {
    for coercion in coercions {
        expr = apply_one(expr, coercion);
    }
    expr
}

pub fn apply_expr_to_slot(expr: RustExpr, actual: RustSlotShape, slot: &Slot) -> RustExpr {
    match slot {
        Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) => {
            apply_expr_to_shape(expr, &actual, target)
        }
        _ => {
            let plan = plan_coercion(&actual.java_ty, slot);
            apply_coercions(expr, &plan)
        }
    }
}

pub fn apply_expr_to_shape(
    expr: RustExpr,
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> RustExpr {
    let plan = plan_shape_coercion(actual, target);
    apply_coercions(expr, &plan)
}

pub fn apply_instanceof_check(
    expr: RustExpr,
    actual: &RustSlotShape,
    target: &RustSlotShape,
) -> RustExpr {
    match (&actual.dispatch, &target.dispatch) {
        (DispatchShape::ClosedEnum { .. }, DispatchShape::Concrete) => {
            if let Some(variant_name) = rust_nominal_type_name(&target.java_ty) {
                return RustExpr::Raw(format!(
                    "matches!({}, {}::{variant_name}(_))",
                    emit::emit_expr(&expr),
                    closed_enum_name(actual)
                ));
            }
        }
        (
            DispatchShape::DynTrait { .. } | DispatchShape::BoxedDynTrait { .. },
            DispatchShape::Concrete,
        ) => {
            if let Some(target_type) = rust_nominal_type_name(&target.java_ty) {
                return RustExpr::Raw(format!(
                    "{}.as_any().is::<{target_type}>()",
                    emit::emit_expr(&expr)
                ));
            }
        }
        (DispatchShape::ErasedObject, DispatchShape::Concrete) => {
            if let Some(target_type) = rust_nominal_type_name(&target.java_ty) {
                return RustExpr::Raw(format!("{}.is::<{target_type}>()", emit::emit_expr(&expr)));
            }
        }
        (DispatchShape::Concrete, DispatchShape::Concrete) if actual.java_ty == target.java_ty => {
            return RustExpr::Literal("true".to_string());
        }
        _ => {}
    }
    RustExpr::Raw(format!(
        "compile_error!(\"unsupported Java instanceof shape: actual={:?}, target={:?}\")",
        actual.dispatch, target.dispatch
    ))
}

fn closed_enum_name(shape: &RustSlotShape) -> &str {
    match &shape.dispatch {
        DispatchShape::ClosedEnum { enum_name } => enum_name,
        _ => unreachable!("closed_enum_name called for non-enum dispatch"),
    }
}

fn option_target_matches(
    target: &RustSlotShape,
    option_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> bool {
    target.java_ty == option_shape.java_ty && target.is_optional_of(element_shape)
}

fn vec_target_matches(
    target: &RustSlotShape,
    vec_shape: &RustSlotShape,
    element_shape: &RustSlotShape,
) -> bool {
    target.java_ty == vec_shape.java_ty && target.is_sequence_of(element_shape)
}

fn apply_borrowed_option_element_adapter(
    expr: RustExpr,
    element_shape: &RustSlotShape,
) -> RustExpr {
    if element_shape.capabilities().string_like
        && element_shape.ownership == OwnershipShape::SharedBorrow
        && element_shape.view == ViewShape::Str
    {
        RustExpr::MethodCall {
            receiver: Box::new(expr),
            method: "map".to_string(),
            args: vec![RustExpr::Raw("|v| v.to_string()".to_string())],
        }
    } else {
        RustExpr::MethodCall {
            receiver: Box::new(expr),
            method: if element_shape.capabilities().copy {
                "copied".to_string()
            } else {
                "cloned".to_string()
            },
            args: Vec::new(),
        }
    }
}

fn apply_borrowed_vec_element_adapter(expr: RustExpr, element_shape: &RustSlotShape) -> RustExpr {
    if element_shape.capabilities().string_like
        && element_shape.ownership == OwnershipShape::SharedBorrow
        && element_shape.view == ViewShape::Str
    {
        RustExpr::Raw(format!(
            "{}.into_iter().map(|v| v.to_string()).collect::<Vec<_>>()",
            emit::emit_expr(&expr)
        ))
    } else {
        let adapter = if element_shape.capabilities().copy {
            "copied"
        } else {
            "cloned"
        };
        RustExpr::Raw(format!(
            "{}.into_iter().{adapter}().collect::<Vec<_>>()",
            emit::emit_expr(&expr)
        ))
    }
}

fn apply_one(expr: RustExpr, coercion: &Coercion) -> RustExpr {
    match coercion {
        Coercion::Identity => expr,
        Coercion::IndexToUsize => RustExpr::Raw(format!("{} as usize", emit::emit_expr(&expr))),
        Coercion::ToStringForConcat => {
            RustExpr::Raw(format!("{}.to_string()", emit::emit_expr(&expr)))
        }
        Coercion::AsStr => RustExpr::Raw(format!("{}.as_str()", emit::emit_expr(&expr))),
        Coercion::ToOwnedString => RustExpr::Raw(format!("{}.to_string()", emit::emit_expr(&expr))),
        Coercion::WrapClosedEnum {
            enum_name,
            variant_name,
        } => RustExpr::Raw(format!(
            "{enum_name}::{variant_name}({})",
            emit::emit_expr(&expr)
        )),
        Coercion::BorrowDynTrait { trait_name } => {
            RustExpr::Raw(format!("&{} as &dyn {trait_name}", emit::emit_expr(&expr)))
        }
        Coercion::BoxDynTrait { trait_name } => RustExpr::Raw(format!(
            "Box::new({}) as Box<dyn {trait_name}>",
            emit::emit_expr(&expr)
        )),
        Coercion::EraseObject => RustExpr::Raw(format!(
            "Box::new({}) as Box<dyn std::any::Any>",
            emit::emit_expr(&expr)
        )),
        Coercion::DowncastClosedEnum {
            enum_name,
            variant_name,
        } => RustExpr::Raw(format!(
            "match {} {{ {enum_name}::{variant_name}(value) => value, _ => panic!(\"invalid Java cast\") }}",
            emit::emit_expr(&expr)
        )),
        Coercion::DowncastDynTrait { target_type } => RustExpr::Raw(format!(
            "{}.as_any().downcast_ref::<{target_type}>().expect(\"invalid Java cast\")",
            emit::emit_expr(&expr)
        )),
        Coercion::WidenPrimitive { to, .. } => RustExpr::Raw(format!(
            "{} as {}",
            emit::emit_expr(&expr),
            rust_primitive_name(*to)
        )),
        Coercion::WrapOption => RustExpr::Raw(format!("Some({})", emit::emit_expr(&expr))),
        Coercion::UnwrapOption => RustExpr::Raw(format!("{}.unwrap()", emit::emit_expr(&expr))),
        Coercion::Borrow => RustExpr::Raw(format!("&{}", emit::emit_expr(&expr))),
        Coercion::CloneAudit => RustExpr::Raw(format!(
            "{}.clone()/* TODO(translation): validate added clone */",
            emit::emit_expr(&expr)
        )),
    }
}

fn rust_nominal_type_name(java_ty: &JavaType) -> Option<String> {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.clone()),
        JavaType::String => Some("String".to_string()),
        _ => None,
    }
}

fn primitive_widening(
    actual: &JavaType,
    target: &JavaType,
) -> Option<(PrimitiveType, PrimitiveType)> {
    let (JavaType::Primitive(from), JavaType::Primitive(to)) = (actual, target) else {
        return None;
    };
    if from == to || primitive_rank(*from) >= primitive_rank(*to) {
        None
    } else {
        Some((*from, *to))
    }
}

fn primitive_rank(primitive: PrimitiveType) -> u8 {
    match primitive {
        PrimitiveType::Boolean => 0,
        PrimitiveType::Byte => 1,
        PrimitiveType::Short | PrimitiveType::Char => 2,
        PrimitiveType::Int => 3,
        PrimitiveType::Long => 4,
        PrimitiveType::Float => 5,
        PrimitiveType::Double => 6,
    }
}

fn rust_primitive_name(primitive: PrimitiveType) -> &'static str {
    match primitive {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Byte => "i8",
        PrimitiveType::Short => "i16",
        PrimitiveType::Char => "char",
        PrimitiveType::Int => "i32",
        PrimitiveType::Long => "i64",
        PrimitiveType::Float => "f32",
        PrimitiveType::Double => "f64",
    }
}
