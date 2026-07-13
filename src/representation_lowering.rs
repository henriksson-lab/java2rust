use crate::emit::emit_type;
use crate::exception_lowering::exception_result_type;
use crate::facts::FactSet;
use crate::java_ir::{Member, MethodDecl, Modifier, Program, TypeDecl, TypeKind};
use crate::representation::{ObjectRepresentation, RepresentationPlan};
use crate::rust_ir::RustType;
use crate::signature::{ReceiverPlan, SignaturePlan};
use crate::slot_render::rust_type_for_slot_with_facts;

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

impl TraitImplMethodMessagePolicy {
    pub fn compile_error_body(&self, signature: &str) -> String {
        format!(
            "{} {{ compile_error!(\"{}\"); }}",
            signature.trim_end_matches(';'),
            self.message
        )
    }
}

pub fn type_emission_for(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
) -> RepresentationTypeEmission {
    type_emission_for_with_facts(representations, ty, signatures, &FactSet::default())
}

pub fn type_emission_for_with_facts(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> RepresentationTypeEmission {
    let Some(representation) = representations.type_representation(&ty.name) else {
        return RepresentationTypeEmission::LowerAsConcreteStruct;
    };
    match &representation.representation {
        ObjectRepresentation::ConcreteStruct => RepresentationTypeEmission::LowerAsConcreteStruct,
        ObjectRepresentation::DynTrait { trait_name }
        | ObjectRepresentation::BoxedDynTrait { trait_name } => {
            RepresentationTypeEmission::RawRustItem(trait_item_for_type(
                ty, trait_name, signatures, facts,
            ))
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
            "pub type {} = Box<dyn std::any::Any>;",
            ty.name
        )),
    }
}

pub fn trait_impl_emissions_for_type(
    representations: &RepresentationPlan,
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
) -> Vec<String> {
    trait_impl_emissions_for_type_with_facts(
        representations,
        program,
        ty,
        signatures,
        &FactSet::default(),
    )
}

pub fn trait_impl_emissions_for_type_with_facts(
    representations: &RepresentationPlan,
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Vec<String> {
    if ty.kind != TypeKind::Class {
        return Vec::new();
    }
    ty.implements
        .iter()
        .filter_map(|interface_name| {
            let trait_name = dyn_trait_name_for(representations, interface_name)?;
            let interface = find_type_decl(program, interface_name)?;
            Some(trait_impl_for_type(
                ty,
                interface,
                &trait_name,
                signatures,
                facts,
            ))
        })
        .collect()
}

fn trait_item_for_type(
    ty: &TypeDecl,
    trait_name: &str,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> String {
    let methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(trait_method_signature(ty, method, signatures, facts)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if methods.is_empty() {
        format!("pub trait {trait_name} {{}}")
    } else {
        format!("pub trait {trait_name} {{\n{}\n}}", methods.join("\n"))
    }
}

fn trait_impl_for_type(
    ty: &TypeDecl,
    interface: &TypeDecl,
    trait_name: &str,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> String {
    let methods = interface
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(interface_method) => Some(trait_impl_method(
                ty,
                interface,
                interface_method,
                signatures,
                facts,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if methods.is_empty() {
        format!("impl {trait_name} for {} {{}}", ty.name)
    } else {
        format!(
            "impl {trait_name} for {} {{\n{}\n}}",
            ty.name,
            methods.join("\n")
        )
    }
}

fn trait_impl_method(
    ty: &TypeDecl,
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> String {
    let signature = trait_method_signature(interface, interface_method, signatures, facts);
    let candidates = implementing_method_candidates(ty, interface_method);
    if candidates.is_empty() {
        return trait_impl_method_message_policy(
            ty,
            interface_method,
            TraitImplMethodFallbackPolicy::MissingImplementation,
        )
        .compile_error_body(&signature);
    };
    let implementing_method = candidates
        .iter()
        .copied()
        .find(|candidate| {
            trait_impl_method_compatibility_error(interface_method, candidate).is_none()
        })
        .unwrap_or(candidates[0]);
    if let Some(message) =
        trait_impl_method_compatibility_error(interface_method, implementing_method)
    {
        return trait_impl_method_message_policy(
            ty,
            interface_method,
            TraitImplMethodFallbackPolicy::IncompatibleImplementation { error: message },
        )
        .compile_error_body(&signature);
    }
    let rust_name = implementing_method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&implementing_method.name));
    let args = signatures
        .required_method_signature(interface, interface_method)
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let receiver_arg = if interface_method.modifiers.contains(&Modifier::Static) {
        String::new()
    } else if args.is_empty() {
        "self".to_string()
    } else {
        format!("self, {args}")
    };
    let call = format!("{}::{}({})", ty.name, rust_name, receiver_arg);
    let body = if signatures
        .required_method_signature(interface, interface_method)
        .return_shape
        .is_some()
    {
        call
    } else {
        format!("{call};")
    };
    format!("{} {{ {body} }}", signature.trim_end_matches(';'))
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
) -> String {
    let signature = signatures.required_method_signature(ty, method);
    let mut params = Vec::new();
    if !method.modifiers.contains(&Modifier::Static) {
        params.push(match signature.receiver {
            ReceiverPlan::Mutable => "&mut self".to_string(),
            ReceiverPlan::Shared | ReceiverPlan::None => "&self".to_string(),
        });
    }
    params.extend(signature.params.iter().map(|param| {
        format!(
            "{}: {}",
            param.name,
            emit_type(&rust_type_for_slot_with_facts(&param.shape, facts))
        )
    }));
    let return_type = trait_method_return_type(method, signatures, ty, facts)
        .map(|ty| format!(" -> {}", emit_type(&ty)))
        .unwrap_or_default();
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    format!("    fn {rust_name}({}){return_type};", params.join(", "))
}

fn trait_method_return_type(
    method: &MethodDecl,
    signatures: &SignaturePlan,
    ty: &TypeDecl,
    facts: &FactSet,
) -> Option<RustType> {
    let rust_return = signatures
        .required_method_signature(ty, method)
        .return_shape
        .as_ref()
        .map(|shape| rust_type_for_slot_with_facts(shape, facts));
    if method.throws.is_empty() {
        rust_return
    } else {
        Some(exception_result_type(rust_return.unwrap_or(RustType::Unit)))
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
        ObjectRepresentation::DynTrait { trait_name }
        | ObjectRepresentation::BoxedDynTrait { trait_name } => Some(trait_name.clone()),
        _ => None,
    }
}

fn implementing_method_candidates<'a>(
    ty: &'a TypeDecl,
    interface_method: &MethodDecl,
) -> Vec<&'a MethodDecl> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if method.name == interface_method.name => Some(method),
            _ => None,
        })
        .collect()
}

fn find_type_decl<'a>(program: &'a Program, type_name: &str) -> Option<&'a TypeDecl> {
    program
        .types
        .iter()
        .find_map(|ty| find_type_decl_in_type(ty, type_name))
}

fn find_type_decl_in_type<'a>(ty: &'a TypeDecl, type_name: &str) -> Option<&'a TypeDecl> {
    if ty.name == type_name {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_type_decl_in_type(nested, type_name),
        _ => None,
    })
}
