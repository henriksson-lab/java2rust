use std::collections::{HashMap, HashSet};

use crate::emit::emit_type;
use crate::exception_lowering::exception_result_type;
use crate::exception_plan::{MethodExceptionPlan, ProgramExceptionPlan};
use crate::facts::FactSet;
use crate::java_ir::{
    ConstructorDecl, Expr, FieldDecl, JavaType, Literal, Member, MethodDecl, Modifier,
    PrimitiveType, Program, Stmt, TypeDecl, TypeKind,
};
use crate::java_naming::rust_name_for_java_named_type;
use crate::representation::{ObjectRepresentation, RepresentationPlan};
use crate::rust_ir::RustType;
use crate::signature::{ReceiverPlan, SignaturePlan};
use crate::slot_render::{rust_type_for_java_type_with_facts, rust_type_for_slot_with_facts};
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RepresentationTypeEmission {
    LowerAsConcreteStruct,
    RawRustItem(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TraitImplMethodFallbackPolicy {
    MissingImplementation,
    IncompatibleImplementation {
        error: TraitImplMethodCompatibilityError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TraitImplMethodCompatibilityError {
    ReturnTypeMismatch,
    ParameterCountMismatch,
    ParameterTypeMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraitImplMethodMessagePolicy {
    message: String,
}

pub(crate) fn source_class_bound_trait_name(type_name: &str) -> String {
    format!("{}Bound", rust_name_for_java_named_type(type_name))
}

impl TraitImplMethodMessagePolicy {
    fn compile_error_body(&self, signature: &str) -> String {
        format!(
            "{} {{ panic!({:?}); }}",
            signature.trim_end_matches(';'),
            self.message
        )
    }
}

#[cfg(test)]
pub(crate) fn type_emission_for_with_facts(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> RepresentationTypeEmission {
    type_emission_for_with_facts_and_exceptions(
        representations,
        ty,
        signatures,
        facts,
        &ProgramExceptionPlan::default(),
    )
}

fn type_emission_for_with_facts_and_exceptions(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> RepresentationTypeEmission {
    let Some(representation) = representations.type_representation(&ty.name) else {
        return RepresentationTypeEmission::LowerAsConcreteStruct;
    };
    match representation.representation() {
        ObjectRepresentation::ConcreteStruct => RepresentationTypeEmission::LowerAsConcreteStruct,
        ObjectRepresentation::DynTrait { trait_name } => RepresentationTypeEmission::RawRustItem(
            trait_item_for_type(ty, trait_name, signatures, facts, exceptions),
        ),
        ObjectRepresentation::JavaEnum {
            enum_name,
            variants,
        } => {
            let rust_enum_name = rust_name_for_java_named_type(enum_name);
            let variant_items = variants
                .iter()
                .map(|variant| format!("    {variant},"))
                .collect::<Vec<_>>()
                .join("\n");
            let values = variants
                .iter()
                .map(|variant| format!("Self::{variant}"))
                .collect::<Vec<_>>()
                .join(", ");
            let value_of_arms = variants
                .iter()
                .map(|variant| format!("            {variant:?} => Self::{variant},"))
                .collect::<Vec<_>>()
                .join("\n");
            let name_arms = variants
                .iter()
                .map(|variant| format!("            Self::{variant} => {variant:?},"))
                .collect::<Vec<_>>()
                .join("\n");
            let ordinal_arms = variants
                .iter()
                .enumerate()
                .map(|(index, variant)| format!("            Self::{variant} => {index},"))
                .collect::<Vec<_>>()
                .join("\n");
            let default_impl = variants.first().map_or_else(String::new, |variant| {
                format!(
                    "\n\nimpl Default for {rust_enum_name} {{\n    fn default() -> Self {{ Self::{variant} }}\n}}"
                )
            });
            let payload_accessors = java_enum_payload_accessor_methods(ty, variants);
            RepresentationTypeEmission::RawRustItem(format!(
                "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum {rust_enum_name} {{\n{variant_items}\n}}\n{default_impl}\n\nimpl {rust_enum_name} {{\n    pub fn values() -> Vec<Self> {{ vec![{values}] }}\n\n    pub fn value_of<T: AsRef<str>>(name: T) -> Self {{\n        match name.as_ref() {{\n{value_of_arms}\n            other => panic!(\"unknown Java enum constant {{}}.{{}}\", {enum_name:?}, other),\n        }}\n    }}\n\n    #[allow(non_snake_case)]\n    pub fn valueOf<T: AsRef<str>>(name: T) -> Self {{ Self::value_of(name) }}\n\n    pub fn name(&self) -> &'static str {{\n        match self {{\n{name_arms}\n        }}\n    }}\n\n    pub fn ordinal(&self) -> i32 {{\n        match self {{\n{ordinal_arms}\n        }}\n    }}\n\n    pub fn compare_to<B: std::borrow::Borrow<Self>>(&self, other: B) -> i32 {{\n        let other = other.borrow();\n        if self.ordinal() < other.ordinal() {{ -1 }} else if self.ordinal() > other.ordinal() {{ 1 }} else {{ 0 }}\n    }}\n{payload_accessors}\n}}\n\nimpl std::fmt::Display for {rust_enum_name} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        write!(f, \"{{}}\", self.name())\n    }}\n}}"
            ))
        }
        ObjectRepresentation::ClosedEnum {
            enum_name,
            variants,
        } => RepresentationTypeEmission::RawRustItem(closed_enum_carrier_item(
            enum_name, variants, facts,
        )),
        ObjectRepresentation::SourceHierarchyCarrier { .. } => {
            RepresentationTypeEmission::LowerAsConcreteStruct
        }
        ObjectRepresentation::ErasedObject => RepresentationTypeEmission::RawRustItem(format!(
            "pub type {} = Box<dyn std::any::Any>;",
            rust_name_for_java_named_type(&ty.name)
        )),
    }
}

pub(crate) fn source_hierarchy_carrier_item_for_type(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Option<String> {
    let representation = representations.type_representation(&ty.name)?;
    let ObjectRepresentation::SourceHierarchyCarrier {
        carrier_name,
        variants,
    } = representation.representation()
    else {
        return None;
    };
    Some(source_hierarchy_carrier_item(
        carrier_name,
        variants,
        representations,
        facts,
        signatures,
        ty,
    ))
}

fn closed_enum_carrier_item(enum_name: &str, variants: &[String], facts: &FactSet) -> String {
    carrier_enum_item("#[derive(Clone, PartialEq)]", enum_name, variants, facts)
}

fn source_hierarchy_carrier_item(
    enum_name: &str,
    variants: &[String],
    representations: &RepresentationPlan,
    facts: &FactSet,
    signatures: &SignaturePlan,
    ty: &TypeDecl,
) -> String {
    let mut item = source_hierarchy_carrier_enum_item(enum_name, variants, representations);
    let mut impl_members = Vec::new();
    impl_members.extend(source_hierarchy_carrier_field_accessors(
        variants, facts, signatures, ty,
    ));
    impl_members.extend(source_hierarchy_carrier_forwarding_methods(
        enum_name, variants, facts, ty,
    ));
    impl_members.push(source_hierarchy_carrier_identity_method(
        variants,
        representations,
    ));
    if !impl_members.is_empty() {
        item.push_str("\n\n");
        item.push_str(&format!(
            "impl {enum_name} {{\n{}\n}}",
            impl_members.join("\n\n")
        ));
    }
    item
}

fn source_hierarchy_carrier_identity_method(
    variants: &[String],
    representations: &RepresentationPlan,
) -> String {
    let arms = variants
        .iter()
        .map(|variant| {
            let placement = storage_placement_for_emitted_type_name(representations, variant);
            let comparison = if placement.shared() && placement.is_implemented() {
                let handle = if placement.sync() {
                    "std::sync::Arc"
                } else {
                    "std::rc::Rc"
                };
                format!("{handle}::ptr_eq(left, right)")
            } else {
                "std::ptr::eq(left, right)".to_string()
            };
            format!("            (Self::{variant}(left), Self::{variant}(right)) => {comparison},")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "    pub fn java_identity_eq(&self, other: &Self) -> bool {{\n        match (self, other) {{\n{arms}\n            _ => false,\n        }}\n    }}"
    )
}

fn source_hierarchy_carrier_enum_item(
    enum_name: &str,
    variants: &[String],
    representations: &RepresentationPlan,
) -> String {
    let variants = variants
        .iter()
        .map(|variant| {
            format!(
                "    {variant}({}),",
                source_hierarchy_carrier_variant_payload_type(variant, representations)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("#[derive(Clone)]\npub enum {enum_name} {{\n{variants}\n}}")
}

fn carrier_enum_item(
    derive: &str,
    enum_name: &str,
    variants: &[String],
    facts: &FactSet,
) -> String {
    let variants = variants
        .iter()
        .map(|variant| {
            format!(
                "    {variant}({}),",
                carrier_variant_payload_type(variant, facts)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("{derive}\npub enum {enum_name} {{\n{variants}\n}}")
}

fn source_hierarchy_carrier_forwarding_methods(
    enum_name: &str,
    variants: &[String],
    facts: &FactSet,
    ty: &TypeDecl,
) -> Vec<String> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Method(method) = member else {
                return None;
            };
            source_hierarchy_carrier_forwarding_method(enum_name, variants, facts, method)
        })
        .collect()
}

fn source_hierarchy_carrier_forwarding_method(
    _enum_name: &str,
    variants: &[String],
    facts: &FactSet,
    method: &MethodDecl,
) -> Option<String> {
    if method.modifiers.contains(&Modifier::Static) || !method.params.is_empty() {
        return None;
    }
    let rust_name = rust_identifier_token(&crate::java_signature::source_method_rust_name(method));
    let return_ty = emit_type(&rust_type_for_java_type_with_facts(
        &method.return_type,
        facts,
    ));
    let arms = variants
        .iter()
        .map(|variant| format!("            Self::{variant}(value) => value.{rust_name}(),"))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "    pub fn {rust_name}(&self) -> {return_ty} {{\n        match self {{\n{arms}\n        }}\n    }}"
    ))
}

fn source_hierarchy_carrier_field_accessors(
    variants: &[String],
    facts: &FactSet,
    signatures: &SignaturePlan,
    ty: &TypeDecl,
) -> Vec<String> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            source_hierarchy_carrier_field_accessor(variants, facts, signatures, ty, field)
        })
        .collect()
}

fn source_hierarchy_carrier_field_accessor(
    variants: &[String],
    facts: &FactSet,
    signatures: &SignaturePlan,
    ty: &TypeDecl,
    field: &FieldDecl,
) -> Option<String> {
    if field.modifiers.contains(&Modifier::Static) {
        return None;
    }
    let rust_name = rust_identifier_token(&crate::parser::to_snake_case(&field.name));
    let field_shape = signatures.required_declared_field_shape(ty, field);
    let return_ty = emit_type(&rust_type_for_slot_with_facts(&field_shape, facts));
    let projection = if source_hierarchy_carrier_field_accessor_returns_copy(&field.ty) {
        format!("value.{rust_name}")
    } else {
        format!("value.{rust_name}.clone()")
    };
    let arms = variants
        .iter()
        .map(|variant| format!("            Self::{variant}(value) => {projection},"))
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "    pub fn {rust_name}(&self) -> {return_ty} {{\n        match self {{\n{arms}\n        }}\n    }}"
    ))
}

fn source_hierarchy_carrier_field_accessor_returns_copy(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Primitive(_))
}

/// The payload a carrier variant stores.
///
/// A variant payload is a *slot* of its type like any other, so a shared type
/// sits behind its handle here too. Storing it bare made extraction hand back a
/// bare value regardless of the consumer --
/// `match line { HeaderLineValue::FormatLine(v) => v }` typed `FormatLine` into
/// a slot expecting `Rc<FormatLine>` -- and no wrapping at consumers fixes that
/// in general. Construction uses the same placement, so the two agree.
/// See [0002b](../claude_audits/0002b-shared-representation-design.md).
fn carrier_variant_payload_type(variant: &str, facts: &FactSet) -> String {
    let placement = facts.storage_placement_for(variant);
    if placement.shared() && placement.is_implemented() {
        placement.wrap(variant.to_string())
    } else {
        variant.to_string()
    }
}

fn source_hierarchy_carrier_variant_payload_type(
    variant: &str,
    representations: &RepresentationPlan,
) -> String {
    let placement = storage_placement_for_emitted_type_name(representations, variant);
    if placement.shared() && placement.is_implemented() {
        placement.wrap(variant.to_string())
    } else {
        variant.to_string()
    }
}

fn storage_placement_for_emitted_type_name(
    representations: &RepresentationPlan,
    emitted_name: &str,
) -> crate::storage_strategy::StoragePlacement {
    representations
        .type_representations()
        .find(|entry| rust_name_for_java_named_type(entry.java_name()) == emitted_name)
        .map(|entry| representations.storage_placement_for(entry.java_name()))
        .unwrap_or_else(|| representations.storage_placement_for(emitted_name))
}

fn java_enum_payload_accessor_methods(ty: &TypeDecl, variants: &[String]) -> String {
    let Some(constructor) = ty.members.iter().find_map(|member| match member {
        Member::Constructor(constructor) => Some(constructor),
        _ => None,
    }) else {
        return String::new();
    };
    let field_arg_indices = java_enum_constructor_field_arg_indices(constructor);
    if field_arg_indices.is_empty() {
        return String::new();
    }
    let field_types = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if !field.modifiers.contains(&Modifier::Static) => {
                Some((field.name.clone(), field.ty.clone()))
            }
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    let constants = java_enum_constant_args(ty);
    field_arg_indices
        .into_iter()
        .filter_map(|(field_name, arg_index)| {
            let field_ty = field_types.get(&field_name)?;
            let rust_return_ty = java_enum_payload_accessor_return_type(field_ty)?;
            let accessor_name = rust_identifier_token(&crate::parser::to_snake_case(&field_name));
            let arms = variants
                .iter()
                .map(|variant| {
                    let value = constants
                        .get(variant)
                        .and_then(|args| args.get(arg_index))
                        .and_then(|arg| java_enum_payload_arg_value(arg, field_ty))
                        .unwrap_or_else(|| java_enum_payload_default_value(field_ty));
                    format!("            Self::{variant} => {value},")
                })
                .collect::<Vec<_>>()
                .join("\n");
            Some(format!(
                "\n\n    pub fn {accessor_name}(&self) -> {rust_return_ty} {{\n        match self {{\n{arms}\n        }}\n    }}"
            ))
        })
        .collect::<Vec<_>>()
        .join("")
}

fn java_enum_constructor_field_arg_indices(constructor: &ConstructorDecl) -> Vec<(String, usize)> {
    let Some(body) = constructor.body.as_ref() else {
        return Vec::new();
    };
    let param_indices = constructor
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| (param.source_name.clone(), index))
        .collect::<HashMap<_, _>>();
    body.statements
        .iter()
        .filter_map(|stmt| {
            let Stmt::Assign { target, value } = stmt else {
                return None;
            };
            let field_name = match target {
                Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
                    crate::parser::to_snake_case(field)
                }
                Expr::Var(name) => crate::parser::to_snake_case(name.strip_prefix("this.")?),
                _ => return None,
            };
            let Expr::Var(param_name) = value else {
                return None;
            };
            param_indices
                .get(param_name)
                .copied()
                .map(|arg_index| (field_name, arg_index))
        })
        .collect()
}

fn rust_identifier_token(name: &str) -> String {
    if crate::java_naming::rust_keyword_is_rawable_identifier(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

fn java_enum_constant_args(ty: &TypeDecl) -> HashMap<String, Vec<Expr>> {
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            if !java_enum_constant_field_matches_type(field, &ty.name) {
                return None;
            }
            let Expr::New { args, .. } = field.initializer.as_ref()? else {
                return None;
            };
            Some((field.name.clone(), args.clone()))
        })
        .collect()
}

fn java_enum_constant_field_matches_type(field: &FieldDecl, enum_name: &str) -> bool {
    field.modifiers.contains(&Modifier::Static)
        && field.modifiers.contains(&Modifier::Final)
        && matches!(&field.ty, JavaType::Named { name, .. } if name == enum_name)
}

fn java_enum_payload_accessor_return_type(field_ty: &JavaType) -> Option<String> {
    match field_ty {
        JavaType::String => Some("String".to_string()),
        JavaType::Primitive(primitive) => Some(emit_type(&rust_type_for_primitive(*primitive))),
        _ => None,
    }
}

fn rust_type_for_primitive(primitive: PrimitiveType) -> RustType {
    match primitive {
        PrimitiveType::Boolean => RustType::Bool,
        PrimitiveType::Byte => RustType::I8,
        PrimitiveType::Short => RustType::I16,
        PrimitiveType::Char => RustType::Char,
        PrimitiveType::Int => RustType::I32,
        PrimitiveType::Long => RustType::I64,
        PrimitiveType::Float => RustType::F32,
        PrimitiveType::Double => RustType::F64,
    }
}

fn java_enum_payload_arg_value(arg: &Expr, field_ty: &JavaType) -> Option<String> {
    match (field_ty, arg) {
        (JavaType::String, Expr::Literal(Literal::String(value))) => {
            Some(format!("{value:?}.to_string()"))
        }
        (_, Expr::Literal(Literal::Bool(value))) => Some(value.to_string()),
        (_, Expr::Literal(Literal::Int(value))) => Some(value.to_string()),
        (_, Expr::Literal(Literal::Long(value))) => Some(format!("{value}i64")),
        (_, Expr::Literal(Literal::Float(value))) => Some(format!("{value}f32")),
        (_, Expr::Literal(Literal::Double(value))) => Some(format!("{value}f64")),
        (_, Expr::Literal(Literal::Char(value))) => Some(format!("{value:?}")),
        _ => None,
    }
}

fn java_enum_payload_default_value(field_ty: &JavaType) -> String {
    match field_ty {
        JavaType::String => "String::new()".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean) => "false".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Byte)
        | JavaType::Primitive(crate::java_ir::PrimitiveType::Short)
        | JavaType::Primitive(crate::java_ir::PrimitiveType::Int)
        | JavaType::Primitive(crate::java_ir::PrimitiveType::Long) => "0".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Float) => "0.0f32".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Double) => "0.0f64".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Char) => "'\\0'".to_string(),
        _ => "Default::default()".to_string(),
    }
}

pub(crate) fn type_emission_for_program_with_facts_and_exceptions(
    representations: &RepresentationPlan,
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> RepresentationTypeEmission {
    let Some(representation) = representations.type_representation(&ty.name) else {
        return RepresentationTypeEmission::LowerAsConcreteStruct;
    };
    match representation.representation() {
        ObjectRepresentation::DynTrait { trait_name } => {
            RepresentationTypeEmission::RawRustItem(trait_item_for_type_with_program(
                ty, trait_name, program, signatures, facts, exceptions,
            ))
        }
        _ => type_emission_for_with_facts_and_exceptions(
            representations,
            ty,
            signatures,
            facts,
            exceptions,
        ),
    }
}

#[cfg(test)]
fn raw_type_item_for_with_facts(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Option<String> {
    raw_type_item_for_with_facts_and_exceptions(
        representations,
        ty,
        signatures,
        facts,
        &ProgramExceptionPlan::default(),
    )
}

#[cfg(test)]
fn raw_type_item_for_with_facts_and_exceptions(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> Option<String> {
    match type_emission_for_with_facts_and_exceptions(
        representations,
        ty,
        signatures,
        facts,
        exceptions,
    ) {
        RepresentationTypeEmission::RawRustItem(item) => Some(item),
        RepresentationTypeEmission::LowerAsConcreteStruct => None,
    }
}

pub(crate) fn raw_type_item_for_program_with_facts_and_exceptions(
    representations: &RepresentationPlan,
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> Option<String> {
    match type_emission_for_program_with_facts_and_exceptions(
        representations,
        program,
        ty,
        signatures,
        facts,
        exceptions,
    ) {
        RepresentationTypeEmission::RawRustItem(item) => Some(item),
        RepresentationTypeEmission::LowerAsConcreteStruct => None,
    }
}

#[cfg(test)]
pub(crate) fn trait_impl_emissions_for_type_with_facts(
    representations: &RepresentationPlan,
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Vec<String> {
    trait_impl_emissions_for_type_with_facts_and_exceptions(
        representations,
        program,
        ty,
        signatures,
        facts,
        &ProgramExceptionPlan::default(),
    )
}

pub(crate) fn trait_impl_emissions_for_type_with_facts_and_exceptions(
    representations: &RepresentationPlan,
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> Vec<String> {
    if !matches!(ty.kind, TypeKind::Class | TypeKind::Enum) {
        return Vec::new();
    }
    implemented_interfaces_for_type(program, ty)
        .into_iter()
        .filter_map(|interface_name| {
            let trait_name = dyn_trait_name_for(representations, &interface_name)?;
            let interface = find_type_decl(program, &interface_name)?;
            trait_impl_for_type(
                ty,
                interface,
                program,
                &trait_name,
                signatures,
                facts,
                exceptions,
            )
        })
        .collect()
}

pub(crate) fn source_class_bound_trait_item_for_type(
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> Option<String> {
    if !source_class_is_used_as_type_param_bound(program, &ty.name) {
        return None;
    }
    if !matches!(ty.kind, TypeKind::Class) {
        return None;
    }
    let trait_name = source_class_bound_trait_name(&ty.name);
    let declared_method_keys = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if !method.modifiers.contains(&Modifier::Static) => Some((
                crate::java_signature::source_method_rust_name(method),
                method.params.len(),
            )),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if !method.modifiers.contains(&Modifier::Static) => {
                let candidates =
                    source_class_bound_trait_method_implementation_candidates(program, ty, method);
                let return_shape = source_class_bound_trait_method_return_shape_plan(
                    ty,
                    method,
                    candidates.as_slice(),
                    signatures,
                );
                Some(source_class_bound_trait_method_signature_with_return_shape(
                    ty,
                    method,
                    program,
                    signatures,
                    facts,
                    exceptions.method_plan(ty, method),
                    return_shape,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    methods.extend(source_class_bound_trait_external_fact_methods(
        ty,
        facts,
        &declared_method_keys,
    ));
    // Class-bound traits need the handle forwarding just as interface traits do:
    // this is the trait a Java `<T extends SomeClass>` bound lowers to, so
    // `Standards<Rc<FormatLine>>` fails without it (claude_audits/0002b).
    let forwarding = shared_handle_forwarding_impl(&trait_name, &methods)
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    Some(format!(
        "pub trait {trait_name} {{\n{}\n}}{forwarding}",
        methods.join("\n")
    ))
}

fn source_class_bound_trait_external_fact_methods(
    ty: &TypeDecl,
    facts: &FactSet,
    declared_method_keys: &HashSet<(String, usize)>,
) -> Vec<String> {
    facts
        .external_api_effect_facts
        .iter()
        .filter_map(|fact| {
            let receiver = fact.receiver_type.as_deref()?;
            if !crate::facts::external_type_name_matches(&ty.name, receiver) {
                return None;
            }
            let arg_count = fact.arg_count.unwrap_or(0);
            let rust_name = crate::call_plan::external_method_rust_name(
                receiver,
                &fact.method_name,
                arg_count,
                facts,
            );
            if declared_method_keys.contains(&(rust_name.clone(), arg_count)) {
                return None;
            }
            Some(source_class_bound_trait_external_fact_method(
                fact, &rust_name, arg_count, facts,
            ))
        })
        .collect()
}

fn source_class_bound_trait_external_fact_method(
    fact: &crate::facts::ExternalApiEffectFact,
    rust_name: &str,
    arg_count: usize,
    facts: &FactSet,
) -> String {
    let receiver = match fact.receiver_mutability {
        crate::facts::ReceiverMutabilityFact::MutatesReceiver => "&mut self",
        _ => "&self",
    };
    let args = (0..arg_count)
        .map(|index| format!("_arg{index}: T{index}"))
        .collect::<Vec<_>>();
    let type_params = if arg_count == 0 {
        String::new()
    } else {
        format!(
            "<{}>",
            (0..arg_count)
                .map(|index| format!("T{index}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let params = std::iter::once(receiver.to_string())
        .chain(args)
        .collect::<Vec<_>>()
        .join(", ");
    let rust_name = raw_ident_if_needed(rust_name);
    let return_type = source_class_bound_trait_external_fact_return_type(fact, facts);
    let return_sig = return_type
        .as_ref()
        .map(|return_type| format!(" -> {}", emit_type(return_type)))
        .unwrap_or_default();
    let body = source_class_bound_trait_external_fact_body(return_type.as_ref());
    format!("    fn {rust_name}{type_params}({params}){return_sig} {{ {body} }}")
}

fn raw_ident_if_needed(name: &str) -> String {
    if name.starts_with("r#")
        || name.contains("::")
        || name.contains('<')
        || name.contains('>')
        || name.contains(' ')
        || name.is_empty()
    {
        return name.to_string();
    }
    if crate::java_naming::rust_keyword_is_rawable_identifier(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

fn source_class_bound_trait_external_fact_return_type(
    fact: &crate::facts::ExternalApiEffectFact,
    facts: &FactSet,
) -> Option<RustType> {
    let base = fact
        .return_type
        .as_ref()
        .map(|return_type| rust_type_for_java_type_with_facts(return_type, facts));
    if fact.thrown.is_empty() {
        return base;
    }
    Some(exception_result_type(base.unwrap_or(RustType::Unit)))
}

fn source_class_bound_trait_external_fact_body(return_type: Option<&RustType>) -> String {
    match return_type {
        Some(RustType::String) => "String::new()".to_string(),
        Some(RustType::Generic { base, .. }) if base == "Result" => {
            "Err(JavaException::new(\"UnsupportedOperationException\", \"external type-parameter bound method is not modelled\".to_string()))".to_string()
        }
        Some(_) => "Default::default()".to_string(),
        None => String::new(),
    }
}

pub(crate) fn source_class_bound_trait_impl_emissions_for_type(
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> Vec<String> {
    if !matches!(ty.kind, TypeKind::Class | TypeKind::Enum) {
        return Vec::new();
    }
    source_class_bounds_for_type(program, ty)
        .into_iter()
        .filter_map(|bound| {
            let trait_name = source_class_bound_trait_name(&bound.name);
            let impl_type_name = rust_name_for_java_named_type(&ty.name);
            let methods = bound
                .members
                .iter()
                .filter_map(|member| match member {
                    Member::Method(method) if !method.modifiers.contains(&Modifier::Static) => {
                        Some(source_class_bound_trait_impl_method(
                            ty,
                            bound,
                            method,
                            program,
                            signatures,
                            facts,
                            exceptions.method_plan(bound, method),
                        ))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            Some(format!(
                "impl {trait_name} for {impl_type_name} {{\n{}\n}}",
                methods.join("\n")
            ))
        })
        .collect()
}

fn source_class_bound_trait_method_signature_with_return_shape(
    ty: &TypeDecl,
    method: &MethodDecl,
    program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    return_shape_override: Option<RustSlotShape>,
) -> String {
    let signature = signatures.required_method_signature(ty, method);
    let mut params = Vec::new();
    params.push(match signature.receiver() {
        ReceiverPlan::Mutable => "&mut self".to_string(),
        ReceiverPlan::Shared | ReceiverPlan::None => "&self".to_string(),
    });
    let param_shapes = trait_method_param_shape_plan(ty, method, program, signatures);
    params.extend(signature.params().iter().enumerate().map(|(index, param)| {
        let shape = param_shapes.get(index).unwrap_or(&param.shape());
        format!(
            "{}: {}",
            param.name(),
            emit_type(&rust_type_for_slot_with_facts(shape, facts))
        )
    }));
    let return_type = source_class_bound_trait_method_return_type_with_shape(
        method,
        facts,
        exceptions,
        return_shape_override.as_ref(),
    )
    .map(|ty| format!(" -> {}", emit_type(&ty)))
    .unwrap_or_default();
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    let type_params = rust_method_type_params(method, program, &ty.name);
    let where_clause = rust_generic_trait_method_where_clause(method);
    format!(
        "    fn {rust_name}{type_params}({}){return_type}{where_clause};",
        params.join(", ")
    )
}

fn source_class_bound_trait_method_return_type_with_shape(
    method: &MethodDecl,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    return_shape_override: Option<&RustSlotShape>,
) -> Option<RustType> {
    if method.return_type == JavaType::Void {
        if exceptions.wrap_returns_in_result() {
            return Some(exception_result_type(RustType::Unit));
        }
        return None;
    }
    let rust_return = return_shape_override
        .map(|shape| rust_type_for_slot_with_facts(shape, facts))
        .unwrap_or_else(|| rust_type_for_java_type_with_facts(&method.return_type, facts));
    if exceptions.wrap_returns_in_result() {
        Some(exception_result_type(rust_return))
    } else {
        Some(rust_return)
    }
}

fn source_class_bound_trait_impl_method(
    ty: &TypeDecl,
    bound: &TypeDecl,
    bound_method: &MethodDecl,
    program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
) -> String {
    let candidates = implementing_method_candidates_in_hierarchy(ty, bound_method, program);
    let trait_return_shape = source_class_bound_trait_method_return_shape_plan(
        bound,
        bound_method,
        candidates.as_slice(),
        signatures,
    );
    let trait_return_shape_for_body = trait_return_shape.clone();
    let signature = source_class_bound_trait_method_signature_with_return_shape(
        bound,
        bound_method,
        program,
        signatures,
        facts,
        exceptions,
        trait_return_shape,
    );
    if candidates.is_empty() {
        return trait_impl_method_message_policy(
            ty,
            bound_method,
            TraitImplMethodFallbackPolicy::MissingImplementation,
        )
        .compile_error_body(&signature);
    }
    let (implementing_owner, implementing_method) = candidates
        .iter()
        .copied()
        .find(|(_, candidate)| {
            trait_impl_method_compatibility_error(bound_method, candidate).is_none()
        })
        .unwrap_or(candidates[0]);
    if let Some(message) = trait_impl_method_compatibility_error(bound_method, implementing_method)
    {
        return trait_impl_method_message_policy(
            ty,
            bound_method,
            TraitImplMethodFallbackPolicy::IncompatibleImplementation { error: message },
        )
        .compile_error_body(&signature);
    }
    let rust_name = implementing_method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&implementing_method.name));
    let bound_signature = signatures.required_method_signature(bound, bound_method);
    let forwarding_owner = trait_impl_forwarding_owner_for_inherited_source_method(
        ty,
        implementing_owner,
        implementing_method,
    );
    let implementation_signature = signatures
        .method_signature(forwarding_owner, implementing_method)
        .unwrap_or_else(|| {
            signatures.required_method_signature(implementing_owner, implementing_method)
        });
    let args = bound_signature
        .params()
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let implementation_shape = implementation_signature
                .params()
                .get(index)
                .map(|param| param.shape());
            trait_impl_forwarded_arg(param.name(), param.shape(), implementation_shape)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let receiver_arg = if args.is_empty() {
        trait_impl_forwarding_receiver_arg(
            ty,
            forwarding_owner,
            Some(implementation_signature.receiver()),
        )
    } else {
        format!(
            "{}, {args}",
            trait_impl_forwarding_receiver_arg(
                ty,
                forwarding_owner,
                Some(implementation_signature.receiver())
            )
        )
    };
    let implementing_owner_name = rust_name_for_java_named_type(&forwarding_owner.name);
    let call = format!("{implementing_owner_name}::{rust_name}({receiver_arg})");
    let call = adapt_source_class_bound_trait_impl_return(
        call,
        trait_return_shape_for_body
            .as_ref()
            .or(bound_signature.return_shape()),
        &implementation_signature.return_shape_cloned(),
        exceptions,
    );
    let body = if source_class_bound_trait_method_return_type_with_shape(
        bound_method,
        facts,
        exceptions,
        trait_return_shape_for_body.as_ref(),
    )
    .is_some()
    {
        call
    } else {
        format!("{call};")
    };
    format!("{} {{ {body} }}", signature.trim_end_matches(';'))
}

fn source_class_bound_trait_method_return_shape_plan(
    bound: &TypeDecl,
    bound_method: &MethodDecl,
    candidates: &[(&TypeDecl, &MethodDecl)],
    signatures: &SignaturePlan,
) -> Option<RustSlotShape> {
    let bound_shape = signatures
        .required_method_signature(bound, bound_method)
        .return_shape()
        .cloned()?;
    for (implementation_owner, implementation_method) in candidates {
        if trait_impl_method_compatibility_error(bound_method, implementation_method).is_some() {
            continue;
        }
        let Some(implementation_shape) = signatures
            .method_signature(implementation_owner, implementation_method)
            .unwrap_or_else(|| {
                signatures.required_method_signature(implementation_owner, implementation_method)
            })
            .return_shape()
            .cloned()
        else {
            continue;
        };
        if trait_return_shape_should_widen_to_implementation(&bound_shape, &implementation_shape) {
            return Some(implementation_shape);
        }
    }
    Some(bound_shape)
}

fn source_class_bound_trait_method_implementation_candidates<'a>(
    program: &'a Program,
    bound: &'a TypeDecl,
    bound_method: &MethodDecl,
) -> Vec<(&'a TypeDecl, &'a MethodDecl)> {
    program
        .types
        .iter()
        .filter(|ty| {
            matches!(ty.kind, TypeKind::Class | TypeKind::Enum)
                && source_class_bounds_for_type(program, ty)
                    .into_iter()
                    .any(|candidate_bound| candidate_bound.name == bound.name)
        })
        .filter_map(|ty| {
            implementing_method_candidates_in_hierarchy(ty, bound_method, program)
                .into_iter()
                .find(|candidate| {
                    trait_impl_method_compatibility_error(bound_method, candidate.1).is_none()
                })
        })
        .collect()
}

fn adapt_source_class_bound_trait_impl_return(
    call: String,
    trait_return_shape: Option<&RustSlotShape>,
    implementation_return_shape: &Option<RustSlotShape>,
    exceptions: MethodExceptionPlan,
) -> String {
    if exceptions.wrap_returns_in_result() {
        return call;
    }
    let Some(trait_shape) = trait_return_shape else {
        return call;
    };
    if trait_shape.optional_element_type().is_some() {
        return call;
    }
    if let Some(shape) = implementation_return_shape
        && shape
            .optional_element_type()
            .is_some_and(|element_ty| element_ty == trait_shape.java_ty)
    {
        return format!("{call}.expect(\"Java null receiver\")");
    }
    call
}

fn implemented_interfaces_for_type(program: &Program, ty: &TypeDecl) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    collect_implemented_interfaces_for_type(program, ty, &mut seen, &mut out);
    out
}

fn collect_implemented_interfaces_for_type(
    program: &Program,
    ty: &TypeDecl,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    for interface_name in &ty.implements {
        collect_implemented_interface_name(program, &ty.name, interface_name, seen, out);
    }
    if let Some(super_name) = ty.extends.as_deref() {
        if ty.kind == TypeKind::Interface {
            // For an interface, `extends` names a *super-interface*, which the
            // type implements. Treating it as a superclass meant a class
            // implementing `P extends G` never got `impl G for C`, so the
            // subtrait bound could not be satisfied (`E0277`).
            collect_implemented_interface_name(program, &ty.name, super_name, seen, out);
        } else if let Some(super_ty) =
            find_type_decl_for_owner_context(program, &ty.name, super_name)
        {
            collect_implemented_interfaces_for_type(program, super_ty, seen, out);
        }
    }
}

fn collect_implemented_interface_name(
    program: &Program,
    owner_name: &str,
    interface_name: &str,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    let interface = find_type_decl_for_owner_context(program, owner_name, interface_name);
    let canonical_name = interface
        .map(|interface| interface.name.as_str())
        .unwrap_or(interface_name);
    if !seen.insert(canonical_name.to_string()) {
        return;
    }
    out.push(canonical_name.to_string());
    if let Some(interface) = interface {
        collect_implemented_interfaces_for_type(program, interface, seen, out);
    }
}

fn trait_item_for_type(
    ty: &TypeDecl,
    trait_name: &str,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> String {
    let trait_name = rust_name_for_java_named_type(trait_name);
    let mut methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(trait_method_signature(
                ty,
                method,
                signatures,
                facts,
                exceptions.method_plan(ty, method),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    methods.extend(trait_object_support_method_signatures(&trait_name));
    let forwarding = shared_handle_forwarding_impl(&trait_name, &methods)
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    let closure_adapter = zero_arg_closure_trait_impl(ty, &trait_name, facts)
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    format!(
        "pub trait {trait_name} {{\n{}\n}}\n\n{}\n\n{}{forwarding}{closure_adapter}",
        methods.join("\n"),
        dyn_trait_anonymous_impl(&trait_name),
        boxed_dyn_trait_clone_impl(&trait_name)
    )
}

fn trait_item_for_type_with_program(
    ty: &TypeDecl,
    trait_name: &str,
    program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> String {
    let trait_name = rust_name_for_java_named_type(trait_name);
    let mut methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(trait_method_signature_with_receiver_and_return_shape(
                ty,
                method,
                signatures,
                facts,
                trait_method_exception_plan(ty, method, program, exceptions),
                trait_method_receiver_plan(ty, method, program, signatures),
                trait_method_return_shape_plan(ty, method, program, signatures),
                program,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    methods.extend(trait_object_support_method_signatures(&trait_name));
    let forwarding = shared_handle_forwarding_impl(&trait_name, &methods)
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    let closure_adapter = zero_arg_closure_trait_impl(ty, &trait_name, facts)
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    format!(
        "pub trait {trait_name} {{\n{}\n}}\n\n{}\n\n{}{forwarding}{closure_adapter}",
        methods.join("\n"),
        dyn_trait_anonymous_impl(&trait_name),
        boxed_dyn_trait_clone_impl(&trait_name)
    )
}

pub(crate) fn zero_arg_closure_trait_impl(
    ty: &TypeDecl,
    trait_name: &str,
    facts: &FactSet,
) -> Option<String> {
    let method = zero_arg_functional_method(ty)?;
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    let (fn_bound, method_return, method_body) = if matches!(method.return_type, JavaType::Void) {
        ("Fn()".to_string(), String::new(), "self()".to_string())
    } else {
        let return_shape = RustSlotShape::for_storage(method.return_type.clone());
        let return_ty = emit_type(&rust_type_for_slot_with_facts(&return_shape, facts));
        (
            format!("Fn() -> {return_ty}"),
            format!(" -> {return_ty}"),
            "self()".to_string(),
        )
    };
    Some(format!(
        "impl<__JavaAnonymous> {trait_name} for __JavaAnonymous\nwhere\n    __JavaAnonymous: {fn_bound} + Clone + 'static,\n{{\n    fn {rust_name}(&self){method_return} {{ {method_body} }}\n    fn as_any(&self) -> &dyn std::any::Any {{ self }}\n    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}> {{ std::boxed::Box::new(self.clone()) }}\n}}"
    ))
}

fn zero_arg_functional_method(ty: &TypeDecl) -> Option<&MethodDecl> {
    if ty.kind != TypeKind::Interface {
        return None;
    }
    let mut methods = ty.members.iter().filter_map(|member| match member {
        Member::Method(method) if method.body.is_none() => Some(method),
        _ => None,
    });
    let method = methods.next()?;
    if methods.next().is_some()
        || !method.params.is_empty()
        || !method.type_params.is_empty()
        || method.modifiers.contains(&Modifier::Static)
    {
        return None;
    }
    Some(method)
}

fn trait_impl_for_type(
    ty: &TypeDecl,
    interface: &TypeDecl,
    program: &Program,
    trait_name: &str,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> Option<String> {
    let trait_name = rust_name_for_java_named_type(trait_name);
    let impl_type_name = rust_name_for_java_named_type(&ty.name);
    let methods = interface
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(interface_method) => trait_impl_method(
                ty,
                interface,
                interface_method,
                program,
                trait_method_receiver_plan(interface, interface_method, program, signatures),
                signatures,
                facts,
                trait_method_exception_plan(interface, interface_method, program, exceptions),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut methods = methods;
    methods.extend(trait_object_support_impl_methods(&trait_name));
    Some(format!(
        "impl {trait_name} for {impl_type_name} {{\n{}\n}}",
        methods.join("\n")
    ))
}

pub(crate) fn trait_object_support_method_signatures(trait_name: &str) -> Vec<String> {
    vec![
        "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
        format!("    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}>;"),
    ]
}

fn trait_object_support_impl_methods(trait_name: &str) -> Vec<String> {
    vec![
        "    fn as_any(&self) -> &dyn std::any::Any { self }".to_string(),
        format!(
            "    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}> {{ panic!(\"java2rust: clone_box for {trait_name} requires a cloneable Java object carrier\") }}"
        ),
    ]
}

pub(crate) fn boxed_dyn_trait_clone_impl(trait_name: &str) -> String {
    // Qualified rather than `self.clone_box()`. A Java interface that extends
    // another lowers to a subtrait, and both declare `clone_box`, so an
    // unqualified call is ambiguous (`E0034`). Naming the trait picks the one
    // whose return type matches this `Box`.
    format!(
        "impl Clone for std::boxed::Box<dyn {trait_name}> {{\n    fn clone(&self) -> Self {{ {trait_name}::clone_box(&**self) }}\n}}"
    )
}

/// Forwards a trait through a shared handle: `impl<T: Trait> Trait for Rc<T>`.
///
/// `Rc<T>` does not implement what `T` implements, so without this a wrapped
/// value cannot satisfy a Java bound like `Standards<T extends Named>` --
/// which is how the shared representation reaches user generics at all
/// ([0002b](../claude_audits/0002b-shared-representation-design.md)).
///
/// A direct `Rc<T>`/`Arc<T>` implementation is valid only for an `&self`-only
/// surface. Mutable shared storage instead gets implementations for
/// `Rc<RefCell<T>>` and `Arc<Mutex<T>>`, which acquire the handle's interior
/// mutability per call. A borrowed return can be forwarded through a direct
/// `Rc<T>`/`Arc<T>` because it is tied to the handle's own `&self` lifetime;
/// it is deliberately not forwarded through either cell handle, whose guard
/// would be dropped at the method boundary.
pub(crate) fn shared_handle_forwarding_impl(
    trait_name: &str,
    signatures: &[String],
) -> Option<String> {
    let direct_methods = signatures
        .iter()
        .map(|signature| forwarded_trait_method(signature, SharedTraitHandle::Direct))
        .collect::<Option<Vec<_>>>();
    let local_methods = signatures
        .iter()
        .map(|signature| forwarded_trait_method(signature, SharedTraitHandle::Local))
        .collect::<Option<Vec<_>>>();
    let sync_methods = signatures
        .iter()
        .map(|signature| forwarded_trait_method(signature, SharedTraitHandle::Sync))
        .collect::<Option<Vec<_>>>();
    // An empty trait is *trivially* forwardable, and the impl is still required:
    // a Java class used as a bare type-parameter bound (`<T extends HeaderLine>`)
    // may declare no methods at all, and without `impl .. for Rc<..>` the bound
    // `Rc<FormatLine>: HeaderLineBound` still fails. Returning `None` here was a
    // bug that made exactly that case unfixable.
    // `__JavaShared`, not `T`: a Java interface method may declare its own type
    // parameter (`<T> T id(T value)`), and an inner generic that shadows the
    // impl's is E0403. Pinned by `does_not_shadow_a_method_type_parameter`.
    //
    // Both handles: the tier picks `Arc` for anything reachable from a Java
    // `static` (which must be `Sync`) and `Rc` otherwise, so a bound like
    // `Standards<T extends Named>` can be met by either.
    //
    // No `?Sized`: a forwarded method may need `Self: Sized`, and the wrap only
    // ever targets concrete source types, never `Rc<dyn Trait>`. The `'static`
    // bound is required by the trait-object support methods (`as_any` and
    // `clone_box`) once the handle itself can be boxed as `dyn Trait`.
    let mut implementations = Vec::new();
    if let Some(methods) = direct_methods {
        let body = methods.join("\n");
        for handle in ["std::rc::Rc", "std::sync::Arc"] {
            implementations.push(format!(
                "impl<__JavaShared: {trait_name} + 'static> {trait_name} for {handle}<__JavaShared> {{\n{body}\n}}"
            ));
        }
    }
    for (handle, methods) in [
        (
            "std::rc::Rc<std::cell::RefCell<__JavaShared>>",
            local_methods,
        ),
        (
            "std::sync::Arc<std::sync::Mutex<__JavaShared>>",
            sync_methods,
        ),
    ] {
        if let Some(methods) = methods {
            implementations.push(format!(
                "impl<__JavaShared: {trait_name} + 'static> {trait_name} for {handle} {{\n{}\n}}",
                methods.join("\n")
            ));
        }
    }
    (!implementations.is_empty()).then(|| implementations.join("\n\n"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SharedTraitHandle {
    Direct,
    Local,
    Sync,
}

/// Turns one rendered trait-method signature into a forwarding implementation.
///
/// The signature is text this module just produced, so the parse is closed over
/// our own output rather than arbitrary Rust: `    fn name<generics>(&self, a: A) -> R;`
fn forwarded_trait_method(signature: &str, handle: SharedTraitHandle) -> Option<String> {
    let trimmed = signature.trim();
    let body = trimmed.strip_prefix("fn ")?.strip_suffix(';')?;
    let mutable_receiver = trimmed.contains("&mut self");
    if !mutable_receiver && !trimmed.contains("&self") {
        return None;
    }
    let open = body.find('(')?;
    let name = body[..open].split('<').next()?.trim();
    let close = matching_paren(body, open)?;
    let args = forwarded_argument_names(&body[open + 1..close])?;
    let signature_without_semicolon = trimmed.strip_suffix(';')?;
    if name == "clone_box" && args.is_empty() {
        return Some(format!(
            "    {signature_without_semicolon} {{ std::boxed::Box::new(self.clone()) }}"
        ));
    }
    if name == "as_any" && args.is_empty() {
        let body = match handle {
            SharedTraitHandle::Direct => "(**self).as_any()",
            SharedTraitHandle::Local | SharedTraitHandle::Sync => "self",
        };
        return Some(format!("    {signature_without_semicolon} {{ {body} }}"));
    }
    if (signature_return_is_borrowed(signature) && handle != SharedTraitHandle::Direct)
        || (mutable_receiver && handle == SharedTraitHandle::Direct)
    {
        return None;
    }
    let receiver = match handle {
        SharedTraitHandle::Direct => "(**self)".to_string(),
        SharedTraitHandle::Local if mutable_receiver => "self.borrow_mut()".to_string(),
        SharedTraitHandle::Local => "self.borrow()".to_string(),
        SharedTraitHandle::Sync if mutable_receiver => "self.lock().unwrap()".to_string(),
        SharedTraitHandle::Sync => "self.lock().unwrap()".to_string(),
    };
    Some(format!(
        "    {signature_without_semicolon} {{ {receiver}.{name}({}) }}",
        args.join(", ")
    ))
}

fn signature_return_is_borrowed(signature: &str) -> bool {
    signature
        .split_once(" -> ")
        .is_some_and(|(_, return_and_bounds)| return_and_bounds.trim_start().starts_with('&'))
}

fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in text.char_indices().skip(open) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parameter names after the receiver, splitting on commas that are not inside
/// a generic argument list or a nested parameter list.
fn forwarded_argument_names(params: &str) -> Option<Vec<String>> {
    let mut names = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in params.chars() {
        match ch {
            '<' | '(' | '[' => {
                depth += 1;
                current.push(ch);
            }
            '>' | ')' | ']' => {
                depth = depth.checked_sub(1)?;
                current.push(ch);
            }
            ',' if depth == 0 => {
                names.push(std::mem::take(&mut current));
            }
            _ => current.push(ch),
        }
    }
    names.push(current);
    Some(
        names
            .into_iter()
            .filter_map(|param| {
                let param = param.trim();
                if param.is_empty() || param.ends_with("self") {
                    return None;
                }
                Some(param.split(':').next()?.trim().to_string())
            })
            .collect(),
    )
}

pub(crate) fn dyn_trait_anonymous_impl(trait_name: &str) -> String {
    format!(
        "impl dyn {trait_name} {{\n    pub fn anonymous() -> std::boxed::Box<dyn {trait_name}> {{ panic!(\"java2rust: anonymous class body for {trait_name} requires object lowering\") }}\n}}"
    )
}

fn trait_impl_method(
    ty: &TypeDecl,
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    program: &Program,
    trait_receiver: Option<ReceiverPlan>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    interface_exceptions: MethodExceptionPlan,
) -> Option<String> {
    let trait_return_shape =
        trait_method_return_shape_plan(interface, interface_method, program, signatures);
    let trait_return_shape_for_body = trait_return_shape.clone();
    let signature = trait_method_signature_with_receiver_and_return_shape(
        interface,
        interface_method,
        signatures,
        facts,
        interface_exceptions,
        trait_receiver,
        trait_return_shape,
        program,
    );
    let candidates = implementing_method_candidates_in_hierarchy(ty, interface_method, program);
    if candidates.is_empty() {
        if interface_method.body.is_some() {
            return None;
        }
        if let Some(body) = external_template_method_trait_impl_body(ty, interface_method) {
            return Some(format!("{} {{ {body} }}", signature.trim_end_matches(';')));
        }
        return Some(
            trait_impl_method_message_policy(
                ty,
                interface_method,
                TraitImplMethodFallbackPolicy::MissingImplementation,
            )
            .compile_error_body(&signature),
        );
    };
    let compatible = candidates.iter().copied().find(|(_, candidate)| {
        trait_impl_method_compatibility_error(interface_method, candidate).is_none()
    });
    // A `default` method that nothing overrides keeps the interface's body --
    // that is what `default` means. Candidates are gathered by *name*, so an
    // unrelated overload (`f(String)`) is offered for `f(int)`, and taking
    // `candidates[0]` regardless then reported the two as incompatible and
    // emitted `panic!("incompatible Java interface method implementation")`
    // beside the method that does exist. That compiled with no diagnostic and
    // aborted at run time through the trait object, while a call on the concrete
    // type was fine. The no-candidates branch above already defers to the
    // default; this is the same case reached with a namesake in the way.
    // See claude_audits/0065.
    if compatible.is_none() && interface_method.body.is_some() {
        return None;
    }
    let (implementing_owner, implementing_method) = compatible.unwrap_or(candidates[0]);
    if let Some(message) =
        trait_impl_method_compatibility_error(interface_method, implementing_method)
    {
        return Some(
            trait_impl_method_message_policy(
                ty,
                interface_method,
                TraitImplMethodFallbackPolicy::IncompatibleImplementation { error: message },
            )
            .compile_error_body(&signature),
        );
    }
    if let Some(body) = enum_trait_impl_method_body(ty, interface_method) {
        return Some(format!("{} {{ {body} }}", signature.trim_end_matches(';')));
    }
    let rust_name = implementing_method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&implementing_method.name));
    let interface_signature = signatures.required_method_signature(interface, interface_method);
    let forwarding_owner = trait_impl_forwarding_owner_for_inherited_source_method(
        ty,
        implementing_owner,
        implementing_method,
    );
    let implementation_signature = signatures
        .method_signature(forwarding_owner, implementing_method)
        .unwrap_or_else(|| {
            signatures.required_method_signature(implementing_owner, implementing_method)
        });
    let args = interface_signature
        .params()
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let implementation_shape = implementation_signature
                .params()
                .get(index)
                .map(|param| param.shape());
            trait_impl_forwarded_arg(param.name(), param.shape(), implementation_shape)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let receiver_arg = if interface_method.modifiers.contains(&Modifier::Static) {
        String::new()
    } else if args.is_empty() {
        trait_impl_forwarding_receiver_arg(ty, forwarding_owner, trait_receiver)
    } else {
        format!(
            "{}, {args}",
            trait_impl_forwarding_receiver_arg(ty, forwarding_owner, trait_receiver)
        )
    };
    let implementing_owner_name = rust_name_for_java_named_type(&forwarding_owner.name);
    let call = format!("{implementing_owner_name}::{rust_name}({receiver_arg})");
    let call = adapt_trait_impl_forwarded_return(
        call,
        trait_return_shape_for_body
            .as_ref()
            .or(interface_signature.return_shape()),
        &implementation_signature.return_shape_cloned(),
        interface_exceptions,
    );
    let body = if interface_exceptions.wrap_returns_in_result()
        || signatures
            .required_method_signature(interface, interface_method)
            .return_shape()
            .is_some()
    {
        call
    } else {
        format!("{call};")
    };
    Some(format!("{} {{ {body} }}", signature.trim_end_matches(';')))
}

fn adapt_trait_impl_forwarded_return(
    call: String,
    trait_return_shape: Option<&RustSlotShape>,
    implementation_return_shape: &Option<RustSlotShape>,
    interface_exceptions: MethodExceptionPlan,
) -> String {
    if interface_exceptions.wrap_returns_in_result() {
        return call;
    }
    let Some(trait_shape) = trait_return_shape else {
        return call;
    };
    if trait_shape.optional_element_type().is_some() {
        return call;
    }
    if let Some(implementation_shape) = implementation_return_shape
        && implementation_shape
            .optional_element_type()
            .is_some_and(|element_ty| element_ty == trait_shape.java_ty)
    {
        return format!("{call}.expect(\"Java null receiver\")");
    }
    call
}

fn trait_impl_forwarding_owner_for_inherited_source_method<'a>(
    ty: &'a TypeDecl,
    implementing_owner: &'a TypeDecl,
    implementing_method: &MethodDecl,
) -> &'a TypeDecl {
    if implementing_owner.name != ty.name && implementing_method.body.is_some() {
        ty
    } else {
        implementing_owner
    }
}

fn trait_impl_forwarding_receiver_arg(
    ty: &TypeDecl,
    implementing_owner: &TypeDecl,
    trait_receiver: Option<ReceiverPlan>,
) -> String {
    if implementing_owner.name == ty.name {
        return "self".to_string();
    }
    let owner_name = rust_name_for_java_named_type(&implementing_owner.name);
    match trait_receiver.unwrap_or(ReceiverPlan::Shared) {
        ReceiverPlan::Mutable => {
            format!(
                "&mut {{ let mut __java_super: {owner_name} = self.clone().into(); __java_super }}"
            )
        }
        ReceiverPlan::Shared | ReceiverPlan::None => {
            format!("&{{ let __java_super: {owner_name} = self.clone().into(); __java_super }}")
        }
    }
}

fn trait_impl_forwarded_arg(
    arg_name: &str,
    trait_shape: &RustSlotShape,
    implementation_shape: Option<&RustSlotShape>,
) -> String {
    if implementation_shape.is_some_and(|shape| {
        trait_shape.ownership == OwnershipShape::SharedBorrow
            && trait_shape.view == ViewShape::Slice
            && shape.ownership == OwnershipShape::Owned
            && shape.view == ViewShape::Native
            && shape.java_ty == trait_shape.java_ty
    }) {
        return format!("{arg_name}.to_vec()");
    }
    if implementation_shape.is_some_and(|shape| {
        trait_shape.ownership == OwnershipShape::SharedBorrow
            && trait_shape.view == ViewShape::Str
            && shape.ownership == OwnershipShape::Owned
            && shape.view == ViewShape::Native
            && shape.java_ty == trait_shape.java_ty
    }) {
        return format!("{arg_name}.to_string()");
    }
    arg_name.to_string()
}

fn enum_trait_impl_method_body(ty: &TypeDecl, interface_method: &MethodDecl) -> Option<String> {
    if ty.kind != TypeKind::Enum
        || !interface_method.params.is_empty()
        || interface_method.return_type != crate::java_ir::JavaType::String
    {
        return None;
    }
    Some("self.to_string()".to_string())
}

fn external_template_method_trait_impl_body(
    ty: &TypeDecl,
    interface_method: &MethodDecl,
) -> Option<String> {
    let super_type = ty.extends.as_deref()?;
    let hook = crate::external_packs::external_template_method_hook_for_super_method(
        super_type,
        &interface_method.name,
        interface_method.params.len(),
    )?;
    let hook_method = ty.members.iter().find_map(|member| match member {
        Member::Method(method)
            if method
                .rust_name
                .as_deref()
                .unwrap_or_else(|| method.name.as_str())
                == hook.hook_method
                || crate::parser::to_snake_case(&method.name) == hook.hook_method =>
        {
            Some(method)
        }
        _ => None,
    })?;
    if hook_method.params.len() != interface_method.params.len() {
        return None;
    }
    let args = interface_method
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let call_args = if args.is_empty() {
        "self".to_string()
    } else {
        format!("self, {args}")
    };
    Some(format!("{}::{}({call_args});", ty.name, hook.hook_method))
}

fn trait_impl_method_compatibility_error(
    interface_method: &MethodDecl,
    implementing_method: &MethodDecl,
) -> Option<TraitImplMethodCompatibilityError> {
    if implementing_method.return_type != interface_method.return_type {
        return Some(TraitImplMethodCompatibilityError::ReturnTypeMismatch);
    }
    if implementing_method.params.len() != interface_method.params.len() {
        return Some(TraitImplMethodCompatibilityError::ParameterCountMismatch);
    }
    if implementing_method
        .params
        .iter()
        .zip(interface_method.params.iter())
        .any(|(actual, expected)| {
            actual.ty != expected.ty || actual.is_varargs != expected.is_varargs
        })
    {
        return Some(TraitImplMethodCompatibilityError::ParameterTypeMismatch);
    }
    None
}

pub(crate) fn trait_impl_method_message_policy(
    ty: &TypeDecl,
    interface_method: &MethodDecl,
    policy: TraitImplMethodFallbackPolicy,
) -> TraitImplMethodMessagePolicy {
    let message = match policy {
        TraitImplMethodFallbackPolicy::MissingImplementation => {
            format!(
                "missing Java interface method implementation: {}.{}",
                ty.name, interface_method.name
            )
        }
        TraitImplMethodFallbackPolicy::IncompatibleImplementation { error } => {
            let reason = match error {
                TraitImplMethodCompatibilityError::ReturnTypeMismatch => {
                    "return type does not match interface"
                }
                TraitImplMethodCompatibilityError::ParameterCountMismatch => {
                    "parameter count does not match interface"
                }
                TraitImplMethodCompatibilityError::ParameterTypeMismatch => {
                    "parameter types do not match interface"
                }
            };
            format!(
                "incompatible Java interface method implementation: {}.{} {reason}",
                ty.name, interface_method.name
            )
        }
    };
    TraitImplMethodMessagePolicy { message }
}

fn trait_method_signature(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
) -> String {
    trait_method_signature_with_receiver(
        ty,
        method,
        signatures,
        facts,
        exceptions,
        None,
        &Program::default(),
    )
}

fn trait_method_signature_with_receiver(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    receiver_override: Option<ReceiverPlan>,
    program: &Program,
) -> String {
    trait_method_signature_with_receiver_and_return_shape(
        ty,
        method,
        signatures,
        facts,
        exceptions,
        receiver_override,
        None,
        program,
    )
}

pub(crate) fn trait_method_signature_with_receiver_and_return_shape(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    receiver_override: Option<ReceiverPlan>,
    return_shape_override: Option<RustSlotShape>,
    program: &Program,
) -> String {
    let signature = signatures.required_method_signature(ty, method);
    let mut params = Vec::new();
    if !method.modifiers.contains(&Modifier::Static) {
        params.push(match receiver_override.unwrap_or(signature.receiver()) {
            ReceiverPlan::Mutable => "&mut self".to_string(),
            ReceiverPlan::Shared | ReceiverPlan::None => "&self".to_string(),
        });
    }
    let param_shapes = trait_method_param_shape_plan(ty, method, program, signatures);
    params.extend(signature.params().iter().enumerate().map(|(index, param)| {
        let shape = param_shapes.get(index).unwrap_or(&param.shape());
        format!(
            "{}: {}",
            param.name(),
            emit_type(&rust_type_for_slot_with_facts(shape, facts))
        )
    }));
    let return_type = trait_method_return_type_with_exception_plan(
        method,
        signatures,
        ty,
        facts,
        exceptions,
        return_shape_override,
    )
    .map(|ty| format!(" -> {}", emit_type(&ty)))
    .unwrap_or_default();
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    let type_params = rust_method_type_params(method, program, &ty.name);
    let where_clause = rust_generic_trait_method_where_clause(method);
    format!(
        "    fn {rust_name}{type_params}({}){return_type}{where_clause};",
        params.join(", ")
    )
}

fn rust_method_type_params(method: &MethodDecl, program: &Program, owner_name: &str) -> String {
    if method.type_params.is_empty() {
        return String::new();
    }
    let params = method
        .type_params
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let Some(Some(JavaType::Named { name: bound, .. })) =
                method.type_param_bounds.get(index)
            else {
                return name.clone();
            };
            match find_type_decl_for_owner_context(program, owner_name, bound) {
                Some(ty) if matches!(ty.kind, TypeKind::Interface) => {
                    format!("{name}: {}", rust_name_for_java_named_type(&ty.name))
                }
                Some(ty) if matches!(ty.kind, TypeKind::Class) => {
                    format!("{name}: {}", source_class_bound_trait_name(&ty.name))
                }
                _ => name.clone(),
            }
        })
        .collect::<Vec<_>>();
    format!("<{}>", params.join(", "))
}

fn rust_generic_trait_method_where_clause(method: &MethodDecl) -> &'static str {
    if method.type_params.is_empty() {
        ""
    } else {
        " where Self: Sized"
    }
}

pub(crate) fn trait_method_return_shape_plan(
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    program: &Program,
    signatures: &SignaturePlan,
) -> Option<RustSlotShape> {
    let interface_shape = signatures
        .required_method_signature(interface, interface_method)
        .return_shape()
        .cloned()?;
    for implementor in program
        .types
        .iter()
        .filter(|ty| type_implements_interface(program, ty, interface))
    {
        let Some((implementation_owner, implementation)) =
            implementing_method_candidates_in_hierarchy(implementor, interface_method, program)
                .into_iter()
                .find(|candidate| {
                    trait_impl_method_compatibility_error(interface_method, candidate.1).is_none()
                })
        else {
            continue;
        };
        let Some(implementation_shape) = signatures
            .method_signature(implementation_owner, implementation)
            .unwrap_or_else(|| {
                signatures.required_method_signature(implementation_owner, implementation)
            })
            .return_shape()
            .cloned()
        else {
            continue;
        };
        if trait_return_shape_should_widen_to_implementation(
            &interface_shape,
            &implementation_shape,
        ) {
            return Some(implementation_shape);
        }
    }
    Some(interface_shape)
}

pub(crate) fn trait_method_param_shape_plan(
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    program: &Program,
    signatures: &SignaturePlan,
) -> Vec<RustSlotShape> {
    let interface_signature = signatures.required_method_signature(interface, interface_method);
    let mut param_shapes = interface_signature
        .params()
        .iter()
        .map(|param| param.shape().clone())
        .collect::<Vec<_>>();
    if interface.kind != TypeKind::Interface {
        return param_shapes;
    }
    for implementor in program
        .types
        .iter()
        .filter(|ty| type_implements_interface(program, ty, interface))
    {
        let Some((implementation_owner, implementation)) =
            implementing_method_candidates_in_hierarchy(implementor, interface_method, program)
                .into_iter()
                .find(|candidate| {
                    trait_impl_method_compatibility_error(interface_method, candidate.1).is_none()
                })
        else {
            continue;
        };
        let implementation_signature = signatures
            .method_signature(implementation_owner, implementation)
            .unwrap_or_else(|| {
                signatures.required_method_signature(implementation_owner, implementation)
            });
        for (index, implementation_param) in implementation_signature.params().iter().enumerate() {
            let Some(interface_shape) = param_shapes.get_mut(index) else {
                continue;
            };
            if trait_param_shape_should_widen_to_implementation(
                interface_shape,
                &implementation_param.shape(),
            ) {
                *interface_shape = implementation_param.shape().clone();
            } else if let Some(declared_param) = implementation.params.get(index)
                && trait_param_shape_should_widen_to_declared_source_param(
                    interface_shape,
                    declared_param,
                )
            {
                *interface_shape = RustSlotShape::for_storage(declared_param.ty.clone());
            }
        }
    }
    param_shapes
}

fn trait_param_shape_should_widen_to_implementation(
    interface_shape: &RustSlotShape,
    implementation_shape: &RustSlotShape,
) -> bool {
    if interface_shape.ownership != OwnershipShape::SharedBorrow
        || interface_shape.java_ty != implementation_shape.java_ty
    {
        return false;
    }
    if matches!(interface_shape.java_ty, JavaType::Array(_)) {
        return false;
    }
    if implementation_shape.ownership == OwnershipShape::Owned {
        return true;
    }
    implementation_shape.ownership == OwnershipShape::SharedBorrow
        && implementation_shape.view == ViewShape::DynTraitView
        && matches!(
            implementation_shape.dispatch,
            DispatchShape::DynTrait { .. } | DispatchShape::BoxedDynTrait { .. }
        )
}

fn trait_param_shape_should_widen_to_declared_source_param(
    interface_shape: &RustSlotShape,
    implementation_param: &crate::java_ir::Param,
) -> bool {
    interface_shape.ownership == OwnershipShape::SharedBorrow
        && interface_shape.java_ty == implementation_param.ty
        && !matches!(
            implementation_param.ty,
            JavaType::String | JavaType::Array(_) | JavaType::Primitive(_) | JavaType::Void
        )
        && crate::stdlib::java_collection_kind_for_type(&implementation_param.ty).is_none()
}

fn trait_return_shape_should_widen_to_implementation(
    interface_shape: &RustSlotShape,
    implementation_shape: &RustSlotShape,
) -> bool {
    if implementation_shape
        .optional_element_type()
        .is_some_and(|element_ty| element_ty == interface_shape.java_ty)
    {
        return true;
    }
    interface_shape.ownership == OwnershipShape::SharedBorrow
        && implementation_shape.ownership == OwnershipShape::Owned
        && interface_shape.java_ty == implementation_shape.java_ty
}

pub(crate) fn trait_method_receiver_plan(
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    program: &Program,
    signatures: &SignaturePlan,
) -> Option<ReceiverPlan> {
    if interface_method.modifiers.contains(&Modifier::Static) {
        return None;
    }
    let interface_receiver = signatures
        .required_method_signature(interface, interface_method)
        .receiver();
    let mut receiver = interface_receiver;
    for implementor in program
        .types
        .iter()
        .filter(|ty| type_implements_interface(program, ty, interface))
    {
        let Some((implementation_owner, implementation)) =
            implementing_method_candidates_in_hierarchy(implementor, interface_method, program)
                .into_iter()
                .find(|candidate| {
                    trait_impl_method_compatibility_error(interface_method, candidate.1).is_none()
                })
        else {
            continue;
        };
        let implementation_receiver = signatures
            .method_signature(implementation_owner, implementation)
            .unwrap_or_else(|| {
                signatures.required_method_signature(implementation_owner, implementation)
            })
            .receiver();
        if implementation_receiver == ReceiverPlan::Mutable {
            receiver = ReceiverPlan::Mutable;
        }
    }
    Some(receiver)
}

/// The trait declaration's exception plan.
///
/// This used to join over every implementor with `|=`, re-doing the override
/// equalization the fallibility fixpoint already performs — a second store, and
/// the one 0010 is named for. That workaround was load-bearing only because
/// `override_groups_for_program` keyed its groups on the *Java* method name
/// while the plan is keyed on the Rust name, so the fixpoint's equalization
/// never matched. With the keys agreeing, the solved plan is authoritative and
/// this is a plain lookup.
pub(crate) fn trait_method_exception_plan(
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    program: &Program,
    exceptions: &ProgramExceptionPlan,
) -> MethodExceptionPlan {
    let _ = program;
    exceptions.method_plan(interface, interface_method)
}

fn trait_method_return_type_with_exception_plan(
    method: &MethodDecl,
    signatures: &SignaturePlan,
    ty: &TypeDecl,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    return_shape_override: Option<RustSlotShape>,
) -> Option<RustType> {
    let return_shape = return_shape_override.or_else(|| {
        signatures
            .required_method_signature(ty, method)
            .return_shape()
            .cloned()
    });
    let rust_return = return_shape
        .as_ref()
        .map(|shape| rust_type_for_slot_with_facts(shape, facts));
    if exceptions.wrap_returns_in_result() {
        Some(exception_result_type(rust_return.unwrap_or(RustType::Unit)))
    } else {
        rust_return
    }
}

fn dyn_trait_name_for(
    representations: &RepresentationPlan,
    interface_name: &str,
) -> Option<String> {
    match &representations
        .type_representation(interface_name)?
        .representation()
    {
        ObjectRepresentation::DynTrait { trait_name } => Some(trait_name.clone()),
        _ => None,
    }
}

fn implementing_method_candidates<'a>(
    ty: &'a TypeDecl,
    interface_method: &MethodDecl,
) -> Vec<&'a MethodDecl> {
    let candidates = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if method.name == interface_method.name => Some(method),
            _ => None,
        })
        .collect();
    order_candidates_by_signature_identity(candidates, interface_method)
}

/// See the note on the `lower.rs` twin: candidates stay name-collected so a
/// differently-spelled parameter type cannot lose a real implementation, but the
/// exact signature match is tried first so an inherited overload cannot displace
/// an override ([0009](../claude_audits/done/0009-overload-identity-collapses-onto-rust-name.md)).
fn order_candidates_by_signature_identity<'a>(
    mut candidates: Vec<&'a MethodDecl>,
    interface_method: &MethodDecl,
) -> Vec<&'a MethodDecl> {
    let wanted = crate::java_signature::JavaCallableSlotKey::source_method(interface_method);
    candidates.sort_by_key(|candidate| {
        crate::java_signature::JavaCallableSlotKey::source_method(candidate) != wanted
    });
    candidates
}

fn implementing_method_candidates_in_hierarchy<'a>(
    ty: &'a TypeDecl,
    interface_method: &MethodDecl,
    program: &'a Program,
) -> Vec<(&'a TypeDecl, &'a MethodDecl)> {
    let direct_candidates = implementing_method_candidates(ty, interface_method);
    let mut seen_slots = direct_candidates
        .iter()
        .map(|method| crate::java_signature::JavaCallableSlotKey::source_method(method))
        .collect::<HashSet<_>>();
    let mut out = direct_candidates
        .into_iter()
        .map(|method| (ty, method))
        .collect::<Vec<_>>();
    if let Some(super_name) = ty.extends.as_deref()
        && let Some(super_ty) = find_type_decl_for_owner_context(program, &ty.name, super_name)
    {
        out.extend(
            implementing_method_candidates_in_hierarchy(super_ty, interface_method, program)
                .into_iter()
                .filter(|(_, method)| {
                    seen_slots.insert(crate::java_signature::JavaCallableSlotKey::source_method(
                        method,
                    ))
                }),
        );
    }
    out
}

fn type_implements_interface(program: &Program, ty: &TypeDecl, interface: &TypeDecl) -> bool {
    ty.kind == TypeKind::Class
        && implemented_interfaces_for_type(program, ty)
            .iter()
            .any(|implemented| implemented == &interface.name)
}

fn source_class_bounds_for_type<'a>(program: &'a Program, ty: &TypeDecl) -> Vec<&'a TypeDecl> {
    source_type_param_bounds(program)
        .into_iter()
        .filter_map(|bound| {
            find_type_decl_for_owner_context(program, &bound.owner_name, &bound.bound_name)
        })
        .filter(|bound| {
            matches!(bound.kind, TypeKind::Class)
                && (bound.name == ty.name || source_type_extends(program, ty, &bound.name))
        })
        .collect()
}

fn source_class_is_used_as_type_param_bound(program: &Program, type_name: &str) -> bool {
    source_type_param_bounds(program).into_iter().any(|bound| {
        find_type_decl_for_owner_context(program, &bound.owner_name, &bound.bound_name)
            .is_some_and(|ty| ty.name == type_name)
            || bound.bound_name == type_name
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SourceTypeParamBound {
    owner_name: String,
    bound_name: String,
}

fn source_type_param_bounds(program: &Program) -> Vec<SourceTypeParamBound> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ty in &program.types {
        collect_source_type_param_bounds_in_type(ty, &mut seen, &mut out);
    }
    out
}

fn collect_source_type_param_bounds_in_type(
    ty: &TypeDecl,
    seen: &mut HashSet<SourceTypeParamBound>,
    out: &mut Vec<SourceTypeParamBound>,
) {
    collect_bound_names(&ty.name, &ty.type_param_bounds, seen, out);
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                collect_bound_names(&ty.name, &method.type_param_bounds, seen, out)
            }
            Member::Constructor(constructor) => {
                collect_bound_names(&ty.name, &constructor.type_param_bounds, seen, out)
            }
            Member::Type(nested) => collect_source_type_param_bounds_in_type(nested, seen, out),
            _ => {}
        }
    }
}

fn collect_bound_names(
    owner_name: &str,
    bounds: &[Option<JavaType>],
    seen: &mut HashSet<SourceTypeParamBound>,
    out: &mut Vec<SourceTypeParamBound>,
) {
    for bound in bounds {
        if let Some(JavaType::Named { name, .. }) = bound {
            let bound = SourceTypeParamBound {
                owner_name: owner_name.to_string(),
                bound_name: name.clone(),
            };
            if seen.insert(bound.clone()) {
                out.push(bound);
            }
        }
    }
}

fn source_type_extends(program: &Program, ty: &TypeDecl, target_name: &str) -> bool {
    let mut next = ty.extends.as_deref();
    let mut owner_name = ty.name.as_str();
    while let Some(name) = next {
        if name == target_name {
            return true;
        }
        let Some(super_ty) = find_type_decl_for_owner_context(program, owner_name, name) else {
            return false;
        };
        if super_ty.name == target_name {
            return true;
        }
        owner_name = &super_ty.name;
        next = super_ty.extends.as_deref();
    }
    false
}

fn find_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    let mut matches = TypeDeclMatches::default();
    for ty in &program.types {
        collect_type_decl_matches(ty, type_name, &mut matches);
    }
    unique_type_decl(matches.exact)
        .or_else(|| unique_type_decl(matches.emitted_alias))
        .or_else(|| unique_type_decl(matches.simple))
}

fn find_type_decl_for_owner_context<'a>(
    program: &'a Program,
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    let mut matches = TypeDeclMatches::default();
    for ty in &program.types {
        collect_type_decl_matches(ty, type_name, &mut matches);
    }
    unique_type_decl(matches.exact)
        .or_else(|| unique_same_package_type_decl(matches.simple.as_slice(), owner_name))
        .or_else(|| unique_type_decl(matches.emitted_alias))
        .or_else(|| unique_type_decl(matches.simple))
}

#[derive(Default)]
struct TypeDeclMatches<'a> {
    exact: Vec<&'a TypeDecl>,
    emitted_alias: Vec<&'a TypeDecl>,
    simple: Vec<&'a TypeDecl>,
}

fn unique_type_decl(matches: Vec<&TypeDecl>) -> Option<&TypeDecl> {
    match matches.as_slice() {
        [ty] => Some(*ty),
        _ => None,
    }
}

fn unique_same_package_type_decl<'a>(
    matches: &[&'a TypeDecl],
    owner_name: &str,
) -> Option<&'a TypeDecl> {
    let owner_package = java_owner_package_prefix(owner_name)?;
    let mut same_package = matches
        .iter()
        .copied()
        .filter(|ty| java_owner_package_prefix(&ty.name) == Some(owner_package))
        .collect::<Vec<_>>();
    same_package.dedup_by(|left, right| left.name == right.name);
    unique_type_decl(same_package)
}

fn java_owner_package_prefix(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(package, _)| package)
}

fn collect_type_decl_matches<'a>(
    ty: &'a TypeDecl,
    type_name: &str,
    matches: &mut TypeDeclMatches<'a>,
) {
    let simple_name = ty.name.rsplit('.').next().unwrap_or(&ty.name);
    if ty.name == type_name {
        matches.exact.push(ty);
    } else if rust_name_for_java_named_type(&ty.name) == type_name {
        matches.emitted_alias.push(ty);
    } else if simple_name == type_name || rust_name_for_java_named_type(simple_name) == type_name {
        matches.simple.push(ty);
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_decl_matches(nested, type_name, matches);
        }
    }
}

#[cfg(test)]
mod type_emission_tests {
    use super::{
        RepresentationTypeEmission, TraitImplMethodCompatibilityError,
        TraitImplMethodFallbackPolicy, TraitImplMethodMessagePolicy, raw_type_item_for_with_facts,
        trait_impl_emissions_for_type_with_facts, trait_impl_method_message_policy,
        type_emission_for_program_with_facts_and_exceptions, type_emission_for_with_facts,
    };
    use crate::exception_plan::ProgramExceptionPlan;
    use crate::facts::{
        FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy, TypeCapabilityOverride,
    };
    use crate::java_ir::{JavaType, Member};
    use crate::parser::parse_java_source;
    use crate::representation::{ObjectRepresentation, plan_representations};
    use crate::resolve::resolve_program;
    use crate::semantic_analysis::analyze_program;
    use crate::signature::solve_signatures_with_facts;
    use crate::usage::{analyze_usage, analyze_usage_with_facts};

    #[test]
    fn raw_type_item_bridge_is_test_only_projection_of_type_emission() {
        let parsed = parse_java_source(
            r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = FactSet::default();
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: "Expr".to_string(),
            strategy: HierarchyStrategy::ClosedEnum {
                enum_name: "ExprValue".to_string(),
                variants: vec!["Add".to_string(), "Num".to_string()],
            },
            provenance: FactProvenance::UserOverride,
        });

        let representations = plan_representations(&resolved.program, &facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &analyze_usage(&resolved.program),
            &facts,
        );
        let expr_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Expr")
            .unwrap();
        let add_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Add")
            .unwrap();

        let expected =
            "#[derive(Clone, PartialEq)]\npub enum ExprValue {\n    Add(Add),\n    Num(Num),\n}"
                .to_string();
        assert_eq!(
            type_emission_for_with_facts(&representations, expr_ty, &signatures, &facts),
            RepresentationTypeEmission::RawRustItem(expected.clone())
        );
        assert_eq!(
            raw_type_item_for_with_facts(&representations, expr_ty, &signatures, &facts),
            Some(expected)
        );
        assert_eq!(
            raw_type_item_for_with_facts(&representations, add_ty, &signatures, &facts),
            None
        );
    }

    #[test]
    fn representation_lowering_owns_non_struct_type_emission() {
        let parsed = parse_java_source(
            r#"
interface Expr {}
class Add implements Expr {}
class Num implements Expr {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = FactSet::default();
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: "Expr".to_string(),
            strategy: HierarchyStrategy::ClosedEnum {
                enum_name: "ExprValue".to_string(),
                variants: vec!["Add".to_string(), "Num".to_string()],
            },
            provenance: FactProvenance::UserOverride,
        });

        let representations = plan_representations(&resolved.program, &facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &analyze_usage(&resolved.program),
            &facts,
        );
        let expr_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Expr")
            .unwrap();
        let add_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Add")
            .unwrap();

        assert_eq!(
            type_emission_for_with_facts(&representations, expr_ty, &signatures, &FactSet::default()),
            RepresentationTypeEmission::RawRustItem(
                "#[derive(Clone, PartialEq)]\npub enum ExprValue {\n    Add(Add),\n    Num(Num),\n}"
                    .to_string()
            )
        );
        assert_eq!(
            type_emission_for_with_facts(
                &representations,
                add_ty,
                &signatures,
                &FactSet::default()
            ),
            RepresentationTypeEmission::LowerAsConcreteStruct
        );
        let enum_parsed = parse_java_source(
            r#"
class Holder {
    enum Type {
        SNP,
        INDEL
    }
}
"#,
        );
        let enum_resolved = resolve_program(&enum_parsed.program, enum_parsed.diagnostics);
        let enum_representations =
            plan_representations(&enum_resolved.program, &FactSet::default());
        let enum_signatures = solve_signatures_with_facts(
            &enum_resolved.program,
            &enum_representations,
            &analyze_usage(&enum_resolved.program),
            &FactSet::default(),
        );
        let holder_ty = enum_resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Holder")
            .unwrap();
        let enum_ty = holder_ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Type(ty) if ty.name.ends_with(".Type") => Some(ty),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            crate::object_model::rust_enum_variant_name_from_parsed_constant("s_n_p"),
            "SNP"
        );
        assert!(matches!(
            enum_representations
                .type_representation("Holder.Type")
                .map(|representation| representation.representation()),
            Some(ObjectRepresentation::JavaEnum { enum_name, variants })
                if enum_name == "Holder.Type" && variants == &vec!["SNP".to_string(), "INDEL".to_string()]
        ));
        let RepresentationTypeEmission::RawRustItem(enum_item) = type_emission_for_with_facts(
            &enum_representations,
            enum_ty,
            &enum_signatures,
            &FactSet::default(),
        ) else {
            panic!("Java enum should emit a raw Rust enum item");
        };
        assert!(enum_item.contains("pub enum Holder_Type"));
        assert!(enum_item.contains("SNP"));
        assert!(enum_item.contains("INDEL"));
        assert!(enum_item.contains("impl Default for Holder_Type"));
        assert!(enum_item.contains("pub fn values() -> Vec<Self>"));
        assert!(enum_item.contains("pub fn value_of<T: AsRef<str>>(name: T) -> Self"));
        assert!(enum_item.contains("pub fn ordinal(&self) -> i32"));
        assert!(enum_item.contains("impl std::fmt::Display for Holder_Type"));
    }

    #[test]
    fn representation_lowering_emits_dyn_trait_method_signatures_from_signature_plan() {
        let parsed = parse_java_source(
            r#"
interface Named {
    String name();
    int score(String prefix);
}

"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations = plan_representations(&resolved.program, &semantic.facts);
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let named_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Named")
            .unwrap();

        let RepresentationTypeEmission::RawRustItem(item) = type_emission_for_with_facts(
            &representations,
            named_ty,
            &signatures,
            &FactSet::default(),
        ) else {
            panic!("interface should emit a raw trait item");
        };
        assert!(item.contains("pub trait Named"));
        assert!(item.contains("fn name(&self) -> String;"));
        assert!(
            item.contains("fn score(&self, prefix: String) -> i32;"),
            "{item}"
        );
        assert!(item.contains("impl Clone for std::boxed::Box<dyn Named>"));
        assert!(
            item.contains(
                "impl<__JavaShared: Named + 'static> Named for std::rc::Rc<__JavaShared>"
            )
        );
        assert!(item.contains(
            "impl<__JavaShared: Named + 'static> Named for std::sync::Arc<__JavaShared>"
        ));
    }

    #[test]
    fn representation_lowering_renders_trait_signatures_with_facts() {
        let parsed = parse_java_source(
            r#"
interface IdSource {
    ExternalIds ids(ExternalIds values);
}
class ExternalIds {}
class ExternalId {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = resolved.facts.clone();
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: "ExternalIds".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: Some(JavaType::Named {
                    name: "ExternalId".to_string(),
                    args: Vec::new(),
                }),
                implements: Vec::new(),
                extends: None,
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::UserOverride,
            });
        let representations = plan_representations(&resolved.program, &facts);
        let usage = analyze_usage(&resolved.program);
        let signatures =
            solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
        let source_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "IdSource")
            .unwrap();

        assert_eq!(
            type_emission_for_with_facts(&representations, source_ty, &signatures, &facts),
            RepresentationTypeEmission::RawRustItem(
                "pub trait IdSource {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId>;\n    fn as_any(&self) -> &dyn std::any::Any;\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource>;\n}\n\nimpl dyn IdSource {\n    pub fn anonymous() -> std::boxed::Box<dyn IdSource> { panic!(\"java2rust: anonymous class body for IdSource requires object lowering\") }\n}\n\nimpl Clone for std::boxed::Box<dyn IdSource> {\n    fn clone(&self) -> Self { IdSource::clone_box(&**self) }\n}\n\nimpl<__JavaShared: IdSource + 'static> IdSource for std::rc::Rc<__JavaShared> {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { (**self).ids(values) }\n    fn as_any(&self) -> &dyn std::any::Any { (**self).as_any() }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { std::boxed::Box::new(self.clone()) }\n}\n\nimpl<__JavaShared: IdSource + 'static> IdSource for std::sync::Arc<__JavaShared> {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { (**self).ids(values) }\n    fn as_any(&self) -> &dyn std::any::Any { (**self).as_any() }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { std::boxed::Box::new(self.clone()) }\n}\n\nimpl<__JavaShared: IdSource + 'static> IdSource for std::rc::Rc<std::cell::RefCell<__JavaShared>> {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { self.borrow().ids(values) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { std::boxed::Box::new(self.clone()) }\n}\n\nimpl<__JavaShared: IdSource + 'static> IdSource for std::sync::Arc<std::sync::Mutex<__JavaShared>> {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { self.lock().unwrap().ids(values) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { std::boxed::Box::new(self.clone()) }\n}"
                    .to_string()
            )
        );
    }

    #[test]
    fn representation_lowering_emits_dyn_trait_impls_for_implementing_classes() {
        let parsed = parse_java_source(
            r#"
interface Named {
    String name();
}

class Dog implements Named {
    public String name() { return "dog"; }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations = plan_representations(&resolved.program, &semantic.facts);
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let dog_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Dog")
            .unwrap();

        assert_eq!(
            trait_impl_emissions_for_type_with_facts(
                &representations,
                &resolved.program,
                dog_ty,
                &signatures,
                &semantic.facts,
            ),
            vec![
                "impl Named for Dog {\n    fn name(&self) -> String { Dog::name(self) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn Named> { panic!(\"java2rust: clone_box for Named requires a cloneable Java object carrier\") }\n}".to_string()
            ]
        );
    }

    #[test]
    fn trait_impl_forwarding_keeps_nullable_implementation_return_shape() {
        let parsed = parse_java_source(
            r#"
interface Named {
    String name();
}

class NullName implements Named {
    public String name() { return null; }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations = plan_representations(&resolved.program, &semantic.facts);
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let null_name = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "NullName")
            .unwrap();

        let impls = trait_impl_emissions_for_type_with_facts(
            &representations,
            &resolved.program,
            null_name,
            &signatures,
            &semantic.facts,
        );

        assert_eq!(impls.len(), 1);
        assert!(
            impls[0].contains("fn name(&self) -> Option<String> { NullName::name(self) }"),
            "{}",
            impls[0]
        );
        assert!(
            !impls[0].contains("NullName::name(self).expect(\"Java null receiver\")"),
            "{}",
            impls[0]
        );
    }

    #[test]
    fn representation_lowering_emits_stubbed_trait_impls_for_abstract_classes() {
        let parsed = parse_java_source(
            r#"
interface Module {
    void reset();
    String name();
}

abstract class AbstractModule implements Module {
    public void helper() {}
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let representations = plan_representations(&resolved.program, &resolved.facts);
        let usage = analyze_usage(&resolved.program);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &resolved.facts,
        );
        let abstract_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "AbstractModule")
            .unwrap();

        let impls = trait_impl_emissions_for_type_with_facts(
            &representations,
            &resolved.program,
            abstract_ty,
            &signatures,
            &FactSet::default(),
        );

        assert_eq!(impls.len(), 1);
        assert!(impls[0].contains("impl Module for AbstractModule"));
        assert!(impls[0].contains(
            "panic!(\"missing Java interface method implementation: AbstractModule.reset\")"
        ));
        assert!(impls[0].contains(
            "panic!(\"missing Java interface method implementation: AbstractModule.name\")"
        ));
        assert!(!impls[0].contains("compile_error!"));
    }

    #[test]
    fn representation_lowering_renders_trait_impls_with_facts() {
        let parsed = parse_java_source(
            r#"
interface IdSource {
    ExternalIds ids(ExternalIds values);
}
class ExternalIds {}
class ExternalId {}
class Source implements IdSource {
    public ExternalIds ids(ExternalIds values) { return values; }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = resolved.facts.clone();
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: "ExternalIds".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: Some(JavaType::Named {
                    name: "ExternalId".to_string(),
                    args: Vec::new(),
                }),
                implements: Vec::new(),
                extends: None,
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::UserOverride,
            });
        let representations = plan_representations(&resolved.program, &facts);
        let usage = analyze_usage(&resolved.program);
        let signatures =
            solve_signatures_with_facts(&resolved.program, &representations, &usage, &facts);
        let source_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Source")
            .unwrap();

        assert_eq!(
            trait_impl_emissions_for_type_with_facts(
                &representations,
                &resolved.program,
                source_ty,
                &signatures,
                &facts,
            ),
            vec![
                "impl IdSource for Source {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { Source::ids(self, values) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { panic!(\"java2rust: clone_box for IdSource requires a cloneable Java object carrier\") }\n}"
                    .to_string()
            ]
        );
    }

    #[test]
    fn representation_lowering_reports_incompatible_dyn_trait_impl_methods() {
        let parsed = parse_java_source(
            r#"
interface Named {
    String name();
    int score(String prefix);
    int arity(String left, String right);
}

class BrokenNamed implements Named {
    public int name() { return 1; }
    public int score(int prefix) { return prefix; }
    public int arity(String left) { return 1; }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let representations = plan_representations(&resolved.program, &resolved.facts);
        let usage = analyze_usage(&resolved.program);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &resolved.facts,
        );
        let broken_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "BrokenNamed")
            .unwrap();
        let named_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Named")
            .unwrap();
        let name_method = named_ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "name" => Some(method),
                _ => None,
            })
            .unwrap();

        let impls = trait_impl_emissions_for_type_with_facts(
            &representations,
            &resolved.program,
            broken_ty,
            &signatures,
            &FactSet::default(),
        );

        let message_policy = trait_impl_method_message_policy(
            broken_ty,
            name_method,
            TraitImplMethodFallbackPolicy::IncompatibleImplementation {
                error: TraitImplMethodCompatibilityError::ReturnTypeMismatch,
            },
        );
        assert_eq!(
            message_policy,
            TraitImplMethodMessagePolicy {
                message:
                    "incompatible Java interface method implementation: BrokenNamed.name return type does not match interface"
                        .to_string(),
            }
        );
        assert_eq!(
            message_policy.compile_error_body("fn name(&self) -> String;"),
            "fn name(&self) -> String { panic!(\"incompatible Java interface method implementation: BrokenNamed.name return type does not match interface\"); }"
        );

        assert_eq!(impls.len(), 1);
        assert!(impls[0].contains("impl Named for BrokenNamed"));
        assert!(
            impls[0].contains(
                "panic!(\"incompatible Java interface method implementation: BrokenNamed.name return type does not match interface\")"
            ),
            "{}",
            impls[0]
        );
        assert!(
            impls[0].contains(
                "panic!(\"incompatible Java interface method implementation: BrokenNamed.score parameter types do not match interface\")"
            ),
            "{}",
            impls[0]
        );
        assert!(
            impls[0].contains(
                "panic!(\"incompatible Java interface method implementation: BrokenNamed.arity parameter count does not match interface\")"
            ),
            "{}",
            impls[0]
        );
    }

    #[test]
    fn representation_lowering_prefers_compatible_dyn_trait_impl_overloads() {
        let parsed = parse_java_source(
            r#"
interface Named {
    int score(String prefix);
}

class OverloadedNamed implements Named {
    public int score(int prefix) { return prefix; }
    public int score(String prefix) { return 1; }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let representations = plan_representations(&resolved.program, &resolved.facts);
        let usage = analyze_usage(&resolved.program);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &resolved.facts,
        );
        let overloaded_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "OverloadedNamed")
            .unwrap();

        let impls = trait_impl_emissions_for_type_with_facts(
            &representations,
            &resolved.program,
            overloaded_ty,
            &signatures,
            &FactSet::default(),
        );

        assert_eq!(impls.len(), 1);
        assert!(impls[0].contains("impl Named for OverloadedNamed"));
        assert!(!impls[0].contains("compile_error!"), "{}", impls[0]);
        assert!(
            impls[0].contains(
                "fn score(&self, prefix: String) -> i32 { OverloadedNamed::score_string(self, prefix) }"
            ),
            "{}",
            impls[0]
        );
    }

    #[test]
    fn erased_object_alias_uses_rust_name_for_nested_java_type() {
        let parsed = parse_java_source(
            r#"
class AliasMin {
    static class A {}
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = FactSet::default();
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: "AliasMin.A".to_string(),
            strategy: HierarchyStrategy::ErasedObject,
            provenance: FactProvenance::UserOverride,
        });

        let representations = plan_representations(&resolved.program, &facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &analyze_usage(&resolved.program),
            &facts,
        );
        let nested_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "AliasMin")
            .and_then(|ty| {
                ty.members.iter().find_map(|member| {
                    if let crate::java_ir::Member::Type(nested) = member {
                        (nested.name == "AliasMin.A").then_some(nested)
                    } else {
                        None
                    }
                })
            })
            .expect("nested type should be parsed with Java owner path");

        assert_eq!(
            raw_type_item_for_with_facts(&representations, nested_ty, &signatures, &facts),
            Some("pub type AliasMin_A = Box<dyn std::any::Any>;".to_string())
        );
    }

    #[test]
    fn program_type_emission_renders_interface_method_type_params() {
        let parsed = parse_java_source(
            r#"
interface Mapper {
    <T> T id(T value);
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations = plan_representations(&resolved.program, &semantic.facts);
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let mapper_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Mapper")
            .unwrap();

        let RepresentationTypeEmission::RawRustItem(rendered) =
            type_emission_for_program_with_facts_and_exceptions(
                &representations,
                &resolved.program,
                mapper_ty,
                &signatures,
                &FactSet::default(),
                &ProgramExceptionPlan::default(),
            )
        else {
            panic!("Mapper must lower to a trait item");
        };
        // `__JavaShared` rather than `T`: `id` declares its own `T` and an
        // inner generic shadowing the implementation's is E0403. The same
        // method must remain callable through all four supported handle forms.
        for expected in [
            "pub trait Mapper {\n    fn id<T>(&self, value: T) -> T where Self: Sized;",
            "Mapper for std::rc::Rc<__JavaShared>",
            "Mapper for std::sync::Arc<__JavaShared>",
            "Mapper for std::rc::Rc<std::cell::RefCell<__JavaShared>>",
            "Mapper for std::sync::Arc<std::sync::Mutex<__JavaShared>>",
            "fn id<T>(&self, value: T) -> T where Self: Sized { self.borrow().id(value) }",
        ] {
            assert!(rendered.contains(expected), "{rendered}");
        }
    }

    #[test]
    fn program_type_emission_renders_interface_method_type_param_bounds() {
        let parsed = parse_java_source(
            r#"
interface Base {
    String name();
}

interface Mapper {
    <T extends Base> String nameOf(T value);
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations = plan_representations(&resolved.program, &semantic.facts);
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let mapper_ty = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Mapper")
            .unwrap();

        let emission = type_emission_for_program_with_facts_and_exceptions(
            &representations,
            &resolved.program,
            mapper_ty,
            &signatures,
            &semantic.facts,
            &ProgramExceptionPlan::default(),
        );

        assert!(
            matches!(
                emission,
                RepresentationTypeEmission::RawRustItem(ref item)
                    if item.contains("fn name_of<T: Base>(&self, value: &T) -> String where Self: Sized;")
            ),
            "{emission:?}"
        );
    }
}

#[cfg(test)]
mod type_lookup_tests {
    use super::{
        find_type_decl, find_type_decl_for_owner_context, implemented_interfaces_for_type,
        rust_method_type_params, source_type_extends,
    };
    use crate::diagnostics::Span;
    use crate::java_ir::{JavaType, Member, MethodDecl, Program, TypeDecl, TypeKind};

    fn empty_type(name: &str) -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Class,
            name: name.to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            static_init_blocks: Vec::new(),
            span: Span::default(),
        }
    }

    fn empty_interface(name: &str) -> TypeDecl {
        TypeDecl {
            kind: TypeKind::Interface,
            ..empty_type(name)
        }
    }

    #[test]
    fn find_type_decl_refuses_ambiguous_simple_package_peer() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![empty_type("a.Named"), empty_type("b.Named")],
        };

        assert!(find_type_decl(&program, "Named").is_none());
        assert_eq!(
            find_type_decl(&program, "a.Named").map(|ty| ty.name.as_str()),
            Some("a.Named")
        );
        assert_eq!(
            find_type_decl(&program, "ANamed").map(|ty| ty.name.as_str()),
            Some("a.Named")
        );
    }

    #[test]
    fn find_type_decl_for_owner_context_prefers_same_package_peer() {
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                empty_type("a.Base"),
                empty_type("b.Base"),
                empty_type("b.Child"),
            ],
        };

        assert!(find_type_decl(&program, "Base").is_none());
        assert_eq!(
            find_type_decl_for_owner_context(&program, "b.Child", "Base")
                .map(|ty| ty.name.as_str()),
            Some("b.Base")
        );
    }

    #[test]
    fn source_type_extends_uses_same_package_before_ambiguous_simple_peer() {
        let mut child = empty_type("b.Child");
        child.extends = Some("Base".to_string());
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![empty_type("a.Base"), empty_type("b.Base"), child.clone()],
        };

        assert!(source_type_extends(&program, &child, "b.Base"));
        assert!(!source_type_extends(&program, &child, "a.Base"));
    }

    #[test]
    fn implemented_interfaces_use_same_package_before_ambiguous_simple_peer() {
        let mut child = empty_type("b.Child");
        child.implements = vec!["Named".to_string()];
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                empty_interface("a.Named"),
                empty_interface("b.Named"),
                child,
            ],
        };

        assert_eq!(
            implemented_interfaces_for_type(&program, &program.types[2]),
            vec!["b.Named".to_string()]
        );
    }

    #[test]
    fn method_type_param_bounds_use_owner_package_before_ambiguous_simple_peer() {
        let method = MethodDecl {
            name: "id".to_string(),
            rust_name: Some("id".to_string()),
            return_type: JavaType::Void,
            params: Vec::new(),
            type_params: vec!["T".to_string()],
            type_param_bounds: vec![Some(JavaType::Named {
                name: "Named".to_string(),
                args: Vec::new(),
            })],
            modifiers: Vec::new(),
            throws: Vec::new(),
            body: None,
            span: Span::default(),
        };
        let mut owner = empty_type("b.Owner");
        owner.members = vec![Member::Method(method.clone())];
        let program = Program {
            package: None,
            imports: Vec::new(),
            types: vec![
                empty_interface("a.Named"),
                empty_interface("b.Named"),
                owner,
            ],
        };

        assert_eq!(
            rust_method_type_params(&method, &program, "b.Owner"),
            "<T: BNamed>"
        );
    }
}

#[cfg(test)]
mod trait_return_shape_tests {
    use super::trait_return_shape_should_widen_to_implementation;
    use crate::java_ir::JavaType;
    use crate::slot_shape::{OwnershipShape, RustSlotShape};

    #[test]
    fn trait_return_shape_widens_to_optional_implementation_return() {
        let interface_shape = RustSlotShape::conservative(JavaType::String);
        let implementation_shape =
            RustSlotShape::for_storage(crate::types::optional_java_type(JavaType::String));

        assert!(trait_return_shape_should_widen_to_implementation(
            &interface_shape,
            &implementation_shape
        ));
    }

    #[test]
    fn trait_return_shape_does_not_widen_to_unrelated_optional_return() {
        let interface_shape = RustSlotShape::conservative(JavaType::String);
        let implementation_shape = RustSlotShape::for_storage(crate::types::optional_java_type(
            JavaType::Primitive(crate::java_ir::PrimitiveType::Int),
        ));

        assert!(!trait_return_shape_should_widen_to_implementation(
            &interface_shape,
            &implementation_shape
        ));
    }

    #[test]
    fn trait_return_shape_keeps_owned_implementation_widening() {
        let mut interface_shape = RustSlotShape::conservative(JavaType::String);
        interface_shape.ownership = OwnershipShape::SharedBorrow;
        let implementation_shape = RustSlotShape::for_storage(JavaType::String);

        assert!(trait_return_shape_should_widen_to_implementation(
            &interface_shape,
            &implementation_shape
        ));
    }
}

#[cfg(test)]
mod shared_handle_forwarding_tests {
    use super::shared_handle_forwarding_impl;

    #[test]
    fn forwards_a_read_only_surface() {
        let rendered = shared_handle_forwarding_impl(
            "Named",
            &[
                "    fn id(&self) -> String;".to_string(),
                "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
            ],
        )
        .expect("a &self-only surface forwards");
        assert!(
            rendered.contains(
                "impl<__JavaShared: Named + 'static> Named for std::rc::Rc<__JavaShared>"
            )
        );
        assert!(rendered.contains("fn id(&self) -> String { (**self).id() }"));
        assert!(rendered.contains("fn as_any(&self) -> &dyn std::any::Any { self }"));
    }

    #[test]
    fn clone_box_forwarding_preserves_the_handle() {
        let rendered = shared_handle_forwarding_impl(
            "Named",
            &["    fn clone_box(&self) -> std::boxed::Box<dyn Named>;".to_string()],
        )
        .expect("forwards");

        assert!(
            rendered.contains(
                "fn clone_box(&self) -> std::boxed::Box<dyn Named> { std::boxed::Box::new(self.clone()) }"
            ),
            "{rendered}"
        );
        assert!(!rendered.contains("(**self).clone_box()"), "{rendered}");
    }

    /// Arguments are forwarded by name, and a generic parameter list must not be
    /// mistaken for one: the comma in `HashMap<K, V>` is inside `<>`.
    #[test]
    fn forwards_arguments_without_splitting_generics() {
        let rendered = shared_handle_forwarding_impl(
            "Sink",
            &["    fn put<T0: ?Sized>(&self, key: std::collections::HashMap<i32, String>, value: &T0) -> bool;"
                .to_string()],
        )
        .expect("forwards");
        assert!(
            rendered.contains("{ (**self).put(key, value) }"),
            "{rendered}"
        );
    }

    /// Mutating trait methods forward through the cell-backed shared handles.
    #[test]
    fn a_mutating_method_forwards_through_cell_handles() {
        let rendered = shared_handle_forwarding_impl(
            "Shape",
            &[
                "    fn size(&self) -> i32;".to_string(),
                "    fn grow(&mut self);".to_string(),
            ],
        )
        .expect("cell-backed handles forward mutating surfaces");
        assert!(rendered.contains("Shape for std::rc::Rc<std::cell::RefCell<__JavaShared>>"));
        assert!(rendered.contains("fn size(&self) -> i32 { self.borrow().size() }"));
        assert!(rendered.contains("fn grow(&mut self) { self.borrow_mut().grow() }"));
        assert!(rendered.contains("Shape for std::sync::Arc<std::sync::Mutex<__JavaShared>>"));
        assert!(rendered.contains("fn grow(&mut self) { self.lock().unwrap().grow() }"));
        assert!(!rendered.contains("Shape for std::rc::Rc<__JavaShared>"));
    }

    #[test]
    fn borrowed_returns_forward_through_direct_handles_only() {
        let rendered = shared_handle_forwarding_impl(
            "LabelSource",
            &[
                "    fn label(&self) -> &str;".to_string(),
                "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
            ],
        )
        .expect("direct handles can tie the returned borrow to &self");
        assert!(
            rendered.contains("LabelSource for std::rc::Rc<__JavaShared>"),
            "{rendered}"
        );
        assert!(rendered.contains("fn label(&self) -> &str { (**self).label() }"));
        assert!(
            !rendered.contains("LabelSource for std::rc::Rc<std::cell::RefCell<__JavaShared>>"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("LabelSource for std::sync::Arc<std::sync::Mutex<__JavaShared>>"),
            "{rendered}"
        );
    }

    /// A Java interface method may declare its own type parameter; the impl's
    /// must not collide with it (E0403).
    #[test]
    fn does_not_shadow_a_method_type_parameter() {
        let rendered = shared_handle_forwarding_impl(
            "Mapper",
            &["    fn id<T>(&self, value: T) -> T where Self: Sized;".to_string()],
        )
        .expect("forwards");
        assert!(!rendered.contains("impl<T:"), "{rendered}");
        assert!(
            rendered.contains("for std::rc::Rc<__JavaShared>"),
            "{rendered}"
        );
    }

    /// A static (receiverless) method cannot be reached through a handle either.
    #[test]
    fn a_receiverless_method_rejects_the_surface() {
        assert!(
            shared_handle_forwarding_impl("Factory", &["    fn make() -> i32;".to_string()])
                .is_none()
        );
    }
}
