use std::collections::{HashMap, HashSet};

use crate::emit::emit_type;
use crate::exception_lowering::exception_result_type;
use crate::exception_plan::{MethodExceptionPlan, ProgramExceptionPlan};
use crate::facts::FactSet;
use crate::java_ir::{
    ConstructorDecl, Expr, FieldDecl, JavaType, Literal, Member, MethodDecl, Modifier,
    PrimitiveType, Program, Stmt, TypeDecl, TypeKind,
};
use crate::representation::{ObjectRepresentation, RepresentationPlan};
use crate::rust_ir::RustType;
use crate::signature::{ReceiverPlan, SignaturePlan};
use crate::slot_render::{
    rust_name_for_java_named_type, rust_type_for_java_type_with_facts,
    rust_type_for_slot_with_facts,
};
use crate::slot_shape::{DispatchShape, OwnershipShape, RustSlotShape, ViewShape};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepresentationTypeEmission {
    LowerAsConcreteStruct,
    RawRustItem(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraitImplMethodFallbackPolicy {
    MissingImplementation,
    IncompatibleImplementation {
        error: TraitImplMethodCompatibilityError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraitImplMethodCompatibilityError {
    ReturnTypeMismatch,
    ParameterCountMismatch,
    ParameterTypeMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImplMethodMessagePolicy {
    pub message: String,
}

pub fn source_class_bound_trait_name(type_name: &str) -> String {
    format!("{}Bound", rust_name_for_java_named_type(type_name))
}

impl TraitImplMethodMessagePolicy {
    pub fn compile_error_body(&self, signature: &str) -> String {
        format!(
            "{} {{ panic!({:?}); }}",
            signature.trim_end_matches(';'),
            self.message
        )
    }
}

pub fn type_emission_for_with_facts(
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

pub fn type_emission_for_with_facts_and_exceptions(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
) -> RepresentationTypeEmission {
    let Some(representation) = representations.type_representation(&ty.name) else {
        return RepresentationTypeEmission::LowerAsConcreteStruct;
    };
    match &representation.representation {
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
            ty.name
        )),
    }
}

pub fn source_hierarchy_carrier_item_for_type(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    facts: &FactSet,
) -> Option<String> {
    let representation = representations.type_representation(&ty.name)?;
    let ObjectRepresentation::SourceHierarchyCarrier {
        carrier_name,
        variants,
    } = &representation.representation
    else {
        return None;
    };
    Some(source_hierarchy_carrier_item(carrier_name, variants, facts))
}

fn closed_enum_carrier_item(enum_name: &str, variants: &[String], facts: &FactSet) -> String {
    carrier_enum_item("#[derive(Clone, PartialEq)]", enum_name, variants, facts)
}

fn source_hierarchy_carrier_item(enum_name: &str, variants: &[String], facts: &FactSet) -> String {
    carrier_enum_item("#[derive(Clone)]", enum_name, variants, facts)
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
    if placement.shared && placement.is_implemented() {
        placement.wrap(variant.to_string())
    } else {
        variant.to_string()
    }
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
                "\n\n    pub fn {field_name}(&self) -> {rust_return_ty} {{\n        match self {{\n{arms}\n        }}\n    }}"
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
        .map(|(index, param)| (param.name.clone(), index))
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

pub fn type_emission_for_program_with_facts_and_exceptions(
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
    match &representation.representation {
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

pub fn raw_type_item_for_with_facts(
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

pub fn raw_type_item_for_with_facts_and_exceptions(
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

pub fn raw_type_item_for_program_with_facts_and_exceptions(
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

pub fn trait_impl_emissions_for_type_with_facts(
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

pub fn trait_impl_emissions_for_type_with_facts_and_exceptions(
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

pub fn source_class_bound_trait_item_for_type(
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
    let methods = ty
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

pub fn source_class_bound_trait_impl_emissions_for_type(
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
    params.push(match signature.receiver {
        ReceiverPlan::Mutable => "&mut self".to_string(),
        ReceiverPlan::Shared | ReceiverPlan::None => "&self".to_string(),
    });
    let param_shapes = trait_method_param_shape_plan(ty, method, program, signatures);
    params.extend(signature.params.iter().enumerate().map(|(index, param)| {
        let shape = param_shapes.get(index).unwrap_or(&param.shape);
        format!(
            "{}: {}",
            param.name,
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
    let type_params = rust_method_type_params(method, program);
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
        if exceptions.wrap_returns_in_result {
            return Some(exception_result_type(RustType::Unit));
        }
        return None;
    }
    let rust_return = return_shape_override
        .map(|shape| rust_type_for_slot_with_facts(shape, facts))
        .unwrap_or_else(|| rust_type_for_java_type_with_facts(&method.return_type, facts));
    if exceptions.wrap_returns_in_result {
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
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let implementation_shape = implementation_signature
                .params
                .get(index)
                .map(|param| &param.shape);
            trait_impl_forwarded_arg(param.name.as_str(), &param.shape, implementation_shape)
        })
        .collect::<Vec<_>>()
        .join(", ");
    let receiver_arg = if args.is_empty() {
        trait_impl_forwarding_receiver_arg(
            ty,
            forwarding_owner,
            Some(implementation_signature.receiver),
        )
    } else {
        format!(
            "{}, {args}",
            trait_impl_forwarding_receiver_arg(
                ty,
                forwarding_owner,
                Some(implementation_signature.receiver)
            )
        )
    };
    let implementing_owner_name = rust_name_for_java_named_type(&forwarding_owner.name);
    let call = format!("{implementing_owner_name}::{rust_name}({receiver_arg})");
    let call = adapt_source_class_bound_trait_impl_return(
        call,
        trait_return_shape_for_body
            .as_ref()
            .or(bound_signature.return_shape.as_ref()),
        &implementation_signature.return_shape,
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
        .return_shape
        .clone()?;
    for (implementation_owner, implementation_method) in candidates {
        if trait_impl_method_compatibility_error(bound_method, implementation_method).is_some() {
            continue;
        }
        let Some(implementation_shape) = signatures
            .method_signature(implementation_owner, implementation_method)
            .unwrap_or_else(|| {
                signatures.required_method_signature(implementation_owner, implementation_method)
            })
            .return_shape
            .clone()
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
    if exceptions.wrap_returns_in_result {
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
        collect_implemented_interface_name(program, interface_name, seen, out);
    }
    if let Some(super_name) = ty.extends.as_deref() {
        if ty.kind == TypeKind::Interface {
            // For an interface, `extends` names a *super-interface*, which the
            // type implements. Treating it as a superclass meant a class
            // implementing `P extends G` never got `impl G for C`, so the
            // subtrait bound could not be satisfied (`E0277`).
            collect_implemented_interface_name(program, super_name, seen, out);
        } else if let Some(super_ty) = find_type_decl(program, super_name) {
            collect_implemented_interfaces_for_type(program, super_ty, seen, out);
        }
    }
}

fn collect_implemented_interface_name(
    program: &Program,
    interface_name: &str,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    let interface = find_type_decl(program, interface_name);
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
    format!(
        "pub trait {trait_name} {{\n{}\n}}\n\n{}\n\n{}{forwarding}",
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
    format!(
        "pub trait {trait_name} {{\n{}\n}}\n\n{}\n\n{}{forwarding}",
        methods.join("\n"),
        dyn_trait_anonymous_impl(&trait_name),
        boxed_dyn_trait_clone_impl(&trait_name)
    )
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
/// `None` when any method takes `&mut self`: `Rc` has no `DerefMut`. That is the
/// same precondition `storage_strategy::interface_surface_is_handle_forwardable`
/// reports, checked here against the rendered surface so the two cannot drift.
pub(crate) fn shared_handle_forwarding_impl(
    trait_name: &str,
    signatures: &[String],
) -> Option<String> {
    let methods = signatures
        .iter()
        .map(|signature| forwarded_trait_method(signature))
        .collect::<Option<Vec<_>>>()?;
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
    let body = methods.join("\n");
    Some(
        ["std::rc::Rc", "std::sync::Arc"]
            .into_iter()
            .map(|handle| {
                format!(
                    "impl<__JavaShared: {trait_name} + 'static> {trait_name} for {handle}<__JavaShared> {{\n{body}\n}}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

/// Turns one rendered trait-method signature into a forwarding implementation.
///
/// The signature is text this module just produced, so the parse is closed over
/// our own output rather than arbitrary Rust: `    fn name<generics>(&self, a: A) -> R;`
fn forwarded_trait_method(signature: &str) -> Option<String> {
    let trimmed = signature.trim();
    let body = trimmed.strip_prefix("fn ")?.strip_suffix(';')?;
    if trimmed.contains("&mut self") || !trimmed.contains("&self") {
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
    Some(format!(
        "    {signature_without_semicolon} {{ (**self).{name}({}) }}",
        args.join(", ")
    ))
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
    let (implementing_owner, implementing_method) = candidates
        .iter()
        .copied()
        .find(|(_, candidate)| {
            trait_impl_method_compatibility_error(interface_method, candidate).is_none()
        })
        .unwrap_or(candidates[0]);
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
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let implementation_shape = implementation_signature
                .params
                .get(index)
                .map(|param| &param.shape);
            trait_impl_forwarded_arg(param.name.as_str(), &param.shape, implementation_shape)
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
            .or(interface_signature.return_shape.as_ref()),
        &implementation_signature.return_shape,
        interface_exceptions,
    );
    let body = if interface_exceptions.wrap_returns_in_result
        || signatures
            .required_method_signature(interface, interface_method)
            .return_shape
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
    if interface_exceptions.wrap_returns_in_result {
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

pub fn trait_impl_method_message_policy(
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
        params.push(match receiver_override.unwrap_or(signature.receiver) {
            ReceiverPlan::Mutable => "&mut self".to_string(),
            ReceiverPlan::Shared | ReceiverPlan::None => "&self".to_string(),
        });
    }
    let param_shapes = trait_method_param_shape_plan(ty, method, program, signatures);
    params.extend(signature.params.iter().enumerate().map(|(index, param)| {
        let shape = param_shapes.get(index).unwrap_or(&param.shape);
        format!(
            "{}: {}",
            param.name,
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
    let type_params = rust_method_type_params(method, program);
    let where_clause = rust_generic_trait_method_where_clause(method);
    format!(
        "    fn {rust_name}{type_params}({}){return_type}{where_clause};",
        params.join(", ")
    )
}

fn rust_method_type_params(method: &MethodDecl, program: &Program) -> String {
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
            match find_type_decl(program, bound) {
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
        .return_shape
        .clone()?;
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
            .return_shape
            .clone()
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
        .params
        .iter()
        .map(|param| param.shape.clone())
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
        for (index, implementation_param) in implementation_signature.params.iter().enumerate() {
            let Some(interface_shape) = param_shapes.get_mut(index) else {
                continue;
            };
            if trait_param_shape_should_widen_to_implementation(
                interface_shape,
                &implementation_param.shape,
            ) {
                *interface_shape = implementation_param.shape.clone();
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
        .receiver;
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
            .receiver;
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
            .return_shape
            .clone()
    });
    let rust_return = return_shape
        .as_ref()
        .map(|shape| rust_type_for_slot_with_facts(shape, facts));
    if exceptions.wrap_returns_in_result {
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
        .representation
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
        && let Some(super_ty) = find_type_decl(program, super_name)
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
    source_type_param_bound_names(program)
        .into_iter()
        .filter_map(|bound_name| find_type_decl(program, &bound_name))
        .filter(|bound| {
            matches!(bound.kind, TypeKind::Class)
                && (bound.name == ty.name || source_type_extends(program, ty, &bound.name))
        })
        .collect()
}

fn source_class_is_used_as_type_param_bound(program: &Program, type_name: &str) -> bool {
    source_type_param_bound_names(program)
        .into_iter()
        .any(|bound| bound == type_name)
}

fn source_type_param_bound_names(program: &Program) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for ty in &program.types {
        collect_source_type_param_bound_names_in_type(ty, &mut seen, &mut out);
    }
    out
}

fn collect_source_type_param_bound_names_in_type(
    ty: &TypeDecl,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    collect_bound_names(&ty.type_param_bounds, seen, out);
    for member in &ty.members {
        match member {
            Member::Method(method) => collect_bound_names(&method.type_param_bounds, seen, out),
            Member::Constructor(constructor) => {
                collect_bound_names(&constructor.type_param_bounds, seen, out)
            }
            Member::Type(nested) => {
                collect_source_type_param_bound_names_in_type(nested, seen, out)
            }
            _ => {}
        }
    }
}

fn collect_bound_names(
    bounds: &[Option<JavaType>],
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    for bound in bounds {
        if let Some(JavaType::Named { name, .. }) = bound
            && seen.insert(name.clone())
        {
            out.push(name.clone());
        }
    }
}

fn source_type_extends(program: &Program, ty: &TypeDecl, target_name: &str) -> bool {
    let mut next = ty.extends.as_deref();
    while let Some(name) = next {
        if name == target_name {
            return true;
        }
        next = find_type_decl(program, name).and_then(|super_ty| super_ty.extends.as_deref());
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
mod type_lookup_tests {
    use super::find_type_decl;
    use crate::diagnostics::Span;
    use crate::java_ir::{Program, TypeDecl, TypeKind};

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
        assert!(rendered.contains("fn as_any(&self) -> &dyn std::any::Any { (**self).as_any() }"));
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

    /// `Rc` has no `DerefMut`, so a mutating surface has no forwarding impl at
    /// all -- the whole trait is rejected, not the single method.
    #[test]
    fn a_mutating_method_rejects_the_whole_surface() {
        assert!(
            shared_handle_forwarding_impl(
                "Shape",
                &[
                    "    fn size(&self) -> i32;".to_string(),
                    "    fn grow(&mut self);".to_string(),
                ],
            )
            .is_none()
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
