use crate::effective_facts::EffectiveFacts;
use crate::facts::{
    DispatchRequirementKind, ExternalLibraryMode, FactProvenance, FactSet, HierarchyStrategy,
};
use crate::java_ir::{JavaType, PrimitiveType, WildcardBound};
use crate::java_naming::rust_name_for_java_named_type;
use crate::representation::{RepresentationPlan, RepresentationProvenance};
use crate::rust_ir::{RustExpr, RustType, rust_type_arg_projection_or_raw};
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::storage_strategy::StoragePlacement;
use crate::types::optional_element_type;

pub(crate) fn rust_generic_type(base: impl Into<String>, args: Vec<RustType>) -> RustType {
    RustType::Generic {
        base: base.into(),
        args,
    }
}

pub(crate) fn rust_boxed_dyn_trait_type(trait_name: impl Into<String>) -> RustType {
    rust_generic_type("Box", vec![RustType::DynTrait(trait_name.into())])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotDefaultPolicy {
    NoDefaultForClosedEnum { enum_name: String },
    NoDefaultForDynTrait { trait_name: String },
    NoDefaultForBoxedDynTrait { trait_name: String },
    NoDefaultForErasedObject,
    Value { policy: SlotDefaultValuePolicy },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SlotNoDefaultMessagePolicy {
    message: String,
}

impl SlotNoDefaultMessagePolicy {
    fn compile_error_expr(&self) -> RustExpr {
        crate::unsupported::compile_error_expr(&self.message)
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
    BTreeSet,
    LinkedHashSet,
    HashFreeSet,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NativeNamedJavaRustTypePolicy {
    name: String,
    policy: NativeJavaRustTypePolicy,
}

impl NativeNamedJavaRustTypePolicy {
    fn into_policy(self) -> NativeJavaRustTypePolicy {
        self.policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotContentCapabilityPolicy {
    string_content: bool,
    sequence_element: Option<JavaType>,
}

impl SlotContentCapabilityPolicy {
    pub fn new(string_content: bool, sequence_element: Option<JavaType>) -> Self {
        Self {
            string_content,
            sequence_element,
        }
    }

    pub fn string_content(&self) -> bool {
        self.string_content
    }

    pub fn sequence_element(&self) -> Option<&JavaType> {
        self.sequence_element.as_ref()
    }

    fn into_sequence_element(self) -> Option<JavaType> {
        self.sequence_element
    }
}

pub fn rust_type_for_slot(shape: &RustSlotShape) -> RustType {
    rust_type_for_slot_with_facts(shape, &FactSet::default())
}

pub fn erased_object_rust_type() -> RustType {
    rust_boxed_dyn_trait_type("std::any::Any")
}

pub fn rust_type_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    rust_type_for_slot_with_effective_facts(shape, &effective_facts)
}

pub fn rust_type_for_slot_with_facts_and_representations(
    shape: &RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustType {
    let rendered =
        rust_type_for_slot_with_facts_and_representations_unwrapped(shape, facts, representations);
    shared_handle_slot_rust_type(shape, &EffectiveFacts::new(facts), rendered)
}

fn rust_type_for_slot_with_facts_and_representations_unwrapped(
    shape: &RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    if let Some(element_ty) = shape.optional_element_type() {
        let mut element_shape = shape.clone();
        element_shape.java_ty = element_ty;
        // Through the wrapping entry point: the element is where the handle
        // belongs, giving `Option<Rc<T>>`.
        return RustType::Option(Box::new(rust_type_for_slot_with_facts_and_representations(
            &element_shape,
            facts,
            representations,
        )));
    }

    let viewed = match (&shape.view, &shape.dispatch) {
        (ViewShape::Str, _) => return rust_type_for_slot_with_facts(shape, facts),
        (ViewShape::Slice, _) => return rust_type_for_slot_with_facts(shape, facts),
        (_, DispatchShape::Concrete) => {
            if crate::stdlib::java_collection_kind_for_type(&shape.java_ty).is_some() {
                rust_type_for_java_type_with_facts_and_representations(
                    &shape.java_ty,
                    facts,
                    representations,
                )
            } else {
                rust_type_for_java_type_with_facts(&shape.java_ty, facts)
            }
        }
        _ => render_slot_value_rust_type_policy_with_effective_facts(
            &slot_value_rust_type_policy_with_effective_facts(shape, &effective_facts),
            &effective_facts,
        ),
    };

    match shape.ownership {
        OwnershipShape::Owned | OwnershipShape::CopyValue => viewed,
        OwnershipShape::SharedBorrow => RustType::Ref {
            mutable: false,
            lifetime: None,
            inner: Box::new(viewed),
        },
        OwnershipShape::MutableBorrow => RustType::Ref {
            mutable: true,
            lifetime: None,
            inner: Box::new(viewed),
        },
    }
}

pub fn rust_type_for_slot_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    if let Some(element_ty) = shape.optional_element_type() {
        let mut element_shape = shape.clone();
        element_shape.java_ty = element_ty;
        return RustType::Option(Box::new(rust_type_for_slot_with_effective_facts(
            &element_shape,
            facts,
        )));
    }
    let rendered =
        render_slot_rust_type_policy(&slot_rust_type_policy_with_effective_facts(shape, facts));
    shared_handle_slot_rust_type(shape, facts, rendered)
}

/// Wraps an **owned** slot of a shared type in its handle (0002/0002b).
fn shared_handle_slot_rust_type(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    rendered: RustType,
) -> RustType {
    if !matches!(
        shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) || shape.optional_element_type().is_some()
    {
        // An optional slot is wrapped at its element by the recursion above;
        // wrapping here too would give `Rc<Option<T>>`.
        return rendered;
    }
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return rendered;
    };
    let placement = facts.storage_placement_for(name);
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_type(placement, rendered)
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
        (ViewShape::Str, _, _) if content.string_content() => SlotValueRustTypePolicy::StringView,
        (ViewShape::Slice, _, java_ty)
            if crate::types::ndarray_array2_element_type(java_ty).is_some() =>
        {
            slot_dispatch_rust_type_policy(shape)
        }
        (ViewShape::Slice, _, _) => {
            if let Some(element_ty) = content.into_sequence_element() {
                SlotValueRustTypePolicy::SliceView {
                    element_ty: rust_storage_sequence_element_java_type(
                        &shape.java_ty,
                        element_ty,
                        facts,
                    ),
                }
            } else {
                slot_dispatch_rust_type_policy(shape)
            }
        }
        (_, DispatchShape::ErasedObject, java_ty)
            if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(java_ty) =>
        {
            SlotValueRustTypePolicy::ConcreteDispatch {
                java_ty: java_ty.clone(),
            }
        }
        _ => slot_dispatch_rust_type_policy(shape),
    }
}

pub(crate) fn render_slot_rust_type_policy(policy: &SlotRustTypePolicy) -> RustType {
    match policy {
        SlotRustTypePolicy::OwnedOrCopy { ty } => ty.clone(),
        SlotRustTypePolicy::SharedBorrow { ty } => RustType::Ref {
            mutable: false,
            lifetime: None,
            inner: Box::new(ty.clone()),
        },
        SlotRustTypePolicy::MutableBorrow { ty } => RustType::Ref {
            mutable: true,
            lifetime: None,
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

pub(crate) fn render_slot_value_rust_type_policy_with_effective_facts(
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
            rust_boxed_dyn_trait_type(dyn_trait_rust_path_with_effective_facts(trait_name, facts))
        }
        SlotValueRustTypePolicy::ErasedObject => erased_object_rust_type(),
        SlotValueRustTypePolicy::DynTrait { trait_name }
        | SlotValueRustTypePolicy::DynTraitView { trait_name } => {
            if let Some(trait_path) = external_crate_mapped_dyn_trait_path(trait_name, facts) {
                return RustType::ImplTrait(trait_path);
            }
            RustType::DynTrait(dyn_trait_rust_path_with_effective_facts(trait_name, facts))
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
            } else if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(java_ty)
                && let JavaType::Named { name, .. } = java_ty
            {
                RustType::Named(rust_name_for_java_named_type(name))
            } else {
                rust_type_for_java_type_with_effective_facts(java_ty, facts)
            }
        }
        SlotValueRustTypePolicy::StringView => RustType::Str,
        SlotValueRustTypePolicy::SliceView { element_ty } => RustType::Slice(Box::new(
            rust_type_for_java_generic_arg_with_effective_facts(element_ty, facts),
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

pub(crate) fn dyn_trait_rust_path(trait_name: &str, facts: &FactSet) -> String {
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

pub fn default_expr_for_slot_with_facts(shape: &RustSlotShape, facts: &FactSet) -> RustExpr {
    let effective_facts = EffectiveFacts::new(facts);
    default_expr_for_slot_with_effective_facts(shape, &effective_facts)
}

pub fn default_expr_for_slot_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> RustExpr {
    let rendered =
        slot_default_policy_expr(&slot_default_policy_with_effective_facts(shape, facts));
    shared_handle_construction_expr(shape, facts, rendered)
}

/// Value-form counterpart of the type wrap, for **fresh constructions** only.
pub(crate) fn shared_handle_construction_expr(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
    rendered: RustExpr,
) -> RustExpr {
    if !matches!(
        shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) || shape.optional_element_type().is_some()
    {
        return rendered;
    }
    let JavaType::Named { name, .. } = &shape.java_ty else {
        return rendered;
    };
    let placement = facts.storage_placement_for(name);
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_construction_expr(placement, rendered)
}

pub(crate) fn storage_placement_construction_expr(
    placement: StoragePlacement,
    rendered: RustExpr,
) -> RustExpr {
    let rendered = recover_storage_placement_construction_expr(placement, rendered);
    if storage_placement_construction_expr_is_wrapped(placement, &rendered) {
        return rendered;
    }
    let celled = placement
        .cell_path()
        .map(|cell| storage_wrapper_new_call_expr(cell, rendered.clone()))
        .unwrap_or(rendered);
    placement
        .handle_path()
        .map(|handle| storage_wrapper_new_call_expr(handle, celled.clone()))
        .unwrap_or(celled)
}

fn recover_storage_placement_construction_expr(
    placement: StoragePlacement,
    expr: RustExpr,
) -> RustExpr {
    match expr {
        RustExpr::Raw(rendered) => {
            let recovered = crate::coercion::recover_operator_expr_or_raw_leaf(&rendered);
            if storage_placement_construction_expr_is_wrapped(placement, &recovered) {
                recovered
            } else {
                RustExpr::raw_leaf(rendered)
            }
        }
        other => other,
    }
}

fn storage_placement_construction_expr_is_wrapped(
    placement: StoragePlacement,
    expr: &RustExpr,
) -> bool {
    let Some(handle) = placement.handle_path() else {
        return false;
    };
    match expr.unshaped() {
        RustExpr::Call { function, .. } => rust_path_expr_matches(function, handle, "new"),
        _ => false,
    }
}

fn rust_path_expr_matches(expr: &RustExpr, base: &str, leaf: &str) -> bool {
    match expr.unshaped() {
        RustExpr::Path(segments) => segments
            .iter()
            .map(String::as_str)
            .eq(base.split("::").chain(std::iter::once(leaf))),
        RustExpr::GenericPath {
            base: segments,
            member: Some(member),
            ..
        } if member == leaf => segments.iter().map(String::as_str).eq(base.split("::")),
        _ => false,
    }
}

fn storage_placement_type(placement: StoragePlacement, rendered: RustType) -> RustType {
    let celled = placement
        .cell_path()
        .map(|cell| rust_generic_type(cell, vec![rendered.clone()]))
        .unwrap_or(rendered);
    placement
        .handle_path()
        .map(|handle| rust_generic_type(handle, vec![celled.clone()]))
        .unwrap_or(celled)
}

fn storage_wrapper_new_call_expr(wrapper_path: &str, value: RustExpr) -> RustExpr {
    let mut segments = wrapper_path.split("::").collect::<Vec<_>>();
    segments.push("new");
    RustExpr::Call {
        function: Box::new(RustExpr::path(segments)),
        args: vec![value],
    }
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

pub fn slot_default_value_policy_with_effective_facts(
    shape: &RustSlotShape,
    facts: &EffectiveFacts<'_>,
) -> SlotDefaultValuePolicy {
    let content = slot_content_capability_policy_with_effective_facts(&shape.java_ty, facts);
    match (&shape.ownership, &shape.view, &shape.java_ty) {
        (OwnershipShape::SharedBorrow, ViewShape::Str, _) if content.string_content() => {
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
        (OwnershipShape::Owned, ViewShape::Native, JavaType::Named { name, args })
            if external_crate_mapped_named_rust_type(name, args, facts).is_some() =>
        {
            SlotDefaultValuePolicy::DefaultTrait
        }
        (OwnershipShape::Owned, ViewShape::Native, JavaType::Unknown(name))
            if external_crate_mapped_named_rust_type(name, &[], facts).is_some() =>
        {
            SlotDefaultValuePolicy::DefaultTrait
        }
        (OwnershipShape::Owned, ViewShape::Native, _) if content.sequence_element().is_some() => {
            SlotDefaultValuePolicy::EmptySequence
        }
        _ => SlotDefaultValuePolicy::DefaultTrait,
    }
}

fn slot_default_policy_expr(policy: &SlotDefaultPolicy) -> RustExpr {
    match policy {
        SlotDefaultPolicy::NoDefaultForClosedEnum { .. }
        | SlotDefaultPolicy::NoDefaultForDynTrait { .. }
        | SlotDefaultPolicy::NoDefaultForBoxedDynTrait { .. }
        | SlotDefaultPolicy::NoDefaultForErasedObject => {
            slot_no_default_message_policy(policy).compile_error_expr()
        }
        SlotDefaultPolicy::Value { policy } => slot_default_value_policy_expr(policy),
    }
}

pub(crate) fn slot_no_default_message_policy(
    policy: &SlotDefaultPolicy,
) -> SlotNoDefaultMessagePolicy {
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

fn slot_default_value_policy_expr(policy: &SlotDefaultValuePolicy) -> RustExpr {
    match policy {
        SlotDefaultValuePolicy::EmptyStringView => RustExpr::Literal("\"\"".to_string()),
        SlotDefaultValuePolicy::EmptySliceView => RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(RustExpr::ArrayLiteral(Vec::new())),
        },
        SlotDefaultValuePolicy::PrimitiveBooleanZero => RustExpr::Literal("false".to_string()),
        SlotDefaultValuePolicy::PrimitiveNumericZero => RustExpr::Literal("0".to_string()),
        SlotDefaultValuePolicy::PrimitiveFloatZero => RustExpr::Literal("0f32".to_string()),
        SlotDefaultValuePolicy::PrimitiveDoubleZero => RustExpr::Literal("0f64".to_string()),
        SlotDefaultValuePolicy::EmptyString => RustExpr::Call {
            function: Box::new(RustExpr::path(["String", "new"])),
            args: Vec::new(),
        },
        SlotDefaultValuePolicy::EmptySequence => RustExpr::Call {
            function: Box::new(RustExpr::path(["Vec", "new"])),
            args: Vec::new(),
        },
        SlotDefaultValuePolicy::DefaultTrait => RustExpr::Call {
            function: Box::new(RustExpr::path(["Default", "default"])),
            args: Vec::new(),
        },
    }
}

pub fn rust_type_for_java_type(ty: &JavaType) -> RustType {
    rust_type_for_java_type_with_facts(ty, &FactSet::default())
}

pub fn rust_type_for_java_type_with_facts(ty: &JavaType, facts: &FactSet) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    rust_type_for_java_type_with_effective_facts(ty, &effective_facts)
}

pub fn rust_type_for_java_type_with_facts_and_representations(
    ty: &JavaType,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustType {
    let effective_facts = EffectiveFacts::new(facts);
    if let JavaType::Named { name, args } = ty
        && (!args.is_empty() || crate::stdlib::java_collection_kind_for_name(name).is_some())
        && let Some(kind) = crate::stdlib::java_collection_kind_for_type(ty)
    {
        match kind {
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_priority_queue(ty) =>
            {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                return rust_generic_type(
                    "JavaPriorityQueue",
                    vec![rust_type_for_java_generic_arg_with_representations(
                        &element_ty,
                        &effective_facts,
                        representations,
                    )],
                );
            }
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_array_blocking_queue(ty) =>
            {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                return rust_generic_type(
                    "JavaArrayBlockingQueue",
                    vec![rust_type_for_java_generic_arg_with_representations(
                        &element_ty,
                        &effective_facts,
                        representations,
                    )],
                );
            }
            crate::stdlib::JavaCollectionKind::List => {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                return RustType::Vec(Box::new(
                    rust_type_for_java_generic_arg_with_representations(
                        &element_ty,
                        &effective_facts,
                        representations,
                    ),
                ));
            }
            kind if kind.is_set_like() => {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    &effective_facts,
                );
                let element_rust = rust_type_for_java_generic_arg_with_representations(
                    &element_ty,
                    &effective_facts,
                    representations,
                );
                if java_set_element_uses_hash_free_carrier_with_representations(
                    Some(&element_ty),
                    &effective_facts,
                    representations,
                ) {
                    return RustType::Vec(Box::new(element_rust));
                }
                return rust_generic_type(
                    match kind {
                        crate::stdlib::JavaCollectionKind::SortedSet => {
                            "std::collections::BTreeSet"
                        }
                        crate::stdlib::JavaCollectionKind::InsertionOrderedSet => {
                            "JavaLinkedHashSet"
                        }
                        _ => "std::collections::HashSet",
                    },
                    vec![element_rust],
                );
            }
            kind if kind.is_map_like() => {}
            _ => {}
        }
    }

    if let JavaType::Named { name, args } = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, args, &effective_facts)
    {
        return ty;
    }
    if let JavaType::Unknown(name) = ty
        && let Some(ty) = external_crate_mapped_named_rust_type(name, &[], &effective_facts)
    {
        return ty;
    }

    if let Some(dispatch_ty) = rust_type_for_representation_dispatch(ty, representations) {
        return dispatch_ty;
    }

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
    if erased_facade_concrete_dispatch_should_keep_named_type(ty, facts)
        && let JavaType::Named { name, .. } = ty
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named(rust_name_for_java_named_type(name)),
        };
    }
    if matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && !external_final_facade_should_keep_named_type(ty, facts)
        && !crate::javalibs::generic_facade_interface_erases_to_named_rust_type(ty)
        && crate::stdlib::java_collection_kind_for_type(ty).is_none()
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
    if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(ty)
        && let JavaType::Named { name, .. } = ty
    {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named(rust_name_for_java_named_type(name)),
        };
    }
    if let Some(enum_name) = closed_runtime_case_split_enum_name(ty, facts) {
        return NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named(enum_name),
        };
    }
    if let JavaType::Named { name, args } = ty
        && (!args.is_empty() || crate::stdlib::java_collection_kind_for_name(name).is_some())
        && let Some(kind) = crate::stdlib::java_collection_kind_for_type(ty)
    {
        match kind {
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_priority_queue(ty) =>
            {
                return NativeJavaRustTypePolicy::Direct {
                    ty: rust_generic_type(
                        "JavaPriorityQueue",
                        vec![rust_type_for_java_generic_arg_with_effective_facts(
                            &rust_storage_collection_element_java_type(
                                args.first()
                                    .cloned()
                                    .unwrap_or_else(erased_object_java_type),
                                facts,
                            ),
                            facts,
                        )],
                    ),
                };
            }
            crate::stdlib::JavaCollectionKind::List
                if crate::stdlib::java_type_is_array_blocking_queue(ty) =>
            {
                return NativeJavaRustTypePolicy::Direct {
                    ty: rust_generic_type(
                        "JavaArrayBlockingQueue",
                        vec![rust_type_for_java_generic_arg_with_effective_facts(
                            &rust_storage_collection_element_java_type(
                                args.first()
                                    .cloned()
                                    .unwrap_or_else(erased_object_java_type),
                                facts,
                            ),
                            facts,
                        )],
                    ),
                };
            }
            crate::stdlib::JavaCollectionKind::List => {
                return NativeJavaRustTypePolicy::Sequence {
                    container: NativeSequenceContainer::Vec,
                    element_ty: rust_storage_collection_element_java_type(
                        args.first()
                            .cloned()
                            .unwrap_or_else(erased_object_java_type),
                        facts,
                    ),
                };
            }
            kind if kind.is_set_like() => {
                let element_ty = rust_storage_collection_element_java_type(
                    args.first()
                        .cloned()
                        .unwrap_or_else(erased_object_java_type),
                    facts,
                );
                return NativeJavaRustTypePolicy::Sequence {
                    container: match kind {
                        crate::stdlib::JavaCollectionKind::SortedSet => {
                            NativeSequenceContainer::BTreeSet
                        }
                        crate::stdlib::JavaCollectionKind::InsertionOrderedSet => {
                            NativeSequenceContainer::LinkedHashSet
                        }
                        _ if java_set_element_uses_hash_free_carrier(&element_ty, facts) => {
                            NativeSequenceContainer::HashFreeSet
                        }
                        _ => NativeSequenceContainer::HashSet,
                    },
                    element_ty,
                };
            }
            kind if kind.is_map_like() && args.len() == 2 => {}
            kind if kind.is_map_like() => {
                return NativeJavaRustTypePolicy::Direct {
                    ty: rust_generic_type(
                        match kind {
                            crate::stdlib::JavaCollectionKind::SortedMap => {
                                "std::collections::BTreeMap"
                            }
                            crate::stdlib::JavaCollectionKind::InsertionOrderedMap => {
                                "JavaLinkedHashMap"
                            }
                            _ => "std::collections::HashMap",
                        },
                        vec![RustType::String, erased_object_rust_type()],
                    ),
                };
            }
            crate::stdlib::JavaCollectionKind::Set
            | crate::stdlib::JavaCollectionKind::SortedSet
            | crate::stdlib::JavaCollectionKind::InsertionOrderedSet
            | crate::stdlib::JavaCollectionKind::Map
            | crate::stdlib::JavaCollectionKind::SortedMap
            | crate::stdlib::JavaCollectionKind::InsertionOrderedMap => {
                unreachable!("set/map collection kinds are handled by guarded arms")
            }
        }
    }
    if let Some(element_ty) =
        slot_content_capability_policy_with_effective_facts(ty, facts).into_sequence_element()
    {
        return NativeJavaRustTypePolicy::Sequence {
            container: NativeSequenceContainer::Vec,
            element_ty: rust_storage_sequence_element_java_type(ty, element_ty, facts),
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
                || RustType::Unit,
                |arg| class_token_arg_rust_type(arg, facts),
            );
            NativeJavaRustTypePolicy::Direct {
                ty: rust_generic_type("JavaClass", vec![token_ty]),
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
                ty: rust_generic_type(
                    "JavaIterator",
                    vec![rust_type_for_java_generic_arg_with_effective_facts(
                        &element_ty,
                        facts,
                    )],
                ),
            }
        }
        JavaType::Named { name, args }
            if matches!(
                crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
                "ListIterator" | "java.util.ListIterator"
            ) =>
        {
            let element_ty = args
                .first()
                .cloned()
                .unwrap_or_else(erased_object_java_type);
            NativeJavaRustTypePolicy::Direct {
                ty: rust_generic_type(
                    "JavaListIterator",
                    vec![rust_type_for_java_generic_arg_with_effective_facts(
                        &element_ty,
                        facts,
                    )],
                ),
            }
        }
        JavaType::Named { .. } if crate::stdlib::java_map_type_args(ty).is_some() => {
            let (key, value) = crate::stdlib::java_map_type_args(ty).unwrap();
            let kind = crate::stdlib::java_collection_kind_for_type(ty)
                .unwrap_or(crate::stdlib::JavaCollectionKind::Map);
            NativeJavaRustTypePolicy::Direct {
                ty: rust_generic_type(
                    if matches!(kind, crate::stdlib::JavaCollectionKind::SortedMap) {
                        "std::collections::BTreeMap"
                    } else if matches!(kind, crate::stdlib::JavaCollectionKind::InsertionOrderedMap)
                    {
                        "JavaLinkedHashMap"
                    } else {
                        "std::collections::HashMap"
                    },
                    vec![
                        rust_type_for_java_generic_arg_with_effective_facts(key, facts),
                        rust_type_for_java_generic_arg_with_effective_facts(value, facts),
                    ],
                ),
            }
        }
        JavaType::Named { name, args }
            if matches!(
                crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
                "Entry" | "Map.Entry" | "java.util.Map.Entry"
            ) && args.len() == 2 =>
        {
            NativeJavaRustTypePolicy::Direct {
                ty: RustType::Tuple(vec![
                    rust_type_for_java_generic_arg_with_effective_facts(&args[0], facts),
                    rust_type_for_java_generic_arg_with_effective_facts(&args[1], facts),
                ]),
            }
        }
        JavaType::Named { name, args } if !args.is_empty() => NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                rust_name_for_java_named_type(name),
                args.iter()
                    .map(|arg| rust_type_for_java_generic_arg_with_effective_facts(arg, facts))
                    .collect(),
            ),
        },
        JavaType::Named { name, .. } => {
            if let JavaType::Named { args, .. } = ty
                && let Some(ty) = external_crate_mapped_named_rust_type(name, args, facts)
            {
                NativeJavaRustTypePolicy::Direct { ty }
            } else {
                native_named_java_rust_type_policy(name).into_policy()
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
    let pack = facts.external_rust_crate_mapping_pack_for_type(name)?;
    let crate_name = pack.rust_target.as_deref()?;
    let type_name = facts
        .external_crate_rust_type_name_for(name)
        .map(str::to_string)
        .unwrap_or_else(|| crate::stdlib::default_external_crate_rust_type_name(name));
    let generic_args = args
        .iter()
        .map(|arg| rust_type_for_java_generic_arg_with_effective_facts(arg, facts))
        .collect::<Vec<_>>();
    if generic_args.is_empty() {
        Some(RustType::Named(format!("{crate_name}::{type_name}")))
    } else {
        Some(rust_generic_type(
            format!("{crate_name}::{type_name}"),
            generic_args,
        ))
    }
}

fn class_token_arg_rust_type(arg: &JavaType, facts: &EffectiveFacts<'_>) -> RustType {
    match arg {
        JavaType::Wildcard(None) => RustType::Unit,
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            class_token_arg_rust_type(bound, facts)
        }
        _ => rust_type_for_java_type_with_effective_facts(arg, facts),
    }
}

pub fn slot_content_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> SlotContentCapabilityPolicy {
    SlotContentCapabilityPolicy::new(
        facts.is_string_like_type(java_ty),
        facts.sequence_element_type_for(java_ty),
    )
}

pub(crate) fn native_named_java_rust_type_policy(name: &str) -> NativeNamedJavaRustTypePolicy {
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
        name if java_exception_like_type_name(name) => NativeJavaRustTypePolicy::Direct {
            ty: RustType::Named("JavaException".to_string()),
        },
        "Properties" => NativeJavaRustTypePolicy::Direct {
            ty: rust_generic_type(
                "std::collections::HashMap",
                vec![RustType::String, RustType::String],
            ),
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

fn java_exception_like_type_name(name: &str) -> bool {
    crate::stdlib::is_known_throwable_carrier_type(name)
}

fn erased_object_java_type() -> JavaType {
    JavaType::Named {
        name: "Object".to_string(),
        args: Vec::new(),
    }
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
    )
}

/// Renders a generic argument, wrapping a shared type in its handle. Sound for
/// user generics because every generated trait carries the `Rc`/`Arc` forwarding
/// impls.
fn rust_type_for_java_generic_arg_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustType {
    let rendered = rust_type_for_java_generic_arg_unwrapped(ty, facts);
    let JavaType::Named { name, .. } = ty else {
        return rendered;
    };
    let placement = facts.storage_placement_for(name);
    if !placement.is_implemented() || !placement.shared() {
        return rendered;
    }
    storage_placement_type(placement, rendered)
}

fn rust_type_for_java_generic_arg_unwrapped(ty: &JavaType, facts: &EffectiveFacts<'_>) -> RustType {
    if let JavaType::Wildcard(Some(WildcardBound::Extends(bound))) = ty {
        return rust_type_for_java_generic_arg_unwrapped(bound, facts);
    }
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
    {
        return rust_type_for_java_type_with_effective_facts(
            &JavaType::Primitive(primitive),
            facts,
        );
    }
    if let JavaType::Named { name, args } = ty
        && matches!(
            crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
            "Entry" | "Map.Entry" | "java.util.Map.Entry"
        )
        && args.len() == 2
    {
        return RustType::Tuple(vec![
            rust_type_for_java_generic_arg_with_effective_facts(&args[0], facts),
            rust_type_for_java_generic_arg_with_effective_facts(&args[1], facts),
        ]);
    }
    if let JavaType::Array(element_ty) = ty {
        return RustType::Vec(Box::new(
            rust_type_for_java_generic_arg_with_effective_facts(
                &rust_storage_array_element_java_type(element_ty.as_ref().clone()),
                facts,
            ),
        ));
    }
    if let Some(enum_name) = closed_runtime_case_split_enum_name(ty, facts) {
        return RustType::Named(enum_name);
    }
    if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        facts.object_representation_override_strategy_for(ty)
    {
        return rust_boxed_dyn_trait_type(dyn_trait_rust_path_with_effective_facts(
            trait_name, facts,
        ));
    }
    if let JavaType::Named { name, args } = ty
        && args.is_empty()
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        return rust_boxed_dyn_trait_type(dyn_trait_rust_path_with_effective_facts(name, facts));
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
    if matches!(
        facts.object_representation_override_strategy_for(ty),
        Some(HierarchyStrategy::ErasedObject)
    ) {
        return erased_object_rust_type();
    }
    rust_type_for_java_type_with_effective_facts(ty, facts)
}

fn rust_storage_sequence_element_java_type(
    container_ty: &JavaType,
    element_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> JavaType {
    if matches!(container_ty, JavaType::Array(_)) {
        rust_storage_array_element_java_type(element_ty)
    } else {
        rust_storage_collection_element_java_type(element_ty, facts)
    }
}

fn rust_storage_collection_element_java_type(
    element_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> JavaType {
    if let Some(optional_element) = optional_element_type(&element_ty) {
        return crate::types::optional_java_type(rust_storage_collection_element_java_type(
            optional_element,
            facts,
        ));
    }
    if let JavaType::Named { name, args } = &element_ty
        && args.is_empty()
        && let Some(primitive) = boxed_primitive_storage_java_type(name, facts)
    {
        return JavaType::Primitive(primitive);
    }
    element_ty
}

fn boxed_primitive_storage_java_type(
    name: &str,
    facts: &EffectiveFacts<'_>,
) -> Option<PrimitiveType> {
    let simple = name.rsplit('.').next().unwrap_or(name);
    let primitive = crate::stdlib::boxed_primitive_type(simple)?;
    if name.starts_with("java.lang.") {
        return Some(primitive);
    }

    let java_ty = JavaType::Named {
        name: name.to_string(),
        args: Vec::new(),
    };
    if facts
        .type_capability_overrides_for(&java_ty)
        .any(|fact| fact.provenance == FactProvenance::ParsedSource)
    {
        return None;
    }
    facts
        .type_capability_overrides_for(&java_ty)
        .any(|fact| fact.provenance == FactProvenance::BuiltinExternalModel)
        .then_some(primitive)
}

fn rust_storage_array_element_java_type(element_ty: JavaType) -> JavaType {
    if reference_array_element_needs_nullable_carrier(&element_ty) {
        crate::types::optional_java_type(element_ty)
    } else {
        element_ty
    }
}

fn reference_array_element_needs_nullable_carrier(element_ty: &JavaType) -> bool {
    !matches!(
        element_ty,
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_)
    ) && optional_element_type(element_ty).is_none()
}

fn rust_type_for_java_generic_arg_with_representations(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> RustType {
    if let JavaType::Wildcard(Some(WildcardBound::Extends(bound))) = ty {
        return rust_type_for_java_generic_arg_with_representations(bound, facts, representations);
    }
    if builtin_or_conservative_erased_external_representation(ty, representations)
        && let JavaType::Named { name, .. } | JavaType::Unknown(name) = ty
    {
        return RustType::Named(rust_name_for_java_named_type(name));
    }
    if let Some(dispatch_ty) =
        rust_type_for_facade_aware_representation_dispatch(ty, representations)
    {
        return dispatch_ty;
    }
    rust_type_for_java_generic_arg_with_effective_facts(ty, facts)
}

fn rust_type_for_representation_dispatch(
    ty: &JavaType,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    match representations.dispatch_for(ty) {
        DispatchShape::ClosedEnum { enum_name } => Some(RustType::Named(enum_name)),
        DispatchShape::ErasedObject => Some(erased_object_rust_type()),
        DispatchShape::BoxedDynTrait { trait_name } => Some(rust_boxed_dyn_trait_type(trait_name)),
        DispatchShape::DynTrait { trait_name } => Some(RustType::DynTrait(trait_name)),
        DispatchShape::Concrete => None,
    }
}

fn rust_type_for_facade_aware_representation_dispatch(
    ty: &JavaType,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    if builtin_or_conservative_erased_external_representation(ty, representations) {
        return None;
    }
    let dispatch = representations.dispatch_for(ty);
    match dispatch {
        DispatchShape::DynTrait { trait_name } => Some(rust_boxed_dyn_trait_type(trait_name)),
        _ => rust_type_for_representation_dispatch(ty, representations),
    }
}

fn closed_runtime_case_split_enum_name(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> Option<String> {
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    if !args.is_empty() || facts.closed_hierarchy_fact_for_name(name) != Some(true) {
        return None;
    }
    let requirements = facts.dispatch_requirements_for(name);
    if !requirements.iter().any(|requirement| {
        matches!(
            requirement,
            DispatchRequirementKind::NeedsClosedRuntimeCaseSplit
                | DispatchRequirementKind::NeedsInstanceof
                | DispatchRequirementKind::NeedsDowncast
                | DispatchRequirementKind::NeedsUnknownRuntimeObject
        )
    }) {
        return None;
    }
    let permitted = facts.sealed_permitted_subtypes_for(name)?.into_value();
    if permitted.is_empty()
        || !permitted.iter().all(|subtype| {
            facts.is_final_type(&JavaType::Named {
                name: subtype.clone(),
                args: Vec::new(),
            })
        })
    {
        return None;
    }
    Some(format!("{name}Value"))
}

fn java_type_variable_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

pub(crate) fn render_native_java_rust_type_policy_with_effective_facts(
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
                NativeSequenceContainer::HashFreeSet => RustType::Vec(Box::new(element_ty)),
                NativeSequenceContainer::HashSet => {
                    rust_generic_type("std::collections::HashSet", vec![element_ty])
                }
                NativeSequenceContainer::BTreeSet => {
                    rust_generic_type("std::collections::BTreeSet", vec![element_ty])
                }
                NativeSequenceContainer::LinkedHashSet => {
                    rust_generic_type("JavaLinkedHashSet", vec![element_ty])
                }
            }
        }
        NativeJavaRustTypePolicy::Optional { element_ty } => RustType::Option(Box::new(
            rust_type_for_java_generic_arg_with_effective_facts(element_ty, facts),
        )),
        NativeJavaRustTypePolicy::Direct { ty } => ty.clone(),
        NativeJavaRustTypePolicy::Named { name } => RustType::Named(name.clone()),
        NativeJavaRustTypePolicy::WildcardObject => erased_object_rust_type(),
        NativeJavaRustTypePolicy::UnknownRaw { raw } => rust_type_arg_projection_or_raw(raw),
        NativeJavaRustTypePolicy::ArrayRequiresSequenceElement => {
            unreachable!("arrays are handled by sequence_element_type")
        }
    }
}

pub(crate) fn java_set_uses_hash_free_carrier(ty: &JavaType, facts: &FactSet) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    java_set_uses_hash_free_carrier_with_effective_facts(ty, &effective_facts)
}

pub(crate) fn java_set_uses_hash_free_carrier_with_representations(
    ty: &JavaType,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> bool {
    let effective_facts = EffectiveFacts::new(facts);
    java_set_uses_hash_free_carrier_with_effective_facts(ty, &effective_facts)
        || java_set_element_uses_hash_free_carrier_with_representations(
            java_set_element_type_or_erased(ty).as_ref(),
            &effective_facts,
            representations,
        )
}

pub(crate) fn java_set_uses_hash_free_carrier_with_effective_facts(
    ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    let JavaType::Named { args, .. } = ty else {
        return false;
    };
    if !matches!(
        crate::stdlib::java_collection_kind_for_type(ty),
        Some(crate::stdlib::JavaCollectionKind::Set)
    ) {
        return false;
    }
    let Some(element_ty) = args.first() else {
        return false;
    };
    java_set_element_uses_hash_free_carrier(&element_ty, facts)
}

fn java_set_element_type_or_erased(ty: &JavaType) -> Option<JavaType> {
    let JavaType::Named { args, .. } = ty else {
        return None;
    };
    if !matches!(
        crate::stdlib::java_collection_kind_for_type(ty),
        Some(crate::stdlib::JavaCollectionKind::Set)
    ) {
        return None;
    }
    args.first().cloned()
}

fn java_set_element_uses_hash_free_carrier(
    element_ty: &JavaType,
    facts: &EffectiveFacts<'_>,
) -> bool {
    if matches!(element_ty, JavaType::String | JavaType::Primitive(_)) {
        return false;
    }
    let representation_policy =
        crate::signature::signature_boundary_representation_policy_with_effective_facts(
            element_ty, facts,
        );
    representation_policy.preserve_object_representation()
        && !external_final_facade_should_keep_named_type(element_ty, facts)
        && !crate::javalibs::generic_facade_interface_erases_to_named_rust_type(element_ty)
}

fn java_set_element_uses_hash_free_carrier_with_representations(
    element_ty: Option<&JavaType>,
    facts: &EffectiveFacts<'_>,
    representations: &RepresentationPlan,
) -> bool {
    let Some(element_ty) = element_ty else {
        return false;
    };
    if builtin_or_conservative_erased_external_representation(element_ty, representations) {
        return false;
    }
    java_set_element_uses_hash_free_carrier(element_ty, facts)
        || rust_type_for_representation_dispatch(element_ty, representations).is_some()
}

fn builtin_or_conservative_erased_external_representation(
    ty: &JavaType,
    representations: &RepresentationPlan,
) -> bool {
    let type_name = match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return false,
    };
    let Some(representation) = representations.type_representation(type_name) else {
        return false;
    };
    matches!(
        representations.dispatch_for(ty),
        DispatchShape::ErasedObject
    ) && matches!(
        representation.provenance(),
        RepresentationProvenance::BuiltinExternalModel
            | RepresentationProvenance::ConservativeFallback
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_default_message_policy_stays_inside_slot_render() {
        let policy = SlotDefaultPolicy::NoDefaultForClosedEnum {
            enum_name: "AnimalValue".to_string(),
        };
        let no_default = slot_no_default_message_policy(&policy);

        assert_eq!(
            no_default,
            SlotNoDefaultMessagePolicy {
                message: "no default value is available for closed enum slot AnimalValue"
                    .to_string(),
            }
        );
        assert_eq!(
            no_default.compile_error_expr(),
            crate::unsupported::compile_error_expr(
                "no default value is available for closed enum slot AnimalValue"
            )
        );
    }

    #[test]
    fn unknown_raw_native_type_policy_recovers_simple_type_text() {
        let facts = FactSet::default();
        let effective_facts = EffectiveFacts::new(&facts);

        assert_eq!(
            render_native_java_rust_type_policy_with_effective_facts(
                &NativeJavaRustTypePolicy::UnknownRaw {
                    raw: "Object".to_string(),
                },
                &effective_facts,
            ),
            RustType::Named("Object".to_string())
        );
        assert_eq!(
            render_native_java_rust_type_policy_with_effective_facts(
                &NativeJavaRustTypePolicy::UnknownRaw {
                    raw: "None::<Void>".to_string(),
                },
                &effective_facts,
            ),
            RustType::Raw("None::<Void>".to_string())
        );
    }

    #[test]
    fn storage_placement_construction_builds_nested_calls_structurally() {
        let expr = storage_placement_construction_expr(
            StoragePlacement::new(true, true, false),
            RustExpr::Var("value".to_string()),
        );

        assert_eq!(
            crate::emit::emit_expr(&expr),
            "std::rc::Rc::new(std::cell::RefCell::new(value))"
        );
        assert_eq!(
            expr,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "rc", "Rc", "new"])),
                args: vec![RustExpr::Call {
                    function: Box::new(RustExpr::path(["std", "cell", "RefCell", "new"])),
                    args: vec![RustExpr::Var("value".to_string())],
                }],
            }
        );

        let already_wrapped = storage_placement_construction_expr(
            StoragePlacement::new(true, false, true),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "sync", "Arc", "new"])),
                args: vec![RustExpr::Var("value".to_string())],
            },
        );
        assert_eq!(
            already_wrapped,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "sync", "Arc", "new"])),
                args: vec![RustExpr::Var("value".to_string())],
            }
        );

        let raw_already_wrapped = storage_placement_construction_expr(
            StoragePlacement::new(true, false, false),
            RustExpr::Raw("std::rc::Rc::new(value)".to_string()),
        );
        assert_eq!(
            raw_already_wrapped,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "rc", "Rc", "new"])),
                args: vec![RustExpr::Var("value".to_string())],
            }
        );

        let generic_already_wrapped = storage_placement_construction_expr(
            StoragePlacement::new(true, false, false),
            RustExpr::Call {
                function: Box::new(RustExpr::GenericPath {
                    base: vec!["std".to_string(), "rc".to_string(), "Rc".to_string()],
                    args: vec![RustType::Named("Value".to_string())],
                    member: Some("new".to_string()),
                }),
                args: vec![RustExpr::Var("value".to_string())],
            },
        );
        assert_eq!(
            crate::emit::emit_expr(&generic_already_wrapped),
            "std::rc::Rc::<Value>::new(value)"
        );

        let opaque_raw_payload = storage_placement_construction_expr(
            StoragePlacement::new(true, false, false),
            RustExpr::Raw("left + right".to_string()),
        );
        assert_eq!(
            opaque_raw_payload,
            RustExpr::Call {
                function: Box::new(RustExpr::path(["std", "rc", "Rc", "new"])),
                args: vec![RustExpr::Raw("left + right".to_string())],
            }
        );
    }

    #[test]
    fn storage_placement_type_builds_nested_generics_structurally() {
        let ty = storage_placement_type(
            StoragePlacement::new(true, true, false),
            RustType::Named("Value".to_string()),
        );

        assert_eq!(
            crate::emit::emit_type(&ty),
            "std::rc::Rc<std::cell::RefCell<Value>>"
        );
        assert_eq!(
            ty,
            rust_generic_type(
                "std::rc::Rc",
                vec![rust_generic_type(
                    "std::cell::RefCell",
                    vec![RustType::Named("Value".to_string())]
                )]
            )
        );
    }

    #[test]
    fn map_entry_renders_as_tuple_at_slot_and_generic_positions() {
        let entry = JavaType::Named {
            name: "java.util.Map.Entry".to_string(),
            args: vec![
                JavaType::String,
                JavaType::Primitive(crate::java_ir::PrimitiveType::Int),
            ],
        };

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type(&entry)),
            "(String, i32)"
        );

        let values = JavaType::Named {
            name: "java.util.List".to_string(),
            args: vec![entry],
        };
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type(&values)),
            "Vec<(String, i32)>"
        );
    }

    #[test]
    fn boxed_primitive_collection_elements_render_as_primitive_slots() {
        let stdlib_facts = crate::stdlib::rust_stdlib_signature_facts();
        for (java_name, expected) in [
            ("Byte", "Vec<i8>"),
            ("Short", "Vec<i16>"),
            ("Integer", "Vec<i32>"),
            ("Long", "Vec<i64>"),
            ("Float", "Vec<f32>"),
            ("Double", "Vec<f64>"),
            ("Boolean", "Vec<bool>"),
            ("Character", "Vec<char>"),
        ] {
            let values = JavaType::Named {
                name: "java.util.ArrayList".to_string(),
                args: vec![JavaType::Named {
                    name: java_name.to_string(),
                    args: Vec::new(),
                }],
            };

            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts(&values, &stdlib_facts)),
                expected,
                "{java_name}"
            );

            let nullable_values = JavaType::Named {
                name: "java.util.ArrayList".to_string(),
                args: vec![crate::types::optional_java_type(JavaType::Named {
                    name: java_name.to_string(),
                    args: Vec::new(),
                })],
            };
            let nullable_expected = format!(
                "Vec<Option<{}>>",
                expected.trim_start_matches("Vec<").trim_end_matches('>')
            );
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts(
                    &nullable_values,
                    &stdlib_facts
                )),
                nullable_expected,
                "nullable {java_name}"
            );

            let nullable_value = crate::types::optional_java_type(JavaType::Named {
                name: java_name.to_string(),
                args: Vec::new(),
            });
            let element_expected = expected
                .trim_start_matches("Vec<")
                .trim_end_matches('>')
                .to_string();
            assert_eq!(
                crate::emit::emit_type(&rust_type_for_java_type_with_facts(
                    &nullable_value,
                    &stdlib_facts
                )),
                format!("Option<{element_expected}>"),
                "optional {java_name}"
            );
        }
    }

    #[test]
    fn source_type_named_like_boxed_primitive_keeps_source_identity() {
        let mut facts = crate::stdlib::rust_stdlib_signature_facts();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "Byte".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: Some(false),
                object_safe_trait: None,
                provenance: FactProvenance::ParsedSource,
            });

        let source_byte = JavaType::Named {
            name: "Byte".to_string(),
            args: Vec::new(),
        };
        let nullable_source_byte = crate::types::optional_java_type(source_byte.clone());
        let values = JavaType::Named {
            name: "java.util.ArrayList".to_string(),
            args: vec![nullable_source_byte.clone()],
        };

        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts(
                &nullable_source_byte,
                &facts
            )),
            "Option<Byte>"
        );
        assert_eq!(
            crate::emit::emit_type(&rust_type_for_java_type_with_facts(&values, &facts)),
            "Vec<Option<Byte>>"
        );
    }

    #[test]
    fn exception_like_java_types_render_to_java_exception_carrier() {
        assert_eq!(
            rust_type_for_java_type(&JavaType::Named {
                name: "Throwable".to_string(),
                args: Vec::new(),
            }),
            RustType::Named("JavaException".to_string())
        );
        assert_eq!(
            rust_type_for_java_type(&JavaType::Named {
                name: "RuntimeException".to_string(),
                args: Vec::new(),
            }),
            RustType::Named("JavaException".to_string())
        );
        assert_eq!(
            rust_type_for_java_type(&JavaType::Array(Box::new(JavaType::Named {
                name: "java.lang.Throwable".to_string(),
                args: Vec::new(),
            }))),
            RustType::Vec(Box::new(RustType::Option(Box::new(RustType::Named(
                "JavaException".to_string()
            )))))
        );
        assert_eq!(
            rust_type_for_java_type(&JavaType::Named {
                name: "CauseStateException".to_string(),
                args: Vec::new(),
            }),
            RustType::Named("CauseStateException".to_string())
        );
    }
}
