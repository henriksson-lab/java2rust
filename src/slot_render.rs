use crate::facts::FactSet;
use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::{RustExpr, RustType};
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::types::optional_element_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotDefaultPolicy {
    NoDefaultForClosedEnum { enum_name: String },
    NoDefaultForDynTrait { trait_name: String },
    NoDefaultForBoxedDynTrait { trait_name: String },
    NoDefaultForErasedObject,
    Value { policy: SlotDefaultValuePolicy },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotNoDefaultMessagePolicy {
    pub message: String,
}

impl SlotNoDefaultMessagePolicy {
    pub fn compile_error_expr(&self) -> RustExpr {
        RustExpr::Raw(format!("compile_error!(\"{}\")", self.message))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotDefaultValuePolicy {
    EmptyStringView,
    EmptySliceView,
    PrimitiveBooleanZero,
    PrimitiveNumericZero,
    EmptyString,
    EmptySequence,
    DefaultTrait,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SlotRustTypePolicy {
    OwnedOrCopy { ty: RustType },
    SharedBorrow { ty: RustType },
    MutableBorrow { ty: RustType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotValueRustTypePolicy {
    ClosedEnum { enum_name: String },
    BoxedDynTrait { trait_name: String },
    ErasedObject,
    DynTrait { trait_name: String },
    NativeDispatch { java_ty: JavaType },
    DynTraitView { trait_name: String },
    BoxedDynTraitView { trait_name: String },
    StringView,
    SliceView { element_ty: JavaType },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeJavaRustTypePolicy {
    Sequence { element_ty: JavaType },
    Optional { element_ty: JavaType },
    Direct { ty: RustType },
    Named { name: String },
    WildcardObject,
    UnknownRaw { raw: String },
    ArrayRequiresSequenceElement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeNamedJavaRustTypePolicy {
    pub name: String,
    pub policy: NativeJavaRustTypePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotContentCapabilityPolicy {
    pub java_ty: JavaType,
    pub string_content: bool,
    pub sequence_element: Option<JavaType>,
}

pub fn rust_type_for_slot(shape: &RustSlotShape) -> RustType {
    rust_type_for_slot_with_facts(shape, &FactSet::default())
}

pub fn rust_type_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustType {
    render_slot_rust_type_policy(&slot_rust_type_policy(shape, facts))
}

pub fn slot_rust_type_policy(shape: &RustSlotShape, facts: &FactSet) -> SlotRustTypePolicy {
    let viewed =
        render_slot_value_rust_type_policy(&slot_value_rust_type_policy(shape, facts), facts);

    match shape.ownership {
        OwnershipShape::Owned | OwnershipShape::CopyValue => {
            SlotRustTypePolicy::OwnedOrCopy { ty: viewed }
        }
        OwnershipShape::SharedBorrow => SlotRustTypePolicy::SharedBorrow { ty: viewed },
        OwnershipShape::MutableBorrow => SlotRustTypePolicy::MutableBorrow { ty: viewed },
    }
}

pub fn slot_value_rust_type_policy(
    shape: &RustSlotShape,
    facts: &FactSet,
) -> SlotValueRustTypePolicy {
    let content = slot_content_capability_policy(&shape.java_ty, facts);
    match (&shape.view, &shape.dispatch, &shape.java_ty) {
        (ViewShape::DynTraitView, DispatchShape::DynTrait { trait_name }, _) => {
            SlotValueRustTypePolicy::DynTraitView {
                trait_name: trait_name.clone(),
            }
        }
        (ViewShape::DynTraitView, DispatchShape::BoxedDynTrait { trait_name }, _) => {
            SlotValueRustTypePolicy::BoxedDynTraitView {
                trait_name: trait_name.clone(),
            }
        }
        (ViewShape::Str, _, _) if content.string_content => SlotValueRustTypePolicy::StringView,
        (ViewShape::Slice, _, _) => {
            if let Some(element_ty) = content.sequence_element {
                SlotValueRustTypePolicy::SliceView { element_ty }
            } else {
                slot_dispatch_rust_type_policy(shape)
            }
        }
        _ => slot_dispatch_rust_type_policy(shape),
    }
}

pub fn render_slot_rust_type_policy(policy: &SlotRustTypePolicy) -> RustType {
    match policy {
        SlotRustTypePolicy::OwnedOrCopy { ty } => ty.clone(),
        SlotRustTypePolicy::SharedBorrow { ty } => RustType::Ref {
            mutable: false,
            inner: Box::new(ty.clone()),
        },
        SlotRustTypePolicy::MutableBorrow { ty } => RustType::Ref {
            mutable: true,
            inner: Box::new(ty.clone()),
        },
    }
}

fn slot_dispatch_rust_type_policy(shape: &RustSlotShape) -> SlotValueRustTypePolicy {
    match &shape.dispatch {
        DispatchShape::ClosedEnum { enum_name } => SlotValueRustTypePolicy::ClosedEnum {
            enum_name: enum_name.clone(),
        },
        DispatchShape::BoxedDynTrait { trait_name } => SlotValueRustTypePolicy::BoxedDynTrait {
            trait_name: trait_name.clone(),
        },
        DispatchShape::ErasedObject => SlotValueRustTypePolicy::ErasedObject,
        DispatchShape::DynTrait { trait_name } => SlotValueRustTypePolicy::DynTrait {
            trait_name: trait_name.clone(),
        },
        DispatchShape::Concrete => SlotValueRustTypePolicy::NativeDispatch {
            java_ty: shape.java_ty.clone(),
        },
    }
}

pub fn render_slot_value_rust_type_policy(
    policy: &SlotValueRustTypePolicy,
    facts: &FactSet,
) -> RustType {
    match policy {
        SlotValueRustTypePolicy::ClosedEnum { enum_name } => RustType::Named(enum_name.clone()),
        SlotValueRustTypePolicy::BoxedDynTrait { trait_name }
        | SlotValueRustTypePolicy::BoxedDynTraitView { trait_name } => {
            RustType::Raw(format!("Box<dyn {trait_name}>"))
        }
        SlotValueRustTypePolicy::ErasedObject => {
            RustType::Raw("Box<dyn std::any::Any>".to_string())
        }
        SlotValueRustTypePolicy::DynTrait { trait_name }
        | SlotValueRustTypePolicy::DynTraitView { trait_name } => {
            RustType::Raw(format!("dyn {trait_name}"))
        }
        SlotValueRustTypePolicy::NativeDispatch { java_ty } => {
            rust_type_for_java_type_with_facts(java_ty, facts)
        }
        SlotValueRustTypePolicy::StringView => RustType::Raw("str".to_string()),
        SlotValueRustTypePolicy::SliceView { element_ty } => RustType::Slice(Box::new(
            rust_type_for_java_type_with_facts(element_ty, facts),
        )),
    }
}

pub fn default_expr_for_slot(shape: &RustSlotShape) -> RustExpr {
    default_expr_for_slot_with_facts(shape, &FactSet::default())
}

pub fn default_expr_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustExpr {
    render_slot_default_policy(&slot_default_policy(shape, facts))
}

pub fn slot_default_policy(shape: &RustSlotShape, facts: &FactSet) -> SlotDefaultPolicy {
    match &shape.dispatch {
        DispatchShape::ClosedEnum { enum_name } => {
            return SlotDefaultPolicy::NoDefaultForClosedEnum {
                enum_name: enum_name.clone(),
            };
        }
        DispatchShape::DynTrait { trait_name } => {
            return SlotDefaultPolicy::NoDefaultForDynTrait {
                trait_name: trait_name.clone(),
            };
        }
        DispatchShape::BoxedDynTrait { trait_name } => {
            return SlotDefaultPolicy::NoDefaultForBoxedDynTrait {
                trait_name: trait_name.clone(),
            };
        }
        DispatchShape::ErasedObject => return SlotDefaultPolicy::NoDefaultForErasedObject,
        DispatchShape::Concrete => {}
    }
    SlotDefaultPolicy::Value {
        policy: slot_default_value_policy(shape, facts),
    }
}

pub fn slot_default_value_policy(shape: &RustSlotShape, facts: &FactSet) -> SlotDefaultValuePolicy {
    let content = slot_content_capability_policy(&shape.java_ty, facts);
    match (&shape.ownership, &shape.view, &shape.java_ty) {
        (OwnershipShape::SharedBorrow, ViewShape::Str, _) if content.string_content => {
            SlotDefaultValuePolicy::EmptyStringView
        }
        (OwnershipShape::SharedBorrow, ViewShape::Slice, _) => {
            SlotDefaultValuePolicy::EmptySliceView
        }
        (_, _, JavaType::Primitive(PrimitiveType::Boolean)) => {
            SlotDefaultValuePolicy::PrimitiveBooleanZero
        }
        (_, _, JavaType::Primitive(_)) => SlotDefaultValuePolicy::PrimitiveNumericZero,
        (OwnershipShape::Owned, ViewShape::Native, JavaType::String) => {
            SlotDefaultValuePolicy::EmptyString
        }
        (OwnershipShape::Owned, ViewShape::Native, _) if content.sequence_element.is_some() => {
            SlotDefaultValuePolicy::EmptySequence
        }
        _ => SlotDefaultValuePolicy::DefaultTrait,
    }
}

pub fn render_slot_default_policy(policy: &SlotDefaultPolicy) -> RustExpr {
    match policy {
        SlotDefaultPolicy::NoDefaultForClosedEnum { .. }
        | SlotDefaultPolicy::NoDefaultForDynTrait { .. }
        | SlotDefaultPolicy::NoDefaultForBoxedDynTrait { .. }
        | SlotDefaultPolicy::NoDefaultForErasedObject => {
            slot_no_default_message_policy(policy).compile_error_expr()
        }
        SlotDefaultPolicy::Value { policy } => render_slot_default_value_policy(policy),
    }
}

pub fn slot_no_default_message_policy(policy: &SlotDefaultPolicy) -> SlotNoDefaultMessagePolicy {
    let message = match policy {
        SlotDefaultPolicy::NoDefaultForClosedEnum { enum_name } => {
            format!("no default value is available for closed enum slot {enum_name}")
        }
        SlotDefaultPolicy::NoDefaultForDynTrait { trait_name } => {
            format!("no default value is available for dyn trait slot {trait_name}")
        }
        SlotDefaultPolicy::NoDefaultForBoxedDynTrait { trait_name } => {
            format!("no default value is available for boxed dyn trait slot {trait_name}")
        }
        SlotDefaultPolicy::NoDefaultForErasedObject => {
            "no default value is available for erased object slot".to_string()
        }
        SlotDefaultPolicy::Value { .. } => {
            "default value is available; no no-default fallback is required".to_string()
        }
    };
    SlotNoDefaultMessagePolicy { message }
}

pub fn render_slot_default_value_policy(policy: &SlotDefaultValuePolicy) -> RustExpr {
    match policy {
        SlotDefaultValuePolicy::EmptyStringView => RustExpr::Literal("\"\"".to_string()),
        SlotDefaultValuePolicy::EmptySliceView => RustExpr::Raw("&[]".to_string()),
        SlotDefaultValuePolicy::PrimitiveBooleanZero => RustExpr::Literal("false".to_string()),
        SlotDefaultValuePolicy::PrimitiveNumericZero => RustExpr::Literal("0".to_string()),
        SlotDefaultValuePolicy::EmptyString => RustExpr::Raw("String::new()".to_string()),
        SlotDefaultValuePolicy::EmptySequence => RustExpr::Raw("Vec::new()".to_string()),
        SlotDefaultValuePolicy::DefaultTrait => RustExpr::Raw("Default::default()".to_string()),
    }
}

pub fn rust_type_for_java_type(ty: &JavaType) -> RustType {
    rust_type_for_java_type_with_facts(ty, &FactSet::default())
}

pub fn rust_type_for_java_type_with_facts(ty: &JavaType, facts: &FactSet) -> RustType {
    render_native_java_rust_type_policy(&native_java_rust_type_policy(ty, facts), facts)
}

pub fn native_java_rust_type_policy(ty: &JavaType, facts: &FactSet) -> NativeJavaRustTypePolicy {
    if let Some(element_ty) = slot_content_capability_policy(ty, facts).sequence_element {
        return NativeJavaRustTypePolicy::Sequence { element_ty };
    }
    if let Some(element_ty) = optional_element_type(ty) {
        return NativeJavaRustTypePolicy::Optional { element_ty };
    }
    match ty {
        JavaType::Void => NativeJavaRustTypePolicy::Direct { ty: RustType::Unit },
        JavaType::Primitive(PrimitiveType::Boolean) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::Bool }
        }
        JavaType::Primitive(PrimitiveType::Byte) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I8 }
        }
        JavaType::Primitive(PrimitiveType::Short) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I16 }
        }
        JavaType::Primitive(PrimitiveType::Char) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::Char }
        }
        JavaType::Primitive(PrimitiveType::Int) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I32 }
        }
        JavaType::Primitive(PrimitiveType::Long) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::I64 }
        }
        JavaType::Primitive(PrimitiveType::Float) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::F32 }
        }
        JavaType::Primitive(PrimitiveType::Double) => {
            NativeJavaRustTypePolicy::Direct { ty: RustType::F64 }
        }
        JavaType::String => NativeJavaRustTypePolicy::Direct {
            ty: RustType::String,
        },
        JavaType::Array(_) => NativeJavaRustTypePolicy::ArrayRequiresSequenceElement,
        JavaType::Named { name, .. } => native_named_java_rust_type_policy(name).policy,
        JavaType::Wildcard(_) => NativeJavaRustTypePolicy::WildcardObject,
        JavaType::Unknown(raw) => NativeJavaRustTypePolicy::UnknownRaw { raw: raw.clone() },
    }
}

pub fn slot_content_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SlotContentCapabilityPolicy {
    SlotContentCapabilityPolicy {
        java_ty: java_ty.clone(),
        string_content: facts.is_string_like_type(java_ty),
        sequence_element: facts.sequence_element_type_for(java_ty),
    }
}

pub fn native_named_java_rust_type_policy(name: &str) -> NativeNamedJavaRustTypePolicy {
    let policy = match name {
        "Integer" => NativeJavaRustTypePolicy::Direct { ty: RustType::I32 },
        "Long" => NativeJavaRustTypePolicy::Direct { ty: RustType::I64 },
        "Double" => NativeJavaRustTypePolicy::Direct { ty: RustType::F64 },
        "Float" => NativeJavaRustTypePolicy::Direct { ty: RustType::F32 },
        "Boolean" => NativeJavaRustTypePolicy::Direct { ty: RustType::Bool },
        "Character" => NativeJavaRustTypePolicy::Direct { ty: RustType::Char },
        _ => NativeJavaRustTypePolicy::Named {
            name: name.to_string(),
        },
    };
    NativeNamedJavaRustTypePolicy {
        name: name.to_string(),
        policy,
    }
}

pub fn render_native_java_rust_type_policy(
    policy: &NativeJavaRustTypePolicy,
    facts: &FactSet,
) -> RustType {
    match policy {
        NativeJavaRustTypePolicy::Sequence { element_ty } => RustType::Vec(Box::new(
            rust_type_for_java_type_with_facts(element_ty, facts),
        )),
        NativeJavaRustTypePolicy::Optional { element_ty } => RustType::Option(Box::new(
            rust_type_for_java_type_with_facts(element_ty, facts),
        )),
        NativeJavaRustTypePolicy::Direct { ty } => ty.clone(),
        NativeJavaRustTypePolicy::Named { name } => RustType::Named(name.clone()),
        NativeJavaRustTypePolicy::WildcardObject => RustType::Named("JavaObject".to_string()),
        NativeJavaRustTypePolicy::UnknownRaw { raw } => RustType::Raw(raw.clone()),
        NativeJavaRustTypePolicy::ArrayRequiresSequenceElement => {
            unreachable!("arrays are handled by sequence_element_type")
        }
    }
}
