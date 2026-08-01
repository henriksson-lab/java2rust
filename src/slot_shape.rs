use crate::java_ir::{JavaType, WildcardBound};
use crate::types::{is_final_by_default, optional_element_type, sequence_element_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RustSlotShape {
    pub java_ty: JavaType,
    pub ownership: OwnershipShape,
    pub view: ViewShape,
    pub dispatch: DispatchShape,
    pub mutability: MutabilityShape,
    pub type_identity: TypeIdentity,
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
    pub declared_java_ty: JavaType,
    pub runtime_bounds: Vec<JavaType>,
    pub generic_args: Vec<JavaType>,
    pub erasure: Option<JavaType>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeCapabilities {
    pub copy: bool,
    pub clone: bool,
    pub string_like: bool,
    pub sequence_like: Option<JavaType>,
    pub implements: Vec<JavaType>,
    pub extends: Option<JavaType>,
    pub final_type: bool,
    pub object_safe_trait: bool,
}

impl RustSlotShape {
    pub fn conservative(java_ty: JavaType) -> Self {
        Self {
            ownership: conservative_ownership(&java_ty),
            view: ViewShape::Native,
            dispatch: DispatchShape::Concrete,
            mutability: MutabilityShape::Immutable,
            type_identity: TypeIdentity::from_java_type(java_ty.clone()),
            java_ty,
        }
    }

    pub fn for_storage(java_ty: JavaType) -> Self {
        Self {
            ownership: conservative_ownership(&java_ty),
            view: ViewShape::Native,
            dispatch: DispatchShape::Concrete,
            mutability: MutabilityShape::Mutable,
            type_identity: TypeIdentity::from_java_type(java_ty.clone()),
            java_ty,
        }
    }

    pub fn capabilities(&self) -> TypeCapabilities {
        TypeCapabilities::conservative(&self.java_ty)
    }

    pub fn sequence_element_type(&self) -> Option<JavaType> {
        sequence_element_type(&self.java_ty)
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
}

pub fn conservative_ownership(java_ty: &JavaType) -> OwnershipShape {
    if TypeCapabilities::conservative(java_ty).copy {
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

    fn erased_java_type(&self) -> &JavaType {
        self.type_identity.erasure.as_ref().unwrap_or(&self.java_ty)
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
}
