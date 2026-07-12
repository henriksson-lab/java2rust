use crate::java_ir::{JavaType, PrimitiveType};
use crate::rust_ir::{RustExpr, RustType};

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

#[derive(Debug, Clone, PartialEq)]
pub struct LoweredExpr {
    pub expr: RustExpr,
    pub actual: LoweredType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredType {
    Slot(Box<RustSlotShape>),
    OptionalElementBorrow {
        option_shape: Box<RustSlotShape>,
        element_shape: Box<RustSlotShape>,
    },
    VecElementBorrow {
        vec_shape: Box<RustSlotShape>,
        element_shape: Box<RustSlotShape>,
    },
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

    pub fn rust_type(&self) -> RustType {
        let native = match &self.dispatch {
            DispatchShape::ClosedEnum { enum_name } => RustType::Named(enum_name.clone()),
            DispatchShape::BoxedDynTrait { trait_name } => {
                RustType::Raw(format!("Box<dyn {trait_name}>"))
            }
            DispatchShape::ErasedObject => RustType::Raw("Box<dyn std::any::Any>".to_string()),
            DispatchShape::Concrete | DispatchShape::DynTrait { .. } => {
                rust_type_for_java_type(&self.java_ty)
            }
        };
        let viewed = match (&self.view, &self.java_ty) {
            (ViewShape::Str, JavaType::String) => RustType::Raw("str".to_string()),
            (ViewShape::Slice, java_ty) => {
                if let Some(element_ty) = sequence_element_type(java_ty) {
                    RustType::Slice(Box::new(rust_type_for_java_type(&element_ty)))
                } else {
                    native
                }
            }
            _ => native,
        };

        match self.ownership {
            OwnershipShape::Owned | OwnershipShape::CopyValue => viewed,
            OwnershipShape::SharedBorrow => RustType::Ref {
                mutable: false,
                inner: Box::new(viewed),
            },
            OwnershipShape::MutableBorrow => RustType::Ref {
                mutable: true,
                inner: Box::new(viewed),
            },
        }
    }

    pub fn default_expr(&self) -> RustExpr {
        match (&self.ownership, &self.view, &self.java_ty) {
            (OwnershipShape::SharedBorrow, ViewShape::Str, java_ty)
                if TypeCapabilities::conservative(java_ty).string_like =>
            {
                RustExpr::Literal("\"\"".to_string())
            }
            (OwnershipShape::SharedBorrow, ViewShape::Slice, _) => RustExpr::Raw("&[]".to_string()),
            (_, _, JavaType::Primitive(PrimitiveType::Boolean)) => {
                RustExpr::Literal("false".to_string())
            }
            (_, _, JavaType::Primitive(_)) => RustExpr::Literal("0".to_string()),
            (OwnershipShape::Owned, ViewShape::Native, JavaType::String) => {
                RustExpr::Raw("String::new()".to_string())
            }
            (OwnershipShape::Owned, ViewShape::Native, java_ty)
                if sequence_element_type(java_ty).is_some() =>
            {
                RustExpr::Raw("Vec::new()".to_string())
            }
            _ => RustExpr::Raw("Default::default()".to_string()),
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
        Self {
            declared_java_ty: java_ty,
            runtime_bounds: Vec::new(),
            generic_args,
            erasure: None,
        }
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

impl LoweredType {
    pub fn from_java(java_ty: JavaType) -> Self {
        Self::Slot(Box::new(RustSlotShape::conservative(java_ty)))
    }

    pub fn optional_element_borrow(element_ty: JavaType) -> Self {
        let mut element_shape = RustSlotShape::conservative(element_ty);
        element_shape.ownership = OwnershipShape::SharedBorrow;
        Self::optional_element_borrow_shape(element_shape)
    }

    pub fn optional_element_borrow_shape(element_shape: RustSlotShape) -> Self {
        let option_shape =
            RustSlotShape::conservative(optional_java_type(element_shape.java_ty.clone()));
        Self::OptionalElementBorrow {
            option_shape: Box::new(option_shape),
            element_shape: Box::new(element_shape),
        }
    }

    pub fn vec_element_borrow(element_shape: RustSlotShape) -> Self {
        let vec_shape = RustSlotShape::conservative(list_java_type(element_shape.java_ty.clone()));
        Self::VecElementBorrow {
            vec_shape: Box::new(vec_shape),
            element_shape: Box::new(element_shape),
        }
    }

    pub fn stream_sum() -> Self {
        Self::from_java(JavaType::Primitive(PrimitiveType::Int))
    }

    pub fn stream_any_match() -> Self {
        Self::from_java(JavaType::Primitive(PrimitiveType::Boolean))
    }

    pub fn stream_collect(element_shape: RustSlotShape) -> Self {
        if element_shape.ownership == OwnershipShape::SharedBorrow {
            Self::vec_element_borrow(element_shape)
        } else {
            Self::from_java(list_java_type(element_shape.java_ty))
        }
    }

    pub fn stream_find_first(element_shape: RustSlotShape) -> Self {
        Self::optional_element_borrow_shape(element_shape)
    }

    pub fn java_ty(&self) -> Option<&JavaType> {
        match self {
            LoweredType::Slot(shape) => Some(&shape.java_ty),
            LoweredType::OptionalElementBorrow { option_shape, .. } => Some(&option_shape.java_ty),
            LoweredType::VecElementBorrow { vec_shape, .. } => Some(&vec_shape.java_ty),
        }
    }
}

pub fn rust_type_for_java_type(ty: &JavaType) -> RustType {
    if let Some(element_ty) = sequence_element_type(ty) {
        return RustType::Vec(Box::new(rust_type_for_java_type(&element_ty)));
    }
    if let Some(element_ty) = optional_element_type(ty) {
        return RustType::Option(Box::new(rust_type_for_java_type(&element_ty)));
    }
    match ty {
        JavaType::Void => RustType::Unit,
        JavaType::Primitive(PrimitiveType::Boolean) => RustType::Bool,
        JavaType::Primitive(PrimitiveType::Byte) => RustType::I8,
        JavaType::Primitive(PrimitiveType::Short) => RustType::I16,
        JavaType::Primitive(PrimitiveType::Char) => RustType::Char,
        JavaType::Primitive(PrimitiveType::Int) => RustType::I32,
        JavaType::Primitive(PrimitiveType::Long) => RustType::I64,
        JavaType::Primitive(PrimitiveType::Float) => RustType::F32,
        JavaType::Primitive(PrimitiveType::Double) => RustType::F64,
        JavaType::String => RustType::String,
        JavaType::Array(_) => unreachable!("arrays are handled by sequence_element_type"),
        JavaType::Named { name, .. } if name == "Integer" => RustType::I32,
        JavaType::Named { name, .. } if name == "Long" => RustType::I64,
        JavaType::Named { name, .. } if name == "Double" => RustType::F64,
        JavaType::Named { name, .. } if name == "Float" => RustType::F32,
        JavaType::Named { name, .. } if name == "Boolean" => RustType::Bool,
        JavaType::Named { name, .. } if name == "Character" => RustType::Char,
        JavaType::Named { name, .. } => RustType::Named(name.clone()),
        JavaType::Wildcard(_) => RustType::Named("JavaObject".to_string()),
        JavaType::Unknown(raw) => RustType::Raw(raw.clone()),
    }
}

pub fn conservative_ownership(java_ty: &JavaType) -> OwnershipShape {
    if TypeCapabilities::conservative(java_ty).copy {
        OwnershipShape::CopyValue
    } else {
        OwnershipShape::Owned
    }
}

fn sequence_element_type(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Array(inner) => Some((**inner).clone()),
        JavaType::Named { name, args } if name == "List" && args.len() == 1 => {
            Some(args[0].clone())
        }
        _ => None,
    }
}

pub fn list_java_type(element_ty: JavaType) -> JavaType {
    JavaType::Named {
        name: "List".to_string(),
        args: vec![element_ty],
    }
}

pub fn optional_java_type(element_ty: JavaType) -> JavaType {
    JavaType::Named {
        name: "Optional".to_string(),
        args: vec![element_ty],
    }
}

fn optional_element_type(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Named { name, args } if name == "Optional" && args.len() == 1 => {
            Some(args[0].clone())
        }
        _ => None,
    }
}

fn is_final_by_default(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Primitive(_) | JavaType::String)
}
