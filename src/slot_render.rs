use crate::effective_facts::EffectiveFacts;
use crate::facts::{ExternalLibraryMode, FactSet, HierarchyStrategy};
use crate::java_ir::{JavaType, PrimitiveType, WildcardBound};
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
        RustExpr::Raw(format!("compile_error!({:?})", self.message))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotDefaultValuePolicy {
    EmptyStringView,
    EmptySliceView,
    PrimitiveBooleanZero,
    PrimitiveNumericZero,
    PrimitiveFloatZero,
    PrimitiveDoubleZero,
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
    ConcreteDispatch { java_ty: JavaType },
    NativeDispatch { java_ty: JavaType },
    DynTraitView { trait_name: String },
    BoxedDynTraitView { trait_name: String },
    StringView,
    SliceView { element_ty: JavaType },
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeJavaRustTypePolicy {
    Sequence {
        container: NativeSequenceContainer,
        element_ty: JavaType,
    },
    Optional {
        element_ty: JavaType,
    },
    Direct {
        ty: RustType,
    },
    Named {
        name: String,
    },
    WildcardObject,
    UnknownRaw {
        raw: String,
    },
    ArrayRequiresSequenceElement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSequenceContainer {
    Vec,
    HashSet,
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

pub fn erased_object_rust_type() -> RustType {
    RustType::Raw("Box<dyn std::any::Any>".to_string())
}

pub fn rust_type_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    rust_type_for_slot_with_effective_facts(shape, &effective_facts)
}

pub fn rust_type_for_slot_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    if let Some(element_ty) = shape.optional_element_type() {
        let mut element_shape = shape.clone();
        element_shape.java_ty = element_ty;
        element_shape.type_identity =
            crate::slot_shape::TypeIdentity::from_java_type(element_shape.java_ty.clone());
        return RustType::Option(Box::new(rust_type_for_slot_with_effective_facts(
            &element_shape,
            facts,
        )));
    }
    render_slot_rust_type_policy(&slot_rust_type_policy_with_effective_facts(shape, facts))
}

pub fn slot_rust_type_policy(shape: &RustSlotShape, facts: &FactSet) -> SlotRustTypePolicy {
    let effective_facts = EffectiveFacts::new(facts);
    slot_rust_type_policy_with_effective_facts(shape, &effective_facts)
}

pub fn slot_rust_type_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotRustTypePolicy {
    let viewed = render_slot_value_rust_type_policy_with_effective_facts(
        &slot_value_rust_type_policy_with_effective_facts(shape, facts),
        facts,
    );

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
    let effective_facts = EffectiveFacts::new(facts);
    slot_value_rust_type_policy_with_effective_facts(shape, &effective_facts)
}

pub fn slot_value_rust_type_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotValueRustTypePolicy {
    let content = slot_content_capability_policy_with_effective_facts(&shape.java_ty, facts);
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
        (ViewShape::Slice, _, java_ty)
            if crate::types::ndarray_array2_element_type(java_ty).is_some() =>
        {
            slot_dispatch_rust_type_policy(shape)
        }
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
        DispatchShape::Concrete => SlotValueRustTypePolicy::ConcreteDispatch {
            java_ty: shape.java_ty.clone(),
        },
    }
}

pub fn render_slot_value_rust_type_policy(
    policy: &SlotValueRustTypePolicy,
    facts: &FactSet,
) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    render_slot_value_rust_type_policy_with_effective_facts(policy, &effective_facts)
}

pub fn render_slot_value_rust_type_policy_with_effective_facts(
    policy: &SlotValueRustTypePolicy,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    match policy {
        SlotValueRustTypePolicy::ClosedEnum { enum_name } => RustType::Named(enum_name.clone()),
        SlotValueRustTypePolicy::BoxedDynTrait { trait_name }
        | SlotValueRustTypePolicy::BoxedDynTraitView { trait_name } => {
            if external_crate_mapped_dyn_trait_path(trait_name, facts).is_some() {
                return erased_object_rust_type();
            }
            RustType::Raw(format!(
                "Box<dyn {}>",
                dyn_trait_rust_path_with_effective_facts(trait_name, facts)
            ))
        }
        SlotValueRustTypePolicy::ErasedObject => erased_object_rust_type(),
        SlotValueRustTypePolicy::DynTrait { trait_name }
        | SlotValueRustTypePolicy::DynTraitView { trait_name } => {
            if let Some(trait_path) = external_crate_mapped_dyn_trait_path(trait_name, facts) {
                return RustType::Raw(format!("impl {trait_path}"));
            }
            RustType::Raw(format!(
                "dyn {}",
                dyn_trait_rust_path_with_effective_facts(trait_name, facts)
            ))
        }
        SlotValueRustTypePolicy::ConcreteDispatch { java_ty } => {
            if let JavaType::Named { name, args } = java_ty
                && let Some(ty) = external_crate_mapped_named_rust_type(name, args, facts)
            {
                ty
            } else if let JavaType::Unknown(name) = java_ty
                && let Some(ty) = external_crate_mapped_named_rust_type(name, &[], facts)
            {
                ty
            } else if erased_facade_concrete_dispatch_should_keep_named_type(java_ty, facts)
                && let JavaType::Named { name, .. } = java_ty
            {
                RustType::Named(rust_name_for_java_named_type(name))
            } else {
                rust_type_for_java_type_with_effective_facts(java_ty, facts)
            }
        }
        SlotValueRustTypePolicy::NativeDispatch { java_ty } => {
            rust_type_for_java_type_with_effective_facts(java_ty, facts)
        }
        SlotValueRustTypePolicy::StringView => RustType::Raw("str".to_string()),
        SlotValueRustTypePolicy::SliceView { element_ty } => RustType::Slice(Box::new(
            rust_type_for_java_type_with_effective_facts(element_ty, facts),
        )),
    }
}

fn external_crate_mapped_dyn_trait_path(
    trait_name: &str,
    facts: &EffectiveFacts<'_>,
) -> Option<String> {
    let java_ty = JavaType::Named {
        name: trait_name.to_string(),
        args: Vec::new(),
    };
    let Some(HierarchyStrategy::DynTrait {
        trait_name: fact_trait_name,
    }) = facts.object_representation_override_strategy_for(&java_ty)
    else {
        return None;
    };
    let type_key = crate::stdlib::canonical_external_receiver_type_key(trait_name);
    if let Some(pack) = facts.external_library_pack_for_type(&type_key)
        && pack.mode == ExternalLibraryMode::RustCrateMapping
        && let Some(crate_name) = pack.rust_target.as_deref()
    {
        return Some(format!("{crate_name}::{fact_trait_name}"));
    }
    None
}

fn dyn_trait_rust_path_with_effective_facts(
    trait_name: &str,
    facts: &EffectiveFacts<'_>,
) -> String {
    let java_ty = JavaType::Named {
        name: trait_name.to_string(),
        args: Vec::new(),
    };
    if let Some(HierarchyStrategy::DynTrait {
        trait_name: fact_trait_name,
    }) = facts.object_representation_override_strategy_for(&java_ty)
    {
        let type_key = crate::stdlib::canonical_external_receiver_type_key(trait_name);
        if let Some(pack) = facts.external_library_pack_for_type(&type_key)
            && pack.mode == ExternalLibraryMode::RustCrateMapping
            && let Some(crate_name) = pack.rust_target.as_deref()
        {
            return format!("{crate_name}::{fact_trait_name}");
        }
        return rust_trait_name_path(fact_trait_name);
    }
    rust_trait_name_path(trait_name)
}

pub fn dyn_trait_rust_path(trait_name: &str, facts: &FactSet) -> String {
    let effective_facts = EffectiveFacts::new(facts);
    dyn_trait_rust_path_with_effective_facts(trait_name, &effective_facts)
}

fn rust_trait_name_path(trait_name: &str) -> String {
    if trait_name.contains("::") {
        trait_name.to_string()
    } else {
        rust_name_for_java_named_type(trait_name)
    }
}

pub fn default_expr_for_slot(shape: &RustSlotShape) -> RustExpr {
    default_expr_for_slot_with_facts(shape, &FactSet::default())
}

pub fn default_expr_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustExpr {
    let effective_facts = EffectiveFacts::new(facts);
    default_expr_for_slot_with_effective_facts(shape, &effective_facts)
}

pub fn default_expr_for_slot_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> RustExpr {
    render_slot_default_policy(&slot_default_policy_with_effective_facts(shape, facts))
}

pub fn slot_default_policy(shape: &RustSlotShape, facts: &FactSet) -> SlotDefaultPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    slot_default_policy_with_effective_facts(shape, &effective_facts)
}

pub fn slot_default_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotDefaultPolicy {
    if shape.optional_element_type().is_some() {
        return SlotDefaultPolicy::Value {
            policy: SlotDefaultValuePolicy::DefaultTrait,
        };
    }
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
        policy: slot_default_value_policy_with_effective_facts(shape, facts),
    }
}

pub fn slot_default_value_policy(shape: &RustSlotShape, facts: &FactSet) -> SlotDefaultValuePolicy {
    let effective_facts = EffectiveFacts::new(facts);
    slot_default_value_policy_with_effective_facts(shape, &effective_facts)
}

pub fn slot_default_value_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotDefaultValuePolicy {
    let content = slot_content_capability_policy_with_effective_facts(&shape.java_ty, facts);
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
        (_, _, JavaType::Primitive(PrimitiveType::Float)) => {
            SlotDefaultValuePolicy::PrimitiveFloatZero
        }
        (_, _, JavaType::Primitive(PrimitiveType::Double)) => {
            SlotDefaultValuePolicy::PrimitiveDoubleZero
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
        SlotDefaultValuePolicy::PrimitiveFloatZero => RustExpr::Literal("0f32".to_string()),
        SlotDefaultValuePolicy::PrimitiveDoubleZero => RustExpr::Literal("0f64".to_string()),
        SlotDefaultValuePolicy::EmptyString => RustExpr::Raw("String::new()".to_string()),
        SlotDefaultValuePolicy::EmptySequence => RustExpr::Raw("Vec::new()".to_string()),
        SlotDefaultValuePolicy::DefaultTrait => RustExpr::Raw("Default::default()".to_string()),
    }
}

pub fn rust_type_for_java_type(ty: &JavaType) -> RustType {
    rust_type_for_java_type_with_facts(ty, &FactSet::default())
}

pub fn rust_type_for_java_type_with_facts(ty: &JavaType, facts: &FactSet) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    rust_type_for_java_type_with_effective_facts(ty, &effective_facts)
}

pub fn rust_type_for_java_type_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    render_native_java_rust_type_policy_with_effective_facts(
        &native_java_rust_type_policy_with_effective_facts(ty, facts),
        facts,
    )
}

pub fn native_java_rust_type_policy(ty: &JavaType, facts: &FactSet) -> NativeJavaRustTypePolicy {
    let effective_facts = EffectiveFacts::new(facts);
    native_java_rust_type_policy_with_effective_facts(ty, &effective_facts)
}

pub fn native_java_rust_type_policy_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> NativeJavaRustTypePolicy {
    if let Some(element_ty) = crate::types::ndarray_array2_element_type(ty) {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Array2(Box::new(
                rust_type_for_java_generic_arg_with_effective_facts(&element_ty, facts),
            )),
        };
    }
    if matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && !external_final_facade_should_keep_named_type(ty, facts)
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: erased_object_rust_type(),
        };
    }
    if external_final_facade_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named(rust_name_for_java_named_type(name)),
        };
    }
    if let JavaType::Named { name, args } = ty
        && (!args.is_empty() || raw_java_collection_erasure_name(name))
        && let Some(kind) = crate::stdlib::java_collection_kind_for_type(ty)
    {
        match kind {
            crate::stdlib::JavaCollectionKind::List => {
                return NativeJavaRustTypePolicy::Sequence {
                    container: NativeSequenceContainer::Vec,
                    element_ty: args
                        .first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                };
            }
            crate::stdlib::JavaCollectionKind::Set => {
                return NativeJavaRustTypePolicy::Sequence {
                    container: NativeSequenceContainer::HashSet,
                    element_ty: args
                        .first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                };
            }
            crate::stdlib::JavaCollectionKind::Map if args.len() == 2 => {}
            crate::stdlib::JavaCollectionKind::Map => {
                return NativeJavaRustTypePolicy::Direct {
                    ty: RustType::Raw(format!(
                        "std::collections::HashMap<String, {}>",
                        crate::emit::emit_type(&erased_object_rust_type())
                    )),
                };
            }
        }
    }
    if let JavaType::Named { name, args } = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, args, facts)
    {
        return NativeJavaRustTypePolicy::Direct { ty };
    }
    if let JavaType::Unknown(name) = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, &[], facts)
    {
        return NativeJavaRustTypePolicy::Direct { ty };
    }
    if let Some(element_ty) =
        slot_content_capability_policy_with_effective_facts(ty, facts).sequence_element
    {
        return NativeJavaRustTypePolicy::Sequence {
            container: NativeSequenceContainer::Vec,
            element_ty,
        };
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
        JavaType::Named { name, args }
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Class" =>
        {
            let token_ty = args.first().map_or_else(
                || "()".to_string(),
                |arg| class_token_arg_rust_type(arg, facts),
            );
            NativeJavaRustTypePolicy::Direct {
                ty: RustType::Raw(format!("JavaClass<{token_ty}>")),
            }
        }
        JavaType::Named { name, args }
            if matches!(
                crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
                "Iterator" | "java.util.Iterator"
            ) =>
        {
            let element_ty = args
                .first()
                .cloned()
                .unwrap_or_else(erased_object_java_type);
            NativeJavaRustTypePolicy::Direct {
                ty: RustType::Raw(format!(
                    "JavaIterator<{}>",
                    crate::emit::emit_type(&rust_type_for_java_generic_arg_with_effective_facts(
                        &element_ty,
                        facts
                    ))
                )),
            }
        }
        JavaType::Named { .. } if crate::stdlib::java_map_type_args(ty).is_some() => {
            let (key, value) = crate::stdlib::java_map_type_args(ty).unwrap();
            NativeJavaRustTypePolicy::Direct {
                ty: RustType::Raw(format!(
                    "std::collections::HashMap<{}, {}>",
                    crate::emit::emit_type(&rust_type_for_java_generic_arg_with_effective_facts(
                        key, facts
                    )),
                    crate::emit::emit_type(&rust_type_for_java_generic_arg_with_effective_facts(
                        value, facts
                    ))
                )),
            }
        }
        JavaType::Named { name, .. } => {
            if let JavaType::Named { args, .. } = ty
                && let Some(ty) = external_crate_mapped_named_rust_type(name, args, facts)
            {
                NativeJavaRustTypePolicy::Direct { ty }
            } else {
                native_named_java_rust_type_policy(name).policy
            }
        }
        JavaType::Wildcard(_) => NativeJavaRustTypePolicy::WildcardObject,
        JavaType::Unknown(raw) => {
            if let Some(ty) = external_crate_mapped_named_rust_type(raw, &[], facts) {
                NativeJavaRustTypePolicy::Direct { ty }
            } else {
                NativeJavaRustTypePolicy::UnknownRaw { raw: raw.clone() }
            }
        }
    }
}

fn external_crate_mapped_named_rust_type(
    name: &str,
    args: &[JavaType],
    facts: &EffectiveFacts<'_>,
) -> Option<RustType> {
    let pack = facts.external_library_pack_for_type(name)?;
    if pack.mode != ExternalLibraryMode::RustCrateMapping {
        return None;
    }
    let crate_name = pack.rust_target.as_deref()?;
    let type_name = facts
        .external_crate_rust_type_name_for(name)
        .map(str::to_string)
        .unwrap_or_else(|| rust_name_for_java_named_type(name.rsplit('.').next().unwrap_or(name)));
    let generic_args = args
        .iter()
        .map(|arg| {
            crate::emit::emit_type(&rust_type_for_java_generic_arg_with_effective_facts(
                arg, facts,
            ))
        })
        .collect::<Vec<_>>();
    if generic_args.is_empty() {
        Some(RustType::Raw(format!("{crate_name}::{type_name}")))
    } else {
        Some(RustType::Raw(format!(
            "{crate_name}::{type_name}<{}>",
            generic_args.join(", ")
        )))
    }
}

fn class_token_arg_rust_type(arg: &JavaType, facts: &EffectiveFacts<'_>) -> String {
    match arg {
        JavaType::Wildcard(None) => "()".to_string(),
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            class_token_arg_rust_type(bound, facts)
        }
        _ => crate::emit::emit_type(&rust_type_for_java_type_with_effective_facts(arg, facts)),
    }
}

pub fn slot_content_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SlotContentCapabilityPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    slot_content_capability_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn slot_content_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SlotContentCapabilityPolicy {
    SlotContentCapabilityPolicy {
        java_ty: java_ty.clone(),
        string_content: facts.is_string_like_type(java_ty),
        sequence_element: facts.sequence_element_type_for(java_ty),
    }
}

pub fn native_named_java_rust_type_policy(name: &str) -> NativeNamedJavaRustTypePolicy {
    let rust_name = rust_name_for_java_named_type(name);
    let policy = match name {
        "Integer" => NativeJavaRustTypePolicy::Direct { ty: RustType::I32 },
        "Long" => NativeJavaRustTypePolicy::Direct { ty: RustType::I64 },
        "Double" => NativeJavaRustTypePolicy::Direct { ty: RustType::F64 },
        "Float" => NativeJavaRustTypePolicy::Direct { ty: RustType::F32 },
        "Boolean" => NativeJavaRustTypePolicy::Direct { ty: RustType::Bool },
        "Character" => NativeJavaRustTypePolicy::Direct { ty: RustType::Char },
        "Object" => NativeJavaRustTypePolicy::Direct {
            ty: erased_object_rust_type(),
        },
        "Properties" => NativeJavaRustTypePolicy::Direct {
            ty: RustType::Raw("std::collections::HashMap<String, String>".to_string()),
        },
        _ => NativeJavaRustTypePolicy::Named {
            name: rust_name.clone(),
        },
    };
    NativeNamedJavaRustTypePolicy {
        name: name.to_string(),
        policy,
    }
}

fn erased_object_java_type() -> JavaType {
    JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    }
}

fn raw_java_collection_erasure_name(name: &str) -> bool {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(&canonical);
    matches!(
        simple,
        "Collection"
            | "Iterable"
            | "List"
            | "ArrayList"
            | "LinkedList"
            | "Set"
            | "HashSet"
            | "LinkedHashSet"
            | "SortedSet"
            | "TreeSet"
            | "Map"
            | "HashMap"
            | "LinkedHashMap"
            | "SortedMap"
            | "TreeMap"
    )
}

fn external_final_facade_should_keep_named_type(ty: &JavaType, facts: &EffectiveFacts<'_>) -> bool {
    let erased;
    let ty = if let JavaType::Named { name, args } = ty
        && !args.is_empty()
    {
        erased = JavaType::Named {
            name: name.clone(),
            args: Vec::new(),
        };
        &erased
    } else {
        ty
    };
    matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && facts.is_final_type(ty)
        || java_type_name_for_builtin_final_erased_lookup(ty)
            .is_some_and(crate::external_packs::builtin_external_final_erased_object_type)
}

fn java_type_name_for_builtin_final_erased_lookup(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } => Some(name.as_str()),
        JavaType::Unknown(name) => Some(name.as_str()),
        _ => None,
    }
}

fn erased_final_facade_generic_arg_should_keep_named_type(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && facts.is_final_type(ty)
}

fn erased_facade_concrete_dispatch_should_keep_named_type(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    )
}

fn rust_type_for_java_generic_arg_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        facts.object_representation_override_strategy_for(ty)
    {
        return RustType::Raw(format!(
            "Box<dyn {}>",
            dyn_trait_rust_path_with_effective_facts(trait_name, facts)
        ));
    }
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        return RustType::Raw(format!(
            "Box<dyn {}>",
            dyn_trait_rust_path_with_effective_facts(name, facts)
        ));
    }
    if let JavaType::Unknown(raw) = ty
        && java_type_variable_name(raw)
    {
        return erased_object_rust_type();
    }
    if erased_final_facade_generic_arg_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return RustType::Named(rust_name_for_java_named_type(name));
    }
    rust_type_for_java_type_with_effective_facts(ty, facts)
}

fn java_type_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

pub fn rust_name_for_java_named_type(name: &str) -> String {
    if !name.contains('.') {
        return name.to_string();
    }
    name.split('.')
        .filter(|part| !part.trim().is_empty())
        .map(upper_camel_identifier_part)
        .collect::<String>()
}

fn upper_camel_identifier_part(part: &str) -> String {
    let mut out = String::new();
    let mut uppercase_next = true;
    for ch in part.chars() {
        if ch == '_' || ch == '-' || ch == '$' {
            uppercase_next = true;
        } else if uppercase_next {
            out.push(ch.to_ascii_uppercase());
            uppercase_next = false;
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
        }
    }
    out
}

pub fn render_native_java_rust_type_policy(
    policy: &NativeJavaRustTypePolicy,
    facts: &FactSet,
) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    render_native_java_rust_type_policy_with_effective_facts(policy, &effective_facts)
}

pub fn render_native_java_rust_type_policy_with_effective_facts(
    policy: &NativeJavaRustTypePolicy,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    match policy {
        NativeJavaRustTypePolicy::Sequence {
            container,
            element_ty,
        } => {
            let element_ty = rust_type_for_java_generic_arg_with_effective_facts(element_ty, facts);
            match container {
                NativeSequenceContainer::Vec => RustType::Vec(Box::new(element_ty)),
                NativeSequenceContainer::HashSet => RustType::Raw(format!(
                    "std::collections::HashSet<{}>",
                    crate::emit::emit_type(&element_ty)
                )),
            }
        }
        NativeJavaRustTypePolicy::Optional { element_ty } => RustType::Option(Box::new(
            rust_type_for_java_type_with_effective_facts(element_ty, facts),
        )),
        NativeJavaRustTypePolicy::Direct { ty } => ty.clone(),
        NativeJavaRustTypePolicy::Named { name } => RustType::Named(name.clone()),
        NativeJavaRustTypePolicy::WildcardObject => erased_object_rust_type(),
        NativeJavaRustTypePolicy::UnknownRaw { raw } => RustType::Raw(raw.clone()),
        NativeJavaRustTypePolicy::ArrayRequiresSequenceElement => {
            unreachable!("arrays are handled by sequence_element_type")
        }
    }
}
