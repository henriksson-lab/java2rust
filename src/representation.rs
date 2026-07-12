use std::collections::HashMap;

use crate::facts::{FactProvenance, FactSet, HierarchyStrategy};
use crate::java_ir::{
    Block, Expr, JavaType, LambdaBody, Member, Program, Stmt, TypeDecl, TypeKind,
};
use crate::rust_ir::RustType;
use crate::slot_shape::{DispatchShape, RustSlotShape};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepresentationPlan {
    types: HashMap<String, TypeRepresentation>,
    diagnostics: Vec<RepresentationDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRepresentation {
    pub java_name: String,
    pub representation: ObjectRepresentation,
    pub provenance: RepresentationProvenance,
    pub direct_subtypes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectRepresentation {
    ConcreteStruct,
    DynTrait {
        trait_name: String,
    },
    BoxedDynTrait {
        trait_name: String,
    },
    ClosedEnum {
        enum_name: String,
        variants: Vec<String>,
    },
    ErasedObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentationProvenance {
    ParsedSource,
    CorpusAnalysis,
    UserOverride,
    ConservativeFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepresentationDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepresentationTypeEmission {
    LowerAsConcreteStruct,
    RawRustItem(String),
}

pub fn plan_representations(program: &Program, facts: &FactSet) -> RepresentationPlan {
    let mut plan = RepresentationPlan::default();
    for ty in &program.types {
        collect_type_representation(ty, &mut plan);
    }
    for ty in all_types(program) {
        collect_hierarchy_edges(ty, &mut plan);
    }
    apply_hierarchy_overrides(facts, &mut plan);
    plan
}

impl RepresentationPlan {
    pub fn type_representation(&self, type_name: &str) -> Option<&TypeRepresentation> {
        self.types.get(type_name)
    }

    pub fn representation_for(&self, java_ty: &JavaType) -> ObjectRepresentation {
        match java_ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => self
                .types
                .get(name)
                .map(|entry| entry.representation.clone())
                .unwrap_or(ObjectRepresentation::ConcreteStruct),
            JavaType::Wildcard(_) => ObjectRepresentation::ErasedObject,
            _ => ObjectRepresentation::ConcreteStruct,
        }
    }

    pub fn direct_subtypes_of(&self, type_name: &str) -> &[String] {
        self.types
            .get(type_name)
            .map(|entry| entry.direct_subtypes.as_slice())
            .unwrap_or(&[])
    }

    pub fn dispatch_for(&self, java_ty: &JavaType) -> DispatchShape {
        match self.representation_for(java_ty) {
            ObjectRepresentation::ConcreteStruct => DispatchShape::Concrete,
            ObjectRepresentation::DynTrait { trait_name } => DispatchShape::DynTrait { trait_name },
            ObjectRepresentation::BoxedDynTrait { trait_name } => {
                DispatchShape::BoxedDynTrait { trait_name }
            }
            ObjectRepresentation::ClosedEnum { enum_name, .. } => {
                DispatchShape::ClosedEnum { enum_name }
            }
            ObjectRepresentation::ErasedObject => DispatchShape::ErasedObject,
        }
    }

    pub fn value_shape_for_java_type(&self, java_ty: JavaType) -> RustSlotShape {
        let mut shape = RustSlotShape::conservative(java_ty.clone());
        shape.dispatch = self.dispatch_for(&java_ty);
        shape
    }

    pub fn constructor_owner_type_for_new(
        &self,
        java_ty: &JavaType,
        resolved_declaring_type: Option<&str>,
    ) -> RustType {
        let owner_ty = resolved_declaring_type
            .map(|name| JavaType::Named {
                name: name.to_string(),
                args: Vec::new(),
            })
            .unwrap_or_else(|| java_ty.clone());
        self.value_shape_for_java_type(owner_ty).rust_type()
    }

    pub fn type_emission_for(&self, type_name: &str) -> RepresentationTypeEmission {
        let Some(representation) = self.type_representation(type_name) else {
            return RepresentationTypeEmission::LowerAsConcreteStruct;
        };
        match &representation.representation {
            ObjectRepresentation::ConcreteStruct => {
                RepresentationTypeEmission::LowerAsConcreteStruct
            }
            ObjectRepresentation::DynTrait { trait_name }
            | ObjectRepresentation::BoxedDynTrait { trait_name } => {
                RepresentationTypeEmission::RawRustItem(format!("pub trait {trait_name} {{}}"))
            }
            ObjectRepresentation::ClosedEnum {
                enum_name,
                variants,
            } => {
                let variants = variants
                    .iter()
                    .map(|variant| format!("    {variant}({variant}),"))
                    .collect::<Vec<_>>()
                    .join("\n");
                RepresentationTypeEmission::RawRustItem(format!(
                    "pub enum {enum_name} {{\n{variants}\n}}"
                ))
            }
            ObjectRepresentation::ErasedObject => RepresentationTypeEmission::RawRustItem(format!(
                "pub type {type_name} = Box<dyn std::any::Any>;"
            )),
        }
    }

    pub fn diagnostics_for_program(&self, program: &Program) -> Vec<RepresentationDiagnostic> {
        self.diagnostics
            .iter()
            .cloned()
            .chain(
                all_types(program)
                    .into_iter()
                    .flat_map(|ty| self.diagnostics_for_type(ty)),
            )
            .chain(anonymous_class_diagnostics(program))
            .collect()
    }

    pub fn diagnostics_for_type(&self, ty: &TypeDecl) -> Vec<RepresentationDiagnostic> {
        let mut diagnostics = Vec::new();
        if ty
            .extends
            .as_ref()
            .is_some_and(|super_name| !self.hierarchy_edge_is_represented(super_name))
        {
            diagnostics.push(RepresentationDiagnostic {
                code: "unsupported_inheritance",
                message: format!(
                    "{} participates in an extends hierarchy, but hierarchy lowering is not implemented yet",
                    ty.name
                ),
            });
        }
        let unsupported_interfaces = ty
            .implements
            .iter()
            .filter(|interface_name| !self.hierarchy_edge_is_represented(interface_name))
            .cloned()
            .collect::<Vec<_>>();
        if !unsupported_interfaces.is_empty() {
            diagnostics.push(RepresentationDiagnostic {
                code: "unsupported_interface_impl",
                message: format!(
                    "{} implements interfaces without a supported representation: {}",
                    ty.name,
                    unsupported_interfaces.join(", ")
                ),
            });
        }
        diagnostics
    }

    fn hierarchy_edge_is_represented(&self, super_name: &str) -> bool {
        matches!(
            self.types
                .get(super_name)
                .map(|entry| &entry.representation),
            Some(
                ObjectRepresentation::ClosedEnum { .. }
                    | ObjectRepresentation::DynTrait { .. }
                    | ObjectRepresentation::BoxedDynTrait { .. }
            )
        )
    }
}

fn anonymous_class_diagnostics(program: &Program) -> Vec<RepresentationDiagnostic> {
    let mut diagnostics = Vec::new();
    for ty in all_types(program) {
        collect_anonymous_class_diagnostics_for_type(ty, &mut diagnostics);
    }
    diagnostics
}

fn collect_anonymous_class_diagnostics_for_type(
    ty: &TypeDecl,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for member in &ty.members {
        match member {
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_anonymous_class_diagnostics_for_expr(initializer, diagnostics);
                }
            }
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    collect_anonymous_class_diagnostics_for_block(body, diagnostics);
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &ctor.body {
                    collect_anonymous_class_diagnostics_for_block(body, diagnostics);
                }
            }
            Member::Type(nested) => {
                collect_anonymous_class_diagnostics_for_type(nested, diagnostics);
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_anonymous_class_diagnostics_for_block(
    block: &Block,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    for stmt in &block.statements {
        collect_anonymous_class_diagnostics_for_stmt(stmt, diagnostics);
    }
}

fn collect_anonymous_class_diagnostics_for_stmt(
    stmt: &Stmt,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(initializer) = initializer {
                collect_anonymous_class_diagnostics_for_expr(initializer, diagnostics);
            }
        }
        Stmt::Assign { target, value } => {
            collect_anonymous_class_diagnostics_for_expr(target, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(value, diagnostics);
        }
        Stmt::Expr(expr) | Stmt::Throw(expr) => {
            collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
        }
        Stmt::Return(expr) => {
            if let Some(expr) = expr {
                collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
            }
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            collect_anonymous_class_diagnostics_for_block(then_block, diagnostics);
            if let Some(else_block) = else_block {
                collect_anonymous_class_diagnostics_for_block(else_block, diagnostics);
            }
        }
        Stmt::While { condition, body } => {
            collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                collect_anonymous_class_diagnostics_for_stmt(stmt, diagnostics);
            }
            if let Some(condition) = condition {
                collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            }
            for expr in update {
                collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
            }
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            collect_anonymous_class_diagnostics_for_expr(iterable, diagnostics);
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
        }
        Stmt::Switch { selector, cases } => {
            collect_anonymous_class_diagnostics_for_expr(selector, diagnostics);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
                    }
                }
                for stmt in &case.statements {
                    collect_anonymous_class_diagnostics_for_stmt(stmt, diagnostics);
                }
            }
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
        } => {
            collect_anonymous_class_diagnostics_for_block(body, diagnostics);
            for catch in catches {
                collect_anonymous_class_diagnostics_for_block(&catch.body, diagnostics);
            }
            if let Some(finally_block) = finally_block {
                collect_anonymous_class_diagnostics_for_block(finally_block, diagnostics);
            }
        }
        Stmt::LocalType(ty) => collect_anonymous_class_diagnostics_for_type(ty, diagnostics),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, diagnostics);
            }
        }
        Stmt::Block(block) => collect_anonymous_class_diagnostics_for_block(block, diagnostics),
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

fn collect_anonymous_class_diagnostics_for_expr(
    expr: &Expr,
    diagnostics: &mut Vec<RepresentationDiagnostic>,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => {
            collect_anonymous_class_diagnostics_for_expr(receiver, diagnostics);
        }
        Expr::ArrayAccess(array, index) => {
            collect_anonymous_class_diagnostics_for_expr(array, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(index, diagnostics);
        }
        Expr::Binary { left, right, .. } => {
            collect_anonymous_class_diagnostics_for_expr(left, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(right, diagnostics);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => {
            collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_anonymous_class_diagnostics_for_expr(condition, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(then_expr, diagnostics);
            collect_anonymous_class_diagnostics_for_expr(else_expr, diagnostics);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_anonymous_class_diagnostics_for_expr(receiver, diagnostics);
            }
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, diagnostics);
            }
        }
        Expr::New { ty, args, body, .. } => {
            for arg in args {
                collect_anonymous_class_diagnostics_for_expr(arg, diagnostics);
            }
            if body.is_some() {
                diagnostics.push(RepresentationDiagnostic {
                    code: "unsupported_anonymous_class",
                    message: format!(
                        "anonymous Java class for {:?} requires object representation lowering",
                        ty
                    ),
                });
            }
        }
        Expr::InstanceOf { expr, .. } => {
            collect_anonymous_class_diagnostics_for_expr(expr, diagnostics);
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_anonymous_class_diagnostics_for_expr(expr, diagnostics)
            }
            LambdaBody::Block(block) => {
                collect_anonymous_class_diagnostics_for_block(block, diagnostics);
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_type_representation(ty: &TypeDecl, plan: &mut RepresentationPlan) {
    let representation = match ty.kind {
        TypeKind::Class => ObjectRepresentation::ConcreteStruct,
        TypeKind::Interface => ObjectRepresentation::DynTrait {
            trait_name: ty.name.clone(),
        },
        TypeKind::Enum => ObjectRepresentation::ClosedEnum {
            enum_name: ty.name.clone(),
            variants: Vec::new(),
        },
    };
    plan.types.insert(
        ty.name.clone(),
        TypeRepresentation {
            java_name: ty.name.clone(),
            representation,
            provenance: RepresentationProvenance::ParsedSource,
            direct_subtypes: Vec::new(),
        },
    );
    for member in &ty.members {
        if let crate::java_ir::Member::Type(nested) = member {
            collect_type_representation(nested, plan);
        }
    }
}

fn collect_hierarchy_edges(ty: &TypeDecl, plan: &mut RepresentationPlan) {
    if let Some(super_name) = &ty.extends {
        add_direct_subtype(plan, super_name, &ty.name);
    }
    for interface_name in &ty.implements {
        add_direct_subtype(plan, interface_name, &ty.name);
    }
}

fn apply_hierarchy_overrides(facts: &FactSet, plan: &mut RepresentationPlan) {
    for override_fact in &facts.hierarchy_overrides {
        validate_hierarchy_override(override_fact, plan);
        let representation = match &override_fact.strategy {
            HierarchyStrategy::ConcreteStruct => ObjectRepresentation::ConcreteStruct,
            HierarchyStrategy::ClosedEnum {
                enum_name,
                variants,
            } => ObjectRepresentation::ClosedEnum {
                enum_name: enum_name.clone(),
                variants: variants.clone(),
            },
            HierarchyStrategy::DynTrait { trait_name } => ObjectRepresentation::DynTrait {
                trait_name: trait_name.clone(),
            },
            HierarchyStrategy::BoxedDynTrait { trait_name } => {
                ObjectRepresentation::BoxedDynTrait {
                    trait_name: trait_name.clone(),
                }
            }
            HierarchyStrategy::ErasedObject => ObjectRepresentation::ErasedObject,
        };
        let provenance = match override_fact.provenance {
            FactProvenance::ParsedSource => RepresentationProvenance::ParsedSource,
            FactProvenance::CorpusAnalysis => RepresentationProvenance::CorpusAnalysis,
            FactProvenance::UserOverride => RepresentationProvenance::UserOverride,
            FactProvenance::ConservativeFallback => RepresentationProvenance::ConservativeFallback,
        };
        let direct_subtypes = plan
            .types
            .get(&override_fact.type_name)
            .map(|entry| entry.direct_subtypes.clone())
            .unwrap_or_default();
        plan.types.insert(
            override_fact.type_name.clone(),
            TypeRepresentation {
                java_name: override_fact.type_name.clone(),
                representation,
                provenance,
                direct_subtypes,
            },
        );
    }
}

fn validate_hierarchy_override(
    override_fact: &crate::facts::HierarchyOverride,
    plan: &mut RepresentationPlan,
) {
    if !plan.types.contains_key(&override_fact.type_name) {
        plan.diagnostics.push(RepresentationDiagnostic {
            code: "representation_override_conflict",
            message: format!(
                "hierarchy override references unknown Java type {}",
                override_fact.type_name
            ),
        });
    }
    if let HierarchyStrategy::ClosedEnum { variants, .. } = &override_fact.strategy {
        let known_subtypes = plan
            .types
            .get(&override_fact.type_name)
            .map(|entry| entry.direct_subtypes.clone())
            .unwrap_or_default();
        for variant in variants {
            if !plan.types.contains_key(variant) {
                plan.diagnostics.push(RepresentationDiagnostic {
                    code: "representation_override_conflict",
                    message: format!(
                        "closed enum override for {} references unknown variant {}",
                        override_fact.type_name, variant
                    ),
                });
            } else if !known_subtypes.iter().any(|subtype| subtype == variant) {
                plan.diagnostics.push(RepresentationDiagnostic {
                    code: "representation_override_conflict",
                    message: format!(
                        "closed enum override for {} includes {}, but source facts do not record it as a direct subtype",
                        override_fact.type_name, variant
                    ),
                });
            }
        }
        for subtype in known_subtypes {
            if !variants.iter().any(|variant| variant == &subtype) {
                plan.diagnostics.push(RepresentationDiagnostic {
                    code: "representation_override_conflict",
                    message: format!(
                        "closed enum override for {} omits source subtype {}",
                        override_fact.type_name, subtype
                    ),
                });
            }
        }
    }
}

fn add_direct_subtype(plan: &mut RepresentationPlan, super_name: &str, subtype_name: &str) {
    let entry = plan
        .types
        .entry(super_name.to_string())
        .or_insert_with(|| TypeRepresentation {
            java_name: super_name.to_string(),
            representation: ObjectRepresentation::ErasedObject,
            provenance: RepresentationProvenance::ConservativeFallback,
            direct_subtypes: Vec::new(),
        });
    if !entry
        .direct_subtypes
        .iter()
        .any(|existing| existing == subtype_name)
    {
        entry.direct_subtypes.push(subtype_name.to_string());
    }
}

fn all_types(program: &Program) -> Vec<&TypeDecl> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_types(ty, &mut types);
    }
    types
}

fn collect_types<'a>(ty: &'a TypeDecl, out: &mut Vec<&'a TypeDecl>) {
    out.push(ty);
    for member in &ty.members {
        if let crate::java_ir::Member::Type(nested) = member {
            collect_types(nested, out);
        }
    }
}
