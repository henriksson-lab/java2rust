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
