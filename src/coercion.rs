use crate::emit;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::{RustExpr, RustType};
use crate::types::{java_type_to_rust, rust_param_type};

#[derive(Debug, Clone, PartialEq)]
pub enum Slot {
    Value,
    Condition,
    Index,
    Assignment(JavaType),
    Return(JavaType),
    Argument(JavaType),
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
    WrapOption,
    UnwrapOption,
    Borrow,
    CloneAudit,
}

pub fn plan_coercion(actual: &JavaType, slot: &Slot) -> Vec<Coercion> {
    match slot {
        Slot::Index => vec![Coercion::IndexToUsize],
        Slot::Condition => vec![Coercion::Identity],
        Slot::StringConcatSegment if !matches!(actual, JavaType::String) => {
            vec![Coercion::ToStringForConcat]
        }
        Slot::Assignment(target) | Slot::Return(target) | Slot::Argument(target) => {
            primitive_widening(actual, target)
                .map(|(from, to)| vec![Coercion::WidenPrimitive { from, to }])
                .unwrap_or_else(|| vec![Coercion::Identity])
        }
        _ => vec![Coercion::Identity],
    }
}

pub fn apply_coercions(mut expr: RustExpr, coercions: &[Coercion]) -> RustExpr {
    for coercion in coercions {
        expr = apply_one(expr, coercion);
    }
    expr
}

pub fn rust_param_slot_type(ty: &JavaType, is_varargs: bool) -> RustType {
    rust_param_type(ty, is_varargs)
}

pub fn rust_storage_type(ty: &JavaType) -> RustType {
    java_type_to_rust(ty)
}

fn apply_one(expr: RustExpr, coercion: &Coercion) -> RustExpr {
    match coercion {
        Coercion::Identity => expr,
        Coercion::IndexToUsize => RustExpr::Raw(format!("{} as usize", emit::emit_expr(&expr))),
        Coercion::ToStringForConcat => {
            RustExpr::Raw(format!("{}.to_string()", emit::emit_expr(&expr)))
        }
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
