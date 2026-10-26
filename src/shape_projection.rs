use crate::effective_facts::EffectiveFacts;
use crate::facts::{DispatchRequirementKind, FacadeSlot, FactSet, HierarchyStrategy};
use crate::java_ir::JavaType;
use crate::representation::RepresentationPlan;
use crate::slot_shape::{DispatchShape, MutabilityShape, OwnershipShape, RustSlotShape, ViewShape};
use crate::types::{optional_element_type, optional_java_type};

/// Carry a solved interface argument domain into one SAM invocation value.
/// For Iterator<T>-style values the interface's T belongs below the result's
/// outer carrier; it must not turn that carrier into an unrelated allocation.
pub(crate) fn inherit_native_functional_value_domain(
    value: &mut RustSlotShape,
    parent: &RustSlotShape,
    source: crate::stdlib::FunctionalTypeSource,
) {
    let Some((index, projection)) = source.type_argument_projection() else {
        return;
    };
    value.inherit_nested_collection_views(parent);
    for (path, placement) in &parent.nested_native_allocations {
        if let Some(suffix) = path.strip_prefix(&[index]) {
            let mut path = projection.to_vec();
            path.extend_from_slice(suffix);
            value.require_native_allocation_at(&path, *placement);
        }
    }
}

/// Storage projection shared by insertions, reads, display callbacks and
/// capability proofs. It must not replace an owned interface with a loan.
pub(crate) fn apply_source_interface_reference_dispatch(
    shape: &mut RustSlotShape,
    representations: &RepresentationPlan,
) {
    let java_ty = shape
        .optional_element_type()
        .unwrap_or_else(|| shape.java_ty.clone());
    if let DispatchShape::ReferenceDynTrait { trait_name, sync } =
        representations.dispatch_for(&java_ty)
    {
        shape.dispatch = DispatchShape::ReferenceDynTrait { trait_name, sync };
        shape.view = ViewShape::DynTraitView;
        shape.borrow_target = crate::slot_shape::BorrowTarget::Carrier;
    }
}

pub(crate) fn native_collection_storage_value_shape(
    shape: RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustSlotShape {
    let mut projected = storage_shape_for_java_type(representations, shape.java_ty.clone());
    projected.ownership = shape.ownership;
    projected.native_allocation = shape.native_allocation;
    projected.nested_native_allocations = shape.nested_native_allocations;
    projected.view = shape.view;
    projected.mutability = shape.mutability;
    projected.nested_collection_views = shape.nested_collection_views;
    if matches!(&shape.dispatch, DispatchShape::BoxedDynTrait { .. })
        && !matches!(projected.dispatch, DispatchShape::ReferenceDynTrait { .. })
    {
        projected.dispatch = shape.dispatch;
    }
    normalize_native_collection_storage_shape(projected, facts, representations)
}

pub(crate) fn normalize_native_collection_storage_shape(
    mut shape: RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> RustSlotShape {
    // The generic-argument renderer owns conservative facade representation.
    if matches!(shape.dispatch, DispatchShape::ErasedObject) {
        let expected = crate::slot_render::rust_type_for_java_generic_arg_with_representations(
            &shape.java_ty,
            &EffectiveFacts::new(facts),
            representations,
        );
        if crate::slot_render::rust_type_for_slot_with_facts_and_representations(
            &shape,
            facts,
            representations,
        ) != expected
        {
            let mut concrete = shape.clone();
            concrete.dispatch = DispatchShape::Concrete;
            concrete.view = ViewShape::Native;
            if crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                &concrete,
                facts,
                representations,
            ) == expected
            {
                shape = concrete;
            }
        }
    }
    shape
}

pub(crate) fn native_collection_stored_child_shape(
    parent: &RustSlotShape,
    index: usize,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<RustSlotShape> {
    let ty = RustSlotShape::native_child_type_at_path(&parent.java_ty, &[index])?;
    let mut shape = native_collection_storage_value_shape(
        storage_arg_shape_with_effective_facts(ty, &EffectiveFacts::new(facts)),
        facts,
        representations,
    );
    shape.inherit_native_collection_child(parent, index);
    Some(shape)
}

/// Retained element operations transfer an owned Java reference out of their
/// short storage loan. Successful indexed reads return exactly the stored
/// carrier; missing and stored-null queue values share one nullable carrier.
pub(crate) fn retained_native_collection_result_shape(
    parent: &RustSlotShape,
    method: crate::call_plan::NativeCollectionMethod,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<RustSlotShape> {
    if !(method.is_queue_endpoint()
        || matches!(
            method,
            crate::call_plan::NativeCollectionMethod::ListGet
                | crate::call_plan::NativeCollectionMethod::ListSet
                | crate::call_plan::NativeCollectionMethod::ListRemoveIndex
        ))
        || !parent.native_allocation.shared()
        || EffectiveFacts::new(facts)
            .retained_native_collection_kind(&parent.java_ty)
            .is_none()
    {
        return None;
    }
    let mut child = native_collection_stored_child_shape(parent, 0, facts, representations)?;
    child.ownership = OwnershipShape::Owned;
    child.borrow_target = crate::slot_shape::BorrowTarget::Carrier;
    if method.queue_endpoint_can_be_empty() && child.optional_element_type().is_none() {
        child.java_ty = optional_java_type(child.java_ty);
    }
    Some(child)
}

/// Instantiate a solved source contract without losing its physical views or
/// nullable carrier. Direct owner parameters additionally need the concrete
/// reference representation, not merely a different Java type spelling.
pub(crate) fn specialize_source_contract_shape(
    shape: &RustSlotShape,
    bindings: &std::collections::HashMap<String, JavaType>,
    representations: &RepresentationPlan,
) -> RustSlotShape {
    let java_ty = crate::resolve::substitute_method_type_variables(&shape.java_ty, bindings);
    if java_ty == shape.java_ty {
        return shape.clone();
    }
    let specialized = specialize_slot_shape_java_type(shape, &java_ty);
    let original = shape
        .optional_element_type()
        .unwrap_or_else(|| shape.java_ty.clone());
    if matches!(&original, JavaType::Named { name, args } if args.is_empty() && bindings.contains_key(name))
    {
        let mut physical = storage_shape_for_java_type(representations, specialized.java_ty);
        physical.ownership = shape.ownership;
        physical.view = shape.view.clone();
        physical.borrow_target = if matches!(
            shape.ownership,
            OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow
        ) {
            crate::slot_shape::BorrowTarget::Carrier
        } else {
            shape.borrow_target
        };
        physical
    } else {
        specialized
    }
}

/// Specialize the declaring implementation's plan, not the concrete owner's
/// potentially covariant override slot. Shared by body and shell emission.
pub(crate) fn specialize_source_method_signature(
    signature: &crate::signature::MethodSignaturePlan,
    method: &crate::java_ir::MethodDecl,
    bindings: &std::collections::HashMap<String, JavaType>,
    representations: &RepresentationPlan,
) -> crate::signature::MethodSignaturePlan {
    let bindings =
        crate::java_type_substitution::without_shadowed_binders(bindings, &method.type_params);
    let project =
        |shape: &RustSlotShape| specialize_source_contract_shape(shape, &bindings, representations);
    let params = signature
        .params()
        .iter()
        .map(|param| {
            crate::signature::ParamPlan::new(
                param.name().to_string(),
                crate::resolve::substitute_method_type_variables(param.java_ty(), &bindings),
                param.is_varargs(),
                project(param.shape()),
                param.binding_mutable(),
            )
        })
        .collect();
    let mut locals =
        crate::java_type_substitution::specialized_locals(method, signature.locals(), &bindings);
    for (local, original) in locals.iter_mut().zip(signature.locals()) {
        *local.shape_mut() = project(original.shape());
    }
    crate::signature::MethodSignaturePlan::new(
        params,
        signature.return_shape().map(project),
        locals,
        signature.receiver(),
    )
}

pub(crate) fn project_native_iterable_list_view(
    actual: &RustSlotShape,
    target: &mut RustSlotShape,
    facts: &FactSet,
) -> bool {
    let Some(element) = EffectiveFacts::new(facts)
        .native_iterable_list_projection_element(&actual.java_ty, &target.java_ty)
    else {
        return false;
    };
    let optional = target.optional_element_type().is_some();
    let mut base = target
        .optional_element_type()
        .unwrap_or_else(|| target.java_ty.clone());
    if let JavaType::Named { args, .. } = &mut base {
        *args = vec![element];
    }
    target.java_ty = if optional {
        optional_java_type(base)
    } else {
        base
    };
    target.view = ViewShape::ListInterface;
    target.ownership = OwnershipShape::Owned;
    target.native_allocation = Default::default();
    target.nested_native_allocations = actual.nested_native_allocations.clone();
    target.inherit_nested_collection_views(actual);
    true
}

/// Reference casting changes the nominal type, not the Java null value.
/// Primitive targets are unboxing/numeric operations and retain their own
/// required-value checks instead of acquiring an Option result.
pub(crate) fn preserve_reference_cast_nullability(
    actual: &RustSlotShape,
    target: &mut RustSlotShape,
) {
    if !matches!(target.java_ty, JavaType::Primitive(_) | JavaType::Void)
        && (actual.optional_element_type().is_some()
            || crate::resolve::is_null_type(&actual.java_ty))
        && target.optional_element_type().is_none()
    {
        target.java_ty = optional_java_type(target.java_ty.clone());
    }
}

/// An owned concrete value widened to a source interface must remain owned.
/// Borrowing a temporary here creates a dangling trait view and disagrees with
/// the owned interface slot used by returns, locals and call arguments.
pub(crate) fn preserve_owned_interface_upcast(
    actual: &RustSlotShape,
    target: &mut RustSlotShape,
    facts: &FactSet,
) {
    if actual.ownership != OwnershipShape::Owned {
        return;
    }
    let DispatchShape::DynTrait { trait_name } = &target.dispatch else {
        return;
    };
    let target_type =
        optional_element_type(&target.java_ty).unwrap_or_else(|| target.java_ty.clone());
    let actual_type =
        optional_element_type(&actual.java_ty).unwrap_or_else(|| actual.java_ty.clone());
    let JavaType::Named { name, .. } = &target_type else {
        return;
    };
    if facts.object_safe_trait_fact_for_name(name) == Some(true)
        && facts
            .hierarchy_subtype_policy(&actual_type, &target_type)
            .reaches()
    {
        target.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: trait_name.clone(),
        };
        target.ownership = OwnershipShape::Owned;
    }
}

/// Applies a resolver-specialized Java type to an already-solved Rust slot.
///
/// Signature solving may add `Optional` at any nesting depth.  A resolved
/// generic call supplies the concrete Java parameter type, but replacing the
/// shape wholesale would discard those nullability decisions.  This overlay
/// keeps optional wrappers from the solved ABI and takes nominal/generic
/// arguments from the resolver everywhere else.
pub(crate) fn specialize_slot_shape_java_type(
    shape: &RustSlotShape,
    specialized: &JavaType,
) -> RustSlotShape {
    let mut specialized_shape = shape.clone();
    specialized_shape.java_ty =
        specialize_java_type_preserving_optionality(&shape.java_ty, specialized);
    if !shape.nested_collection_views.is_empty() {
        fn project(
            old: &JavaType,
            new: &JavaType,
            views: &[(JavaType, ViewShape)],
            out: &mut Vec<(JavaType, ViewShape)>,
        ) {
            let old_element = optional_element_type(old);
            let new_element = optional_element_type(new);
            let old = old_element.as_ref().unwrap_or(old);
            let new = new_element.as_ref().unwrap_or(new);
            for (key, view) in views {
                if key == old && !out.iter().any(|(ty, _)| ty == new) {
                    out.push((new.clone(), view.clone()));
                }
            }
            match (old, new) {
                (JavaType::Named { args: old, .. }, JavaType::Named { args: new, .. })
                | (JavaType::Intersection(old), JavaType::Intersection(new)) => {
                    for (old, new) in old.iter().zip(new) {
                        project(old, new, views, out);
                    }
                }
                (JavaType::Array(old), JavaType::Array(new)) => project(old, new, views, out),
                (
                    JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(old))),
                    JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(new))),
                )
                | (
                    JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(old))),
                    JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(new))),
                ) => project(old, new, views, out),
                _ => {}
            }
        }
        let mut views = Vec::new();
        project(
            &shape.java_ty,
            &specialized_shape.java_ty,
            &shape.nested_collection_views,
            &mut views,
        );
        specialized_shape.nested_collection_views = views;
    }
    specialized_shape
}

/// Applies a resolver argument instantiation without undoing reified wildcard
/// witnesses. Ordinary binders beside those witnesses still specialize.
pub(crate) fn specialize_resolved_argument_preserving_source_wildcards(
    shape: &RustSlotShape,
    inferred: &JavaType,
) -> RustSlotShape {
    fn overlay(solved: &JavaType, inferred: &JavaType) -> JavaType {
        use crate::java_ir::WildcardBound;
        if let Some(element) = optional_element_type(solved) {
            let inferred_element = optional_element_type(inferred);
            return optional_java_type(overlay(
                &element,
                inferred_element.as_ref().unwrap_or(inferred),
            ));
        }
        match (solved, inferred) {
            (JavaType::Named { name, args }, _)
                if args.is_empty()
                    && crate::inherited_members::is_synthesized_source_wildcard_type_param(
                        name,
                    ) =>
            {
                solved.clone()
            }
            (
                JavaType::Named { name, args },
                JavaType::Named {
                    args: inferred_args,
                    ..
                },
            ) if args.len() == inferred_args.len()
                && crate::slot_shape::java_types_have_same_erased_identity(solved, inferred) =>
            {
                JavaType::Named {
                    name: name.clone(),
                    args: args
                        .iter()
                        .zip(inferred_args)
                        .map(|(old, new)| overlay(old, new))
                        .collect(),
                }
            }
            (JavaType::Array(old), JavaType::Array(new)) => {
                JavaType::Array(Box::new(overlay(old, new)))
            }
            (JavaType::Intersection(old), JavaType::Intersection(new))
                if old.len() == new.len() =>
            {
                JavaType::Intersection(
                    old.iter()
                        .zip(new)
                        .map(|(old, new)| overlay(old, new))
                        .collect(),
                )
            }
            (
                JavaType::Wildcard(Some(WildcardBound::Extends(old))),
                JavaType::Wildcard(Some(WildcardBound::Extends(new))),
            ) => JavaType::Wildcard(Some(WildcardBound::Extends(Box::new(overlay(old, new))))),
            (
                JavaType::Wildcard(Some(WildcardBound::Super(old))),
                JavaType::Wildcard(Some(WildcardBound::Super(new))),
            ) => JavaType::Wildcard(Some(WildcardBound::Super(Box::new(overlay(old, new))))),
            _ if crate::inherited_members::java_type_contains_synthesized_source_wildcard(
                solved,
            ) =>
            {
                // Reification can replace an old resolver binder H with an
                // entire emitted carrier Model<W>. Do not undo that rewrite.
                solved.clone()
            }
            _ => inferred.clone(),
        }
    }
    // Preserve only the reified witnesses, not their ordinary sibling binders.
    // Reuse the canonical overlay for nullability and nested collection views.
    specialize_slot_shape_java_type(shape, &overlay(&shape.java_ty, inferred))
}

/// Specializes a solved source-method return at a resolved generic call site.
///
/// A declaration such as `Factory<D>::factor() -> D` necessarily has an
/// erased dispatch shape while its body is planned.  At a call through
/// `Factory<Product>`, however, the resolver has proved the return is the
/// product representation.  Keeping only the substituted Java type while
/// retaining `ErasedObject` dispatch invents an `Any -> dyn Product`
/// downcast that Java never performs.  Re-project the escaping representation
/// when generic substitution changed an erased declaration into a concrete
/// runtime type, while retaining solver-owned optionality recursively.
pub(crate) fn specialize_resolved_return_shape(
    representations: &RepresentationPlan,
    shape: &RustSlotShape,
    specialized: &JavaType,
) -> RustSlotShape {
    let specialized_shape = specialize_slot_shape_java_type(shape, specialized);
    let preserve_allocation = |mut projected: RustSlotShape| {
        projected.native_allocation = projected
            .native_allocation
            .join(specialized_shape.native_allocation);
        for (path, placement) in &specialized_shape.nested_native_allocations {
            projected.require_native_allocation_at(path, *placement);
        }
        representations
            .source_generic_allocations
            .project(&mut projected);
        projected
    };
    if let Some(projected) = return_shape_for_java_type(representations, &specialized_shape.java_ty)
        && projected.dispatch != specialized_shape.dispatch
        && matches!(
            projected.dispatch,
            DispatchShape::ClosedEnum { .. } | DispatchShape::BoxedDynTrait { .. }
        )
    {
        // The emitted source-method ABI is representation-aware even when
        // generic substitution leaves the Java spelling unchanged. A solved
        // `Concrete Hmm<O>` shape and the generated `HmmValue<O>` return are
        // not two values requiring a wrap; they are two descriptions of the
        // same carrier. Make the call-site shape agree with that ABI before
        // assignment coercion plans another carrier construction.
        return preserve_allocation(projected);
    }
    if shape.java_ty == specialized_shape.java_ty
        || !matches!(shape.dispatch, DispatchShape::ErasedObject)
        || java_type_is_erased_object_boundary(&specialized_shape.java_ty)
    {
        return specialized_shape;
    }

    return_shape_for_java_type(representations, &specialized_shape.java_ty)
        .map(preserve_allocation)
        .unwrap_or(specialized_shape)
}

fn specialize_java_type_preserving_optionality(
    solved: &JavaType,
    specialized: &JavaType,
) -> JavaType {
    if let Some(solved_inner) = optional_element_type(solved) {
        let specialized_inner =
            optional_element_type(specialized).unwrap_or_else(|| specialized.clone());
        return optional_java_type(specialize_java_type_preserving_optionality(
            &solved_inner,
            &specialized_inner,
        ));
    }
    match (solved, specialized) {
        (JavaType::Array(solved), JavaType::Array(specialized)) => JavaType::Array(Box::new(
            specialize_java_type_preserving_optionality(solved, specialized),
        )),
        (
            JavaType::Named {
                name: solved_name,
                args: solved_args,
            },
            JavaType::Named {
                name: specialized_name,
                args: specialized_args,
            },
        ) if crate::facts::external_type_name_matches(solved_name, specialized_name)
            && solved_args.len() == specialized_args.len() =>
        {
            JavaType::Named {
                name: specialized_name.clone(),
                args: solved_args
                    .iter()
                    .zip(specialized_args)
                    .map(|(solved, specialized)| {
                        specialize_java_type_preserving_optionality(solved, specialized)
                    })
                    .collect(),
            }
        }
        _ => specialized.clone(),
    }
}

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

/// Checked cursors transfer owned Java references, preserving the physical
/// child carrier of the source allocation rather than a nominal borrowed view.
pub(crate) fn retained_iteration_element_shape(
    parent: &RustSlotShape,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> Option<RustSlotShape> {
    let element = crate::call_plan::retained_native_iteration_element_type(parent, facts)?;
    // A cursor reads a covariant slot as its upper bound, not as an erased
    // existential. Keep solver-added nullability around that read domain.
    // Contravariant/unbounded wildcards remain governed by generic_read_type.
    let element = crate::types::generic_read_type_preserving_optional(&element);
    let mut shape = project_java_value_shape(representations, element);
    shape.ownership = if shape.capabilities().is_copy() {
        OwnershipShape::CopyValue
    } else {
        OwnershipShape::Owned
    };
    shape.inherit_native_collection_child(parent, 0);
    Some(shape)
}

pub(crate) fn java_type_is_erased_object_boundary(java_ty: &JavaType) -> bool {
    if let Some(element) = crate::types::optional_element_type(java_ty) {
        return java_type_is_erased_object_boundary(&element);
    }
    matches!(
        java_ty,
        JavaType::Named { name, .. } | JavaType::Unknown(name)
            if matches!(crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
                "Object" | "java.lang.Object")
    ) || matches!(java_ty, JavaType::Wildcard(_))
}

fn java_type_is_known_platform_facade(java_ty: &JavaType) -> bool {
    // Some Java interfaces are deliberately represented by concrete generic
    // Rust carriers (for example `Future<T>`).  Their representation-plan
    // entry may still describe the Java interface hierarchy as erased, but
    // that must not erase the selected facade ABI at value boundaries.
    if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(java_ty) {
        return true;
    }
    match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            crate::stdlib::is_java_platform_value_carrier_type_name(name)
        }
        _ => false,
    }
}

fn java_type_is_platform_facade_in_plan(
    java_ty: &JavaType,
    representations: &RepresentationPlan,
) -> bool {
    // Most named types here are ordinary source names or generic binders.
    // Only platform names can need the provenance exception; querying the
    // representation catalog first scans its aliases for every missing binder.
    if !java_type_is_known_platform_facade(java_ty) {
        return false;
    }
    let owner = match java_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.as_str()),
        _ => None,
    };
    if owner
        .and_then(|name| representations.type_representation(name))
        .is_some_and(|entry| {
            matches!(
                entry.provenance(),
                crate::representation::RepresentationProvenance::ParsedSource
                    | crate::representation::RepresentationProvenance::UserOverride
            )
        })
    {
        return false;
    }
    true
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
    let java_ty = crate::types::generic_read_type_preserving_optional(&java_ty);
    let mut shape = RustSlotShape::for_storage(java_ty.clone());
    if crate::javalibs::generic_facade_interface_erases_to_named_rust_type(&java_ty) {
        apply_required_dispatch(&mut shape, DispatchShape::Concrete);
        apply_required_view(&mut shape, ViewShape::Native);
        return shape;
    }
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
    if let JavaType::Named { name, .. } = &java_ty
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
                && matches!(crate::stdlib::canonical_external_receiver_type_key(name).as_str(), "Object" | "java.lang.Object")
    )
}

pub fn value_shape(java_ty: JavaType) -> RustSlotShape {
    RustSlotShape::conservative(java_ty)
}

pub fn owned_string_value_shape() -> RustSlotShape {
    RustSlotShape::conservative(JavaType::String)
}

pub fn string_split_array_value_shape() -> RustSlotShape {
    // String.split returns a Java reference array, not a List.  Use the
    // storage shape so its already-nullable element carrier agrees with the
    // `java_string_split` runtime ABI (`Vec<Option<String>>`).  Describing the
    // result as `List<String>` makes destination-slot lowering widen every
    // element a second time into `Option<Option<String>>`.
    RustSlotShape::for_storage(JavaType::Array(Box::new(JavaType::String)))
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

/// Recover an owned collection payload without discarding its equality ABI.
/// A semantic hash view is a physical carrier, not a borrowed projection.
pub(crate) fn apply_owned_collection_value_projection(shape: &mut RustSlotShape) {
    shape.ownership = OwnershipShape::Owned;
    if shape.view != ViewShape::JavaSemanticHash {
        shape.view = ViewShape::Native;
    }
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
        DispatchShape::BoxedDynTrait { .. } | DispatchShape::ReferenceDynTrait { .. } => {
            DispatchProjectionPolicy::new(
                dispatch,
                Some(OwnershipShape::Owned),
                Some(ViewShape::DynTraitView),
            )
        }
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

pub(crate) fn dispatch_for_java_type_or_optional_element(
    representations: &RepresentationPlan,
    java_ty: &JavaType,
) -> DispatchShape {
    if let Some(element_ty) = optional_element_type(java_ty) {
        if java_type_is_erased_object_boundary(&element_ty) {
            DispatchShape::ErasedObject
        } else if java_type_is_platform_facade_in_plan(&element_ty, representations) {
            DispatchShape::Concrete
        } else {
            representations.dispatch_for(&element_ty)
        }
    } else if java_type_is_platform_facade_in_plan(java_ty, representations) {
        DispatchShape::Concrete
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

pub(crate) fn indexed_read_shape_for_access_with_representations(
    array: &RustSlotShape,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<RustSlotShape> {
    let mut shape = indexed_read_shape_for_access_with_facts(array, facts)?;
    apply_source_interface_reference_dispatch(&mut shape, representations);
    Some(shape)
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
    let mut element_shape =
        indexed_storage_shape_with_effective_facts(policy.into_element_ty(), facts);
    element_shape.ownership = ownership;
    element_shape.inherit_native_collection_child(array_shape, 0);
    if element_shape.native_allocation.shared() {
        element_shape.view = ViewShape::Native;
    } else if matches!(element_shape.java_ty, JavaType::Array(_)) {
        apply_required_view(&mut element_shape, ViewShape::Slice);
    }
    // Reference-array storage is `Vec<Option<T>>`, including when the array
    // receiver itself is non-null. Keep that carrier in the read shape so
    // argument lowering can project `Option<&T>` rather than borrow an
    // `Option<T>` container as though it were a `T`.
    element_shape.java_ty =
        array_storage_element_java_type(&array_shape.java_ty, element_shape.java_ty);

    if array_shape.native_allocation.shared()
        && !matches!(ownership, OwnershipShape::CopyValue)
        && (element_shape.native_allocation.shared()
            || !matches!(
                optional_element_type(&element_shape.java_ty)
                    .unwrap_or_else(|| element_shape.java_ty.clone()),
                JavaType::Array(_)
            ))
    {
        // A reference into a Ref/MutexGuard cannot escape the checked-read
        // block. Java copies the stored reference value, not the array.
        element_shape.ownership = OwnershipShape::Owned;
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
    let container_ty =
        optional_element_type(&array_shape.java_ty).unwrap_or_else(|| array_shape.java_ty.clone());
    sequence_element_projection_policy_with_effective_facts(&container_ty, facts)
        .into_element_ty()
        .map(|element| {
            let mut shape = indexed_storage_shape_with_effective_facts(
                array_storage_element_java_type(&container_ty, element),
                facts,
            );
            shape.inherit_native_collection_child(array_shape, 0);
            shape
        })
}

/// Indexed slots must describe the carrier actually stored in the array.
/// In particular Object is already erased; Concrete would erase its Option
/// container again instead of copying the stored nullable object reference.
fn indexed_storage_shape_with_effective_facts(
    java_ty: JavaType,
    facts: &EffectiveFacts<'_>,
) -> RustSlotShape {
    let payload = optional_element_type(&java_ty).unwrap_or_else(|| java_ty.clone());
    let mut shape = RustSlotShape::for_storage(java_ty);
    if java_type_is_erased_object_storage(&payload)
        && !facts.parsed_source_controls_native_carrier(&payload)
    {
        let storage = storage_arg_shape_with_effective_facts(payload, facts);
        shape.dispatch = storage.dispatch;
        shape.view = storage.view;
    }
    shape
}

pub(crate) fn array_storage_element_java_type(
    container_ty: &JavaType,
    element_ty: JavaType,
) -> JavaType {
    let optional_container = optional_element_type(container_ty);
    let container_ty = optional_container.as_ref().unwrap_or(container_ty);
    if matches!(container_ty, JavaType::Array(_))
        && java_array_element_needs_nullable_storage(&element_ty)
    {
        optional_java_type(element_ty)
    } else {
        element_ty
    }
}

/// Java reference-array reads may produce null, including a partially
/// allocated row. This value-domain fact is separate from the selected Rust
/// backend's physical element storage policy below.
pub(crate) fn reference_array_read_may_be_null(array_ty: &JavaType) -> bool {
    let inner = optional_element_type(array_ty);
    let array_ty = inner.as_ref().unwrap_or(array_ty);
    matches!(array_ty, JavaType::Array(element) if !matches!(element.as_ref(), JavaType::Primitive(_) | JavaType::Void))
}

pub(crate) fn java_array_element_needs_nullable_storage(element_ty: &JavaType) -> bool {
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
    #[test]
    fn source_method_contract_specialization_keeps_binding_and_nested_view_evidence() {
        use crate::binding_id::{BindingId, BindingKind};
        use crate::java_ir::Member;
        use crate::signature::{LocalPlan, MethodSignaturePlan, ParamPlan, ReceiverPlan};
        let parse = crate::parser::parse_hierarchy_type;
        let parsed = crate::parser::parse_java_source(
            "class Frame<A> { A echo(A input) { List<Set<A>> local = null; return input; } }",
        );
        let Member::Method(method) = &parsed.program.types[0].members[0] else {
            panic!("method");
        };
        let value = super::RustSlotShape::for_storage(parse("Optional<A>"));
        let mut nested = super::RustSlotShape::for_storage(parse("List<Optional<Set<A>>>"));
        nested
            .nested_collection_views
            .push((parse("Set<A>"), super::ViewShape::JavaSemanticHash));
        let binding = BindingId::new(BindingKind::Local, "local", 9);
        let signature = MethodSignaturePlan::new(
            vec![ParamPlan::new(
                "input".into(),
                parse("A"),
                false,
                value.clone(),
                true,
            )],
            Some(value),
            vec![
                LocalPlan::new(
                    binding.clone(),
                    "local".into(),
                    parse("List<Set<A>>"),
                    nested,
                    true,
                )
                .with_proven_non_null(true),
            ],
            ReceiverPlan::Mutable,
        );
        let bindings = std::collections::HashMap::from([("A".into(), parse("String"))]);
        let actual = super::specialize_source_method_signature(
            &signature,
            method,
            &bindings,
            &super::RepresentationPlan::default(),
        );
        assert_eq!(actual.params()[0].java_ty(), &parse("String"));
        assert_eq!(
            actual.params()[0].shape().java_ty,
            parse("Optional<String>")
        );
        assert!(actual.params()[0].binding_mutable());
        assert_eq!(
            actual.return_shape().unwrap().java_ty,
            parse("Optional<String>")
        );
        let local = &actual.locals()[0];
        assert_eq!(local.binding(), &binding);
        assert!(local.proven_non_null() && local.binding_mutable());
        assert_eq!(local.java_ty(), &parse("List<Set<String>>"));
        assert_eq!(local.shape().java_ty, parse("List<Optional<Set<String>>>"));
        assert_eq!(
            local.shape().nested_collection_views,
            vec![(parse("Set<String>"), super::ViewShape::JavaSemanticHash)]
        );
        assert_eq!(actual.receiver(), ReceiverPlan::Mutable);
        assert_eq!(
            signature.return_shape().unwrap().java_ty,
            parse("Optional<A>")
        );
    }

    #[test]
    fn source_method_contract_specialization_respects_method_binder_shadowing() {
        use crate::java_ir::Member;
        use crate::signature::{MethodSignaturePlan, ParamPlan, ReceiverPlan};
        let parse = crate::parser::parse_hierarchy_type;
        let parsed = crate::parser::parse_java_source(
            "class Shadow<A> { <A> A same(A input) { return input; } }",
        );
        let Member::Method(method) = &parsed.program.types[0].members[0] else {
            panic!("method");
        };
        let value = super::RustSlotShape::for_storage(parse("A"));
        let signature = MethodSignaturePlan::new(
            vec![ParamPlan::new(
                "input".into(),
                parse("A"),
                false,
                value.clone(),
                false,
            )],
            Some(value),
            vec![],
            ReceiverPlan::Shared,
        );
        let actual = super::specialize_source_method_signature(
            &signature,
            method,
            &std::collections::HashMap::from([("A".into(), parse("String"))]),
            &super::RepresentationPlan::default(),
        );
        assert_eq!(actual, signature);
    }

    #[test]
    fn nullable_object_boundaries_keep_erased_dispatch() {
        use crate::slot_shape::DispatchShape;
        for name in ["Object", "java.lang.Object"] {
            let object = crate::parser::parse_hierarchy_type(name);
            for java_ty in [object.clone(), crate::types::optional_java_type(object)] {
                assert!(super::java_type_is_erased_object_boundary(&java_ty));
                let shape = super::project_java_value_shape(None, java_ty);
                assert_eq!(shape.dispatch, DispatchShape::ErasedObject);
            }
        }
        for name in ["String", "Future<String>", "List<Object>", "Object[]"] {
            let java_ty =
                crate::types::optional_java_type(crate::parser::parse_hierarchy_type(name));
            assert!(!super::java_type_is_erased_object_boundary(&java_ty));
        }
    }

    #[test]
    fn owned_collection_projection_keeps_semantic_equality_and_child_metadata() {
        use crate::slot_shape::{OwnershipShape, RustSlotShape, ViewShape};
        for view in [
            ViewShape::Native,
            ViewShape::Slice,
            ViewShape::JavaSemanticHash,
        ] {
            let mut shape =
                RustSlotShape::for_storage(crate::parser::parse_hierarchy_type("Set<T>"));
            shape.ownership = OwnershipShape::SharedBorrow;
            shape.view = view.clone();
            shape.nested_collection_views.push((
                crate::parser::parse_hierarchy_type("Set<T>"),
                ViewShape::JavaSemanticHash,
            ));
            let metadata = shape.nested_collection_views.clone();
            super::apply_owned_collection_value_projection(&mut shape);
            assert_eq!(shape.ownership, OwnershipShape::Owned);
            assert_eq!(
                shape.view,
                if view == ViewShape::JavaSemanticHash {
                    view
                } else {
                    ViewShape::Native
                }
            );
            assert_eq!(shape.nested_collection_views, metadata);
        }
    }

    #[test]
    fn retained_row_reads_describe_owned_handles_only_when_the_parent_is_guarded() {
        use crate::{
            facts::FactSet,
            java_ir::JavaType,
            slot_shape::{OwnershipShape, RustSlotShape, ViewShape},
            storage_strategy::StoragePlacement,
        };
        let placement = StoragePlacement::new(true, true, false);
        for guarded in [false, true] {
            for nullable in [false, true] {
                let row = JavaType::Array(Box::new(JavaType::String));
                let row = if nullable {
                    crate::types::optional_java_type(row)
                } else {
                    row
                };
                let mut array = RustSlotShape::for_storage(JavaType::Array(Box::new(row)));
                if guarded {
                    array.native_allocation = placement;
                }
                array.nested_native_allocations = vec![(vec![0], placement)];
                let read =
                    super::indexed_read_shape_for_access_with_facts(&array, &FactSet::default())
                        .unwrap();
                assert_eq!(
                    read.ownership,
                    if guarded {
                        OwnershipShape::Owned
                    } else {
                        OwnershipShape::SharedBorrow
                    }
                );
                assert_eq!(read.view, ViewShape::Native);
                assert_eq!(read.native_allocation, placement);
                assert_eq!(read.optional_element_type().is_some(), nullable);
            }
        }
    }

    #[test]
    fn collection_projection_non_platform_names_do_not_scan_the_representation_catalog() {
        let plan = super::RepresentationPlan::default();
        crate::representation::take_simple_name_catalog_scan_count();
        for name in ["T", "V", "E", "DomainNode"] {
            let ty = super::JavaType::Named {
                name: name.to_string(),
                args: Vec::new(),
            };
            assert!(!super::java_type_is_platform_facade_in_plan(&ty, &plan));
        }
        assert_eq!(
            crate::representation::take_simple_name_catalog_scan_count(),
            0
        );
        let native = super::JavaType::Named {
            name: "Reader".to_string(),
            args: Vec::new(),
        };
        assert!(super::java_type_is_platform_facade_in_plan(&native, &plan));
        assert_eq!(
            crate::representation::take_simple_name_catalog_scan_count(),
            1
        );
    }

    #[test]
    fn reference_cast_preserves_null_but_primitive_unboxing_does_not() {
        let actual =
            super::RustSlotShape::for_storage(super::optional_java_type(super::JavaType::Named {
                name: "Integer".to_string(),
                args: vec![],
            }));
        let mut reference = super::RustSlotShape::for_storage(super::JavaType::Named {
            name: "Object".to_string(),
            args: vec![],
        });
        super::preserve_reference_cast_nullability(&actual, &mut reference);
        assert!(reference.optional_element_type().is_some());
        let mut primitive = super::RustSlotShape::for_storage(super::JavaType::Primitive(
            crate::java_ir::PrimitiveType::Int,
        ));
        super::preserve_reference_cast_nullability(&actual, &mut primitive);
        assert!(primitive.optional_element_type().is_none());
        let null = super::RustSlotShape::for_storage(super::JavaType::Unknown("null".to_string()));
        let mut literal_target = super::RustSlotShape::for_storage(super::JavaType::String);
        super::preserve_reference_cast_nullability(&null, &mut literal_target);
        assert!(literal_target.optional_element_type().is_some());
    }

    use super::*;

    #[test]
    fn resolver_argument_specialization_protects_only_reified_witnesses() {
        let parse = crate::parser::parse_hierarchy_type;
        let mut solved =
            RustSlotShape::for_storage(parse("Optional<Set<Pair<MethodItem, __JavaWildcard0>>>"));
        solved.ownership = OwnershipShape::MutableBorrow;
        solved.nested_collection_views.push((
            parse("Set<Pair<MethodItem, __JavaWildcard0>>"),
            ViewShape::JavaSemanticHash,
        ));
        let specialized = specialize_resolved_argument_preserving_source_wildcards(
            &solved,
            &parse("Set<Pair<String, ?>>"),
        );
        assert_eq!(
            specialized.java_ty,
            parse("Optional<Set<Pair<String, __JavaWildcard0>>>")
        );
        assert_eq!(specialized.ownership, OwnershipShape::MutableBorrow);
        assert_eq!(
            specialized.nested_collection_views,
            vec![(
                parse("Set<Pair<String, __JavaWildcard0>>"),
                ViewShape::JavaSemanticHash
            )]
        );
        let array = RustSlotShape::for_storage(parse("Pair<MethodItem, __JavaWildcard0>[]"));
        assert_eq!(
            specialize_resolved_argument_preserving_source_wildcards(
                &array,
                &parse("Pair<String, ?>[]")
            )
            .java_ty,
            parse("Pair<String, __JavaWildcard0>[]")
        );
    }

    #[test]
    fn resolver_argument_specialization_does_not_restore_a_pre_reification_owner() {
        let parse = crate::parser::parse_hierarchy_type;
        let solved = RustSlotShape::for_storage(parse("Model<__JavaWildcard0>"));
        let specialized =
            specialize_resolved_argument_preserving_source_wildcards(&solved, &parse("H"));
        assert_eq!(specialized, solved);
    }

    #[test]
    fn collection_child_slots_inherit_only_relevant_carrier_decisions() {
        let parse = crate::parser::parse_hierarchy_type;
        let mut parent = RustSlotShape::for_storage(parse("List<List<Set<Item>>>"));
        parent
            .nested_collection_views
            .push((parse("Set<Item>"), ViewShape::JavaSemanticHash));
        let mut element = RustSlotShape::for_storage(parse("Set<Item>"));
        element.inherit_nested_collection_views(&parent);
        assert_eq!(element.view, ViewShape::JavaSemanticHash);
        assert!(element.nested_collection_views.is_empty());
        let mut row = RustSlotShape::for_storage(parse("List<Set<Item>>"));
        row.inherit_nested_collection_views(&parent);
        assert_eq!(row.nested_collection_views, parent.nested_collection_views);
        let mut index = RustSlotShape::for_storage(parse("int"));
        index.inherit_nested_collection_views(&parent);
        assert!(index.nested_collection_views.is_empty());
    }

    #[test]
    fn retained_iteration_projects_only_covariant_read_bounds() {
        let parse = crate::parser::parse_hierarchy_type;
        let facts = FactSet::default();
        for (source, expected) in [
            ("Set<? extends T>", "T"),
            ("Set<Optional<? extends T>>", "Optional<T>"),
            ("Set<? super T>", "? super T"),
            ("Set<?>", "?"),
        ] {
            let mut parent = RustSlotShape::for_storage(parse(source));
            parent.native_allocation =
                crate::storage_strategy::StoragePlacement::new(true, true, false);
            let element = retained_iteration_element_shape(&parent, &facts, None).unwrap();
            assert_eq!(element.java_ty, parse(expected), "{source}");
            if source.contains("extends") {
                assert_eq!(element.dispatch, DispatchShape::Concrete, "{source}");
            }
            assert_eq!(element.ownership, OwnershipShape::Owned, "{source}");
        }
    }

    #[test]
    fn slot_specialization_retains_nested_collection_carriers_and_nullability() {
        let parse = crate::parser::parse_hierarchy_type;
        let mut shape = RustSlotShape::for_storage(parse("List<Optional<Set<Item>>>"));
        shape
            .nested_collection_views
            .push((parse("Set<Item>"), ViewShape::JavaSemanticHash));
        let specialized = specialize_slot_shape_java_type(&shape, &parse("List<Set<String>>"));
        assert_eq!(specialized.java_ty, parse("List<Optional<Set<String>>>"));
        assert_eq!(
            specialized.nested_collection_views,
            vec![(parse("Set<String>"), ViewShape::JavaSemanticHash)]
        );
        let rendered = crate::slot_render::rust_type_for_slot_with_facts_and_representations(
            &specialized,
            &FactSet::default(),
            &RepresentationPlan::default(),
        );
        assert_eq!(
            crate::emit::emit_type(&rendered),
            "Vec<Option<JavaSemanticHashSet<String>>>"
        );
    }

    #[test]
    fn slot_specialization_preserves_nested_sequence_optionality() {
        let type_param = JavaType::Named {
            name: "O".to_string(),
            args: Vec::new(),
        };
        let concrete = JavaType::Named {
            name: "ConcreteObservation".to_string(),
            args: Vec::new(),
        };
        let solved = RustSlotShape::for_storage(JavaType::Named {
            name: "List".to_string(),
            args: vec![optional_java_type(JavaType::Named {
                name: "List".to_string(),
                args: vec![type_param],
            })],
        });
        let specialized = JavaType::Named {
            name: "List".to_string(),
            args: vec![JavaType::Named {
                name: "List".to_string(),
                args: vec![concrete.clone()],
            }],
        };

        assert_eq!(
            specialize_slot_shape_java_type(&solved, &specialized).java_ty,
            JavaType::Named {
                name: "List".to_string(),
                args: vec![optional_java_type(JavaType::Named {
                    name: "List".to_string(),
                    args: vec![concrete],
                })],
            }
        );
    }

    #[test]
    fn indexed_read_preserves_nullable_elements_of_nullable_reference_array() {
        let array = RustSlotShape::conservative(optional_java_type(JavaType::Array(Box::new(
            JavaType::String,
        ))));

        let indexed = indexed_read_shape_for_access_with_facts(&array, &FactSet::default())
            .expect("nullable Java array remains indexable");

        assert_eq!(indexed.optional_element_type(), Some(JavaType::String));
        assert_eq!(indexed.ownership, OwnershipShape::SharedBorrow);
    }

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
