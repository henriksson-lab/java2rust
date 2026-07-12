use crate::java_ir::JavaType;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactSet {
    pub notes: Vec<String>,
    pub captures: CaptureFacts,
    pub hierarchy_overrides: Vec<HierarchyOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HierarchyOverride {
    pub type_name: String,
    pub strategy: HierarchyStrategy,
    pub provenance: FactProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HierarchyStrategy {
    ConcreteStruct,
    ClosedEnum {
        enum_name: String,
        variants: Vec<String>,
    },
    DynTrait {
        trait_name: String,
    },
    BoxedDynTrait {
        trait_name: String,
    },
    ErasedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactProvenance {
    ParsedSource,
    CorpusAnalysis,
    UserOverride,
    ConservativeFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureFacts {
    pub owners: Vec<CaptureOwner>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOwner {
    pub id: NodeId,
    pub kind: CaptureOwnerKind,
    pub name: Option<String>,
    pub target_type: Option<JavaType>,
    pub origin: CaptureOrigin,
    pub captures: Vec<Capture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureOwnerKind {
    Lambda,
    LocalClass,
    AnonymousClass,
    InnerClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOrigin {
    Method {
        type_name: String,
        method_name: String,
    },
    Constructor {
        type_name: String,
    },
    Type {
        type_name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    pub source: CaptureSource,
    pub ty: JavaType,
    pub effectively_final: bool,
    pub mutable_after_capture: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureSource {
    Local,
    Parameter,
    ThisField,
    OuterThis,
}
