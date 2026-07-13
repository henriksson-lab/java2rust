use crate::effective_facts::EffectiveFacts;
use crate::facts::{DispatchRequirementKind, FactSet};
use crate::java_ir::JavaType;
use crate::representation::RepresentationPlan;
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::types::sequence_element_type;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchProjectionPolicy {
    pub dispatch: DispatchShape,
    pub ownership: Option<OwnershipShape>,
    pub view: Option<ViewShape>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedElementProjectionPolicy {
    pub element_ty: JavaType,
    pub ownership: OwnershipShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceElementProjectionPolicy {
    pub java_ty: JavaType,
    pub element_ty: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeProjectionCapabilityPolicy {
    pub java_ty: JavaType,
    pub value_semantics: bool,
    pub sequence_element: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscapingRoleDispatchPolicy {
    pub source_dispatch: DispatchShape,
    pub projected_dispatch: DispatchShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchRequirementProjectionRole {
    Param,
    Return,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequirementProjectionPolicy {
    pub role: DispatchRequirementProjectionRole,
    pub source_dispatch: DispatchShape,
    pub requirement: DispatchRequirementKind,
    pub projected_dispatch: Option<DispatchShape>,
}

pub fn shape_projection_capability_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> ShapeProjectionCapabilityPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    shape_projection_capability_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn shape_projection_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts,
) -> ShapeProjectionCapabilityPolicy {
    ShapeProjectionCapabilityPolicy {
        java_ty: java_ty.clone(),
        value_semantics: facts.is_copy_type(java_ty),
        sequence_element: facts.sequence_element_type_for(java_ty),
    }
}

pub fn value_shape_for_java_type(
    representations: &RepresentationPlan,
    java_ty: JavaType,
) -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(java_ty.clone());
    apply_dispatch_projection(&mut shape, representations.dispatch_for(&java_ty));
    shape
}

pub fn project_java_value_shape(
    representations: Option<&RepresentationPlan>,
    java_ty: JavaType,
) -> RustSlotShape {
    if let Some(representations) = representations {
        return value_shape_for_java_type(representations, java_ty);
    }
    let mut shape = RustSlotShape::conservative(java_ty);
    if matches!(shape.java_ty, JavaType::Unknown(_) | JavaType::Wildcard(_)) {
        apply_dispatch_projection(&mut shape, DispatchShape::ErasedObject);
    }
    shape
}

pub fn borrowed_string_content_shape() -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(JavaType::String);
    apply_borrowed_string_content_projection(&mut shape);
    shape
}

pub fn apply_borrowed_string_content_projection(shape: &mut RustSlotShape) {
    apply_required_ownership(shape, OwnershipShape::SharedBorrow);
    apply_required_view(shape, ViewShape::Str);
}

pub fn apply_borrowed_sequence_content_projection(shape: &mut RustSlotShape) {
    apply_required_ownership(shape, OwnershipShape::SharedBorrow);
    apply_required_view(shape, ViewShape::Slice);
}

pub fn borrowed_sequence_element_shape(element_ty: JavaType) -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(element_ty);
    apply_borrowed_sequence_element_projection(&mut shape);
    shape
}

pub fn apply_borrowed_sequence_element_projection(shape: &mut RustSlotShape) {
    apply_required_ownership(shape, OwnershipShape::SharedBorrow);
}

pub fn apply_required_ownership(shape: &mut RustSlotShape, ownership: OwnershipShape) {
    shape.ownership = ownership;
}

pub fn apply_required_view(shape: &mut RustSlotShape, view: ViewShape) {
    shape.view = view;
}

pub fn apply_required_dispatch(shape: &mut RustSlotShape, dispatch: DispatchShape) {
    apply_dispatch_projection(shape, dispatch);
}

fn apply_dispatch_projection(shape: &mut RustSlotShape, dispatch: DispatchShape) {
    let policy = dispatch_projection_policy(dispatch);
    shape.dispatch = policy.dispatch;
    if let Some(ownership) = policy.ownership {
        shape.ownership = ownership;
    }
    if let Some(view) = policy.view {
        shape.view = view;
    }
}

pub fn dispatch_projection_policy(dispatch: DispatchShape) -> DispatchProjectionPolicy {
    match dispatch {
        DispatchShape::DynTrait { .. } => DispatchProjectionPolicy {
            dispatch,
            ownership: Some(OwnershipShape::SharedBorrow),
            view: Some(ViewShape::DynTraitView),
        },
        DispatchShape::BoxedDynTrait { .. } => DispatchProjectionPolicy {
            dispatch,
            ownership: Some(OwnershipShape::Owned),
            view: Some(ViewShape::DynTraitView),
        },
        DispatchShape::ClosedEnum { .. } => DispatchProjectionPolicy {
            dispatch,
            ownership: None,
            view: Some(ViewShape::EnumView),
        },
        DispatchShape::ErasedObject => DispatchProjectionPolicy {
            dispatch,
            ownership: Some(OwnershipShape::Owned),
            view: Some(ViewShape::ErasedObjectView),
        },
        DispatchShape::Concrete => DispatchProjectionPolicy {
            dispatch,
            ownership: None,
            view: None,
        },
    }
}

pub fn storage_shape_for_java_type(
    representations: &RepresentationPlan,
    java_ty: JavaType,
) -> RustSlotShape {
    let mut shape = RustSlotShape::for_storage(java_ty.clone());
    apply_dispatch_projection(
        &mut shape,
        escaping_role_dispatch_policy(representations.dispatch_for(&java_ty)).projected_dispatch,
    );
    shape
}

pub fn param_shape_for_java_type(
    representations: &RepresentationPlan,
    java_ty: JavaType,
    is_varargs: bool,
) -> RustSlotShape {
    let mut shape = value_shape_for_java_type(representations, java_ty);
    if is_varargs || matches!(shape.java_ty, JavaType::Array(_)) {
        shape.ownership = OwnershipShape::SharedBorrow;
        shape.view = ViewShape::Slice;
    }
    shape
}

pub fn return_shape_for_java_type(
    representations: &RepresentationPlan,
    java_ty: &JavaType,
) -> Option<RustSlotShape> {
    if matches!(java_ty, JavaType::Void) {
        None
    } else {
        let mut shape = RustSlotShape::conservative(java_ty.clone());
        apply_dispatch_projection(
            &mut shape,
            escaping_role_dispatch_policy(representations.dispatch_for(java_ty)).projected_dispatch,
        );
        Some(shape)
    }
}

pub fn escaping_role_dispatch_policy(dispatch: DispatchShape) -> EscapingRoleDispatchPolicy {
    let projected_dispatch = match &dispatch {
        DispatchShape::DynTrait { trait_name } => DispatchShape::BoxedDynTrait {
            trait_name: trait_name.clone(),
        },
        _ => dispatch.clone(),
    };
    EscapingRoleDispatchPolicy {
        source_dispatch: dispatch,
        projected_dispatch,
    }
}

pub fn dispatch_requirement_projection_policy(
    role: DispatchRequirementProjectionRole,
    source_dispatch: DispatchShape,
    requirement: DispatchRequirementKind,
) -> DispatchRequirementProjectionPolicy {
    let projected_dispatch = match (requirement, role) {
        (
            DispatchRequirementKind::NeedsOwnedPolymorphicStorage,
            DispatchRequirementProjectionRole::Storage | DispatchRequirementProjectionRole::Return,
        ) => Some(escaping_role_dispatch_policy(source_dispatch.clone()).projected_dispatch),
        (
            DispatchRequirementKind::NeedsBorrowedTraitView,
            DispatchRequirementProjectionRole::Param,
        ) => Some(source_dispatch.clone()),
        (DispatchRequirementKind::NeedsUnknownRuntimeObject, _)
        | (DispatchRequirementKind::RejectTraitObject, _) => Some(DispatchShape::ErasedObject),
        _ => None,
    };
    DispatchRequirementProjectionPolicy {
        role,
        source_dispatch,
        requirement,
        projected_dispatch,
    }
}

pub fn indexed_read_shape(array_shape: &RustSlotShape) -> RustSlotShape {
    indexed_read_shape_with_facts(array_shape, &FactSet::default())
}

pub fn iteration_element_shape(element_ty: JavaType, facts: &FactSet) -> RustSlotShape {
    let effective_facts = EffectiveFacts::new(facts);
    iteration_element_shape_with_effective_facts(element_ty, &effective_facts)
}

pub fn iteration_element_shape_with_effective_facts(
    element_ty: JavaType,
    facts: &EffectiveFacts,
) -> RustSlotShape {
    let policy = iteration_element_projection_policy_with_effective_facts(element_ty, facts);
    let mut shape = RustSlotShape::conservative(policy.element_ty);
    shape.ownership = policy.ownership;
    shape
}

pub fn indexed_read_shape_with_facts(
    array_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustSlotShape {
    let effective_facts = EffectiveFacts::new(facts);
    indexed_read_shape_with_effective_facts(array_shape, &effective_facts)
}

pub fn indexed_read_shape_with_effective_facts(
    array_shape: &RustSlotShape,
    facts: &EffectiveFacts,
) -> RustSlotShape {
    let element_ty =
        sequence_element_projection_policy_with_effective_facts(&array_shape.java_ty, facts)
            .element_ty
            .unwrap_or_else(|| JavaType::Unknown("array_access".to_string()));
    let policy = indexed_element_projection_policy_with_effective_facts(
        element_ty,
        array_shape.ownership,
        &array_shape.view,
        facts,
    );
    let mut element_shape = RustSlotShape::conservative(policy.element_ty);
    element_shape.ownership = policy.ownership;

    element_shape
}

pub fn indexed_element_projection_policy(
    element_ty: JavaType,
    container_ownership: OwnershipShape,
    container_view: &ViewShape,
    facts: &FactSet,
) -> IndexedElementProjectionPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    indexed_element_projection_policy_with_effective_facts(
        element_ty,
        container_ownership,
        container_view,
        &effective_facts,
    )
}

pub fn indexed_element_projection_policy_with_effective_facts(
    element_ty: JavaType,
    container_ownership: OwnershipShape,
    container_view: &ViewShape,
    facts: &EffectiveFacts,
) -> IndexedElementProjectionPolicy {
    let capability = shape_projection_capability_policy_with_effective_facts(&element_ty, facts);
    let projected_ownership = if capability.value_semantics {
        OwnershipShape::CopyValue
    } else if container_ownership == OwnershipShape::SharedBorrow
        || *container_view == ViewShape::Slice
    {
        OwnershipShape::SharedBorrow
    } else {
        OwnershipShape::Owned
    };
    IndexedElementProjectionPolicy {
        element_ty,
        ownership: projected_ownership,
    }
}

pub fn iteration_element_projection_policy(
    element_ty: JavaType,
    facts: &FactSet,
) -> IndexedElementProjectionPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    iteration_element_projection_policy_with_effective_facts(element_ty, &effective_facts)
}

pub fn iteration_element_projection_policy_with_effective_facts(
    element_ty: JavaType,
    facts: &EffectiveFacts,
) -> IndexedElementProjectionPolicy {
    let capability = shape_projection_capability_policy_with_effective_facts(&element_ty, facts);
    let projected_ownership = if capability.value_semantics {
        RustSlotShape::conservative(element_ty.clone()).ownership
    } else {
        OwnershipShape::SharedBorrow
    };
    IndexedElementProjectionPolicy {
        element_ty,
        ownership: projected_ownership,
    }
}

pub fn indexed_assignment_shape(array_shape: &RustSlotShape) -> Option<RustSlotShape> {
    sequence_element_type(&array_shape.java_ty).map(RustSlotShape::for_storage)
}

pub fn indexed_assignment_shape_with_facts(
    array_shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let effective_facts = EffectiveFacts::new(facts);
    indexed_assignment_shape_with_effective_facts(array_shape, &effective_facts)
}

pub fn indexed_assignment_shape_with_effective_facts(
    array_shape: &RustSlotShape,
    facts: &EffectiveFacts,
) -> Option<RustSlotShape> {
    sequence_element_projection_policy_with_effective_facts(&array_shape.java_ty, facts)
        .element_ty
        .map(RustSlotShape::for_storage)
}

pub fn sequence_element_projection_policy(
    java_ty: &JavaType,
    facts: &FactSet,
) -> SequenceElementProjectionPolicy {
    let effective_facts = EffectiveFacts::new(facts);
    sequence_element_projection_policy_with_effective_facts(java_ty, &effective_facts)
}

pub fn sequence_element_projection_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts,
) -> SequenceElementProjectionPolicy {
    let capability = shape_projection_capability_policy_with_effective_facts(java_ty, facts);
    SequenceElementProjectionPolicy {
        java_ty: capability.java_ty,
        element_ty: capability.sequence_element,
    }
}
