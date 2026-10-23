use crate::java_ir::{JavaType, WildcardBound};
use crate::types::{is_final_by_default, optional_element_type, sequence_element_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSlotShape {
    pub java_ty: JavaType,
    /// Physical allocation identity for native containers, independent of
    /// whether this slot owns a handle or borrows one. Nominal source-object
    /// placement remains owned by the existing type-wide storage plan.
    pub native_allocation: crate::storage_strategy::StoragePlacement,
    /// Physical requirements at generic/array child positions, independent of
    /// the child's spelling (Map<Set<T>, Set<T>> has two distinct slots).
    /// Internal optional wrappers do not consume a path component.
    pub nested_native_allocations: Vec<(Vec<usize>, crate::storage_strategy::StoragePlacement)>,
    pub ownership: OwnershipShape,
    /// A specialized generic `&T` borrows T's canonical Java carrier, not
    /// the object payload that a concrete source-type borrow can project.
    pub borrow_target: BorrowTarget,
    pub view: ViewShape,
    pub dispatch: DispatchShape,
    pub mutability: MutabilityShape,
    /// Solver-owned carrier choices below the outer slot. Keys are Java
    /// subtrees, not global type-variable names; lexical shadowing is resolved
    /// when the signature is planned.
    pub nested_collection_views: Vec<(JavaType, ViewShape)>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorrowTarget {
    #[default]
    Payload,
    Carrier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipShape {
    Owned,
    SharedBorrow,
    MutableBorrow,
    CopyValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewShape {
    Native,
    /// Identity-preserving List interface operations, independent of payload storage.
    ListInterface,
    /// Native hash collection using Java key equality instead of Rust Eq/Hash.
    JavaSemanticHash,
    Str,
    Slice,
    DynTraitView,
    EnumView,
    ErasedObjectView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchShape {
    Concrete,
    DynTrait { trait_name: String },
    BoxedDynTrait { trait_name: String },
    ClosedEnum { enum_name: String },
    ErasedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutabilityShape {
    Immutable,
    Mutable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeIdentity {
    declared_java_ty: JavaType,
    runtime_bounds: Vec<JavaType>,
    generic_args: Vec<JavaType>,
    erasure: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCapabilities {
    copy: bool,
    clone: bool,
    string_like: bool,
    sequence_like: Option<JavaType>,
    implements: Vec<JavaType>,
    extends: Option<JavaType>,
    final_type: bool,
    object_safe_trait: bool,
}

impl RustSlotShape {
    /// A non-borrowed nullable String stores String, never the unsized str.
    /// Keep the physical payload shape shared by rendering and projections:
    /// borrowing this payload produces &String until a view adapter runs.
    pub(crate) fn normalize_owned_string_payload_view(&mut self) {
        if self.java_ty == JavaType::String
            && self.view == ViewShape::Str
            && matches!(
                self.ownership,
                OwnershipShape::Owned | OwnershipShape::CopyValue
            )
        {
            self.view = ViewShape::Native;
            self.ownership = OwnershipShape::Owned;
        }
    }

    pub(crate) fn native_child_type_at_path(ty: &JavaType, path: &[usize]) -> Option<JavaType> {
        let mut ty = ty.clone();
        for index in path {
            ty = optional_element_type(&ty).unwrap_or(ty);
            ty = match ty {
                JavaType::Array(element) if *index == 0 => *element,
                JavaType::Named { args, .. } => args.get(*index)?.clone(),
                _ => return None,
            };
        }
        Some(ty)
    }

    pub(crate) fn require_native_allocation_at(
        &mut self,
        path: &[usize],
        placement: crate::storage_strategy::StoragePlacement,
    ) {
        if path.is_empty() {
            self.native_allocation = self.native_allocation.join(placement);
        } else if let Some((_, current)) = self
            .nested_native_allocations
            .iter_mut()
            .find(|(key, _)| key == path)
        {
            *current = current.join(placement);
        } else {
            self.nested_native_allocations
                .push((path.to_vec(), placement));
            self.nested_native_allocations.sort_by(|a, b| a.0.cmp(&b.0));
        }
    }

    pub(crate) fn inherit_native_collection_child(&mut self, parent: &Self, index: usize) {
        self.inherit_nested_collection_views(parent);
        for (path, placement) in &parent.nested_native_allocations {
            if path.first() == Some(&index) {
                self.require_native_allocation_at(&path[1..], *placement);
            }
        }
    }

    /// Project a child slot from a solved container without rediscovering its
    /// carrier from a context-free Java spelling.
    pub(crate) fn inherit_nested_collection_views(&mut self, parent: &Self) {
        fn contains(ty: &JavaType, child: &JavaType) -> bool {
            ty == child
                || match ty {
                    JavaType::Named { args, .. } | JavaType::Intersection(args) => {
                        args.iter().any(|ty| contains(ty, child))
                    }
                    JavaType::Array(element) => contains(element, child),
                    JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
                    | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
                        contains(bound, child)
                    }
                    _ => false,
                }
        }
        let base = optional_element_type(&self.java_ty).unwrap_or_else(|| self.java_ty.clone());
        for (ty, view) in &parent.nested_collection_views {
            if &base == ty {
                if self.view == ViewShape::Native {
                    self.view = view.clone();
                }
                continue;
            }
            if !contains(&self.java_ty, ty) {
                continue;
            }
            if !self
                .nested_collection_views
                .iter()
                .any(|(key, _)| key == ty)
            {
                self.nested_collection_views
                    .push((ty.clone(), view.clone()));
            }
        }
    }

    pub fn java_ty(&self) -> JavaType {
        self.java_ty.clone()
    }

    pub fn java_ty_mut(&mut self) -> &mut JavaType {
        &mut self.java_ty
    }

    pub fn conservative(java_ty: JavaType) -> Self {
        Self {
            native_allocation: Default::default(),
            nested_native_allocations: Default::default(),
            nested_collection_views: Default::default(),
            ownership: conservative_ownership(&java_ty),
            borrow_target: BorrowTarget::Payload,
            view: ViewShape::Native,
            dispatch: DispatchShape::Concrete,
            mutability: MutabilityShape::Immutable,
            java_ty,
        }
    }

    pub fn for_storage(java_ty: JavaType) -> Self {
        Self {
            native_allocation: Default::default(),
            nested_native_allocations: Default::default(),
            nested_collection_views: Default::default(),
            ownership: conservative_ownership(&java_ty),
            borrow_target: BorrowTarget::Payload,
            view: ViewShape::Native,
            dispatch: DispatchShape::Concrete,
            mutability: MutabilityShape::Mutable,
            java_ty,
        }
    }

    pub fn capabilities(&self) -> TypeCapabilities {
        TypeCapabilities::conservative(&self.java_ty)
    }

    pub fn sequence_element_type(&self) -> Option<JavaType> {
        sequence_element_type(&self.java_ty)
    }

    pub fn has_generic_or_sequence_detail(&self) -> bool {
        java_type_has_generic_or_sequence_detail(&self.java_ty)
    }

    pub fn optional_element_type(&self) -> Option<JavaType> {
        optional_element_type(&self.java_ty)
    }

    pub fn is_optional_of(&self, element_shape: &RustSlotShape) -> bool {
        self.optional_element_type()
            .is_some_and(|element_ty| element_ty == element_shape.java_ty)
    }

    pub fn is_sequence_of(&self, element_shape: &RustSlotShape) -> bool {
        self.sequence_element_type()
            .is_some_and(|element_ty| element_ty == element_shape.java_ty)
    }

    pub fn is_shared_borrowed_value(&self) -> bool {
        self.ownership == OwnershipShape::SharedBorrow
    }

    pub fn type_identity(&self) -> TypeIdentity {
        TypeIdentity::from_java_type(self.java_ty.clone())
    }
}

impl TypeIdentity {
    pub fn from_java_type(java_ty: JavaType) -> Self {
        let generic_args = match &java_ty {
            JavaType::Named { args, .. } => args.clone(),
            _ => Vec::new(),
        };
        let runtime_bounds = match &java_ty {
            JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
            | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => vec![(**bound).clone()],
            _ => Vec::new(),
        };
        let erasure = java_type_erasure(&java_ty);
        Self {
            declared_java_ty: java_ty,
            runtime_bounds,
            generic_args,
            erasure,
        }
    }

    pub fn declared_java_ty(&self) -> &JavaType {
        &self.declared_java_ty
    }

    pub fn runtime_bounds(&self) -> &[JavaType] {
        &self.runtime_bounds
    }

    pub fn generic_args(&self) -> &[JavaType] {
        &self.generic_args
    }

    pub fn erasure(&self) -> Option<&JavaType> {
        self.erasure.as_ref()
    }
}

fn java_type_erasure(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Named { name, args } if !args.is_empty() => Some(JavaType::Named {
            name: name.clone(),
            args: Vec::new(),
        }),
        JavaType::Array(inner) => {
            java_type_erasure(inner).map(|erased_inner| JavaType::Array(Box::new(erased_inner)))
        }
        JavaType::Wildcard(Some(WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(WildcardBound::Super(bound))) => {
            java_type_erasure(bound).or_else(|| Some((**bound).clone()))
        }
        JavaType::Wildcard(None) => Some(JavaType::Unknown("Object".to_string())),
        _ => None,
    }
}

impl TypeCapabilities {
    pub fn conservative(java_ty: &JavaType) -> Self {
        let copy = matches!(java_ty, JavaType::Primitive(_));
        let string_like = matches!(java_ty, JavaType::String);
        Self {
            copy,
            clone: !matches!(java_ty, JavaType::Void),
            string_like,
            sequence_like: sequence_element_type(java_ty),
            implements: Vec::new(),
            extends: None,
            final_type: is_final_by_default(java_ty),
            object_safe_trait: false,
        }
    }

    pub fn is_copy(&self) -> bool {
        self.copy
    }

    pub fn is_clone(&self) -> bool {
        self.clone
    }

    pub fn is_string_like(&self) -> bool {
        self.string_like
    }

    pub fn sequence_like(&self) -> Option<&JavaType> {
        self.sequence_like.as_ref()
    }

    pub fn implements(&self) -> &[JavaType] {
        &self.implements
    }

    pub fn extends(&self) -> Option<&JavaType> {
        self.extends.as_ref()
    }

    pub fn is_final_type(&self) -> bool {
        self.final_type
    }

    pub fn is_object_safe_trait(&self) -> bool {
        self.object_safe_trait
    }

    pub(crate) fn set_copy(&mut self, copy: bool) {
        self.copy = copy;
    }

    pub(crate) fn set_clone(&mut self, clone: bool) {
        self.clone = clone;
    }

    pub(crate) fn set_string_like(&mut self, string_like: bool) {
        self.string_like = string_like;
    }

    pub(crate) fn set_sequence_like(&mut self, sequence_like: JavaType) {
        self.sequence_like = Some(sequence_like);
    }

    pub(crate) fn set_implements(&mut self, implements: Vec<JavaType>) {
        self.implements = implements;
    }

    pub(crate) fn set_extends(&mut self, extends: JavaType) {
        self.extends = Some(extends);
    }

    pub(crate) fn set_final_type(&mut self, final_type: bool) {
        self.final_type = final_type;
    }

    pub(crate) fn set_object_safe_trait(&mut self, object_safe_trait: bool) {
        self.object_safe_trait = object_safe_trait;
    }
}

pub fn conservative_ownership(java_ty: &JavaType) -> OwnershipShape {
    if TypeCapabilities::conservative(java_ty).is_copy() {
        OwnershipShape::CopyValue
    } else {
        OwnershipShape::Owned
    }
}

impl RustSlotShape {
    /// Whether `self` carries strictly more generic detail than `other` for the
    /// same erased type — the partial order that decides which of two competing
    /// shapes for one slot wins.
    ///
    /// Lowering previously compared generic argument counts inline at each
    /// decision site. Keeping the order in one place means the sites cannot
    /// drift apart, and it is where a real lattice `join` belongs when the
    /// remaining shape axes are folded in.
    pub fn has_more_generic_detail_than(&self, other: &Self) -> bool {
        if self.erased_java_type() != other.erased_java_type() {
            return false;
        }
        match (&self.java_ty, &other.java_ty) {
            (
                JavaType::Named {
                    args: candidate_args,
                    ..
                },
                JavaType::Named {
                    args: current_args, ..
                },
            ) => candidate_args.len() > current_args.len(),
            _ => false,
        }
    }

    fn erased_java_type(&self) -> JavaType {
        java_type_erasure(&self.java_ty).unwrap_or_else(|| self.java_ty.clone())
    }
}

/// Whether two Java types name the same runtime type after generic erasure.
pub(crate) fn java_types_have_same_erased_identity(left: &JavaType, right: &JavaType) -> bool {
    match (left, right) {
        (JavaType::Named { name: left, .. }, JavaType::Named { name: right, .. }) => left == right,
        _ => left == right,
    }
}

pub(crate) fn java_type_has_generic_or_sequence_detail(java_ty: &JavaType) -> bool {
    match java_ty {
        JavaType::Named { args, .. } if !args.is_empty() => true,
        _ => sequence_element_type(java_ty).is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str, args: Vec<JavaType>) -> JavaType {
        JavaType::Named {
            name: name.to_string(),
            args,
        }
    }

    #[test]
    fn generic_detail_order_requires_same_erased_type() {
        let list_string = RustSlotShape::for_storage(named("List", vec![JavaType::String]));
        let raw_list = RustSlotShape::for_storage(named("List", Vec::new()));
        let raw_map = RustSlotShape::for_storage(named("Map", Vec::new()));

        assert!(list_string.has_more_generic_detail_than(&raw_list));
        assert!(!list_string.has_more_generic_detail_than(&raw_map));
    }

    #[test]
    fn erased_identity_ignores_named_generic_arguments_only() {
        let list_string = named("List", vec![JavaType::String]);
        let raw_list = named("List", Vec::new());
        let map_string = named("Map", vec![JavaType::String]);

        assert!(java_types_have_same_erased_identity(
            &list_string,
            &raw_list
        ));
        assert!(!java_types_have_same_erased_identity(
            &list_string,
            &map_string
        ));
        assert!(java_types_have_same_erased_identity(
            &JavaType::String,
            &JavaType::String
        ));
    }

    #[test]
    fn generic_or_sequence_detail_covers_named_args_and_facade_sequences() {
        let list_string = named("List", vec![JavaType::String]);
        let raw_list = named("List", Vec::new());
        let string_array = JavaType::Array(Box::new(JavaType::String));
        let plain_string = JavaType::String;

        assert!(java_type_has_generic_or_sequence_detail(&list_string));
        assert!(!java_type_has_generic_or_sequence_detail(&raw_list));
        assert!(java_type_has_generic_or_sequence_detail(&string_array));
        assert!(!java_type_has_generic_or_sequence_detail(&plain_string));
        assert!(RustSlotShape::for_storage(list_string).has_generic_or_sequence_detail());
    }
}
