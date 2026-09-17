use crate::effective_facts::EffectiveFacts;
use crate::facts::{DispatchRequirementKind, FacadeSlot, FactSet, HierarchyStrategy};
use crate::java_ir::JavaType;
use crate::representation::RepresentationPlan;
use crate::slot_shape::{DispatchShape, MutabilityShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::types::{optional_element_type, optional_java_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DispatchProjectionPolicy {
    dispatch: DispatchShape,
    ownership: Option<OwnershipShape>,
    view: Option<ViewShape>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedElementProjectionPolicy {
    element_ty: JavaType,
    ownership: OwnershipShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceElementProjectionPolicy {
    element_ty: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeProjectionCapabilityPolicy {
    value_semantics: bool,
    sequence_element: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscapingRoleDispatchPolicy {
    projected_dispatch: DispatchShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchRequirementProjectionRole {
    Param,
    Return,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchRequirementProjectionPolicy {
    role: DispatchRequirementProjectionRole,
    requirement: DispatchRequirementKind,
    projected_dispatch: Option<DispatchShape>,
}

impl DispatchProjectionPolicy {
    pub(crate) fn new(
        dispatch: DispatchShape,
        ownership: Option<OwnershipShape>,
        view: Option<ViewShape>,
    ) -> Self {
        Self {
            dispatch,
            ownership,
            view,
        }
    }

    pub(crate) fn dispatch(&self) -> &DispatchShape {
        &self.dispatch
    }

    pub(crate) fn ownership(&self) -> Option<OwnershipShape> {
        self.ownership
    }

    pub(crate) fn view(&self) -> Option<&ViewShape> {
        self.view.as_ref()
    }
}

impl IndexedElementProjectionPolicy {
    pub fn new(element_ty: JavaType, ownership: OwnershipShape) -> Self {
        Self {
            element_ty,
            ownership,
        }
    }

    pub fn element_ty(&self) -> &JavaType {
        &self.element_ty
    }

    pub fn into_element_ty(self) -> JavaType {
        self.element_ty
    }

    pub fn ownership(&self) -> OwnershipShape {
        self.ownership
    }
}

impl SequenceElementProjectionPolicy {
    pub fn new(element_ty: Option<JavaType>) -> Self {
        Self { element_ty }
    }

    pub fn element_ty(&self) -> Option<&JavaType> {
        self.element_ty.as_ref()
    }

    pub fn into_element_ty(self) -> Option<JavaType> {
        self.element_ty
    }
}

impl ShapeProjectionCapabilityPolicy {
    pub fn new(value_semantics: bool, sequence_element: Option<JavaType>) -> Self {
        Self {
            value_semantics,
            sequence_element,
        }
    }

    pub fn value_semantics(&self) -> bool {
        self.value_semantics
    }

    pub fn sequence_element(&self) -> Option<&JavaType> {
        self.sequence_element.as_ref()
    }

    pub fn into_sequence_element(self) -> Option<JavaType> {
        self.sequence_element
    }
}

impl EscapingRoleDispatchPolicy {
    pub fn new(projected_dispatch: DispatchShape) -> Self {
        Self { projected_dispatch }
    }

    pub fn projected_dispatch(&self) -> &DispatchShape {
        &self.projected_dispatch
    }

    pub fn into_projected_dispatch(self) -> DispatchShape {
        self.projected_dispatch
    }
}

impl DispatchRequirementProjectionPolicy {
    pub fn new(
        role: DispatchRequirementProjectionRole,
        requirement: DispatchRequirementKind,
        projected_dispatch: Option<DispatchShape>,
    ) -> Self {
        Self {
            role,
            requirement,
            projected_dispatch,
        }
    }

    pub fn role(&self) -> DispatchRequirementProjectionRole {
        self.role
    }

    pub fn requirement(&self) -> DispatchRequirementKind {
        self.requirement
    }

    pub fn projected_dispatch(&self) -> Option<&DispatchShape> {
        self.projected_dispatch.as_ref()
    }

    pub fn into_projected_dispatch(self) -> Option<DispatchShape> {
        self.projected_dispatch
    }
}

pub fn shape_projection_capability_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts,
) -> ShapeProjectionCapabilityPolicy {
    ShapeProjectionCapabilityPolicy::new(
        facts.is_copy_type(java_ty),
        facts.sequence_element_type_for(java_ty),
    )
}

pub fn value_shape_for_java_type(
    representations: &RepresentationPlan,
    java_ty: JavaType,
) -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(java_ty.clone());
    let dispatch = if java_type_is_erased_object_boundary(&java_ty) {
        DispatchShape::ErasedObject
    } else if java_type_is_known_platform_facade(&java_ty) {
        DispatchShape::Concrete
    } else {
        dispatch_for_java_type_or_optional_element(representations, &java_ty)
    };
    apply_dispatch_projection(&mut shape, dispatch);
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
    if java_type_is_erased_object_boundary(&shape.java_ty)
        || (matches!(shape.java_ty, JavaType::Unknown(_) | JavaType::Wildcard(_))
            && !java_type_is_known_platform_facade(&shape.java_ty))
    {
        apply_dispatch_projection(&mut shape, DispatchShape::ErasedObject);
    }
    shape
}

fn java_type_is_erased_object_boundary(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Named { name, .. } | JavaType::Unknown(name)
            if name == "Object" || name == "java.lang.Object"
    ) || matches!(java_ty, JavaType::Wildcard(_))
}

fn java_type_is_known_platform_facade(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::is_java_platform_facade_type_name(name)
        }
        _ => false,
    }
}

pub fn borrowed_string_content_shape() -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(JavaType::String);
    apply_borrowed_string_content_projection(&mut shape);
    shape
}

pub fn storage_shape(java_ty: JavaType) -> RustSlotShape {
    RustSlotShape::for_storage(java_ty)
}

pub fn storage_arg_shape_with_effective_facts(
    java_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustSlotShape {
    let mut shape = RustSlotShape::for_storage(java_ty.clone());
    if let Some(HierarchyStrategy::DynTrait { trait_name }) =
        facts.object_representation_override_strategy_for(&java_ty)
    {
        apply_required_dispatch(
            &mut shape,
            DispatchShape::BoxedDynTrait {
                trait_name: trait_name.clone(),
            },
        );
        return shape;
    }
    if let JavaType::Named { name, args } = &java_ty
        && args.is_empty()
        && facts.object_safe_trait_fact_for_name(name) == Some(true)
    {
        apply_required_dispatch(
            &mut shape,
            DispatchShape::BoxedDynTrait {
                trait_name: name.clone(),
            },
        );
        return shape;
    }
    if matches!(
        facts.object_representation_override_strategy_for(&java_ty),
        Some(HierarchyStrategy::ErasedObject)
    ) && !facts.is_final_type(&java_ty)
        && !crate::javalibs::generic_facade_interface_erases_to_named_rust_type(&java_ty)
        || java_type_is_erased_object_storage(&java_ty) && !facts.is_final_type(&java_ty)
    {
        apply_required_dispatch(&mut shape, DispatchShape::ErasedObject);
    }
    shape
}

pub fn rust_slot_shape_from_facade_arg_slot(
    slot: &FacadeSlot,
    java_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustSlotShape {
    match slot {
        FacadeSlot::Optional(inner) => {
            if let Some(element_ty) = optional_element_type(&java_ty) {
                let mut shape = rust_slot_shape_from_facade_arg_slot(inner, element_ty, facts);
                shape.java_ty = java_ty;
                return shape;
            }
            let mut shape = rust_slot_shape_from_facade_arg_slot(
                inner,
                optional_java_type(java_ty.clone()),
                facts,
            );
            shape.java_ty = optional_java_type(java_ty);
            shape
        }
        FacadeSlot::Borrowed => {
            let mut shape = RustSlotShape::conservative(java_ty.clone());
            if java_ty == JavaType::String {
                apply_borrowed_string_content_projection(&mut shape);
            } else if matches!(java_ty, JavaType::Array(_)) {
                apply_borrowed_sequence_content_projection(&mut shape);
            } else if !matches!(java_ty, JavaType::Primitive(_) | JavaType::Void) {
                shape.ownership = OwnershipShape::SharedBorrow;
            }
            shape
        }
        FacadeSlot::MutBorrowed => {
            let mut shape = RustSlotShape::conservative(java_ty);
            if !matches!(shape.java_ty, JavaType::Primitive(_) | JavaType::Void) {
                shape.ownership = OwnershipShape::MutableBorrow;
                shape.mutability = MutabilityShape::Mutable;
            }
            shape
        }
        FacadeSlot::Slice(inner) => {
            let shape_ty = optional_element_type(&java_ty).unwrap_or(java_ty);
            let mut shape = RustSlotShape::conservative(shape_ty);
            apply_borrowed_sequence_content_projection(&mut shape);
            if inner.contains_mut_borrow() {
                shape.ownership = OwnershipShape::MutableBorrow;
                shape.mutability = MutabilityShape::Mutable;
            }
            shape
        }
        FacadeSlot::Owned
        | FacadeSlot::Index
        | FacadeSlot::DynTrait(_)
        | FacadeSlot::CrateHandle(_) => storage_arg_shape_with_effective_facts(java_ty, facts),
    }
}

pub fn java_type_is_erased_object_storage(java_ty: &JavaType) -> bool {
    matches!(
        java_ty,
        JavaType::Named { name, args }
            if args.is_empty()
                && crate::stdlib::canonical_external_receiver_type_key(name) == "Object"
    )
}

pub fn value_shape(java_ty: JavaType) -> RustSlotShape {
    RustSlotShape::conservative(java_ty)
}

pub fn owned_string_value_shape() -> RustSlotShape {
    RustSlotShape::conservative(JavaType::String)
}

pub fn string_list_value_shape() -> RustSlotShape {
    RustSlotShape::conservative(crate::types::list_java_type(JavaType::String))
}

pub(crate) fn apply_borrowed_string_content_projection(shape: &mut RustSlotShape) {
    apply_required_dispatch(shape, DispatchShape::Concrete);
    apply_required_ownership(shape, OwnershipShape::SharedBorrow);
    apply_required_view(shape, ViewShape::Str);
}

pub(crate) fn apply_borrowed_sequence_content_projection(shape: &mut RustSlotShape) {
    apply_required_ownership(shape, OwnershipShape::SharedBorrow);
    apply_required_view(shape, ViewShape::Slice);
}

pub fn borrowed_sequence_element_shape(element_ty: JavaType) -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(element_ty);
    apply_borrowed_sequence_element_projection(&mut shape);
    if matches!(shape.java_ty, JavaType::Array(_)) {
        apply_required_view(&mut shape, ViewShape::Slice);
    }
    shape
}

pub fn shape_is_erased_object_boundary(shape: &RustSlotShape) -> bool {
    matches!(shape.dispatch, DispatchShape::ErasedObject)
        || matches!(shape.view, ViewShape::ErasedObjectView)
        || shape.optional_element_type().is_some_and(|element_ty| {
            matches!(element_ty, JavaType::Named { name, .. } | JavaType::Unknown(name)
                if name == "Object" || name == "java.lang.Object")
        })
}

pub fn shape_is_concrete_native(shape: &RustSlotShape) -> bool {
    matches!(shape.dispatch, DispatchShape::Concrete) && matches!(shape.view, ViewShape::Native)
}

pub fn shape_is_owned_non_erased(shape: &RustSlotShape) -> bool {
    shape.ownership == OwnershipShape::Owned && !shape_is_erased_object_boundary(shape)
}

pub fn shape_is_borrowed_string_content(shape: &RustSlotShape) -> bool {
    shape.java_ty == JavaType::String
        && shape.ownership == OwnershipShape::SharedBorrow
        && shape.view == ViewShape::Str
}

pub fn shape_is_owned(shape: &RustSlotShape) -> bool {
    shape.ownership == OwnershipShape::Owned
}

pub(crate) fn apply_borrowed_sequence_element_projection(shape: &mut RustSlotShape) {
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
    shape.dispatch = policy.dispatch().clone();
    if let Some(ownership) = policy.ownership() {
        shape.ownership = ownership;
    }
    if let Some(view) = policy.view() {
        shape.view = view.clone();
    }
}

pub(crate) fn dispatch_projection_policy(dispatch: DispatchShape) -> DispatchProjectionPolicy {
    match dispatch {
        DispatchShape::DynTrait { .. } => DispatchProjectionPolicy::new(
            dispatch,
            Some(OwnershipShape::SharedBorrow),
            Some(ViewShape::DynTraitView),
        ),
        DispatchShape::BoxedDynTrait { .. } => DispatchProjectionPolicy::new(
            dispatch,
            Some(OwnershipShape::Owned),
            Some(ViewShape::DynTraitView),
        ),
        DispatchShape::ClosedEnum { .. } => {
            DispatchProjectionPolicy::new(dispatch, None, Some(ViewShape::EnumView))
        }
        DispatchShape::ErasedObject => DispatchProjectionPolicy::new(
            dispatch,
            Some(OwnershipShape::Owned),
            Some(ViewShape::ErasedObjectView),
        ),
        DispatchShape::Concrete => DispatchProjectionPolicy::new(dispatch, None, None),
    }
}

pub fn storage_shape_for_java_type(
    representations: &RepresentationPlan,
    java_ty: JavaType,
) -> RustSlotShape {
    let mut shape = RustSlotShape::for_storage(java_ty.clone());
    let dispatch = if java_type_is_erased_object_boundary(&java_ty) {
        DispatchShape::ErasedObject
    } else {
        dispatch_for_java_type_or_optional_element(representations, &java_ty)
    };
    apply_dispatch_projection(
        &mut shape,
        escaping_role_dispatch_policy(dispatch).into_projected_dispatch(),
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
        let dispatch = if java_type_is_erased_object_boundary(java_ty) {
            DispatchShape::ErasedObject
        } else {
            dispatch_for_java_type_or_optional_element(representations, java_ty)
        };
        apply_dispatch_projection(
            &mut shape,
            escaping_role_dispatch_policy(dispatch).into_projected_dispatch(),
        );
        Some(shape)
    }
}

fn dispatch_for_java_type_or_optional_element(
    representations: &RepresentationPlan,
    java_ty: &JavaType,
) -> DispatchShape {
    if let Some(element_ty) = optional_element_type(java_ty) {
        if java_type_is_erased_object_boundary(&element_ty) {
            DispatchShape::ErasedObject
        } else if java_type_is_known_platform_facade(&element_ty) {
            DispatchShape::Concrete
        } else {
            representations.dispatch_for(&element_ty)
        }
    } else {
        representations.dispatch_for(java_ty)
    }
}

pub fn escaping_role_dispatch_policy(dispatch: DispatchShape) -> EscapingRoleDispatchPolicy {
    let projected_dispatch = match &dispatch {
        DispatchShape::DynTrait { trait_name } => DispatchShape::BoxedDynTrait {
            trait_name: trait_name.clone(),
        },
        _ => dispatch.clone(),
    };
    EscapingRoleDispatchPolicy::new(projected_dispatch)
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
        ) => Some(escaping_role_dispatch_policy(source_dispatch.clone()).into_projected_dispatch()),
        (
            DispatchRequirementKind::NeedsBorrowedTraitView,
            DispatchRequirementProjectionRole::Param,
        ) => Some(source_dispatch.clone()),
        (DispatchRequirementKind::NeedsUnknownRuntimeObject, _) => {
            Some(DispatchShape::ErasedObject)
        }
        // Rejecting a *trait object* does not require erasing to `Box<dyn Any>`:
        // a concrete type is not a trait object to begin with. Erasing a
        // concrete receiver produced `Box<dyn Any>` bindings whose every method
        // call then failed to compile, which was the largest error class in the
        // corpora. Keep concrete receivers concrete and erase only genuinely
        // polymorphic ones.
        (DispatchRequirementKind::RejectTraitObject, _) => Some(match source_dispatch {
            DispatchShape::Concrete => DispatchShape::Concrete,
            _ => DispatchShape::ErasedObject,
        }),
        _ => None,
    };
    DispatchRequirementProjectionPolicy::new(role, requirement, projected_dispatch)
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
    let ownership = policy.ownership();
    let mut shape = RustSlotShape::conservative(policy.into_element_ty());
    shape.ownership = ownership;
    shape
}

pub fn indexed_read_shape_with_facts(
    array_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustSlotShape {
    let effective_facts = EffectiveFacts::new(facts);
    indexed_read_shape_with_effective_facts(array_shape, &effective_facts)
}

pub fn indexed_read_shape_for_access_with_facts(
    array_shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<RustSlotShape> {
    let effective_facts = EffectiveFacts::new(facts);
    indexed_read_shape_for_access_with_effective_facts(array_shape, &effective_facts)
}

pub fn indexed_read_shape_for_access_with_effective_facts(
    array_shape: &RustSlotShape,
    facts: &EffectiveFacts,
) -> Option<RustSlotShape> {
    if sequence_element_projection_policy_with_effective_facts(&array_shape.java_ty, facts)
        .element_ty()
        .is_some()
    {
        return Some(indexed_read_shape_with_effective_facts(array_shape, facts));
    }

    let inner_ty = optional_element_type(&array_shape.java_ty)?;
    if sequence_element_projection_policy_with_effective_facts(&inner_ty, facts)
        .element_ty()
        .is_none()
    {
        return None;
    }
    let mut inner_shape = array_shape.clone();
    inner_shape.java_ty = inner_ty;
    Some(indexed_read_shape_with_effective_facts(&inner_shape, facts))
}

pub fn indexed_read_shape_with_effective_facts(
    array_shape: &RustSlotShape,
    facts: &EffectiveFacts,
) -> RustSlotShape {
    let element_ty =
        sequence_element_projection_policy_with_effective_facts(&array_shape.java_ty, facts)
            .into_element_ty()
            .unwrap_or_else(|| JavaType::Unknown("array_access".to_string()));
    let policy = indexed_element_projection_policy_with_effective_facts(
        element_ty,
        array_shape.ownership,
        &array_shape.view,
        facts,
    );
    let ownership = policy.ownership();
    let mut element_shape = RustSlotShape::conservative(policy.into_element_ty());
    element_shape.ownership = ownership;
    if matches!(element_shape.java_ty, JavaType::Array(_)) {
        apply_required_view(&mut element_shape, ViewShape::Slice);
    }
    // Reference-array storage is `Vec<Option<T>>`, including when the array
    // receiver itself is non-null. Keep that carrier in the read shape so
    // argument lowering can project `Option<&T>` rather than borrow an
    // `Option<T>` container as though it were a `T`.
    if let JavaType::Array(element_ty) = &array_shape.java_ty
        && java_array_element_needs_nullable_access_carrier(element_ty)
    {
        element_shape.java_ty = optional_java_type(element_shape.java_ty);
    }

    element_shape
}

pub fn indexed_element_projection_policy_with_effective_facts(
    element_ty: JavaType,
    _container_ownership: OwnershipShape,
    _container_view: &ViewShape,
    facts: &EffectiveFacts,
) -> IndexedElementProjectionPolicy {
    let capability = shape_projection_capability_policy_with_effective_facts(&element_ty, facts);
    let projected_ownership = if capability.value_semantics() {
        OwnershipShape::CopyValue
    } else {
        OwnershipShape::SharedBorrow
    };
    IndexedElementProjectionPolicy::new(element_ty, projected_ownership)
}

pub fn iteration_element_projection_policy_with_effective_facts(
    element_ty: JavaType,
    facts: &EffectiveFacts,
) -> IndexedElementProjectionPolicy {
    let capability = shape_projection_capability_policy_with_effective_facts(&element_ty, facts);
    let projected_ownership = if capability.value_semantics() {
        RustSlotShape::conservative(element_ty.clone()).ownership
    } else {
        OwnershipShape::SharedBorrow
    };
    IndexedElementProjectionPolicy::new(element_ty, projected_ownership)
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
        .into_element_ty()
        .map(RustSlotShape::for_storage)
}

fn java_array_element_needs_nullable_access_carrier(element_ty: &JavaType) -> bool {
    !matches!(
        element_ty,
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_)
    ) && optional_element_type(element_ty).is_none()
}

pub fn sequence_element_projection_policy_with_effective_facts(
    java_ty: &JavaType,
    facts: &EffectiveFacts,
) -> SequenceElementProjectionPolicy {
    let capability = shape_projection_capability_policy_with_effective_facts(java_ty, facts);
    SequenceElementProjectionPolicy::new(capability.into_sequence_element())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_projection_policy_owns_dispatch_axis_effects() {
        assert_eq!(
            dispatch_projection_policy(DispatchShape::Concrete),
            DispatchProjectionPolicy::new(DispatchShape::Concrete, None, None)
        );
        assert_eq!(
            dispatch_projection_policy(DispatchShape::DynTrait {
                trait_name: "Animal".to_string(),
            }),
            DispatchProjectionPolicy::new(
                DispatchShape::DynTrait {
                    trait_name: "Animal".to_string(),
                },
                Some(OwnershipShape::SharedBorrow),
                Some(ViewShape::DynTraitView),
            )
        );
        assert_eq!(
            dispatch_projection_policy(DispatchShape::BoxedDynTrait {
                trait_name: "Animal".to_string(),
            }),
            DispatchProjectionPolicy::new(
                DispatchShape::BoxedDynTrait {
                    trait_name: "Animal".to_string(),
                },
                Some(OwnershipShape::Owned),
                Some(ViewShape::DynTraitView),
            )
        );
        assert_eq!(
            dispatch_projection_policy(DispatchShape::ClosedEnum {
                enum_name: "AnimalValue".to_string(),
            }),
            DispatchProjectionPolicy::new(
                DispatchShape::ClosedEnum {
                    enum_name: "AnimalValue".to_string(),
                },
                None,
                Some(ViewShape::EnumView),
            )
        );
        assert_eq!(
            dispatch_projection_policy(DispatchShape::ErasedObject),
            DispatchProjectionPolicy::new(
                DispatchShape::ErasedObject,
                Some(OwnershipShape::Owned),
                Some(ViewShape::ErasedObjectView),
            )
        );
    }
}
