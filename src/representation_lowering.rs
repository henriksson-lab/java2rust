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
use crate::slot_shape::{OwnershipShape, RustSlotShape, ViewShape};

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
                "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum {rust_enum_name} {{\n{variant_items}\n}}\n{default_impl}\n\nimpl {rust_enum_name} {{\n    pub fn values() -> Vec<Self> {{ vec![{values}] }}\n\n    pub fn value_of<T: AsRef<str>>(name: T) -> Self {{\n        match name.as_ref() {{\n{value_of_arms}\n            other => panic!(\"unknown Java enum constant {{}}.{{}}\", {enum_name:?}, other),\n        }}\n    }}\n\n    #[allow(non_snake_case)]\n    pub fn valueOf<T: AsRef<str>>(name: T) -> Self {{ Self::value_of(name) }}\n\n    pub fn name(&self) -> &'static str {{\n        match self {{\n{name_arms}\n        }}\n    }}\n\n    pub fn ordinal(&self) -> i32 {{\n        match self {{\n{ordinal_arms}\n        }}\n    }}\n{payload_accessors}\n}}\n\nimpl std::fmt::Display for {rust_enum_name} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        write!(f, \"{{}}\", self.name())\n    }}\n}}"
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
                Some(source_class_bound_trait_method_signature(
                    ty,
                    method,
                    signatures,
                    facts,
                    exceptions.method_plan(ty, method),
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    Some(format!(
        "pub trait {trait_name} {{\n{}\n}}",
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

fn source_class_bound_trait_method_signature(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
) -> String {
    let signature = signatures.required_method_signature(ty, method);
    let mut params = Vec::new();
    params.push(match signature.receiver {
        ReceiverPlan::Mutable => "&mut self".to_string(),
        ReceiverPlan::Shared | ReceiverPlan::None => "&self".to_string(),
    });
    params.extend(signature.params.iter().map(|param| {
        format!(
            "{}: {}",
            param.name,
            emit_type(&rust_type_for_slot_with_facts(&param.shape, facts))
        )
    }));
    let return_type = source_class_bound_trait_method_return_type(method, facts, exceptions)
        .map(|ty| format!(" -> {}", emit_type(&ty)))
        .unwrap_or_default();
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    format!("    fn {rust_name}({}){return_type};", params.join(", "))
}

fn source_class_bound_trait_method_return_type(
    method: &MethodDecl,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
) -> Option<RustType> {
    if method.return_type == JavaType::Void {
        if exceptions.wrap_returns_in_result {
            return Some(exception_result_type(RustType::Unit));
        }
        return None;
    }
    let rust_return = rust_type_for_java_type_with_facts(&method.return_type, facts);
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
    let signature = source_class_bound_trait_method_signature(
        bound,
        bound_method,
        signatures,
        facts,
        exceptions,
    );
    let candidates = implementing_method_candidates_in_hierarchy(ty, bound_method, program);
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
        bound_method,
        &implementation_signature.return_shape,
    );
    let body =
        if source_class_bound_trait_method_return_type(bound_method, facts, exceptions).is_some() {
            call
        } else {
            format!("{call};")
        };
    format!("{} {{ {body} }}", signature.trim_end_matches(';'))
}

fn adapt_source_class_bound_trait_impl_return(
    call: String,
    bound_method: &MethodDecl,
    implementation_return_shape: &Option<RustSlotShape>,
) -> String {
    if let Some(shape) = implementation_return_shape
        && shape
            .optional_element_type()
            .is_some_and(|element_ty| element_ty == bound_method.return_type)
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
    if let Some(super_name) = ty.extends.as_deref()
        && let Some(super_ty) = find_type_decl(program, super_name)
    {
        collect_implemented_interfaces_for_type(program, super_ty, seen, out);
    }
}

fn collect_implemented_interface_name(
    program: &Program,
    interface_name: &str,
    seen: &mut HashSet<String>,
    out: &mut Vec<String>,
) {
    if !seen.insert(interface_name.to_string()) {
        return;
    }
    out.push(interface_name.to_string());
    if let Some(interface) = find_type_decl(program, interface_name) {
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
    format!(
        "pub trait {trait_name} {{\n{}\n}}\n\n{}\n\n{}",
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
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    methods.extend(trait_object_support_method_signatures(&trait_name));
    format!(
        "pub trait {trait_name} {{\n{}\n}}\n\n{}\n\n{}",
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
        format!("    fn clone_box(&self) -> Box<dyn {trait_name}>;"),
    ]
}

fn trait_object_support_impl_methods(trait_name: &str) -> Vec<String> {
    vec![
        "    fn as_any(&self) -> &dyn std::any::Any { self }".to_string(),
        format!(
            "    fn clone_box(&self) -> Box<dyn {trait_name}> {{ panic!(\"java2rust: clone_box for {trait_name} requires a cloneable Java object carrier\") }}"
        ),
    ]
}

pub(crate) fn boxed_dyn_trait_clone_impl(trait_name: &str) -> String {
    format!(
        "impl Clone for Box<dyn {trait_name}> {{\n    fn clone(&self) -> Self {{ self.clone_box() }}\n}}"
    )
}

pub(crate) fn dyn_trait_anonymous_impl(trait_name: &str) -> String {
    format!(
        "impl dyn {trait_name} {{\n    pub fn anonymous() -> Box<dyn {trait_name}> {{ panic!(\"java2rust: anonymous class body for {trait_name} requires object lowering\") }}\n}}"
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
    trait_method_signature_with_receiver(ty, method, signatures, facts, exceptions, None)
}

fn trait_method_signature_with_receiver(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    receiver_override: Option<ReceiverPlan>,
) -> String {
    trait_method_signature_with_receiver_and_return_shape(
        ty,
        method,
        signatures,
        facts,
        exceptions,
        receiver_override,
        None,
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
) -> String {
    let signature = signatures.required_method_signature(ty, method);
    let mut params = Vec::new();
    if !method.modifiers.contains(&Modifier::Static) {
        params.push(match receiver_override.unwrap_or(signature.receiver) {
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
    format!("    fn {rust_name}({}){return_type};", params.join(", "))
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
    for implementor in program.types.iter().filter(|ty| {
        ty.kind == TypeKind::Class
            && ty
                .implements
                .iter()
                .any(|implemented| implemented == &interface.name)
    }) {
        let Some(implementation) = implementing_method_candidates(implementor, interface_method)
            .into_iter()
            .find(|candidate| {
                trait_impl_method_compatibility_error(interface_method, candidate).is_none()
            })
        else {
            continue;
        };
        let Some(implementation_shape) = signatures
            .required_method_signature(implementor, implementation)
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

fn trait_return_shape_should_widen_to_implementation(
    interface_shape: &RustSlotShape,
    implementation_shape: &RustSlotShape,
) -> bool {
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
    for implementor in program.types.iter().filter(|ty| {
        ty.kind == TypeKind::Class
            && ty
                .implements
                .iter()
                .any(|implemented| implemented == &interface.name)
    }) {
        let Some(implementation) = implementing_method_candidates(implementor, interface_method)
            .into_iter()
            .find(|candidate| {
                trait_impl_method_compatibility_error(interface_method, candidate).is_none()
            })
        else {
            continue;
        };
        let implementation_receiver = signatures
            .required_method_signature(implementor, implementation)
            .receiver;
        if implementation_receiver == ReceiverPlan::Mutable {
            receiver = ReceiverPlan::Mutable;
        }
    }
    Some(receiver)
}

pub(crate) fn trait_method_exception_plan(
    interface: &TypeDecl,
    interface_method: &MethodDecl,
    program: &Program,
    exceptions: &ProgramExceptionPlan,
) -> MethodExceptionPlan {
    let mut plan = exceptions.method_plan(interface, interface_method);
    for implementor in program.types.iter().filter(|ty| {
        ty.kind == TypeKind::Class
            && ty
                .implements
                .iter()
                .any(|implemented| implemented == &interface.name)
    }) {
        let Some(implementation) = implementing_method_candidates(implementor, interface_method)
            .into_iter()
            .find(|candidate| {
                trait_impl_method_compatibility_error(interface_method, candidate).is_none()
            })
        else {
            continue;
        };
        plan.wrap_returns_in_result |= exceptions
            .method_plan(implementor, implementation)
            .wrap_returns_in_result;
    }
    plan
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
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if method.name == interface_method.name => Some(method),
            _ => None,
        })
        .collect()
}

fn implementing_method_candidates_in_hierarchy<'a>(
    ty: &'a TypeDecl,
    interface_method: &MethodDecl,
    program: &'a Program,
) -> Vec<(&'a TypeDecl, &'a MethodDecl)> {
    let mut out = implementing_method_candidates(ty, interface_method)
        .into_iter()
        .map(|method| (ty, method))
        .collect::<Vec<_>>();
    if !out.is_empty() {
        return out;
    }
    if let Some(super_name) = ty.extends.as_deref()
        && let Some(super_ty) = find_type_decl(program, super_name)
    {
        out.extend(implementing_method_candidates_in_hierarchy(
            super_ty,
            interface_method,
            program,
        ));
    }
    out
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
