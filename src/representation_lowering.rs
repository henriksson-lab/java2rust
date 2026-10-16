use std::collections::{HashMap, HashSet};

use crate::emit::emit_type;
use crate::exception_lowering::exception_result_type;
use crate::exception_plan::{MethodExceptionPlan, ProgramExceptionPlan};
use crate::facts::FactSet;
use crate::java_ir::{
    Block, ConstructorDecl, Expr, FieldDecl, JavaType, Literal, Member, MethodDecl, Modifier,
    PrimitiveType, Program, Stmt, TypeDecl, TypeKind,
};
use crate::java_naming::rust_name_for_java_named_type;
use crate::representation::{ObjectRepresentation, RepresentationPlan};
use crate::rust_ir::{RustExpr, RustType};
use crate::signature::{ReceiverPlan, SignaturePlan};
use crate::slot_render::{
    rust_type_for_java_type_with_facts, rust_type_for_java_type_with_facts_and_representations,
    rust_type_for_slot_with_facts, rust_type_for_slot_with_facts_and_representations,
};
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
        ObjectRepresentation::DynTrait { trait_name } => {
            RepresentationTypeEmission::RawRustItem(trait_item_for_type(
                ty,
                trait_name,
                signatures,
                facts,
                exceptions,
                representations.storage_placement_for(&ty.name).sync(),
            ))
        }
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
            let payload_accessors = java_enum_payload_accessor_methods(
                ty,
                variants,
                signatures,
                facts,
                representations,
            );
            let enum_item = format!(
                "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]\npub enum {rust_enum_name} {{\n{variant_items}\n}}\n{default_impl}\n\nimpl {rust_enum_name} {{\n    pub fn values() -> Vec<Self> {{ vec![{values}] }}\n\n    pub fn value_of<T: AsRef<str>>(name: T) -> Self {{\n        match name.as_ref() {{\n{value_of_arms}\n            other => panic!(\"unknown Java enum constant {{}}.{{}}\", {enum_name:?}, other),\n        }}\n    }}\n\n    #[allow(non_snake_case)]\n    pub fn valueOf<T: AsRef<str>>(name: T) -> Self {{ Self::value_of(name) }}\n\n    pub fn name(&self) -> &'static str {{\n        match self {{\n{name_arms}\n        }}\n    }}\n\n    pub fn ordinal(&self) -> i32 {{\n        match self {{\n{ordinal_arms}\n        }}\n    }}\n\n    pub fn compare_to<B: std::borrow::Borrow<Self>>(&self, other: B) -> i32 {{\n        let other = other.borrow();\n        if self.ordinal() < other.ordinal() {{ -1 }} else if self.ordinal() > other.ordinal() {{ 1 }} else {{ 0 }}\n    }}\n{payload_accessors}\n}}\n\nimpl std::fmt::Display for {rust_enum_name} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        write!(f, \"{{}}\", self.name())\n    }}\n}}"
            );
            RepresentationTypeEmission::RawRustItem(format!(
                "{enum_item}\n\nimpl JavaAppendValue for {rust_enum_name} {{\n    fn into_java_append_string(self) -> String {{ self.name().to_string() }}\n}}\n\nimpl JavaAppendValue for &{rust_enum_name} {{\n    fn into_java_append_string(self) -> String {{ self.name().to_string() }}\n}}\n\nimpl JavaEnum for {rust_enum_name} {{\n    fn values() -> Vec<Self> {{ {rust_enum_name}::values() }}\n    fn ordinal(&self) -> i32 {{ {rust_enum_name}::ordinal(self) }}\n    fn name(&self) -> &'static str {{ {rust_enum_name}::name(self) }}\n}}"
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
    exceptions: &ProgramExceptionPlan,
    support_program: &Program,
) -> Option<String> {
    let representation = representations.type_representation(&ty.name)?;
    let ObjectRepresentation::SourceHierarchyCarrier {
        carrier_name,
        variants,
    } = representation.representation()
    else {
        return None;
    };
    let virtual_dispatch_trait = representations
        .anonymous_class_base_requires_virtual_carrier(&ty.name)
        .then(|| source_hierarchy_virtual_dispatch_trait_name(&ty.name));
    Some(source_hierarchy_carrier_item(
        carrier_name,
        variants,
        representations,
        facts,
        signatures,
        exceptions,
        ty,
        support_program,
        virtual_dispatch_trait.as_deref(),
    ))
}

/// The object-safe surface an anonymous subclass uses when it occupies a
/// source-class carrier.  It is emitted independently of the carrier enum so
/// the eventual anonymous payload and ordinary named variants have one ABI.
pub(crate) fn source_hierarchy_virtual_dispatch_trait_item_for_type(
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    support_program: &Program,
) -> Option<String> {
    if ty.kind != TypeKind::Class
        || !representations.anonymous_class_base_requires_virtual_carrier(&ty.name)
    {
        return None;
    }
    let trait_name = source_hierarchy_virtual_dispatch_trait_name(&ty.name);
    let carrier_variants = representations
        .type_representation(&ty.name)
        .and_then(|representation| match representation.representation() {
            ObjectRepresentation::SourceHierarchyCarrier { variants, .. } => {
                Some(variants.as_slice())
            }
            _ => None,
        })
        .unwrap_or(&[]);
    let methods = ty
        .members
        .iter()
        .filter_map(|member| {
            let Member::Method(method) = member else {
                return None;
            };
            source_hierarchy_virtual_dispatch_method_signature(
                ty,
                method,
                carrier_variants,
                support_program,
                signatures,
                facts,
                representations,
                exceptions.method_plan(ty, method),
            )
        })
        .chain(
            crate::inherited_members::inherited_instance_method_refs_for_type(ty, support_program)
                .into_iter()
                .filter_map(|inherited| {
                    source_hierarchy_virtual_dispatch_method_signature(
                        ty,
                        inherited.method(),
                        carrier_variants,
                        support_program,
                        signatures,
                        facts,
                        representations,
                        exceptions.method_plan(inherited.declaring_ty(), inherited.method()),
                    )
                }),
        )
        .collect::<Vec<_>>();
    let field_getters = ty
        .members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            (!field.modifiers.contains(&Modifier::Static)).then(|| {
                let getter = source_hierarchy_virtual_dispatch_field_getter_name(&field.name);
                let return_ty = emit_type(&rust_type_for_java_type_with_facts(&field.ty, facts));
                format!("    fn {getter}(&self) -> {return_ty};")
            })
        })
        .collect::<Vec<_>>();
    let clone_trait_name = format!("{trait_name}Clone");
    let mut items = methods;
    items.extend(field_getters);
    Some(format!(
        "pub trait {clone_trait_name} {{\n    fn clone_box(&self) -> Box<dyn {trait_name}>;\n}}\n\nimpl<T> {clone_trait_name} for T\nwhere\n    T: {trait_name} + Clone + 'static,\n{{\n    fn clone_box(&self) -> Box<dyn {trait_name}> {{ Box::new(std::clone::Clone::clone(self)) }}\n}}\n\nimpl Clone for Box<dyn {trait_name}> {{\n    fn clone(&self) -> Self {{ self.clone_box() }}\n}}\n\npub trait {trait_name}: {clone_trait_name} {{\n{}\n}}",
        items.join("\n"),
    ))
}

pub(crate) fn source_hierarchy_virtual_dispatch_trait_name(type_name: &str) -> String {
    format!(
        "{}AnonymousDispatch",
        rust_name_for_java_named_type(type_name)
    )
}

pub(crate) fn source_hierarchy_virtual_dispatch_field_getter_name(field_name: &str) -> String {
    format!("__java_field_{}", crate::parser::to_snake_case(field_name))
}

pub(crate) fn source_hierarchy_carrier_field_setter_name(field_name: &str) -> String {
    format!(
        "__java_set_field_{}",
        crate::parser::to_snake_case(field_name)
    )
}

fn source_hierarchy_virtual_dispatch_method_signature(
    owner: &TypeDecl,
    method: &MethodDecl,
    variants: &[String],
    support_program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
    exceptions: MethodExceptionPlan,
) -> Option<String> {
    if method.modifiers.contains(&Modifier::Static) || !method.type_params.is_empty() {
        return None;
    }
    let rust_name = rust_identifier_token(&crate::java_signature::source_method_rust_name(method));
    let params = method
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let name = rust_identifier_token(&crate::parser::to_snake_case(&param.name));
            let ty = source_hierarchy_virtual_param_type(
                owner,
                method,
                index,
                variants,
                support_program,
                signatures,
                facts,
                representations,
            );
            format!(", {name}: {ty}")
        })
        .collect::<String>();
    let return_ty = source_hierarchy_virtual_return_type(
        owner,
        method,
        signatures,
        facts,
        representations,
        exceptions,
    );
    Some(format!("    fn {rust_name}(&self{params}) -> {return_ty};"))
}

/// Borrowed source parameters must use the solved source-method ABI at the
/// virtual carrier boundary. Primitive arrays may be `&[T]` or `&mut [T]`,
/// while ordinary Java strings are often `&str` rather than owned `String`.
pub(crate) fn source_hierarchy_virtual_param_type(
    owner: &TypeDecl,
    method: &MethodDecl,
    index: usize,
    variants: &[String],
    support_program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> String {
    if let Some(shape) = source_hierarchy_virtual_param_shape(
        owner,
        method,
        index,
        variants,
        support_program,
        signatures,
        representations,
    ) {
        return emit_type(
            &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                &shape,
                facts,
                representations,
            ),
        );
    }
    let ty = &method.params[index].ty;
    if let JavaType::Array(element) = ty
        && matches!(element.as_ref(), JavaType::Primitive(_))
    {
        return format!(
            "&[{}]",
            emit_type(&rust_type_for_java_type_with_facts(element, facts))
        );
    }
    emit_type(&rust_type_for_java_type_with_facts(ty, facts))
}

fn source_hierarchy_virtual_param_shape(
    owner: &TypeDecl,
    method: &MethodDecl,
    index: usize,
    variants: &[String],
    support_program: &Program,
    signatures: &SignaturePlan,
    representations: &RepresentationPlan,
) -> Option<RustSlotShape> {
    // The carrier is the virtual-call boundary for every concrete payload.
    // Prefer the payload ABI when all implementations agree. This is more
    // authoritative than a separately solved declaration ABI (which can be
    // more nullable or borrowed for an abstract/inherited declaration), and
    // avoids impossible conversions such as `Option<&T>` -> `Rc<T>` in a
    // dispatch arm. If implementations disagree, retain the declaration ABI.
    // A package summary can contain a qualified copy of the current source
    // declaration. The carrier may be discovered through that copy, while
    // its payload is the declaration actually emitted in this module. Derive
    // the boundary from that payload contract first so both sides consult the
    // same signature-plan entry.
    if let Some((payload_owner, payload_method)) = variants.first().and_then(|variant| {
        emitted_carrier_payload_type(support_program, variant).and_then(|payload_owner| {
            implementing_method_candidates_in_hierarchy(payload_owner, method, support_program)
                .into_iter()
                .find(|(_, candidate)| {
                    candidate.name == method.name
                        && crate::java_signature::erased_param_types(&candidate.params)
                            == crate::java_signature::erased_param_types(&method.params)
                })
        })
    }) {
        let forwarding_owner = trait_impl_forwarding_owner_for_inherited_source_method(
            emitted_carrier_payload_type(support_program, &variants[0])
                .expect("carrier payload resolved above"),
            payload_owner,
            payload_method,
        );
        if let Some(param) = signatures
            .method_signature(forwarding_owner, payload_method)
            .or_else(|| signatures.method_signature(payload_owner, payload_method))
            .and_then(|signature| signature.params().get(index))
        {
            let shape = crate::lower::emitted_source_method_param_shape(
                payload_method,
                &payload_method.params[index],
                &forwarding_owner.name,
                param.shape(),
                representations,
            );
            return Some(shape);
        }
    }

    let implementation_shapes = variants
        .iter()
        .filter_map(|variant| {
            let variant_ty =
                find_type_decl_for_owner_context(support_program, &owner.name, variant)?;
            let (implementation_owner, implementation_method) =
                implementing_method_candidates_in_hierarchy(variant_ty, method, support_program)
                    .into_iter()
                    .find(|(_, candidate)| {
                        trait_impl_method_compatibility_error(method, candidate).is_none()
                    })?;
            // Inherited concrete methods are emitted on the payload type, so
            // query the same forwarding owner used by trait emission. Looking
            // only at the declaring owner can select its independently solved
            // ABI rather than the ABI the match arm actually invokes.
            let forwarding_owner = trait_impl_forwarding_owner_for_inherited_source_method(
                variant_ty,
                implementation_owner,
                implementation_method,
            );
            let signature = signatures
                .method_signature(forwarding_owner, implementation_method)
                .or_else(|| {
                    signatures.method_signature(implementation_owner, implementation_method)
                })?;
            signature.params().get(index).map(|param| {
                crate::lower::emitted_source_method_param_shape(
                    implementation_method,
                    &implementation_method.params[index],
                    &forwarding_owner.name,
                    param.shape(),
                    representations,
                )
            })
        })
        .collect::<Vec<_>>();
    if let Some(first) = implementation_shapes.first()
        && implementation_shapes.iter().all(|shape| shape == first)
    {
        return Some(first.clone());
    }
    if let Some(plan) = signatures.method_param_plans(owner, method)
        && let Some(param) = plan.get(index)
    {
        return Some(param.shape().clone());
    }
    None
}

/// Locate the declaration whose Rust name is the carrier payload name.
///
/// When both the active compilation unit and a package-qualified summary copy
/// are present they intentionally erase to the same Rust identifier. The
/// active declaration has the shortest source identity (`Outer.Inner` rather
/// than `pkg.Outer.Inner`), which is also the identity used by ordinary item
/// emission in the focused-package pipeline.
fn emitted_carrier_payload_type<'a>(program: &'a Program, variant: &str) -> Option<&'a TypeDecl> {
    emitted_carrier_payload_types(program, variant)
        .into_iter()
        .min_by_key(|ty| {
            (
                ty.name.matches('.').count(),
                ty.name.len(),
                ty.name.as_str(),
            )
        })
}

/// All source declarations that erase to one carrier payload.
///
/// Focused-package translation intentionally carries both the declaration
/// being emitted and package-qualified analysis copies.  Representation and
/// field layout use the emitted declaration selected above, but callable
/// contracts must be allowed to consult every copy: a compact emitted summary
/// can omit a generic bound that is retained by the package declaration.
fn emitted_carrier_payload_types<'a>(program: &'a Program, variant: &str) -> Vec<&'a TypeDecl> {
    fn collect<'a>(ty: &'a TypeDecl, variant: &str, matches: &mut Vec<&'a TypeDecl>) {
        if crate::item_assembly::rust_use_name_for_source_type(ty) == variant {
            matches.push(ty);
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                collect(nested, variant, matches);
            }
        }
    }

    let mut matches = Vec::new();
    for ty in &program.types {
        collect(ty, variant, &mut matches);
    }
    matches
}

pub(crate) fn source_hierarchy_virtual_return_type(
    owner: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
    exceptions: MethodExceptionPlan,
) -> String {
    let ty = signatures
        .method_return_shape(owner, method)
        .map(|shape| {
            rust_type_for_slot_with_facts_and_representations(&shape, facts, representations)
        })
        .unwrap_or_else(|| {
            rust_type_for_java_type_with_facts_and_representations(
                &method.return_type,
                facts,
                representations,
            )
        });
    emit_type(&if exceptions.wrap_returns_in_result() {
        exception_result_type(ty)
    } else {
        ty
    })
}

fn closed_enum_carrier_item(enum_name: &str, variants: &[String], facts: &FactSet) -> String {
    carrier_enum_item("#[derive(Clone, PartialEq)]", enum_name, variants, facts)
}

fn closed_enum_carrier_item_for_program(
    enum_name: &str,
    variants: &[String],
    facts: &FactSet,
    program: &Program,
    owner: &TypeDecl,
) -> String {
    let variants = variants
        .iter()
        .map(|variant| {
            let emitted_name = emitted_carrier_variant_payload_name(program, &owner.name, variant);
            let placement = facts.storage_placement_for(variant);
            let payload = if placement.shared() && placement.is_implemented() {
                placement.wrap(emitted_name)
            } else {
                emitted_name
            };
            format!("    {variant}({payload}),")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("#[derive(Clone, PartialEq)]\npub enum {enum_name} {{\n{variants}\n}}")
}

fn source_hierarchy_carrier_item(
    enum_name: &str,
    variants: &[String],
    representations: &RepresentationPlan,
    facts: &FactSet,
    signatures: &SignaturePlan,
    exceptions: &ProgramExceptionPlan,
    ty: &TypeDecl,
    support_program: &Program,
    virtual_dispatch_trait: Option<&str>,
) -> String {
    let anonymous_sync =
        virtual_dispatch_trait.is_some() && representations.storage_placement_for(&ty.name).sync();
    let mut item = source_hierarchy_carrier_enum_item(
        enum_name,
        variants,
        representations,
        ty,
        support_program,
        virtual_dispatch_trait,
        anonymous_sync,
    );
    if let Some(default_impl) = source_hierarchy_carrier_placeholder_default_impl(
        enum_name,
        variants,
        representations,
        ty,
        support_program,
    ) {
        item.push_str("\n\n");
        item.push_str(&default_impl);
    }
    item.push_str("\n\n");
    item.push_str(&source_hierarchy_carrier_display_impl(
        enum_name,
        variants,
        representations,
        ty,
        support_program,
        virtual_dispatch_trait.is_some(),
        anonymous_sync,
    ));
    let mut impl_members = Vec::new();
    impl_members.extend(source_hierarchy_carrier_field_accessors(
        variants,
        representations,
        facts,
        signatures,
        ty,
        support_program,
        virtual_dispatch_trait.is_some(),
        anonymous_sync,
    ));
    impl_members.extend(source_hierarchy_carrier_forwarding_methods(
        enum_name,
        variants,
        representations,
        facts,
        signatures,
        exceptions,
        ty,
        support_program,
        virtual_dispatch_trait,
        anonymous_sync,
    ));
    impl_members.push(source_hierarchy_carrier_identity_method(
        variants,
        representations,
        virtual_dispatch_trait.is_some(),
        anonymous_sync,
    ));
    if virtual_dispatch_trait.is_none() {
        impl_members.push(source_hierarchy_carrier_runtime_class_method(
            variants,
            support_program,
        ));
    }
    if !impl_members.is_empty() {
        item.push_str("\n\n");
        let impl_target =
            source_hierarchy_carrier_impl_target(enum_name, ty, support_program, representations);
        item.push_str(&format!(
            "{impl_target} {{\n{}\n}}",
            impl_members.join("\n\n")
        ));
    }
    item.push_str("\n\n");
    item.push_str(&source_hierarchy_carrier_java_map_key_impl(
        enum_name,
        variants,
        facts,
        exceptions,
        signatures,
        ty,
        support_program,
        representations,
        virtual_dispatch_trait.is_some(),
    ));
    if let Some(tree_key_impl) = source_hierarchy_carrier_java_tree_key_impl(
        enum_name,
        variants,
        facts,
        exceptions,
        signatures,
        ty,
        support_program,
        representations,
        virtual_dispatch_trait.is_some(),
    ) {
        item.push_str("\n\n");
        item.push_str(&tree_key_impl);
    }
    item.push_str("\n\n");
    item.push_str(&source_hierarchy_carrier_java_append_value_impls(
        enum_name,
        ty,
        support_program,
        representations,
    ));
    for bound_impl in source_hierarchy_carrier_class_bound_trait_impls(
        enum_name,
        ty,
        support_program,
        signatures,
        facts,
        exceptions,
        representations,
    ) {
        item.push_str("\n\n");
        item.push_str(&bound_impl);
    }
    item
}

fn source_hierarchy_carrier_java_append_value_impls(
    enum_name: &str,
    owner: &TypeDecl,
    program: &Program,
    representations: &RepresentationPlan,
) -> String {
    let impl_prefix = source_hierarchy_carrier_impl_prefix(owner, program, representations);
    let use_name = source_hierarchy_carrier_use_name(enum_name, owner);
    format!(
        "{impl_prefix} JavaAppendValue for {use_name} {{\n    fn into_java_append_string(self) -> String {{ format!(\"{{}}\", self) }}\n}}\n\n{impl_prefix} JavaAppendValue for &{use_name} {{\n    fn into_java_append_string(self) -> String {{ format!(\"{{}}\", self) }}\n}}"
    )
}

/// A source hierarchy carrier is the canonical value representation of its
/// Java root, so it must satisfy the same generated class-bound traits as the
/// concrete payload handles it contains. Generic source methods such as
/// `<T extends Base> void add(Map<K, T>, T)` otherwise reject a `BaseValue`
/// even though every variant is a valid `Base` and the carrier already owns
/// the hierarchy-wide dispatch surface.
fn source_hierarchy_carrier_class_bound_trait_impls(
    enum_name: &str,
    owner: &TypeDecl,
    program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    representations: &RepresentationPlan,
) -> Vec<String> {
    source_class_bounds_for_type(program, owner)
        .into_iter()
        .map(|bound| {
            let trait_name = source_class_bound_trait_name(&bound.name);
            let trait_path = parameterized_interface_trait_path(&trait_name, &bound.type_params);
            let impl_prefix = source_hierarchy_carrier_impl_prefix(owner, program, representations);
            let use_name = source_hierarchy_carrier_use_name(enum_name, owner);
            let methods = bound
                .members
                .iter()
                .filter_map(|member| {
                    let Member::Method(method) = member else {
                        return None;
                    };
                    if method.modifiers.contains(&Modifier::Static)
                        || method.modifiers.contains(&Modifier::Private)
                    {
                        return None;
                    }
                    let candidates = source_class_bound_trait_method_implementation_candidates(
                        program, bound, method,
                    );
                    let return_shape = source_class_bound_trait_method_return_shape_plan(
                        bound,
                        method,
                        candidates.as_slice(),
                        signatures,
                    );
                    let signature = source_class_bound_trait_method_signature_with_return_shape(
                        bound,
                        method,
                        program,
                        signatures,
                        facts,
                        exceptions.method_plan(bound, method),
                        return_shape.clone(),
                        representations,
                    );
                    let rust_name = rust_identifier_token(
                        &crate::java_signature::source_method_rust_name(method),
                    );
                    let args = signatures
                        .required_method_signature(bound, method)
                        .params()
                        .iter()
                        .map(|param| rust_identifier_token(param.name()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let call = if args.is_empty() {
                        format!("{use_name}::{rust_name}(self)")
                    } else {
                        format!("{use_name}::{rust_name}(self, {args})")
                    };
                    let body = if source_class_bound_trait_method_return_type_with_shape(
                        method,
                        facts,
                        exceptions.method_plan(bound, method),
                        return_shape.as_ref(),
                        representations,
                    )
                    .is_some()
                    {
                        call
                    } else {
                        format!("{call};")
                    };
                    Some(format!("{} {{ {body} }}", signature.trim_end_matches(';')))
                })
                .collect::<Vec<_>>();
            format!(
                "{impl_prefix} {trait_path} for {use_name} {{\n{}\n}}",
                methods.join("\n")
            )
        })
        .collect()
}

/// Java hash/set equality belongs to the represented Java object, not to the
/// Rust enum discriminant or payload struct.  Give hierarchy carriers the
/// same `JavaMapKey` contract used by semantic maps and sets.  Identity-only
/// hierarchies can use the carrier's pointer comparison directly; a hierarchy
/// whose root overrides `equals(Object)` dispatches that generated method and
/// presents the other carrier's concrete payload as the erased Java object.
fn source_hierarchy_carrier_java_map_key_impl(
    enum_name: &str,
    variants: &[String],
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    signatures: &SignaturePlan,
    owner: &TypeDecl,
    support_program: &Program,
    representations: &RepresentationPlan,
    has_anonymous_variant: bool,
) -> String {
    let impl_prefix = source_hierarchy_carrier_impl_prefix(owner, support_program, representations);
    let use_name = source_hierarchy_carrier_use_name(enum_name, owner);
    let custom_equals = facts.source_equals_override_for(&owner.name).and_then(|_| {
        owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "equals" && method.params.len() == 1 => {
                    Some((owner, method))
                }
                _ => None,
            })
            .or_else(|| {
                crate::inherited_members::inherited_instance_method_refs_for_type(
                    owner,
                    support_program,
                )
                .into_iter()
                .find(|inherited| {
                    inherited.method().name == "equals" && inherited.method().params.len() == 1
                })
                .map(|inherited| (inherited.declaring_ty(), inherited.method()))
            })
    });
    let body = if let Some((declaring_ty, equals_method)) = custom_equals {
        let mut arms = variants
            .iter()
            .map(|variant| {
                format!(
                    "            Self::{variant}(value) => Box::new(std::clone::Clone::clone(value)) as Box<dyn std::any::Any>,"
                )
            })
            .collect::<Vec<_>>();
        if has_anonymous_variant {
            arms.push(
                "            Self::Anonymous(value) => Box::new(std::clone::Clone::clone(value)) as Box<dyn std::any::Any>,"
                    .to_string(),
            );
        }
        let rust_name = rust_identifier_token(&crate::java_signature::source_method_rust_name(
            equals_method,
        ));
        let erased_other = format!("match other {{\n{}\n        }}", arms.join("\n"));
        let accepts_nullable_object = signatures
            .method_signature(declaring_ty, equals_method)
            .and_then(|signature| signature.params().first())
            .is_some_and(|param| {
                crate::types::optional_element_type(&param.shape().java_ty).is_some()
            });
        let argument = if accepts_nullable_object {
            format!("Some({erased_other})")
        } else {
            erased_other
        };
        let call = format!("self.{rust_name}({argument})");
        if exceptions
            .method_plan(declaring_ty, equals_method)
            .wrap_returns_in_result()
        {
            format!("{call}.expect(\"Java equals threw while evaluating a hash/set key\")")
        } else {
            call
        }
    } else {
        "self.java_identity_eq(other)".to_string()
    };
    format!(
        "{impl_prefix} JavaMapKey for {use_name} {{\n    fn java_map_key_eq(&self, other: &Self) -> bool {{\n        {body}\n    }}\n}}"
    )
}

/// Natural-order sorted collections must dispatch the Java hierarchy's
/// `compareTo`, rather than deriving Rust `Ord` from the carrier discriminant
/// and payload layout.  The other carrier is re-erased to the same Java
/// object boundary that an ordinary translated compareTo call receives.
fn source_hierarchy_carrier_java_tree_key_impl(
    enum_name: &str,
    variants: &[String],
    _facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    signatures: &SignaturePlan,
    owner: &TypeDecl,
    support_program: &Program,
    representations: &RepresentationPlan,
    has_anonymous_variant: bool,
) -> Option<String> {
    let direct_compare = owner.members.iter().find_map(|member| match member {
        Member::Method(method)
            if method.name == "compareTo"
                && method.params.len() == 1
                && method.return_type
                    == JavaType::Primitive(crate::java_ir::PrimitiveType::Int) =>
        {
            Some((owner, method))
        }
        _ => None,
    });
    // Most hierarchy carriers are not natural-order keys.  Walking inherited
    // members for every carrier is both unnecessary and very expensive in a
    // focused batch, where the same support program is emitted once per
    // source file.  Only inspect inherited methods when the declared Java
    // hierarchy actually implements Comparable. A directly declared
    // compareTo remains authoritative even for imperfect/partial source.
    let inherits_comparable = direct_compare.is_none()
        && implemented_interfaces_for_type(support_program, owner)
            .iter()
            .any(|interface| {
                crate::facts::external_type_name_matches(interface, "Comparable")
                    || crate::facts::external_type_name_matches(interface, "java.lang.Comparable")
            });
    if direct_compare.is_none() && !inherits_comparable {
        return None;
    }
    let (declaring_ty, compare_method) = direct_compare.or_else(|| {
        crate::inherited_members::inherited_instance_method_refs_for_type(owner, support_program)
            .into_iter()
            .find(|inherited| {
                let method = inherited.method();
                method.name == "compareTo"
                    && method.params.len() == 1
                    && method.return_type == JavaType::Primitive(crate::java_ir::PrimitiveType::Int)
            })
            .map(|inherited| (inherited.declaring_ty(), inherited.method()))
    })?;

    let mut arms = variants
        .iter()
        .map(|variant| {
            format!(
                "            Self::{variant}(value) => Box::new(std::clone::Clone::clone(value)) as Box<dyn std::any::Any>,"
            )
        })
        .collect::<Vec<_>>();
    if has_anonymous_variant {
        arms.push(
            "            Self::Anonymous(value) => Box::new(std::clone::Clone::clone(value)) as Box<dyn std::any::Any>,"
                .to_string(),
        );
    }
    let erased_other = format!("match other {{\n{}\n        }}", arms.join("\n"));
    let parameter_shape = signatures
        .method_signature(declaring_ty, compare_method)
        .and_then(|signature| signature.params().first())
        .map(|param| param.shape());
    let accepts_hierarchy_carrier = parameter_shape.is_some_and(|shape| {
        matches!(
            shape.dispatch,
            crate::slot_shape::DispatchShape::ClosedEnum { .. }
        ) || matches!(
            representations.representation_for(&shape.java_ty),
            ObjectRepresentation::SourceHierarchyCarrier { .. }
        )
    });
    let parameter_is_optional = parameter_shape
        .is_some_and(|shape| crate::types::optional_element_type(&shape.java_ty).is_some());
    let value = if accepts_hierarchy_carrier {
        "std::clone::Clone::clone(other)".to_string()
    } else {
        erased_other
    };
    let argument = if parameter_is_optional {
        format!("Some({value})")
    } else {
        value
    };
    let rust_name = rust_identifier_token(&crate::java_signature::source_method_rust_name(
        compare_method,
    ));
    let call = format!("self.{rust_name}({argument})");
    let comparison = if exceptions
        .method_plan(declaring_ty, compare_method)
        .wrap_returns_in_result()
    {
        format!("{call}.expect(\"Java compareTo threw while ordering a TreeSet key\")")
    } else {
        call
    };
    let impl_prefix = source_hierarchy_carrier_impl_prefix(owner, support_program, representations);
    let use_name = source_hierarchy_carrier_use_name(enum_name, owner);
    Some(format!(
        "{impl_prefix} JavaTreeKey for {use_name} {{\n    fn java_tree_cmp(&self, other: &Self) -> std::cmp::Ordering {{\n        {comparison}.cmp(&0)\n    }}\n}}"
    ))
}

fn source_hierarchy_carrier_display_impl(
    enum_name: &str,
    variants: &[String],
    representations: &RepresentationPlan,
    owner: &TypeDecl,
    support_program: &Program,
    has_anonymous_variant: bool,
    anonymous_sync: bool,
) -> String {
    let mut arms = variants
        .iter()
        .map(|variant| {
            let placement = storage_placement_for_emitted_type_name(representations, variant);
            let value = if placement.shared() && placement.interior_mutable() {
                if placement.sync() {
                    "&*value.lock().unwrap()"
                } else {
                    "&*value.borrow()"
                }
            } else if placement.shared() {
                "&**value"
            } else {
                "value"
            };
            format!("            Self::{variant}(value) => std::fmt::Display::fmt({value}, f),")
        })
        .collect::<Vec<_>>();
    if has_anonymous_variant {
        let value = if anonymous_sync {
            "&*value.lock().unwrap()"
        } else {
            "&**value"
        };
        arms.push(format!(
            "            Self::Anonymous(value) => std::fmt::Display::fmt({value}, f),"
        ));
    }
    let mut params = crate::item_assembly::rust_impl_params_for_source_type(
        owner,
        support_program,
        Some(representations),
    );
    for (index, _type_param) in owner.type_params.iter().enumerate() {
        if let Some(param) = params.get_mut(index)
            && !param.split('+').any(|bound| bound.trim() == "Clone")
        {
            param.push_str(" + Clone");
        }
    }
    let impl_prefix = if params.is_empty() {
        "impl".to_string()
    } else {
        format!("impl<{}>", params.join(", "))
    };
    let use_name = source_hierarchy_carrier_use_name(enum_name, owner);
    format!(
        "{impl_prefix} std::fmt::Display for {use_name} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        match self {{\n{}\n        }}\n    }}\n}}",
        arms.join("\n")
    )
}

fn source_hierarchy_carrier_placeholder_default_impl(
    enum_name: &str,
    variants: &[String],
    representations: &RepresentationPlan,
    ty: &TypeDecl,
    support_program: &Program,
) -> Option<String> {
    if !crate::item_assembly::program_needs_constructor_placeholder_default_for_type(
        support_program,
        ty,
    ) {
        return None;
    }
    let owner_variant = rust_name_for_java_named_type(&ty.name);
    if !variants.iter().any(|variant| variant == &owner_variant) {
        return None;
    }
    let placement = representations.storage_placement_for(&ty.name);
    let payload = if placement.shared() && placement.is_implemented() {
        crate::emit::emit_expr(&crate::slot_render::storage_placement_construction_expr(
            placement,
            RustExpr::raw_leaf("Default::default()"),
        ))
    } else {
        "Default::default()".to_string()
    };
    let impl_prefix = source_hierarchy_carrier_impl_prefix(ty, support_program, representations);
    Some(format!(
        "{impl_prefix} Default for {} {{\n    fn default() -> Self {{ Self::{owner_variant}({payload}) }}\n}}",
        source_hierarchy_carrier_use_name(enum_name, ty),
    ))
}

fn source_hierarchy_carrier_use_name(enum_name: &str, owner: &TypeDecl) -> String {
    if owner.type_params.is_empty() {
        enum_name.to_string()
    } else {
        format!("{enum_name}<{}>", owner.type_params.join(", "))
    }
}

fn source_hierarchy_carrier_impl_target(
    enum_name: &str,
    owner: &TypeDecl,
    support_program: &Program,
    representations: &RepresentationPlan,
) -> String {
    let use_name = source_hierarchy_carrier_use_name(enum_name, owner);
    format!(
        "{} {use_name}",
        source_hierarchy_carrier_impl_prefix(owner, support_program, representations)
    )
}

fn source_hierarchy_carrier_impl_prefix(
    owner: &TypeDecl,
    support_program: &Program,
    representations: &RepresentationPlan,
) -> String {
    let params = crate::item_assembly::rust_impl_params_for_source_type(
        owner,
        support_program,
        Some(representations),
    );
    if params.is_empty() {
        "impl".to_string()
    } else {
        format!("impl<{}>", params.join(", "))
    }
}

fn java_type_contains_named_type(ty: &JavaType, type_name: &str) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            name == type_name
                || args
                    .iter()
                    .any(|arg| java_type_contains_named_type(arg, type_name))
        }
        JavaType::Array(element) => java_type_contains_named_type(element, type_name),
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
            java_type_contains_named_type(bound, type_name)
        }
        JavaType::Unknown(name) => name == type_name,
        _ => false,
    }
}

fn source_hierarchy_carrier_identity_method(
    variants: &[String],
    representations: &RepresentationPlan,
    has_anonymous_variant: bool,
    anonymous_sync: bool,
) -> String {
    let mut arms = variants
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
        .collect::<Vec<_>>();
    if has_anonymous_variant {
        let handle = if anonymous_sync {
            "std::sync::Arc"
        } else {
            "std::rc::Rc"
        };
        arms.push(format!(
            "            (Self::Anonymous(left), Self::Anonymous(right)) => {handle}::ptr_eq(left, right),"
        ));
    }
    let arms = arms.join("\n");
    format!(
        "    pub fn java_identity_eq(&self, other: &Self) -> bool {{\n        match (self, other) {{\n{arms}\n            _ => false,\n        }}\n    }}"
    )
}

fn source_hierarchy_carrier_runtime_class_method(
    variants: &[String],
    support_program: &Program,
) -> String {
    let arms = variants
        .iter()
        .map(|variant| {
            let java_name = emitted_carrier_payload_types(support_program, variant)
                .first()
                .map(|ty| ty.name.as_str())
                .unwrap_or(variant);
            format!("            Self::{variant}(_) => JavaClass::<()>::literal({java_name:?}),")
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "    pub fn get_class(&self) -> JavaClass<()> {{\n        match self {{\n{arms}\n        }}\n    }}"
    )
}

fn source_hierarchy_carrier_enum_item(
    enum_name: &str,
    variants: &[String],
    representations: &RepresentationPlan,
    owner: &TypeDecl,
    support_program: &Program,
    virtual_dispatch_trait: Option<&str>,
    anonymous_sync: bool,
) -> String {
    let mut variants = variants
        .iter()
        .map(|variant| {
            let payload_type =
                emitted_carrier_variant_payload_name(support_program, &owner.name, variant);
            format!(
                "    {variant}({}),",
                source_hierarchy_carrier_variant_payload_type(
                    variant,
                    &payload_type,
                    representations,
                )
            )
        })
        .collect::<Vec<_>>();
    if let Some(trait_name) = virtual_dispatch_trait {
        let payload = if anonymous_sync {
            format!("std::sync::Arc<std::sync::Mutex<dyn {trait_name} + Send>>")
        } else {
            format!("std::rc::Rc<dyn {trait_name}>")
        };
        variants.push(format!("    Anonymous({payload}),"));
    }
    let variants = variants.join("\n");
    let generic_declaration = if owner.type_params.is_empty() {
        String::new()
    } else {
        format!("<{}>", owner.type_params.join(", "))
    };
    format!("#[derive(Clone)]\npub enum {enum_name}{generic_declaration} {{\n{variants}\n}}")
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
    representations: &RepresentationPlan,
    facts: &FactSet,
    signatures: &SignaturePlan,
    exceptions: &ProgramExceptionPlan,
    ty: &TypeDecl,
    support_program: &Program,
    virtual_dispatch_trait: Option<&str>,
    anonymous_sync: bool,
) -> Vec<String> {
    // Signature solving already includes every declaration in the support
    // program (see `build_signature_plan_from_constraints`).  Re-solving the
    // entire support program here once per emitted carrier made batch emission
    // quadratic in the number of hierarchy roots and, with parallel emission,
    // ran several identical whole-program fixpoints at once.  Reuse the
    // authoritative plan for both primary and support lookups.
    let support_signatures = signatures;
    ty.members
        .iter()
        .filter_map(|member| {
            let Member::Method(method) = member else {
                return None;
            };
            source_hierarchy_carrier_forwarding_method(
                enum_name,
                variants,
                representations,
                facts,
                signatures,
                exceptions.method_plan(ty, method),
                ty,
                method,
                support_program,
                &support_signatures,
                virtual_dispatch_trait,
                anonymous_sync,
            )
        })
        .chain(
            crate::inherited_members::inherited_instance_method_refs_for_type(ty, support_program)
                .into_iter()
                .filter_map(|inherited| {
                    source_hierarchy_carrier_forwarding_method(
                        enum_name,
                        variants,
                        representations,
                        facts,
                        signatures,
                        exceptions.method_plan(inherited.declaring_ty(), inherited.method()),
                        ty,
                        inherited.method(),
                        support_program,
                        &support_signatures,
                        virtual_dispatch_trait,
                        anonymous_sync,
                    )
                }),
        )
        .collect()
}

fn source_hierarchy_carrier_forwarding_method(
    enum_name: &str,
    variants: &[String],
    representations: &RepresentationPlan,
    facts: &FactSet,
    signatures: &SignaturePlan,
    exceptions: MethodExceptionPlan,
    owner: &TypeDecl,
    method: &MethodDecl,
    support_program: &Program,
    support_signatures: &SignaturePlan,
    virtual_dispatch_trait: Option<&str>,
    anonymous_sync: bool,
) -> Option<String> {
    if method.modifiers.contains(&Modifier::Static) || method.modifiers.contains(&Modifier::Private)
    {
        return None;
    }
    // A package summary can carry a bodyless/erased copy of this method while
    // the emitted payload declaration retains its source generic bound. The
    // carrier forwards to the payload, so derive its callable bounds from the
    // most informative matching payload declaration and render them through
    // the same function used by ordinary source methods.
    let same_callable = |candidate: &MethodDecl| {
        candidate.name == method.name
            && candidate.type_params.len() == method.type_params.len()
            && crate::java_signature::erased_param_types(&candidate.params)
                == crate::java_signature::erased_param_types(&method.params)
    };
    let mut carrier_methods = vec![method];
    for payload_owner in variants
        .iter()
        .flat_map(|variant| emitted_carrier_payload_types(support_program, variant))
    {
        carrier_methods.extend(
            payload_owner
                .members
                .iter()
                .filter_map(|member| match member {
                    Member::Method(candidate) if same_callable(candidate) => Some(candidate),
                    _ => None,
                }),
        );
        carrier_methods.extend(
            crate::inherited_members::inherited_instance_method_refs_for_type(
                payload_owner,
                support_program,
            )
            .into_iter()
            .map(|inherited| inherited.method())
            .filter(|candidate| same_callable(candidate)),
        );
    }
    let required_clone_bounds = (0..method.type_params.len())
        .map(|index| {
            carrier_methods.iter().any(|candidate| {
                candidate.type_params.get(index).is_some_and(|type_param| {
                    crate::item_assembly::callable_type_param_needs_clone_bound_with_support_contexts(
                        type_param,
                        &candidate.params,
                        candidate.body.as_ref(),
                        &[support_program],
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    let carrier_method = carrier_methods
        .iter()
        .copied()
        .max_by_key(|candidate| {
            candidate.type_param_bounds.iter().flatten().count()
                + usize::from(candidate.body.is_some())
        })
        .unwrap_or(method);
    // A bodyless/root summary can retain the Java base name while emitted
    // payload declarations carry the overload-qualified solved Rust name.
    // The forwarder invokes payload methods, so its name and every arm must
    // reuse the most informative payload callable identity rather than
    // independently falling back to the root Java spelling.
    let rust_name = rust_identifier_token(&crate::java_signature::source_method_rust_name(
        carrier_method,
    ));
    let type_params =
        crate::item_assembly::function_type_params_with_required_clone_bounds_and_support_contexts(
            Some(&carrier_method.name),
            &carrier_method.type_params,
            &carrier_method.type_param_bounds,
            false,
            &[support_program],
            &carrier_method.params,
            carrier_method.body.as_ref(),
            &required_clone_bounds,
        );
    let type_params = if type_params.is_empty() {
        String::new()
    } else {
        format!("<{}>", type_params.join(", "))
    };
    let return_ty = source_hierarchy_virtual_return_type(
        owner,
        method,
        signatures,
        facts,
        representations,
        exceptions,
    );
    let params = method
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let name = rust_identifier_token(&crate::parser::to_snake_case(&param.name));
            let ty = source_hierarchy_virtual_param_type(
                owner,
                method,
                index,
                variants,
                support_program,
                signatures,
                facts,
                representations,
            );
            (name, ty)
        })
        .collect::<Vec<_>>();
    let parameter_declarations = params
        .iter()
        .map(|(name, ty)| format!(", {name}: {ty}"))
        .collect::<String>();
    let arguments = params
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let carrier_param_shapes = method
        .params
        .iter()
        .enumerate()
        .map(|(index, _)| {
            source_hierarchy_virtual_param_shape(
                owner,
                method,
                index,
                variants,
                support_program,
                signatures,
                representations,
            )
        })
        .collect::<Vec<_>>();
    // The enum payload binding shares the method body scope with Java
    // parameters. A user parameter named `value` used to shadow it and made
    // every forwarding arm pass the payload to itself.
    let mut carrier_binding = "__java_carrier_payload".to_string();
    let mut suffix = 0usize;
    while params.iter().any(|(name, _)| name == &carrier_binding) {
        suffix += 1;
        carrier_binding = format!("__java_carrier_payload_{suffix}");
    }
    let receiver_plan = source_hierarchy_carrier_receiver_plan(
        variants,
        owner,
        method,
        representations,
        support_program,
        signatures,
        support_signatures,
    );
    let mut materialized_param_types = Vec::new();
    let mut arms = variants
        .iter()
        .map(|variant| {
            let variant_ty =
                find_type_decl_for_owner_context(support_program, &owner.name, variant);
            let payload_method = variant_ty.and_then(|variant_ty| {
                implementing_method_candidates_in_hierarchy(variant_ty, method, support_program)
                    .into_iter()
                    // Java override identity excludes the return type. A
                    // concrete payload may therefore override this carrier
                    // slot covariantly; selecting only exact return matches
                    // skips that override and dispatches to the inherited
                    // base implementation.
                    .find(|(candidate_owner, candidate)| {
                        same_callable(candidate)
                            || source_hierarchy_methods_share_specialized_slot(
                                support_program,
                                variant_ty,
                                owner,
                                method,
                                candidate_owner,
                                candidate,
                            )
                    })
            });
            let payload_rust_name = payload_method
                .map(|(_, candidate)| {
                    rust_identifier_token(&crate::java_signature::source_method_rust_name(
                        candidate,
                    ))
                })
                .unwrap_or_else(|| rust_name.clone());
            // A carrier exposes one hierarchy-wide ABI, while an individual
            // payload can legitimately use another solved ownership/view
            // shape. Reuse the same shape adapter as trait forwarding so a
            // slice can be materialized for an owned Vec payload (and owned
            // strings/arrays can be borrowed in the opposite direction).
            let arm_arguments = params
                .iter()
                .enumerate()
                .map(|(index, (name, _))| {
                    let payload_shape = variant_ty.zip(payload_method).and_then(
                        |(variant_ty, (implementation_owner, implementation_method))| {
                            let forwarding_owner =
                                trait_impl_forwarding_owner_for_inherited_source_method(
                                    variant_ty,
                                    implementation_owner,
                                    implementation_method,
                                );
                            signatures
                                .method_signature(forwarding_owner, implementation_method)
                                .or_else(|| {
                                    support_signatures
                                        .method_signature(forwarding_owner, implementation_method)
                                })
                                .or_else(|| {
                                    signatures.method_signature(
                                        implementation_owner,
                                        implementation_method,
                                    )
                                })
                                .or_else(|| {
                                    support_signatures.method_signature(
                                        implementation_owner,
                                        implementation_method,
                                    )
                                })
                                .and_then(|signature| signature.params().get(index))
                                .map(|param| {
                                    crate::lower::emitted_source_method_param_shape(
                                        implementation_method,
                                        &implementation_method.params[index],
                                        &forwarding_owner.name,
                                        param.shape(),
                                        representations,
                                    )
                                })
                        },
                    );
                    carrier_param_shapes[index].as_ref().map_or_else(
                        || name.clone(),
                        |carrier_shape| {
                            if let Some(payload_shape) = payload_shape.as_ref()
                                && let Some(adapted) =
                                    source_hierarchy_specialized_type_param_forwarded_arg(
                                        name,
                                        &method.params[index].ty,
                                        owner,
                                        carrier_shape,
                                        payload_shape,
                                        support_program,
                                        facts,
                                        representations,
                                    )
                            {
                                return adapted;
                            }
                            if payload_shape.as_ref().is_some_and(|payload_shape| {
                                forwarded_arg_requires_owned_view_materialization(
                                    carrier_shape,
                                    payload_shape,
                                )
                            }) {
                                materialized_param_types.push(method.params[index].ty.clone());
                            }
                            trait_impl_forwarded_arg(name, carrier_shape, payload_shape.as_ref())
                        },
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let placement = storage_placement_for_emitted_type_name(representations, variant);
            let call =
                if placement.shared() && placement.interior_mutable() && placement.is_implemented()
                {
                    let companion = format!("{payload_rust_name}_with_outer");
                    let forwarded_arguments = if arm_arguments.is_empty() {
                        format!("std::clone::Clone::clone({carrier_binding})")
                    } else {
                        format!("std::clone::Clone::clone({carrier_binding}), {arm_arguments}")
                    };
                    format!("{variant}::{companion}({forwarded_arguments})")
                } else {
                    let receiver = source_hierarchy_carrier_payload_receiver(
                        &carrier_binding,
                        variant,
                        representations,
                        receiver_plan == ReceiverPlan::Mutable,
                    );
                    if payload_rust_name == "to_string" {
                        // A shared payload such as `Arc<T>` implements Rust's
                        // blanket `ToString`, and hierarchy-bound traits can
                        // expose another method with the same spelling. Java
                        // dispatch has already selected the source method, so
                        // preserve that identity with an inherent UFCS call.
                        // Match the representation actually stored in this
                        // source-hierarchy carrier variant. Its declaration
                        // uses `storage_placement_for_emitted_type_name`, so
                        // forwarding must use the same already-computed
                        // placement rather than a separate facts lookup.
                        let receiver = if placement.shared() && placement.is_implemented() {
                            format!("&**{carrier_binding}")
                        } else {
                            receiver
                        };
                        let arguments = if arm_arguments.is_empty() {
                            receiver
                        } else {
                            format!("{receiver}, {arm_arguments}")
                        };
                        format!("{variant}::{payload_rust_name}({arguments})")
                    } else {
                        format!("{receiver}.{payload_rust_name}({arm_arguments})")
                    }
                };
            let call = adapt_source_hierarchy_carrier_covariant_return(
                call,
                enum_name,
                owner,
                method,
                payload_method.map(|(_, candidate)| candidate),
                support_program,
                facts,
                representations,
                exceptions,
            );
            format!("            Self::{variant}({carrier_binding}) => {call},")
        })
        .collect::<Vec<_>>();
    if virtual_dispatch_trait.is_some() {
        let receiver = if anonymous_sync {
            format!("{carrier_binding}.lock().unwrap()")
        } else {
            carrier_binding.clone()
        };
        arms.push(format!(
            "            Self::Anonymous({carrier_binding}) => {receiver}.{rust_name}({arguments}),"
        ));
    }
    let arms = arms.join("\n");
    let self_receiver = if receiver_plan == ReceiverPlan::Mutable {
        "&mut self"
    } else {
        "&self"
    };
    let mut where_bounds = crate::item_assembly::source_method_class_type_param_where_bounds(
        owner,
        method,
        Some(representations),
        &[support_program],
    );
    for type_param in owner.type_params.iter().chain(method.type_params.iter()) {
        if materialized_param_types
            .iter()
            .any(|ty| java_type_mentions_type_param(ty, type_param))
        {
            let bound = format!("{type_param}: Clone");
            if !where_bounds.contains(&bound) {
                where_bounds.push(bound);
            }
        }
    }
    let where_clause = if where_bounds.is_empty() {
        String::new()
    } else {
        format!(" where {}", where_bounds.join(", "))
    };
    Some(format!(
        "    pub fn {rust_name}{type_params}({self_receiver}{parameter_declarations}) -> {return_ty}{where_clause} {{\n        match self {{\n{arms}\n        }}\n    }}"
    ))
}

/// Recover a concrete override argument from a generic hierarchy-carrier
/// slot.
///
/// Rust enums cannot express the Java invariant that a particular variant of
/// `BaseValue<T>` fixes `T` to that subclass's superclass instantiation.  The
/// generated `From<Child> for BaseValue<Concrete>` impl establishes it at the
/// construction boundary; dispatch reifies the same invariant with an `Any`
/// downcast before invoking the concrete override.
fn source_hierarchy_specialized_type_param_forwarded_arg(
    arg_name: &str,
    declared_ty: &JavaType,
    owner: &TypeDecl,
    carrier_shape: &RustSlotShape,
    payload_shape: &RustSlotShape,
    program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<String> {
    if !owner
        .type_params
        .iter()
        .any(|type_param| java_type_mentions_type_param(declared_ty, type_param))
        || carrier_shape.java_ty == payload_shape.java_ty
        || carrier_shape.ownership != OwnershipShape::Owned
    {
        return None;
    }
    let storage_shape = crate::shape_projection::storage_shape_for_java_type(
        representations,
        payload_shape.java_ty.clone(),
    );
    let storage_ty = emit_type(
        &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
            &storage_shape,
            facts,
            representations,
        ),
    );
    let arg_name = rust_identifier_token(arg_name);
    if matches!(
        payload_shape.ownership,
        OwnershipShape::Owned | OwnershipShape::CopyValue
    ) && let Some(type_param) = owner
        .type_params
        .iter()
        .find(|type_param| java_type_mentions_type_param(declared_ty, type_param))
        && let Some((_adapter, source_arg, trait_path, payload_type)) =
            erased_interface_retarget_for_hierarchy_return(
                owner,
                type_param,
                &payload_shape.java_ty,
                program,
                facts,
                representations,
            )
    {
        debug_assert_eq!(payload_type, storage_ty);
        let source_trait = trait_path.replace("JavaErasedValue", &source_arg);
        return Some(format!(
            "{{ let __java_erased_argument = Box::new({arg_name}) as Box<dyn std::any::Any>; match __java_erased_argument.downcast::<{storage_ty}>() {{ Ok(__java_direct_argument) => *__java_direct_argument, Err(__java_erased_argument) => {{ let __java_interface_argument = *__java_erased_argument.downcast::<Box<dyn {source_trait}>>().expect(\"invalid Java boxed generic hierarchy argument carrier\"); std::clone::Clone::clone(__java_interface_argument.as_ref().as_any().downcast_ref::<{storage_ty}>().expect(\"invalid Java generic hierarchy argument\")) }} }} }}"
        ));
    }
    let downcast = format!(
        "(Box::new({arg_name}) as Box<dyn std::any::Any>).downcast::<{storage_ty}>().expect(\"invalid Java generic hierarchy argument\")"
    );
    Some(match payload_shape.ownership {
        OwnershipShape::Owned | OwnershipShape::CopyValue => format!("*{downcast}"),
        OwnershipShape::SharedBorrow => format!("&*{downcast}"),
        OwnershipShape::MutableBorrow => format!("&mut *{downcast}"),
    })
}

/// Compare two hierarchy methods after viewing both declarations through the
/// concrete payload type.
///
/// Java override identity applies superclass substitutions before erasure. A
/// carrier rooted at `Base<T>::write(T)` must therefore dispatch a
/// `Child extends Base<String>` payload to `Child::write(String)`, including
/// the overload-qualified Rust name selected for that concrete declaration.
fn source_hierarchy_methods_share_specialized_slot(
    program: &Program,
    target: &TypeDecl,
    root_owner: &TypeDecl,
    root_method: &MethodDecl,
    candidate_owner: &TypeDecl,
    candidate_method: &MethodDecl,
) -> bool {
    if root_method.name != candidate_method.name
        || root_method.type_params.len() != candidate_method.type_params.len()
    {
        return false;
    }
    let specialized_slot = |owner: &TypeDecl, method: &MethodDecl| {
        let substitutions =
            crate::inherited_members::source_class_ancestor_type_arguments(program, target, owner)
                .unwrap_or_default()
                .into_iter()
                .filter(|(name, _)| !method.type_params.contains(name))
                .collect::<HashMap<_, _>>();
        let mut specialized = method.clone();
        for param in &mut specialized.params {
            param.ty = crate::resolve::substitute_method_type_variables(&param.ty, &substitutions);
        }
        crate::java_signature::JavaCallableSlotKey::source_method(&specialized)
    };
    specialized_slot(root_owner, root_method) == specialized_slot(candidate_owner, candidate_method)
}

fn adapt_source_hierarchy_carrier_covariant_return(
    call: String,
    enum_name: &str,
    carrier_owner: &TypeDecl,
    carrier_method: &MethodDecl,
    payload_method: Option<&MethodDecl>,
    program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
    exceptions: MethodExceptionPlan,
) -> String {
    let Some(payload_method) = payload_method else {
        return call;
    };
    if let Some(type_param) = nominal_java_type_name(&carrier_method.return_type)
        && carrier_owner
            .type_params
            .iter()
            .any(|name| name == type_param)
        && !java_type_mentions_type_param(&payload_method.return_type, type_param)
    {
        // A concrete hierarchy variant can fix the root class's generic
        // return (`Reader<O>::read`) to a concrete type
        // (`RealReader::read -> ObservationReal`).  The carrier variant is
        // only constructible at that matching instantiation, but Rust cannot
        // express the relationship in the enum definition. Reify the same
        // construction invariant at dispatch through `Any`, just as the
        // argument-side carrier adapter does for specialized parameters.
        let erased_interface_retarget = erased_interface_retarget_for_hierarchy_return(
            carrier_owner,
            type_param,
            &payload_method.return_type,
            program,
            facts,
            representations,
        );
        let type_param = rust_identifier_token(type_param);
        let recover = |value: &str| {
            if let Some((adapter, source_arg, trait_path, payload_type)) =
                erased_interface_retarget.as_ref()
            {
                return format!(
                    "{{ let __java_erased_value = Box::new({value}) as Box<dyn std::any::Any>; match __java_erased_value.downcast::<{type_param}>() {{ Ok(__java_direct_value) => *__java_direct_value, Err(__java_erased_value) => {{ let __java_value = *__java_erased_value.downcast::<{payload_type}>().expect(\"invalid Java generic hierarchy payload return\"); *(Box::new(Box::new({adapter}::<{source_arg}, JavaErasedValue>::new(Box::new(__java_value) as Box<dyn {source_trait}>)) as Box<dyn {trait_path}>) as Box<dyn std::any::Any>).downcast::<{type_param}>().expect(\"invalid Java generic hierarchy return\") }} }} }}",
                    source_trait = trait_path.replace("JavaErasedValue", source_arg),
                );
            }
            format!(
                "*(Box::new({value}) as Box<dyn std::any::Any>).downcast::<{type_param}>().expect(\"invalid Java generic hierarchy return\")"
            )
        };
        return if exceptions.wrap_returns_in_result() {
            format!("{call}.map(|__java_value| {})", recover("__java_value"))
        } else {
            recover(&call)
        };
    }
    let Some(carrier_return_name) = nominal_java_type_name(&carrier_method.return_type) else {
        return call;
    };
    let Some(payload_return_name) = nominal_java_type_name(&payload_method.return_type) else {
        return call;
    };
    if crate::stdlib::canonical_external_receiver_type_key(carrier_return_name)
        == crate::stdlib::canonical_external_receiver_type_key(payload_return_name)
    {
        return call;
    }
    let Some((return_carrier, _)) =
        representations.source_hierarchy_carrier_constructor_parts(carrier_return_name)
    else {
        return call;
    };
    let Some((payload_carrier, payload_variant)) =
        representations.source_hierarchy_carrier_parts_for_variant(payload_return_name)
    else {
        return call;
    };
    if return_carrier != enum_name || payload_carrier != enum_name {
        return call;
    }
    let constructor = format!("{enum_name}::{payload_variant}");
    if exceptions.wrap_returns_in_result() {
        format!("{call}.map({constructor})")
    } else {
        format!("{constructor}({call})")
    }
}

fn erased_interface_retarget_for_hierarchy_return(
    carrier_owner: &TypeDecl,
    carrier_type_param: &str,
    payload_return: &JavaType,
    program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<(String, String, String, String)> {
    let index = carrier_owner
        .type_params
        .iter()
        .position(|name| name == carrier_type_param)?;
    let bound = carrier_owner.type_param_bounds.get(index)?.as_ref()?;
    let JavaType::Named {
        name: interface_name,
        args: bound_args,
    } = bound
    else {
        return None;
    };
    if bound_args.len() != 1
        || !matches!(bound_args[0], JavaType::Wildcard(None))
        || representations
            .erased_wildcard_bound_carrier_for(interface_name, 0)
            .is_none()
    {
        return None;
    }
    let payload_name = nominal_java_type_name(payload_return)?;
    let payload_decl =
        find_type_decl_for_owner_context(program, &carrier_owner.name, payload_name)?;
    let interface = find_type_decl_for_owner_context(program, &carrier_owner.name, interface_name)?;
    if interface.kind != TypeKind::Interface
        || !generic_interface_supports_retarget_adapter(program, interface)
    {
        return None;
    }
    let source_args = interface_type_arguments_in_target(program, payload_decl, interface)?;
    let [source_arg] = source_args.as_slice() else {
        return None;
    };
    let source_arg = emit_type(
        &crate::slot_render::rust_type_for_java_generic_arg_with_representations(
            source_arg,
            &crate::effective_facts::EffectiveFacts::new(facts),
            representations,
        ),
    );
    let payload_shape = crate::shape_projection::storage_shape_for_java_type(
        representations,
        payload_return.clone(),
    );
    let payload_type = emit_type(
        &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
            &payload_shape,
            facts,
            representations,
        ),
    );
    let trait_name = rust_name_for_java_named_type(interface_name);
    Some((
        generic_interface_retarget_adapter_name(interface_name),
        source_arg,
        format!("{trait_name}<JavaErasedValue>"),
        payload_type,
    ))
}

fn nominal_java_type_name(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name.as_str()),
        _ => None,
    }
}

fn java_type_mentions_type_param(ty: &JavaType, type_param: &str) -> bool {
    match ty {
        JavaType::Unknown(name) => name == type_param,
        JavaType::Named { name, args } => {
            name == type_param
                || args
                    .iter()
                    .any(|arg| java_type_mentions_type_param(arg, type_param))
        }
        JavaType::Array(element)
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(element)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(element))) => {
            java_type_mentions_type_param(element, type_param)
        }
        _ => false,
    }
}

fn source_hierarchy_carrier_receiver_plan(
    variants: &[String],
    owner: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    support_program: &Program,
    signatures: &SignaturePlan,
    support_signatures: &SignaturePlan,
) -> ReceiverPlan {
    if source_hierarchy_carrier_uses_only_interior_mutable_handles(
        &JavaType::Named {
            name: owner.name.clone(),
            args: owner
                .type_params
                .iter()
                .cloned()
                .map(JavaType::Unknown)
                .collect(),
        },
        representations,
    ) {
        // The enum is the Java reference value and every payload already
        // carries interior mutability.  Forwarding through `_with_outer`
        // mutates the referenced object, not the enum binding, so requiring
        // `&mut self` would incorrectly turn Java reference mutation into
        // Rust variable mutation.
        return ReceiverPlan::Shared;
    }
    let mut receiver = signatures
        .method_receiver(owner, method)
        .unwrap_or(ReceiverPlan::Shared);
    for variant in variants {
        let Some(variant_ty) =
            find_type_decl_for_owner_context(support_program, &owner.name, variant)
        else {
            continue;
        };
        // A hierarchy lookup can return both the inherited declaration and a
        // concrete override.  Receiver capability is a join across every
        // compatible implementation: selecting the first candidate can keep
        // the carrier at `&self` even though a derived payload needs `&mut
        // self`.
        let variant_needs_mutable =
            implementing_method_candidates_in_hierarchy(variant_ty, method, support_program)
                .into_iter()
                .filter(|(_, candidate)| {
                    trait_impl_method_compatibility_error(method, candidate).is_none()
                })
                .any(|(implementation_owner, implementation_method)| {
                    signatures
                        .method_receiver(implementation_owner, implementation_method)
                        .or_else(|| {
                            support_signatures
                                .method_receiver(implementation_owner, implementation_method)
                        })
                        .is_some_and(|receiver| receiver == ReceiverPlan::Mutable)
                });
        if variant_needs_mutable {
            receiver = ReceiverPlan::Mutable;
            break;
        }
    }
    receiver
}

/// Whether a closed source-hierarchy carrier is entirely made of shared,
/// interior-mutable payload handles.
///
/// Such a carrier is itself a cheap Java-reference value.  Cloning it keeps
/// object identity, and mutating a payload does not require mutable access to
/// the enum binding.  Keep this query canonical so method forwarding and
/// emitted call ABIs cannot make opposite ownership decisions.
pub(crate) fn source_hierarchy_carrier_uses_only_interior_mutable_handles(
    java_ty: &JavaType,
    representations: &RepresentationPlan,
) -> bool {
    let java_ty = crate::types::optional_element_type(java_ty).unwrap_or_else(|| java_ty.clone());
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = &java_ty else {
        return false;
    };
    let Some(entry) = representations.type_representation(name) else {
        return false;
    };
    let ObjectRepresentation::SourceHierarchyCarrier { variants, .. } = entry.representation()
    else {
        return false;
    };
    !variants.is_empty()
        && !representations.anonymous_class_base_requires_virtual_carrier(name)
        && variants.iter().all(|variant| {
            let placement = storage_placement_for_emitted_type_name(representations, variant);
            placement.shared() && placement.interior_mutable() && placement.is_implemented()
        })
}

fn source_hierarchy_carrier_field_accessors(
    variants: &[String],
    representations: &RepresentationPlan,
    facts: &FactSet,
    signatures: &SignaturePlan,
    ty: &TypeDecl,
    support_program: &Program,
    has_anonymous_variant: bool,
    anonymous_sync: bool,
) -> Vec<String> {
    let mut accessors = Vec::new();
    let mut emitted_names = HashSet::new();
    for member in &ty.members {
        let Member::Field(field) = member else {
            continue;
        };
        let rust_name = crate::java_naming::source_instance_field_rust_name(ty, &field.name);
        if emitted_names.insert(rust_name.clone())
            && let Some(accessor) = source_hierarchy_carrier_field_accessor(
                variants,
                representations,
                facts,
                signatures,
                ty,
                field,
                &rust_name,
                support_program,
                has_anonymous_variant,
                anonymous_sync,
            )
        {
            accessors.push(accessor);
        }
    }

    // Carrier payload structs flatten inherited instance storage, so the
    // carrier must expose that same field surface.  Otherwise an inherited
    // method body lowered on a concrete child can cast another Java reference
    // to the hierarchy carrier but then has no way to read the declaring
    // superclass's field.  Use the shared inherited-field naming plan so
    // hidden fields and the struct layout keep one canonical identity.
    for inherited in
        crate::inherited_members::inherited_instance_field_refs_for_type(ty, support_program)
    {
        let rust_name =
            crate::inherited_members::inherited_instance_field_rust_name_for_type_in_programs(
                ty,
                &inherited,
                &[support_program],
            );
        if !emitted_names.insert(rust_name.clone()) {
            continue;
        }
        let Some(declaring_ty) =
            find_type_decl_for_owner_context(support_program, &ty.name, inherited.declaring_type())
        else {
            continue;
        };
        if let Some(accessor) = source_hierarchy_carrier_field_accessor(
            variants,
            representations,
            facts,
            signatures,
            declaring_ty,
            inherited.field(),
            &rust_name,
            support_program,
            has_anonymous_variant,
            anonymous_sync,
        ) {
            accessors.push(accessor);
        }
    }
    accessors
}

fn source_hierarchy_carrier_field_accessor(
    variants: &[String],
    representations: &RepresentationPlan,
    facts: &FactSet,
    signatures: &SignaturePlan,
    ty: &TypeDecl,
    field: &FieldDecl,
    rust_name: &str,
    support_program: &Program,
    has_anonymous_variant: bool,
    anonymous_sync: bool,
) -> Option<String> {
    if field.modifiers.contains(&Modifier::Static) {
        return None;
    }
    let rust_name = rust_identifier_token(rust_name);
    let field_shape = signatures
        .field_shape(&ty.name, &rust_name)
        .cloned()
        .unwrap_or_else(|| signatures.required_declared_field_shape(ty, field));
    let return_ty = emit_type(
        &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
            &field_shape,
            facts,
            representations,
        ),
    );
    let mut arms = variants
        .iter()
        .map(|variant| {
            let receiver =
                source_hierarchy_carrier_payload_receiver("value", variant, representations, false);
            let storage_name =
                rust_identifier_token(&source_hierarchy_carrier_variant_field_storage_name(
                    variant,
                    ty,
                    field,
                    support_program,
                ));
            let projection = if source_hierarchy_carrier_field_accessor_returns_copy(&field.ty) {
                format!("{receiver}.{storage_name}")
            } else {
                format!("std::clone::Clone::clone(&{receiver}.{storage_name})")
            };
            format!("            Self::{variant}(value) => {projection},")
        })
        .collect::<Vec<_>>();
    if has_anonymous_variant {
        let getter = source_hierarchy_virtual_dispatch_field_getter_name(&field.name);
        let receiver = if anonymous_sync {
            "value.lock().unwrap()"
        } else {
            "value"
        };
        arms.push(format!(
            "            Self::Anonymous(value) => {receiver}.{getter}(),"
        ));
    }
    let arms = arms.join("\n");
    let clone_bounds = (!source_hierarchy_carrier_field_accessor_returns_copy(&field.ty))
        .then(|| {
            ty.type_params
                .iter()
                .filter(|type_param| java_type_contains_named_type(&field.ty, type_param))
                .map(|type_param| format!("{type_param}: Clone"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let where_clause = if clone_bounds.is_empty() {
        String::new()
    } else {
        format!(" where {}", clone_bounds.join(", "))
    };
    let getter = format!(
        "    pub fn {rust_name}(&self) -> {return_ty}{where_clause} {{\n        match self {{\n{arms}\n        }}\n    }}"
    );
    let all_variants_have_mutable_storage = variants.iter().all(|variant| {
        let placement = storage_placement_for_emitted_type_name(representations, variant);
        placement.shared() && placement.interior_mutable() && placement.is_implemented()
    });
    if !all_variants_have_mutable_storage || has_anonymous_variant {
        return Some(getter);
    }
    let setter_name = source_hierarchy_carrier_field_setter_name(&field.name);
    let setter_arms = variants
        .iter()
        .map(|variant| {
            let receiver =
                source_hierarchy_carrier_payload_receiver("value", variant, representations, true);
            let storage_name =
                rust_identifier_token(&source_hierarchy_carrier_variant_field_storage_name(
                    variant,
                    ty,
                    field,
                    support_program,
                ));
            format!(
                "            Self::{variant}(value) => {receiver}.{storage_name} = __java_value,"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let setter = format!(
        "    pub fn {setter_name}(&self, __java_value: {return_ty}) {{\n        match self {{\n{setter_arms}\n        }}\n    }}"
    );
    if variants
        .iter()
        .any(|variant| storage_placement_for_emitted_type_name(representations, variant).sync())
    {
        return Some(format!("{getter}\n\n{setter}"));
    }
    let mutable_arms = variants
        .iter()
        .map(|variant| {
            let storage_name = rust_identifier_token(
                &source_hierarchy_carrier_variant_field_storage_name(
                    variant,
                    ty,
                    field,
                    support_program,
                ),
            );
            format!(
                "            Self::{variant}(value) => std::cell::RefMut::map(value.borrow_mut(), |value| &mut value.{storage_name}),"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "{getter}\n\n{setter}\n\n    pub fn {rust_name}_mut(&self) -> std::cell::RefMut<'_, {return_ty}> {{\n        match self {{\n{mutable_arms}\n        }}\n    }}"
    ))
}

fn source_hierarchy_carrier_field_accessor_returns_copy(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Primitive(_))
}

fn source_hierarchy_carrier_payload_receiver(
    binding: &str,
    variant: &str,
    representations: &RepresentationPlan,
    mutable: bool,
) -> String {
    let placement = storage_placement_for_emitted_type_name(representations, variant);
    if !placement.shared() || !placement.interior_mutable() {
        return binding.to_string();
    }
    if placement.sync() {
        format!("{binding}.lock().unwrap()")
    } else if mutable {
        format!("{binding}.borrow_mut()")
    } else {
        format!("{binding}.borrow()")
    }
}

fn source_hierarchy_carrier_variant_field_storage_name(
    variant: &str,
    declaring_ty: &TypeDecl,
    field: &FieldDecl,
    support_program: &Program,
) -> String {
    let Some(variant_ty) =
        find_type_decl_for_owner_context(support_program, &declaring_ty.name, variant)
    else {
        return rust_identifier_token(&crate::parser::to_snake_case(&field.name));
    };
    if variant_ty.name == declaring_ty.name {
        return crate::java_naming::source_instance_field_rust_name(variant_ty, &field.name);
    }
    let inherited_fields = crate::inherited_members::inherited_instance_field_refs_for_type(
        variant_ty,
        support_program,
    );
    let Some(inherited) = inherited_fields.iter().find(|inherited| {
        crate::facts::external_type_name_matches(inherited.declaring_type(), &declaring_ty.name)
            && inherited.field().name == field.name
    }) else {
        return rust_identifier_token(&crate::parser::to_snake_case(&field.name));
    };
    crate::inherited_members::inherited_instance_field_rust_name_for_type_in_programs(
        variant_ty,
        inherited,
        &[support_program],
    )
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
    representation_name: &str,
    emitted_name: &str,
    representations: &RepresentationPlan,
) -> String {
    // Nested source types can reach this boundary under their flattened Rust
    // representation name.  Use the same emitted-name-aware lookup as the
    // carrier's forwarding and identity methods so the declared payload and
    // every operation on it agree about whether it is handle-backed.
    let placement = storage_placement_for_emitted_type_name(representations, representation_name);
    if placement.shared() && placement.is_implemented() {
        placement.wrap(emitted_name.to_string())
    } else {
        emitted_name.to_string()
    }
}

fn emitted_carrier_variant_payload_name(
    program: &Program,
    owner_name: &str,
    representation_name: &str,
) -> String {
    find_type_decl_for_owner_context(program, owner_name, representation_name)
        .map(crate::item_assembly::rust_use_name_for_source_type)
        .unwrap_or_else(|| representation_name.to_string())
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

fn java_enum_payload_accessor_methods(
    ty: &TypeDecl,
    variants: &[String],
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> String {
    let constructor = ty.members.iter().find_map(|member| match member {
        Member::Constructor(constructor) => Some(constructor),
        _ => None,
    });
    let field_initializers = constructor
        .map(java_enum_constructor_field_initializers)
        .unwrap_or_default();
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
    let immutable_accessors = field_initializers
        .into_iter()
        .filter(|(field_name, _)| !source_java_enum_field_uses_state(ty, field_name))
        .filter_map(|(field_name, initializer)| {
            let field_ty = field_types.get(&field_name)?;
            let rust_return_ty = java_enum_payload_accessor_return_type_with_representations(
                field_ty,
                facts,
                representations,
            )?;
            let accessor_name = rust_identifier_token(&crate::parser::to_snake_case(&field_name));
            let arms = variants
                .iter()
                .map(|variant| {
                    let rendered = constants
                        .get(variant)
                        .and_then(|args| {
                            java_enum_payload_initializer_value(
                                &initializer,
                                args,
                                field_ty,
                                facts,
                            )
                        });
                    // Source-object payloads represent Java reference identity;
                    // silently substituting `Default::default()` would create a
                    // different object. Emit this accessor only when every
                    // enum constant has a statically representable initializer.
                    let value = if java_enum_payload_is_general_named_source_reference(field_ty) {
                        rendered?
                    } else {
                        rendered.unwrap_or_else(|| java_enum_payload_default_value(field_ty))
                    };
                    Some(format!("            Self::{variant} => {value},"))
                })
                .collect::<Option<Vec<_>>>()?
                .join("\n");
            Some(format!(
                "\n\n    pub fn {accessor_name}(&self) -> {rust_return_ty} {{\n        match self {{\n{arms}\n        }}\n    }}"
            ))
        })
        .collect::<Vec<_>>()
        .join("");
    let directly_initialized_fields = constructor
        .map(java_enum_constructor_field_initializers)
        .unwrap_or_default()
        .into_iter()
        .map(|(name, _)| name)
        .collect::<HashSet<_>>();
    let evaluated_fields = constructor
        .map(|constructor| java_enum_constructor_constant_field_values(ty, constructor))
        .unwrap_or_default();
    let evaluated_accessors = ty
        .members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            let field_name = crate::parser::to_snake_case(&field.name);
            if field.modifiers.contains(&Modifier::Static)
                || source_java_enum_field_uses_state(ty, &field_name)
                || directly_initialized_fields.contains(&field_name)
            {
                return None;
            }
            let values = variants
                .iter()
                .map(|variant| evaluated_fields.get(variant)?.get(&field_name).cloned())
                .collect::<Option<Vec<_>>>()?;
            let (rust_return_ty, rendered_values) =
                java_enum_constant_accessor_values(&field.ty, &values)?;
            let accessor_name = rust_identifier_token(&field_name);
            let arms = variants
                .iter()
                .zip(rendered_values)
                .map(|(variant, value)| format!("            Self::{variant} => {value},"))
                .collect::<Vec<_>>()
                .join("\n");
            Some(format!(
                "\n\n    pub fn {accessor_name}(&self) -> {rust_return_ty} {{\n        match self {{\n{arms}\n        }}\n    }}"
            ))
        })
        .collect::<Vec<_>>()
        .join("");
    let state_accessors = ty
        .members
        .iter()
        .filter_map(|member| {
            let Member::Field(field) = member else {
                return None;
            };
            let field_name = crate::parser::to_snake_case(&field.name);
            if field.modifiers.contains(&Modifier::Static)
                || !source_java_enum_field_uses_state(ty, &field_name)
                || java_enum_payload_accessor_return_type(&field.ty, facts).is_none()
            {
                return None;
            }
            let shape = signatures.required_field_shape(&ty.name, &field.name);
            let rust_ty = crate::emit::emit_type(
                &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                    &shape,
                    facts,
                    representations,
                ),
            );
            let accessor_name = rust_identifier_token(&field_name);
            let state_name = format!("__java_{}_state", field_name);
            let setter_name = format!("__java_set_{}", field_name);
            Some(format!(
                "\n\n    fn {state_name}() -> &'static std::sync::Mutex<Vec<{rust_ty}>> {{\n        static STATE: std::sync::OnceLock<std::sync::Mutex<Vec<{rust_ty}>>> = std::sync::OnceLock::new();\n        STATE.get_or_init(|| std::sync::Mutex::new(std::iter::repeat_with(Default::default).take({}).collect()))\n    }}\n\n    pub fn {accessor_name}(&self) -> {rust_ty} {{\n        Self::{state_name}().lock().expect(\"Java enum state mutex poisoned\")[self.ordinal() as usize].clone()\n    }}\n\n    pub fn {setter_name}(&self, value: {rust_ty}) {{\n        Self::{state_name}().lock().expect(\"Java enum state mutex poisoned\")[self.ordinal() as usize] = value;\n    }}",
                variants.len()
            ))
        })
        .collect::<Vec<_>>()
        .join("");
    immutable_accessors + &evaluated_accessors + &state_accessors
}

pub(crate) fn source_java_enum_payload_field_emits_accessor(
    ty: &TypeDecl,
    field_name: &str,
) -> bool {
    if ty.kind != TypeKind::Enum {
        return false;
    }
    if source_java_enum_field_uses_state(ty, field_name) {
        return ty.members.iter().any(|member| {
            matches!(member, Member::Field(field)
                if !field.modifiers.contains(&Modifier::Static)
                    && crate::parser::to_snake_case(&field.name) == field_name
                    && java_enum_payload_accessor_return_type(&field.ty, &FactSet::default()).is_some())
        });
    }
    let Some(constructor) = ty.members.iter().find_map(|member| match member {
        Member::Constructor(constructor) => Some(constructor),
        _ => None,
    }) else {
        return false;
    };
    let field_initializers = java_enum_constructor_field_initializers(constructor);
    let initializer = field_initializers
        .iter()
        .find_map(|(name, initializer)| (name == field_name).then_some(initializer));
    let has_initializer = initializer.is_some();
    let constants = java_enum_constant_args(ty);
    let evaluated = java_enum_constructor_constant_field_values(ty, constructor);
    let has_evaluated_initializer = !constants.is_empty()
        && evaluated.len() == constants.len()
        && evaluated
            .values()
            .all(|fields| fields.contains_key(field_name));
    (has_initializer || has_evaluated_initializer)
        && ty.members.iter().any(|member| {
            let Member::Field(field) = member else {
                return false;
            };
            if field.modifiers.contains(&Modifier::Static)
                || crate::parser::to_snake_case(&field.name) != field_name
                || java_enum_payload_accessor_return_type(&field.ty, &FactSet::default()).is_none()
            {
                return false;
            }
            !java_enum_payload_is_general_named_source_reference(&field.ty)
                || initializer.is_some_and(|initializer| {
                    !constants.is_empty()
                        && constants.values().all(|args| {
                            java_enum_payload_initializer_value(
                                initializer,
                                args,
                                &field.ty,
                                &FactSet::default(),
                            )
                            .is_some()
                        })
                })
        })
}

pub(crate) fn source_java_enum_field_uses_state(ty: &TypeDecl, field_name: &str) -> bool {
    ty.kind == TypeKind::Enum
        && ty.members.iter().any(|member| {
            let Member::Method(method) = member else {
                return false;
            };
            method
                .body
                .as_ref()
                .is_some_and(|body| block_assigns_source_enum_field(body, field_name))
        })
}

pub(crate) fn source_java_enum_payload_accessor_is_nullable(
    ty: &TypeDecl,
    field_name: &str,
) -> bool {
    let Some(constructor) = ty.members.iter().find_map(|member| match member {
        Member::Constructor(constructor) => Some(constructor),
        _ => None,
    }) else {
        return false;
    };
    let values = java_enum_constructor_constant_field_values(ty, constructor);
    !values.is_empty()
        && values
            .values()
            .all(|fields| fields.contains_key(field_name))
        && values
            .values()
            .any(|fields| matches!(fields.get(field_name), Some(JavaEnumConstantValue::Null)))
}

fn block_assigns_source_enum_field(block: &Block, field_name: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_assigns_source_enum_field(stmt, field_name))
}

fn stmt_assigns_source_enum_field(stmt: &Stmt, field_name: &str) -> bool {
    match stmt {
        Stmt::Assign { target, .. } => match target {
            Expr::Var(name) => {
                crate::parser::to_snake_case(name.strip_prefix("this.").unwrap_or(name))
                    == field_name
            }
            Expr::FieldAccess(receiver, name) if matches!(receiver.as_ref(), Expr::This) => {
                crate::parser::to_snake_case(name) == field_name
            }
            _ => false,
        },
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_assigns_source_enum_field(then_block, field_name)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_assigns_source_enum_field(block, field_name))
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => block_assigns_source_enum_field(body, field_name),
        Stmt::For { init, body, .. } => {
            init.iter()
                .any(|stmt| stmt_assigns_source_enum_field(stmt, field_name))
                || block_assigns_source_enum_field(body, field_name)
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            case.statements
                .iter()
                .any(|stmt| stmt_assigns_source_enum_field(stmt, field_name))
        }),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|stmt| stmt_assigns_source_enum_field(stmt, field_name))
                || block_assigns_source_enum_field(body, field_name)
                || catches
                    .iter()
                    .any(|catch| block_assigns_source_enum_field(&catch.body, field_name))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_assigns_source_enum_field(block, field_name))
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JavaEnumPayloadInitializer {
    DirectParam(usize),
    DirectVarargsParam(usize),
    PatternCompileParam(usize),
}

#[derive(Debug, Clone, PartialEq)]
enum JavaEnumConstantValue {
    Null,
    Bool(bool),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
    Char(char),
    Array(Vec<JavaEnumConstantValue>),
}

fn java_enum_constructor_constant_field_values(
    ty: &TypeDecl,
    constructor: &ConstructorDecl,
) -> HashMap<String, HashMap<String, JavaEnumConstantValue>> {
    java_enum_constant_args(ty)
        .into_iter()
        .filter_map(|(variant, args)| {
            let mut values = HashMap::new();
            let mut arg_index = 0;
            for param in &constructor.params {
                if param.is_varargs {
                    let varargs = args[arg_index..]
                        .iter()
                        .map(|arg| java_enum_constant_expr_value(arg, &values, &HashMap::new()))
                        .collect::<Option<Vec<_>>>()?;
                    values.insert(
                        param.source_name.clone(),
                        JavaEnumConstantValue::Array(varargs),
                    );
                    arg_index = args.len();
                } else {
                    let value = java_enum_constant_expr_value(
                        args.get(arg_index)?,
                        &values,
                        &HashMap::new(),
                    )?;
                    values.insert(param.source_name.clone(), value);
                    arg_index += 1;
                }
            }
            let mut fields = HashMap::new();
            let body = constructor.body.as_ref()?;
            java_enum_evaluate_constructor_block(body, &values, &mut fields)?;
            Some((variant, fields))
        })
        .collect()
}

fn java_enum_evaluate_constructor_block(
    block: &Block,
    params: &HashMap<String, JavaEnumConstantValue>,
    fields: &mut HashMap<String, JavaEnumConstantValue>,
) -> Option<()> {
    for stmt in &block.statements {
        match stmt {
            Stmt::Assign { target, value } => {
                let field_name = match target {
                    Expr::Var(name) => name.strip_prefix("this.").map(str::to_string),
                    Expr::FieldAccess(receiver, name)
                        if matches!(receiver.as_ref(), Expr::This) =>
                    {
                        Some(name.clone())
                    }
                    _ => None,
                };
                let Some(field_name) = field_name else {
                    continue;
                };
                let value = java_enum_constant_expr_value(value, params, fields)?;
                fields.insert(crate::parser::to_snake_case(&field_name), value);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                let JavaEnumConstantValue::Bool(condition) =
                    java_enum_constant_expr_value(condition, params, fields)?
                else {
                    return None;
                };
                if condition {
                    java_enum_evaluate_constructor_block(then_block, params, fields)?;
                } else if let Some(else_block) = else_block {
                    java_enum_evaluate_constructor_block(else_block, params, fields)?;
                }
            }
            Stmt::Block(block) => {
                java_enum_evaluate_constructor_block(block, params, fields)?;
            }
            _ => {}
        }
    }
    Some(())
}

fn java_enum_constant_expr_value(
    expr: &Expr,
    params: &HashMap<String, JavaEnumConstantValue>,
    fields: &HashMap<String, JavaEnumConstantValue>,
) -> Option<JavaEnumConstantValue> {
    match expr {
        Expr::Literal(Literal::Null) => Some(JavaEnumConstantValue::Null),
        Expr::Literal(Literal::Bool(value)) => Some(JavaEnumConstantValue::Bool(*value)),
        Expr::Literal(Literal::Int(value)) => Some(JavaEnumConstantValue::Int(*value)),
        Expr::Literal(Literal::Long(value)) => Some(JavaEnumConstantValue::Long(*value)),
        Expr::Literal(Literal::Float(value)) => Some(JavaEnumConstantValue::Float(*value)),
        Expr::Literal(Literal::Double(value)) => Some(JavaEnumConstantValue::Double(*value)),
        Expr::Literal(Literal::String(value)) => Some(JavaEnumConstantValue::String(value.clone())),
        Expr::Literal(Literal::Char(value)) => Some(JavaEnumConstantValue::Char(*value)),
        Expr::Var(name) => params
            .get(name)
            .or_else(|| fields.get(&crate::parser::to_snake_case(name)))
            .cloned(),
        Expr::FieldAccess(receiver, field) if field == "length" => {
            let JavaEnumConstantValue::Array(values) =
                java_enum_constant_expr_value(receiver, params, fields)?
            else {
                return None;
            };
            Some(JavaEnumConstantValue::Int(values.len() as i32))
        }
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            fields.get(&crate::parser::to_snake_case(field)).cloned()
        }
        Expr::ArrayAccess(array, index) => {
            let JavaEnumConstantValue::Array(values) =
                java_enum_constant_expr_value(array, params, fields)?
            else {
                return None;
            };
            let JavaEnumConstantValue::Int(index) =
                java_enum_constant_expr_value(index, params, fields)?
            else {
                return None;
            };
            values.get(usize::try_from(index).ok()?).cloned()
        }
        Expr::ArrayCreation {
            initializer: Some(values),
            ..
        }
        | Expr::ArrayInitializer(values) => Some(JavaEnumConstantValue::Array(
            values
                .iter()
                .map(|value| java_enum_constant_expr_value(value, params, fields))
                .collect::<Option<Vec<_>>>()?,
        )),
        Expr::Unary {
            op: crate::java_ir::UnaryOp::Not,
            expr,
        } => {
            let JavaEnumConstantValue::Bool(value) =
                java_enum_constant_expr_value(expr, params, fields)?
            else {
                return None;
            };
            Some(JavaEnumConstantValue::Bool(!value))
        }
        Expr::Binary { op, left, right }
            if matches!(
                op,
                crate::java_ir::BinaryOp::And
                    | crate::java_ir::BinaryOp::Or
                    | crate::java_ir::BinaryOp::Eq
                    | crate::java_ir::BinaryOp::Ne
            ) =>
        {
            let left = java_enum_constant_expr_value(left, params, fields)?;
            let right = java_enum_constant_expr_value(right, params, fields)?;
            let value = match op {
                crate::java_ir::BinaryOp::And => {
                    matches!(left, JavaEnumConstantValue::Bool(true))
                        && matches!(right, JavaEnumConstantValue::Bool(true))
                }
                crate::java_ir::BinaryOp::Or => {
                    matches!(left, JavaEnumConstantValue::Bool(true))
                        || matches!(right, JavaEnumConstantValue::Bool(true))
                }
                crate::java_ir::BinaryOp::Eq => left == right,
                crate::java_ir::BinaryOp::Ne => left != right,
                _ => unreachable!(),
            };
            Some(JavaEnumConstantValue::Bool(value))
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if matches!(name.as_str(), "equals") && args.len() == 1 => {
            Some(JavaEnumConstantValue::Bool(
                java_enum_constant_expr_value(receiver, params, fields)?
                    == java_enum_constant_expr_value(&args[0], params, fields)?,
            ))
        }
        _ => None,
    }
}

fn java_enum_constant_accessor_values(
    field_ty: &JavaType,
    values: &[JavaEnumConstantValue],
) -> Option<(String, Vec<String>)> {
    match field_ty {
        JavaType::String => {
            let nullable = values
                .iter()
                .any(|value| matches!(value, JavaEnumConstantValue::Null));
            let rendered = values
                .iter()
                .map(|value| match value {
                    JavaEnumConstantValue::String(value) if nullable => {
                        Some(format!("Some({value:?}.to_string())"))
                    }
                    JavaEnumConstantValue::String(value) => Some(format!("{value:?}.to_string()")),
                    JavaEnumConstantValue::Null if nullable => Some("None".to_string()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some((
                if nullable {
                    "Option<String>".to_string()
                } else {
                    "String".to_string()
                },
                rendered,
            ))
        }
        JavaType::Array(element) if matches!(element.as_ref(), JavaType::String) => {
            let rendered = values
                .iter()
                .map(|value| {
                    let JavaEnumConstantValue::Array(values) = value else {
                        return None;
                    };
                    let values = values
                        .iter()
                        .map(|value| match value {
                            JavaEnumConstantValue::String(value) => {
                                Some(format!("Some({value:?}.to_string())"))
                            }
                            JavaEnumConstantValue::Null => Some("None".to_string()),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>()?;
                    Some(format!("vec![{}]", values.join(", ")))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(("Vec<Option<String>>".to_string(), rendered))
        }
        JavaType::Primitive(primitive) => {
            let rust_ty = emit_type(&rust_type_for_primitive(*primitive));
            let rendered = values
                .iter()
                .map(|value| match value {
                    JavaEnumConstantValue::Bool(value) => Some(value.to_string()),
                    JavaEnumConstantValue::Int(value) => Some(value.to_string()),
                    JavaEnumConstantValue::Long(value) => Some(format!("{value}i64")),
                    JavaEnumConstantValue::Float(value) => Some(format!("{value}f32")),
                    JavaEnumConstantValue::Double(value) => Some(format!("{value}f64")),
                    JavaEnumConstantValue::Char(value) => Some(format!("{value:?}")),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some((rust_ty, rendered))
        }
        _ => None,
    }
}

fn java_enum_constructor_field_initializers(
    constructor: &ConstructorDecl,
) -> Vec<(String, JavaEnumPayloadInitializer)> {
    let Some(body) = constructor.body.as_ref() else {
        return Vec::new();
    };
    let param_indices = constructor
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| (param.source_name.clone(), (index, param.is_varargs)))
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
            java_enum_payload_initializer(value, &param_indices)
                .map(|initializer| (field_name, initializer))
        })
        .collect()
}

fn java_enum_payload_initializer(
    value: &Expr,
    param_indices: &HashMap<String, (usize, bool)>,
) -> Option<JavaEnumPayloadInitializer> {
    if let Expr::Var(param_name) = value {
        return param_indices
            .get(param_name)
            .copied()
            .map(|(index, is_varargs)| {
                if is_varargs {
                    JavaEnumPayloadInitializer::DirectVarargsParam(index)
                } else {
                    JavaEnumPayloadInitializer::DirectParam(index)
                }
            });
    }
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = value
    else {
        return None;
    };
    let Expr::Var(owner) = receiver.as_ref() else {
        return None;
    };
    let [Expr::Var(param_name)] = args.as_slice() else {
        return None;
    };
    (crate::stdlib::canonical_external_receiver_type_key(owner) == "Pattern"
        && matches!(name.as_str(), "compile"))
    .then(|| {
        param_indices
            .get(param_name)
            .map(|(index, _)| JavaEnumPayloadInitializer::PatternCompileParam(*index))
    })
    .flatten()
}

pub(crate) fn rust_identifier_token(name: &str) -> String {
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

fn java_enum_payload_accessor_return_type(field_ty: &JavaType, facts: &FactSet) -> Option<String> {
    match field_ty {
        JavaType::String => Some("String".to_string()),
        JavaType::Array(element) if matches!(element.as_ref(), JavaType::String) => {
            Some("Vec<Option<String>>".to_string())
        }
        JavaType::Primitive(primitive) => Some(emit_type(&rust_type_for_primitive(*primitive))),
        JavaType::Named { name, args }
            if args.is_empty()
                && crate::stdlib::canonical_external_receiver_type_key(name) == "Pattern" =>
        {
            Some("Pattern".to_string())
        }
        JavaType::Named { name, .. }
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Class" =>
        {
            Some(crate::emit::emit_type(
                &crate::slot_render::rust_type_for_java_type_with_facts(field_ty, facts),
            ))
        }
        // A Java enum commonly stores one of a source class's singleton
        // instances (for example `Mode(CONFIG_A)`).  The source object's
        // storage placement is already authoritative in the fact set, so use
        // that same carrier for the match-backed payload accessor.
        JavaType::Named { .. } => Some(crate::emit::emit_type(
            &crate::slot_render::rust_type_for_java_type_with_facts(field_ty, facts),
        )),
        _ => None,
    }
}

fn java_enum_payload_is_general_named_source_reference(field_ty: &JavaType) -> bool {
    matches!(
        field_ty,
        JavaType::Named { name, .. }
            if !matches!(
                crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
                "Pattern" | "Class"
            )
    )
}

fn java_enum_payload_accessor_return_type_with_representations(
    field_ty: &JavaType,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<String> {
    if java_enum_payload_is_general_named_source_reference(field_ty) {
        let shape =
            crate::shape_projection::value_shape_for_java_type(representations, field_ty.clone());
        return Some(crate::emit::emit_type(
            &rust_type_for_slot_with_facts_and_representations(&shape, facts, representations),
        ));
    }
    java_enum_payload_accessor_return_type(field_ty, facts)
}

fn java_enum_payload_initializer_value(
    initializer: &JavaEnumPayloadInitializer,
    args: &[Expr],
    field_ty: &JavaType,
    facts: &FactSet,
) -> Option<String> {
    match initializer {
        JavaEnumPayloadInitializer::DirectParam(index) => {
            java_enum_payload_arg_value(args.get(*index)?, field_ty, facts)
        }
        JavaEnumPayloadInitializer::DirectVarargsParam(index) => {
            java_enum_payload_varargs_value(&args[*index..], field_ty)
        }
        JavaEnumPayloadInitializer::PatternCompileParam(index) => {
            let Expr::Literal(Literal::String(pattern)) = args.get(*index)? else {
                return None;
            };
            Some(format!("Pattern::compile_static({pattern:?})"))
        }
    }
}

fn java_enum_payload_varargs_value(args: &[Expr], field_ty: &JavaType) -> Option<String> {
    let JavaType::Array(element) = field_ty else {
        return None;
    };
    if !matches!(element.as_ref(), JavaType::String) {
        return None;
    }
    let values = args
        .iter()
        .map(|arg| match arg {
            Expr::Literal(Literal::String(value)) => Some(format!("Some({value:?}.to_string())")),
            Expr::Literal(Literal::Null) => Some("None".to_string()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some(format!("vec![{}]", values.join(", ")))
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

fn java_enum_payload_arg_value(arg: &Expr, field_ty: &JavaType, facts: &FactSet) -> Option<String> {
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
        (JavaType::Named { name, .. }, Expr::ClassLiteral(class_ty))
            if crate::stdlib::canonical_external_receiver_type_key(name) == "Class" =>
        {
            let token_ty = java_enum_class_payload_token_type(field_ty, facts);
            Some(crate::emit::emit_expr(
                &crate::call_lowering::apply_class_literal_token_with_rust_type(class_ty, token_ty),
            ))
        }
        (JavaType::Named { .. }, Expr::FieldAccess(receiver, field))
            if matches!(receiver.as_ref(), Expr::Var(_)) =>
        {
            let Expr::Var(owner) = receiver.as_ref() else {
                unreachable!("guarded source static owner")
            };
            Some(format!(
                "{}::__static_{}()",
                rust_name_for_java_named_type(owner),
                rust_identifier_token(field)
            ))
        }
        _ => None,
    }
}

fn java_enum_class_payload_token_type(field_ty: &JavaType, facts: &FactSet) -> RustType {
    let JavaType::Named { args, .. } = field_ty else {
        return RustType::Unit;
    };
    let Some(arg) = args.first() else {
        return RustType::Unit;
    };
    let effective = match arg {
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => bound.as_ref(),
        JavaType::Wildcard(None) => return RustType::Unit,
        other => other,
    };
    match effective {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            RustType::Named(rust_name_for_java_named_type(name))
        }
        other => crate::slot_render::rust_type_for_java_type_with_facts(other, facts),
    }
}

fn java_enum_payload_default_value(field_ty: &JavaType) -> String {
    match field_ty {
        JavaType::String => "String::new()".to_string(),
        JavaType::Array(element) if matches!(element.as_ref(), JavaType::String) => {
            "Vec::new()".to_string()
        }
        JavaType::Primitive(crate::java_ir::PrimitiveType::Boolean) => "false".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Byte)
        | JavaType::Primitive(crate::java_ir::PrimitiveType::Short)
        | JavaType::Primitive(crate::java_ir::PrimitiveType::Int)
        | JavaType::Primitive(crate::java_ir::PrimitiveType::Long) => "0".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Float) => "0.0f32".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Double) => "0.0f64".to_string(),
        JavaType::Primitive(crate::java_ir::PrimitiveType::Char) => "'\\0'".to_string(),
        JavaType::Named { name, args }
            if args.is_empty()
                && crate::stdlib::canonical_external_receiver_type_key(name) == "Pattern" =>
        {
            "Pattern::default()".to_string()
        }
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
        ObjectRepresentation::ClosedEnum {
            enum_name,
            variants,
        } => RepresentationTypeEmission::RawRustItem(closed_enum_carrier_item_for_program(
            enum_name, variants, facts, program, ty,
        )),
        ObjectRepresentation::DynTrait { trait_name } => {
            let trait_item = trait_item_for_type_with_program(
                ty,
                trait_name,
                program,
                signatures,
                facts,
                exceptions,
                representations,
                source_interface_requires_send(program, ty, representations),
            );
            let existential_carrier =
                existential_interface_carrier_item(ty, facts, representations)
                    .map(|carrier| format!("\n\n{carrier}"))
                    .unwrap_or_default();
            RepresentationTypeEmission::RawRustItem(format!("{trait_item}{existential_carrier}"))
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

fn existential_interface_carrier_item(
    interface: &TypeDecl,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> Option<String> {
    let carrier = representations.existential_interface_carrier_for_interface(&interface.name)?;
    let variants = carrier
        .variants
        .iter()
        .map(|variant| {
            let variant_name = rust_name_for_java_named_type(&variant.implementor_name);
            let shape = crate::shape_projection::storage_shape_for_java_type(
                representations,
                JavaType::Named {
                    name: variant.implementor_name.clone(),
                    args: Vec::new(),
                },
            );
            let payload = crate::emit::emit_type(
                &rust_type_for_slot_with_facts_and_representations(&shape, facts, representations),
            );
            format!("    {variant_name}({payload}),")
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "#[derive(Clone)]\npub enum {} {{\n{variants}\n}}",
        carrier.enum_name
    ))
}

/// Interface objects are normally local `Box<dyn Trait>` values. Any interface
/// reachable from a process-wide static, including through class fields,
/// needs its trait object to be `Send`.
pub(crate) fn source_interface_requires_send(
    program: &Program,
    interface: &TypeDecl,
    representations: &RepresentationPlan,
) -> bool {
    if representations
        .storage_placement_for(&interface.name)
        .sync()
    {
        return true;
    }
    crate::storage_strategy::statically_reachable_source_type_names(program)
        .contains(&interface.name)
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
    let explicit_parameterized = ty
        .implements
        .iter()
        .filter_map(|implemented| {
            let JavaType::Named { name, args } = crate::parser::parse_hierarchy_type(implemented)
            else {
                return None;
            };
            (!args.is_empty()).then_some((name, args))
        })
        .collect::<Vec<_>>();
    let mut emitted = explicit_parameterized
        .iter()
        .filter_map(|(interface_name, args)| {
            let trait_name = dyn_trait_name_for(representations, interface_name)?;
            let interface = find_type_decl_for_owner_context(program, &ty.name, interface_name)?;
            let rendered_args = args
                .iter()
                .map(|arg| {
                    crate::emit::emit_type(&rust_type_for_slot_with_facts_and_representations(
                        &RustSlotShape::for_storage(arg.clone()),
                        facts,
                        representations,
                    ))
                })
                .collect::<Vec<_>>()
                .join(", ");
            let trait_path = format!("{trait_name}<{rendered_args}>");
            let substitutions = interface
                .type_params
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect::<HashMap<_, _>>();
            let mut specialized_interface = interface.clone();
            specialized_interface.type_params.clear();
            specialized_interface.type_param_bounds.clear();
            for member in &mut specialized_interface.members {
                if let Member::Method(method) = member {
                    method.return_type = crate::resolve::substitute_method_type_variables(
                        &method.return_type,
                        &substitutions,
                    );
                    for param in &mut method.params {
                        param.ty = crate::resolve::substitute_method_type_variables(
                            &param.ty,
                            &substitutions,
                        );
                    }
                }
            }
            trait_impl_for_type(
                ty,
                &specialized_interface,
                program,
                &trait_path,
                signatures,
                facts,
                exceptions,
                representations,
            )
        })
        .collect::<Vec<_>>();
    emitted.extend(
        implemented_interfaces_for_type(program, ty)
            .into_iter()
            .filter(|interface_name| {
                !explicit_parameterized
                    .iter()
                    .any(|(explicit, _)| explicit == interface_name)
            })
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
                    representations,
                )
            }),
    );
    emitted
}

pub(crate) fn source_class_bound_trait_item_for_type(
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    representations: &RepresentationPlan,
) -> Option<String> {
    if !source_class_is_used_as_type_param_bound(program, &ty.name) {
        return None;
    }
    if !matches!(ty.kind, TypeKind::Class) {
        return None;
    }
    let trait_name = source_class_bound_trait_name(&ty.name);
    let trait_path = parameterized_interface_trait_path(&trait_name, &ty.type_params);
    let declared_method_keys = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if !method.modifiers.contains(&Modifier::Static)
                    && !method.modifiers.contains(&Modifier::Private) =>
            {
                Some((
                    crate::java_signature::source_method_rust_name(method),
                    method.params.len(),
                ))
            }
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if !method.modifiers.contains(&Modifier::Static)
                    && !method.modifiers.contains(&Modifier::Private) =>
            {
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
                    representations,
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
    let erased_methods = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method)
                if !method.modifiers.contains(&Modifier::Static)
                    && !method.modifiers.contains(&Modifier::Private) =>
            {
                source_class_bound_erased_value_impl_method(
                    program,
                    ty,
                    method,
                    signatures,
                    facts,
                    exceptions.method_plan(ty, method),
                    representations,
                )
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let erased_impl = (!erased_methods.is_empty())
        .then(|| {
            format!(
                "\n\nimpl {trait_path} for JavaErasedValue {{\n{}\n}}",
                erased_methods.join("\n")
            )
        })
        .unwrap_or_default();
    // Implementors are emitted for each subtype's canonical storage below.
    // A blanket `Rc<RefCell<T>> where T: Bound` bridge has to borrow `T` and
    // therefore loses the owning handle.  It cannot implement methods whose
    // Java body passes `this` across an identity-sensitive boundary.  Concrete
    // canonical-storage impls can call the subtype's `_with_outer` ABI and
    // preserve that identity.
    Some(format!(
        "pub trait {trait_path}: std::any::Any {{\n{}\n}}{erased_impl}",
        methods.join("\n")
    ))
}

fn source_class_bound_erased_value_impl_method(
    program: &Program,
    bound: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    representations: &RepresentationPlan,
) -> Option<String> {
    let candidates =
        source_class_bound_trait_method_implementation_candidates(program, bound, method)
            .into_iter()
            .filter(|(ty, _)| ty.type_params.is_empty())
            .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }
    let return_shape = source_class_bound_trait_method_return_shape_plan(
        bound,
        method,
        candidates.as_slice(),
        signatures,
    );
    let signature = source_class_bound_trait_method_signature_with_return_shape(
        bound,
        method,
        program,
        signatures,
        facts,
        exceptions,
        return_shape,
        representations,
    );
    let bound_signature = signatures.required_method_signature(bound, method);
    let args = bound_signature
        .params()
        .iter()
        .map(|param| rust_identifier_token(param.name()))
        .collect::<Vec<_>>()
        .join(", ");
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    let trait_name = source_class_bound_trait_name(&bound.name);
    let mutable = bound_signature.receiver() == ReceiverPlan::Mutable;
    let receiver = if mutable {
        "self.as_any_mut()"
    } else {
        "self.as_any()"
    };
    let arms = candidates
        .into_iter()
        .map(|(ty, _)| {
            let placement = crate::storage_strategy::effective_storage_placement(
                facts.storage_placement_for(&ty.name),
                representations.storage_placement_for(&ty.name),
            );
            let rust_ty = crate::item_assembly::rust_use_name_for_source_type(ty);
            let rust_ty = if placement.shared() && placement.is_implemented() {
                placement.wrap(rust_ty)
            } else {
                rust_ty
            };
            let downcast = if mutable { "downcast_mut" } else { "downcast_ref" };
            let comma = if args.is_empty() { "" } else { ", " };
            format!(
                "        if let Some(__java_value) = {receiver}.{downcast}::<{rust_ty}>() {{ return {trait_name}::{rust_name}(__java_value{comma}{args}); }}"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(format!(
        "{} {{\n{arms}\n        panic!(\"invalid Java erased upper-bound value\")\n    }}",
        signature.trim_end_matches(';')
    ))
}

fn source_class_bound_trait_external_fact_methods(
    ty: &TypeDecl,
    facts: &FactSet,
    declared_method_keys: &HashSet<(String, usize)>,
) -> Vec<String> {
    let mut emitted_method_keys = declared_method_keys.clone();
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
            // Qualified and simple external-pack aliases describe the same
            // facade surface. A source class can match both facts, but a Rust
            // trait may declare each method signature only once.
            if !emitted_method_keys.insert((rust_name.clone(), arg_count)) {
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

#[cfg(test)]
mod source_class_bound_external_fact_tests {
    use super::*;

    #[test]
    fn qualified_and_simple_pack_aliases_emit_one_bound_method() {
        let parsed = crate::parser::parse_java_source(
            r#"
class VCFHeaderLine {}
class UsesHeaderLine<T extends VCFHeaderLine> {}
"#,
        );
        let ty = parsed
            .program
            .types
            .iter()
            .find(|ty| ty.name == "VCFHeaderLine")
            .expect("bound type");
        let facts = crate::external_packs::htsjdk_vcf_pack_facts();
        let rendered = source_class_bound_trait_item_for_type(
            &parsed.program,
            ty,
            &SignaturePlan::default(),
            &facts,
            &ProgramExceptionPlan::default(),
            &RepresentationPlan::default(),
        )
        .expect("class-bound trait");

        assert_eq!(rendered.matches("fn get_i_d(").count(), 1, "{rendered}");
        assert_eq!(
            rendered.matches("fn get_description(").count(),
            1,
            "{rendered}"
        );
    }
}

pub(crate) fn source_class_bound_trait_impl_emissions_for_type(
    program: &Program,
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    representations: &RepresentationPlan,
) -> Vec<String> {
    if !matches!(ty.kind, TypeKind::Class | TypeKind::Enum) {
        return Vec::new();
    }
    source_class_bounds_for_type(program, ty)
        .into_iter()
        .filter_map(|bound| {
            let trait_name = source_class_bound_trait_name(&bound.name);
            let trait_path = parameterized_interface_trait_path(&trait_name, &bound.type_params);
            let impl_type_name = crate::item_assembly::rust_use_name_for_source_type(ty);
            let placement = crate::storage_strategy::effective_storage_placement(
                facts.storage_placement_for(&ty.name),
                representations.storage_placement_for(&ty.name),
            );
            let impl_type = if placement.shared() && placement.is_implemented() {
                placement.wrap(impl_type_name)
            } else {
                impl_type_name
            };
            let impl_params =
                crate::item_assembly::rust_impl_params_for_source_type(ty, program, None);
            let impl_prefix = if impl_params.is_empty() {
                "impl".to_string()
            } else {
                format!("impl<{}>", impl_params.join(", "))
            };
            let methods = bound
                .members
                .iter()
                .filter_map(|member| match member {
                    Member::Method(method)
                        if !method.modifiers.contains(&Modifier::Static)
                            && !method.modifiers.contains(&Modifier::Private) =>
                    {
                        Some(source_class_bound_trait_impl_method(
                            ty,
                            bound,
                            method,
                            program,
                            signatures,
                            facts,
                            exceptions.method_plan(bound, method),
                            representations,
                            placement,
                        ))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            Some(format!(
                "{impl_prefix} {trait_path} for {impl_type} {{\n{}\n}}",
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
    representations: &RepresentationPlan,
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
            rust_identifier_token(param.name()),
            emit_type(&rust_type_for_slot_with_facts_and_representations(
                shape,
                facts,
                representations,
            ))
        )
    }));
    let return_type = source_class_bound_trait_method_return_type_with_shape(
        method,
        facts,
        exceptions,
        return_shape_override.as_ref(),
        representations,
    )
    .map(|ty| format!(" -> {}", emit_type(&ty)))
    .unwrap_or_default();
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    let type_params = rust_method_type_params(method, program, &ty.name);
    let where_clause = source_trait_method_where_clause(ty, method, program, representations);
    format!(
        "    fn {rust_name}{type_params}({}){return_type}{where_clause};",
        params.join(", ")
    )
}

fn source_trait_method_where_clause(
    owner: &TypeDecl,
    method: &MethodDecl,
    program: &Program,
    representations: &RepresentationPlan,
) -> String {
    let mut bounds = Vec::new();
    if !method.type_params.is_empty() {
        bounds.push("Self: Sized".to_string());
    }
    bounds.extend(
        crate::item_assembly::source_method_class_type_param_where_bounds(
            owner,
            method,
            Some(representations),
            &[program],
        ),
    );
    if bounds.is_empty() {
        String::new()
    } else {
        format!(" where {}", bounds.join(", "))
    }
}

fn source_class_bound_trait_method_return_type_with_shape(
    method: &MethodDecl,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    return_shape_override: Option<&RustSlotShape>,
    representations: &RepresentationPlan,
) -> Option<RustType> {
    if method.return_type == JavaType::Void {
        if exceptions.wrap_returns_in_result() {
            return Some(exception_result_type(RustType::Unit));
        }
        return None;
    }
    let rust_return = return_shape_override
        .map(|shape| {
            rust_type_for_slot_with_facts_and_representations(shape, facts, representations)
        })
        .unwrap_or_else(|| {
            crate::slot_render::rust_type_for_java_type_with_facts_and_representations(
                &method.return_type,
                facts,
                representations,
            )
        });
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
    representations: &RepresentationPlan,
    placement: crate::storage_strategy::StoragePlacement,
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
        representations,
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
    // The trait is implemented for the shared Java-reference carrier itself.
    // Every method on an implemented shared source receiver has a companion
    // accepting that carrier, while the ordinary borrowed method may be
    // omitted whenever its body needs canonical identity (even transitively).
    // Keep the forwarding ABI uniform instead of trying to rediscover which
    // individual borrowed bodies survived representation lowering.
    let uses_outer_handle_companion = placement.shared();
    let receiver_arg = if uses_outer_handle_companion {
        "std::clone::Clone::clone(self)".to_string()
    } else if placement.shared() {
        "&**self".to_string()
    } else {
        trait_impl_forwarding_receiver_arg(
            ty,
            forwarding_owner,
            Some(implementation_signature.receiver()),
        )
    };
    let receiver_arg = if args.is_empty() {
        receiver_arg
    } else {
        format!("{receiver_arg}, {args}")
    };
    let implementing_owner_name = rust_name_for_java_named_type(&forwarding_owner.name);
    let call_name = if uses_outer_handle_companion {
        format!("{rust_name}_with_outer")
    } else {
        rust_name
    };
    let call = format!("{implementing_owner_name}::{call_name}({receiver_arg})");
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
        representations,
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
    let parsed_name = match crate::parser::parse_hierarchy_type(interface_name) {
        JavaType::Named { name, .. } => name,
        _ => interface_name.to_string(),
    };
    let interface = find_type_decl_for_owner_context(program, owner_name, &parsed_name);
    let canonical_name = interface
        .map(|interface| interface.name.as_str())
        .unwrap_or(&parsed_name);
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
    sync_trait: bool,
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
    let object_defaults =
        external_object_interface_trait_default_methods(ty, &trait_name, Some(facts), None);
    methods.extend(trait_object_support_method_signatures(&trait_name));
    let forwarding = shared_handle_forwarding_impl_with_policy(&trait_name, &methods, sync_trait)
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    let closure_adapter = if sync_trait {
        None
    } else {
        zero_arg_functional_method(ty).and_then(|functional_method| {
            zero_arg_closure_trait_impl(
                ty,
                &trait_name,
                facts,
                signatures,
                exceptions.method_plan(ty, functional_method),
                None,
            )
        })
    };
    // A boxed source-interface object is the canonical owned representation
    // when that interface appears as a generic argument.  Forward the same
    // canonical trait surface through the box so it can satisfy Java bounds
    // such as `F extends Factory<P>`.  Reuse the signature forwarding policy
    // used by Rc/Arc carriers; the closure blanket impl is the one case where
    // a generic Box impl would overlap Rust's coherence rules.
    let boxed_forwarding = closure_adapter
        .is_none()
        .then(|| boxed_trait_forwarding_impl_with_generics(&trait_name, &[], &methods))
        .flatten()
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    // Defaults have bodies rather than forwarding signatures, so append them
    // only after all carrier implementations have consumed the canonical
    // semicolon-terminated surface.
    methods.extend(object_defaults);
    let closure_adapter = closure_adapter
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    format!(
        "pub trait {trait_name}{send_bound} {{\n{}\n}}\n\n{}\n\n{}{forwarding}{boxed_forwarding}{closure_adapter}",
        methods.join("\n"),
        dyn_trait_anonymous_impl(&trait_name),
        boxed_dyn_trait_clone_impl(&trait_name),
        send_bound = if sync_trait { ": Send" } else { "" },
    )
}

/// Rust has no owned trait-object upcast from `Box<dyn Derived>` to
/// `Box<dyn Parent>`.  Java does, so materialize a local carrier for every
/// reachable source parent: the derived box implements the parent's canonical
/// abstract surface, then `From` can box that carrier without losing identity.
struct InterfaceAncestorInstantiation<'a> {
    interface: &'a TypeDecl,
    args: Vec<JavaType>,
}

fn interface_ancestor_instantiations<'a>(
    program: &'a Program,
    interface: &'a TypeDecl,
) -> Vec<InterfaceAncestorInstantiation<'a>> {
    fn visit<'a>(
        program: &'a Program,
        interface: &'a TypeDecl,
        substitutions: &HashMap<String, JavaType>,
        seen: &mut HashSet<String>,
        out: &mut Vec<InterfaceAncestorInstantiation<'a>>,
    ) {
        for parent_name in interface.extends.iter().chain(interface.implements.iter()) {
            let JavaType::Named { name, args } = crate::parser::parse_hierarchy_type(parent_name)
            else {
                continue;
            };
            let Some(parent) = find_type_decl_for_owner_context(program, &interface.name, &name)
            else {
                continue;
            };
            if parent.kind != TypeKind::Interface || !seen.insert(parent.name.clone()) {
                continue;
            }
            let args = parent
                .type_params
                .iter()
                .enumerate()
                .map(|(index, type_param)| {
                    let actual = args.get(index).cloned().unwrap_or_else(|| JavaType::Named {
                        name: type_param.clone(),
                        args: Vec::new(),
                    });
                    crate::resolve::substitute_method_type_variables(&actual, substitutions)
                })
                .collect::<Vec<_>>();
            let parent_substitutions = parent
                .type_params
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect::<HashMap<_, _>>();
            out.push(InterfaceAncestorInstantiation {
                interface: parent,
                args,
            });
            visit(program, parent, &parent_substitutions, seen, out);
        }
    }

    let mut ancestors = Vec::new();
    visit(
        program,
        interface,
        &HashMap::new(),
        &mut HashSet::new(),
        &mut ancestors,
    );
    ancestors
}

pub(crate) fn interface_method_types_in_target(
    program: &Program,
    target: &TypeDecl,
    owner: &TypeDecl,
    method: &MethodDecl,
) -> (JavaType, Vec<JavaType>) {
    let substitutions = interface_ancestor_instantiations(program, target)
        .into_iter()
        .find(|ancestor| ancestor.interface.name == owner.name)
        .map(|ancestor| {
            owner
                .type_params
                .iter()
                .cloned()
                .zip(ancestor.args)
                .filter(|(name, _)| !method.type_params.contains(name))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();
    let return_type =
        crate::resolve::substitute_method_type_variables(&method.return_type, &substitutions);
    let param_types = method
        .params
        .iter()
        .map(|param| crate::resolve::substitute_method_type_variables(&param.ty, &substitutions))
        .collect::<Vec<_>>();
    (return_type, param_types)
}

pub(crate) fn interface_type_arguments_in_target(
    program: &Program,
    target: &TypeDecl,
    owner: &TypeDecl,
) -> Option<Vec<JavaType>> {
    (target.name != owner.name)
        .then(|| interface_ancestor_instantiations(program, target))?
        .into_iter()
        .find(|ancestor| ancestor.interface.name == owner.name)
        .map(|ancestor| ancestor.args)
}

fn boxed_interface_upcast_impls(
    ty: &TypeDecl,
    derived_trait_name: &str,
    program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    representations: &RepresentationPlan,
) -> Option<String> {
    if ty.kind != TypeKind::Interface {
        return None;
    }
    let parents = interface_ancestor_instantiations(program, ty);
    let derived_contracts = specialized_interface_abstract_contracts(program, ty);
    let derived_trait_path =
        parameterized_interface_trait_path(derived_trait_name, &ty.type_params);
    let generic_impl_params = if ty.type_params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            ty.type_params
                .iter()
                .map(|name| format!("{name}: 'static"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    (!parents.is_empty()).then(|| {
        parents
            .into_iter()
            .map(|parent_instantiation| {
                let parent = parent_instantiation.interface;
                let parent_trait_name = rust_name_for_java_named_type(&parent.name);
                let parent_trait_path = crate::slot_render::parameterized_dyn_trait_path(
                    &parent_trait_name,
                    &JavaType::Named {
                        name: parent.name.clone(),
                        args: parent_instantiation.args.clone(),
                    },
                    &crate::effective_facts::EffectiveFacts::new(facts),
                    representations,
                );
                let substitutions = parent
                    .type_params
                    .iter()
                    .cloned()
                    .zip(parent_instantiation.args.iter().cloned())
                    .collect::<HashMap<_, _>>();
                let mut parent_signatures = specialized_interface_abstract_contracts(program, parent)
                    .into_iter()
                    .map(|contract| {
                        let owner = contract.owner;
                        let method = contract.method;
                        let return_type = crate::resolve::substitute_method_type_variables(
                            &contract.return_type,
                            &substitutions,
                        );
                        let param_types = contract
                            .param_types
                            .iter()
                            .map(|ty| crate::resolve::substitute_method_type_variables(ty, &substitutions))
                            .collect::<Vec<_>>();
                        let return_shape = if return_type != method.return_type
                            || matches!(&return_type, JavaType::Named { name, args }
                                if args.is_empty() && ty.type_params.contains(name))
                        {
                            Some(RustSlotShape::for_storage(return_type.clone()))
                        } else {
                            trait_method_return_shape_plan(owner, method, program, signatures)
                        };
                        let signature = trait_method_signature_with_receiver_and_return_shape_and_params(
                            owner,
                            method,
                            signatures,
                            facts,
                            trait_method_exception_plan(owner, method, program, exceptions),
                            trait_method_receiver_plan(owner, method, program, signatures),
                            return_shape,
                            program,
                            Some(&param_types),
                            Some(representations),
                        );
                        let covariant_erased_return = derived_contracts.iter().any(|contract| {
                            contract.method.name == method.name
                                && crate::java_signature::erased_param_types(&contract.method.params)
                                    == crate::java_signature::erased_param_types(&method.params)
                                && interface_contract_return_refines(
                                    program,
                                    &contract.return_type,
                                    &return_type,
                                )
                                && matches!(&return_type, JavaType::Named { name, .. }
                                    if matches!(name.as_str(), "Object" | "java.lang.Object"))
                        });
                        (signature, covariant_erased_return)
                    })
                    .collect::<Vec<_>>();
                parent_signatures.extend(
                    trait_object_support_method_signatures(&parent_trait_path)
                        .into_iter()
                        .map(|signature| (signature, false)),
                );
                let methods = parent_signatures
                    .iter()
                    .map(|(signature, covariant_erased_return)| {
                        if *covariant_erased_return {
                            return forwarded_covariant_object_upcast_method(signature);
                        }
                        forwarded_trait_method(signature, SharedTraitHandle::OwnedField, None)
                    })
                    .collect::<Option<Vec<_>>>()
                    .expect("parent interface methods have a supported receiver")
                    .join("\n");
                let carrier_name = format!(
                    "__JavaInterfaceUpcast{derived_trait_name}To{parent_trait_name}"
                );
                let carrier_path =
                    parameterized_interface_trait_path(&carrier_name, &ty.type_params);
                let carrier_declaration = if ty.type_params.is_empty() {
                    format!(
                        "#[derive(Clone)]\nstruct {carrier_name}(std::boxed::Box<dyn {derived_trait_path}>);"
                    )
                } else {
                    format!(
                        "struct {carrier_name}{generic_impl_params}(std::boxed::Box<dyn {derived_trait_path}>);\n\nimpl{generic_impl_params} Clone for {carrier_path} {{\n    fn clone(&self) -> Self {{ Self(std::clone::Clone::clone(&self.0)) }}\n}}"
                    )
                };
                format!(
                    "{carrier_declaration}\n\nimpl{generic_impl_params} {parent_trait_path} for {carrier_path} {{\n{methods}\n}}\n\nimpl{generic_impl_params} From<std::boxed::Box<dyn {derived_trait_path}>> for std::boxed::Box<dyn {parent_trait_path}> {{\n    fn from(value: std::boxed::Box<dyn {derived_trait_path}>) -> Self {{ std::boxed::Box::new({carrier_name}(value)) }}\n}}"
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    })
}

fn forwarded_covariant_object_upcast_method(signature: &str) -> Option<String> {
    let signature_without_semicolon = signature.trim().strip_suffix(';')?;
    let body = signature_without_semicolon.strip_prefix("fn ")?;
    let open = body.find('(')?;
    let name = body[..open].split('<').next()?.trim();
    let close = matching_paren(body, open)?;
    let args = forwarded_argument_names(&body[open + 1..close])?;
    Some(format!(
        "    {signature_without_semicolon} {{ std::boxed::Box::new((*self.0).{name}({})) as std::boxed::Box<dyn std::any::Any> }}",
        args.join(", ")
    ))
}

fn trait_item_for_type_with_program(
    ty: &TypeDecl,
    trait_name: &str,
    program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    representations: &RepresentationPlan,
    sync_trait: bool,
) -> String {
    let trait_name = rust_name_for_java_named_type(trait_name);
    let trait_path = parameterized_interface_trait_path(&trait_name, &ty.type_params);
    let mut methods = specialized_interface_abstract_contracts(program, ty)
        .into_iter()
        .map(|contract| {
            let owner = contract.owner;
            let method = contract.method;
            let return_shape = if contract.return_type != method.return_type
                || matches!(&contract.return_type, JavaType::Named { name, args }
                    if args.is_empty() && ty.type_params.contains(name))
            {
                Some(RustSlotShape::for_storage(contract.return_type))
            } else {
                trait_method_return_shape_plan(owner, method, program, signatures)
            };
            trait_method_signature_with_receiver_and_return_shape_and_params(
                owner,
                method,
                signatures,
                facts,
                trait_method_exception_plan(owner, method, program, exceptions),
                trait_method_receiver_plan(owner, method, program, signatures),
                return_shape,
                program,
                Some(&contract.param_types),
                Some(representations),
            )
        })
        .collect::<Vec<_>>();
    let object_defaults = external_object_interface_trait_default_methods(
        ty,
        &trait_path,
        Some(facts),
        Some(program),
    );
    methods.extend(trait_object_support_method_signatures(&trait_path));
    let forwarding = shared_handle_forwarding_impl_with_policy_and_generics_and_cell_policy(
        &trait_path,
        &ty.type_params,
        &methods,
        sync_trait,
        !interface_has_interior_mutable_implementor(program, ty, facts, representations),
    )
    .map(|item| format!("\n\n{item}"))
    .unwrap_or_default();
    let closure_adapter = (!sync_trait && ty.type_params.is_empty())
        .then(|| {
            zero_arg_closure_trait_impl(
                ty,
                &trait_name,
                facts,
                signatures,
                zero_arg_functional_method(ty)
                    .map(|method| trait_method_exception_plan(ty, method, program, exceptions))
                    .unwrap_or_else(|| MethodExceptionPlan::new(false)),
                Some(representations),
            )
        })
        .flatten();
    let boxed_forwarding = closure_adapter
        .is_none()
        .then(|| boxed_trait_forwarding_impl_with_generics(&trait_path, &ty.type_params, &methods))
        .flatten()
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    methods.extend(object_defaults);
    let closure_adapter = closure_adapter
        .map(|item| format!("\n\n{item}"))
        .unwrap_or_default();
    let upcasts = boxed_interface_upcast_impls(
        ty,
        &trait_name,
        program,
        signatures,
        facts,
        exceptions,
        representations,
    )
    .map(|item| format!("\n\n{item}"))
    .unwrap_or_default();
    let generic_retarget = generic_interface_retarget_adapter_item(
        ty,
        program,
        signatures,
        facts,
        exceptions,
        representations,
    )
    .map(|item| format!("\n\n{item}"))
    .unwrap_or_default();
    format!(
        "pub trait {trait_path}{send_bound} {{\n{}\n}}\n\n{}\n\n{}{forwarding}{boxed_forwarding}{closure_adapter}{upcasts}{generic_retarget}",
        methods.join("\n"),
        dyn_trait_anonymous_impl_with_generics(&trait_path, &ty.type_params),
        boxed_dyn_trait_clone_impl_with_generics(&trait_path, &ty.type_params),
        send_bound = if sync_trait { ": Send" } else { "" },
    )
}

/// Emit an invariant-generic trait adapter for Java's explicitly unchecked
/// casts between two instantiations of the same interface.
///
/// Java erases the interface argument and checks concrete values when a
/// bridge method eventually consumes them. Rust bakes that argument into the
/// trait-object vtable, so reinterpreting `Box<dyn Interface<A>>` as
/// `Box<dyn Interface<B>>` would be unsound. The adapter keeps the original
/// vtable and performs owned, runtime-checked conversions at every method
/// boundary. It is emitted only when the complete abstract contract uses the
/// interface parameter in shapes that can be converted without borrowing or
/// alias fabrication.
fn generic_interface_retarget_adapter_item(
    interface: &TypeDecl,
    program: &Program,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: &ProgramExceptionPlan,
    representations: &RepresentationPlan,
) -> Option<String> {
    let [type_param] = interface.type_params.as_slice() else {
        return None;
    };
    let contracts = specialized_interface_abstract_contracts(program, interface);
    if contracts.is_empty()
        || contracts.iter().any(|contract| {
            contract.method.modifiers.contains(&Modifier::Static)
                || contract
                    .method
                    .type_params
                    .iter()
                    .any(|name| name == type_param)
                || contract
                    .param_types
                    .iter()
                    .any(|ty| !generic_retarget_type_is_supported(ty, type_param, &interface.name))
                || !generic_retarget_type_is_supported(
                    &contract.return_type,
                    type_param,
                    &interface.name,
                )
        })
    {
        return None;
    }

    let trait_name = rust_name_for_java_named_type(&interface.name);
    let adapter_name = generic_interface_retarget_adapter_name(&interface.name);
    let from = "__JavaFrom";
    let to = "__JavaTo";
    let supports_erased_bound = representations
        .erased_wildcard_bound_carrier_for(&interface.name, 0)
        .is_some()
        && crate::representation::program_declares_unbounded_wildcard_use(program, &interface.name);
    let substitutions = |replacement: &str| {
        HashMap::from([(
            type_param.clone(),
            JavaType::Named {
                name: replacement.to_string(),
                args: Vec::new(),
            },
        )])
    };
    let target_substitutions = substitutions(to);
    let mut methods = Vec::new();
    for contract in contracts {
        let target_return = crate::resolve::substitute_method_type_variables(
            &contract.return_type,
            &target_substitutions,
        );
        let target_params = contract
            .param_types
            .iter()
            .map(|ty| crate::resolve::substitute_method_type_variables(ty, &target_substitutions))
            .collect::<Vec<_>>();
        let return_shape = (!matches!(target_return, JavaType::Void))
            .then(|| RustSlotShape::for_storage(target_return));
        let signature = trait_method_signature_with_receiver_and_return_shape_and_params(
            contract.owner,
            contract.method,
            signatures,
            facts,
            trait_method_exception_plan(contract.owner, contract.method, program, exceptions),
            trait_method_receiver_plan(contract.owner, contract.method, program, signatures),
            return_shape,
            program,
            Some(&target_params),
            Some(representations),
        );
        let signature = signature.trim().strip_suffix(';')?;
        let args = contract
            .method
            .params
            .iter()
            .zip(&contract.param_types)
            .map(|(param, ty)| {
                let name = rust_identifier_token(&param.name);
                generic_retarget_value_expr(
                    &name,
                    ty,
                    type_param,
                    &interface.name,
                    to,
                    from,
                    supports_erased_bound,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        let rust_name = contract
            .method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&contract.method.name));
        let receiver =
            match trait_method_receiver_plan(contract.owner, contract.method, program, signatures)
                .unwrap_or(ReceiverPlan::Shared)
            {
                ReceiverPlan::Mutable => "&mut *self.__java_inner",
                ReceiverPlan::Shared | ReceiverPlan::None => "&*self.__java_inner",
            };
        let call = format!(
            "{trait_name}::<{from}>::{rust_name}({receiver}{comma}{args})",
            comma = if args.is_empty() { "" } else { ", " },
            args = args.join(", "),
        );
        let exception_plan =
            trait_method_exception_plan(contract.owner, contract.method, program, exceptions);
        let body = if matches!(contract.return_type, JavaType::Void) {
            call
        } else {
            let converted = generic_retarget_value_expr(
                "__java_value",
                &contract.return_type,
                type_param,
                &interface.name,
                from,
                to,
                supports_erased_bound,
            )?;
            if exception_plan.wrap_returns_in_result() {
                format!("{call}.map(|__java_value| {converted})")
            } else {
                format!("{{ let __java_value = {call}; {converted} }}")
            }
        };
        methods.push(format!("    {signature} {{ {body} }}"));
    }
    methods.push("    fn as_any(&self) -> &dyn std::any::Any { self }".to_string());
    methods.push(format!(
        "    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}<{to}>> {{ std::boxed::Box::new(std::clone::Clone::clone(self)) }}"
    ));
    let generic_bound = if supports_erased_bound {
        "Clone + 'static"
    } else {
        "'static"
    };
    Some(format!(
        "pub struct {adapter_name}<{from}: {generic_bound}, {to}: {generic_bound}> {{\n    __java_inner: std::boxed::Box<dyn {trait_name}<{from}>>,\n    __java_marker: std::marker::PhantomData<fn({to})>,\n}}\n\nimpl<{from}: {generic_bound}, {to}: {generic_bound}> Clone for {adapter_name}<{from}, {to}> {{\n    fn clone(&self) -> Self {{ Self {{ __java_inner: std::clone::Clone::clone(&self.__java_inner), __java_marker: std::marker::PhantomData }} }}\n}}\n\nimpl<{from}: {generic_bound}, {to}: {generic_bound}> {adapter_name}<{from}, {to}> {{\n    pub fn new(__java_inner: std::boxed::Box<dyn {trait_name}<{from}>>) -> Self {{ Self {{ __java_inner, __java_marker: std::marker::PhantomData }} }}\n}}\n\nimpl<{from}: {generic_bound}, {to}: {generic_bound}> {trait_name}<{to}> for {adapter_name}<{from}, {to}> {{\n{}\n}}",
        methods.join("\n")
    ))
}

pub(crate) fn generic_interface_retarget_adapter_name(interface_name: &str) -> String {
    format!(
        "__JavaGenericRetarget{}",
        rust_name_for_java_named_type(interface_name)
    )
}

pub(crate) fn generic_interface_supports_retarget_adapter(
    program: &Program,
    interface: &TypeDecl,
) -> bool {
    let [type_param] = interface.type_params.as_slice() else {
        return false;
    };
    let contracts = specialized_interface_abstract_contracts(program, interface);
    !contracts.is_empty()
        && contracts.iter().all(|contract| {
            !contract.method.modifiers.contains(&Modifier::Static)
                && contract
                    .param_types
                    .iter()
                    .all(|ty| generic_retarget_type_is_supported(ty, type_param, &interface.name))
                && generic_retarget_type_is_supported(
                    &contract.return_type,
                    type_param,
                    &interface.name,
                )
        })
}

fn generic_retarget_type_is_supported(
    ty: &JavaType,
    type_param: &str,
    interface_name: &str,
) -> bool {
    if !java_type_mentions_type_param(ty, type_param) {
        return true;
    }
    match ty {
        JavaType::Named { name, args } if name == type_param && args.is_empty() => true,
        JavaType::Array(element) => {
            generic_retarget_type_is_supported(element, type_param, interface_name)
        }
        JavaType::Named { name, args }
            if args.len() == 1
                && (crate::facts::external_type_name_matches(name, interface_name)
                    || generic_retarget_collection_name(name)) =>
        {
            generic_retarget_type_is_supported(&args[0], type_param, interface_name)
        }
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
            generic_retarget_type_is_supported(bound, type_param, interface_name)
        }
        _ => false,
    }
}

fn generic_retarget_value_expr(
    value: &str,
    ty: &JavaType,
    type_param: &str,
    interface_name: &str,
    source_param: &str,
    target_param: &str,
    supports_erased_bound: bool,
) -> Option<String> {
    if !java_type_mentions_type_param(ty, type_param) {
        return Some(value.to_string());
    }
    match ty {
        JavaType::Named { name, args } if name == type_param && args.is_empty() => {
            Some(if supports_erased_bound {
                format!(
                    "java_retarget_erased_generic_value::<{source_param}, {target_param}>({value})"
                )
            } else {
                format!(
                    "*(std::boxed::Box::new({value}) as std::boxed::Box<dyn std::any::Any>).downcast::<{target_param}>().expect(\"invalid Java erased generic interface cast\")"
                )
            })
        }
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
            generic_retarget_value_expr(
                value,
                bound,
                type_param,
                interface_name,
                source_param,
                target_param,
                supports_erased_bound,
            )
        }
        JavaType::Array(element) => {
            let converted = generic_retarget_value_expr(
                "__java_element",
                element,
                type_param,
                interface_name,
                source_param,
                target_param,
                supports_erased_bound,
            )?;
            Some(format!(
                "{value}.into_iter().map(|__java_element| __java_element.map(|__java_element| {converted})).collect()"
            ))
        }
        JavaType::Named { name, args }
            if args.len() == 1 && generic_retarget_collection_name(name) =>
        {
            let converted = generic_retarget_value_expr(
                "__java_element",
                &args[0],
                type_param,
                interface_name,
                source_param,
                target_param,
                supports_erased_bound,
            )?;
            Some(format!(
                "{value}.into_iter().map(|__java_element| {converted}).collect()"
            ))
        }
        JavaType::Named { name, args }
            if args.len() == 1
                && crate::facts::external_type_name_matches(name, interface_name) =>
        {
            let adapter = generic_interface_retarget_adapter_name(interface_name);
            let trait_name = rust_name_for_java_named_type(interface_name);
            Some(format!(
                "std::boxed::Box::new({adapter}::<{source_param}, {target_param}>::new({value})) as std::boxed::Box<dyn {trait_name}<{target_param}>>"
            ))
        }
        _ => None,
    }
}

fn generic_retarget_collection_name(name: &str) -> bool {
    matches!(
        crate::stdlib::canonical_external_receiver_type_key(name).as_str(),
        "Collection" | "List" | "ArrayList" | "LinkedList" | "Set" | "HashSet" | "LinkedHashSet"
    )
}

fn interface_has_interior_mutable_implementor(
    program: &Program,
    interface: &TypeDecl,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> bool {
    fn visit(
        program: &Program,
        ty: &TypeDecl,
        interface: &TypeDecl,
        facts: &FactSet,
        representations: &RepresentationPlan,
    ) -> bool {
        let placement = crate::storage_strategy::effective_storage_placement(
            facts.storage_placement_for(&ty.name),
            representations.storage_placement_for(&ty.name),
        );
        (matches!(ty.kind, TypeKind::Class | TypeKind::Enum)
            && placement.shared()
            && placement.interior_mutable()
            && placement.is_implemented()
            && implemented_interfaces_for_type(program, ty).iter().any(|name| {
                crate::facts::external_type_name_matches(name, &interface.name)
            })) || ty
            .members
            .iter()
            .any(|member| matches!(member, Member::Type(nested) if visit(program, nested, interface, facts, representations)))
    }

    program
        .types
        .iter()
        .any(|ty| visit(program, ty, interface, facts, representations))
}

pub(crate) fn parameterized_interface_trait_path(
    trait_name: &str,
    type_params: &[String],
) -> String {
    if type_params.is_empty() {
        trait_name.to_string()
    } else {
        format!("{trait_name}<{}>", type_params.join(", "))
    }
}

/// An inherited Java method is viewed through the type arguments on each
/// `extends` edge.  Keep its declaration for signature/exception lookup, but
/// record the effective types separately; replacing only the owner name loses
/// `Source<String>.value(): String` when the declaration returns `T`.
pub(crate) struct SpecializedInterfaceContract<'a> {
    pub(crate) owner: &'a TypeDecl,
    pub(crate) method: &'a MethodDecl,
    pub(crate) return_type: JavaType,
    pub(crate) param_types: Vec<JavaType>,
    depth: usize,
}

fn interface_contract_return_refines(
    program: &Program,
    candidate: &JavaType,
    inherited: &JavaType,
) -> bool {
    if candidate == inherited {
        return false;
    }
    let is_object = |ty: &JavaType| {
        matches!(ty, JavaType::Named { name, .. } | JavaType::Unknown(name)
            if matches!(name.as_str(), "Object" | "java.lang.Object"))
    };
    if is_object(inherited) {
        return matches!(
            candidate,
            JavaType::String | JavaType::Named { .. } | JavaType::Array(_)
        );
    }
    let (
        JavaType::Named {
            name: candidate, ..
        },
        JavaType::Named { name: target, .. },
    ) = (candidate, inherited)
    else {
        return false;
    };
    let mut pending = vec![candidate.clone()];
    let mut seen = HashSet::new();
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(ty) = find_type_decl_for_owner_context(program, &name, &name) else {
            continue;
        };
        for parent in ty.extends.iter().chain(ty.implements.iter()) {
            let JavaType::Named { name: parent, .. } = crate::parser::parse_hierarchy_type(parent)
            else {
                continue;
            };
            if parent == *target {
                return true;
            }
            pending.push(parent);
        }
    }
    false
}

pub(crate) fn specialized_interface_abstract_contracts<'a>(
    program: &'a Program,
    interface: &'a TypeDecl,
) -> Vec<SpecializedInterfaceContract<'a>> {
    specialized_interface_contracts(program, interface, false)
}

fn specialized_interface_instance_contracts<'a>(
    program: &'a Program,
    interface: &'a TypeDecl,
) -> Vec<SpecializedInterfaceContract<'a>> {
    specialized_interface_contracts(program, interface, true)
}

fn specialized_interface_contracts<'a>(
    program: &'a Program,
    interface: &'a TypeDecl,
    include_defaults: bool,
) -> Vec<SpecializedInterfaceContract<'a>> {
    fn visit<'a>(
        program: &'a Program,
        interface: &'a TypeDecl,
        substitutions: &HashMap<String, JavaType>,
        depth: usize,
        include_defaults: bool,
        path: &mut HashSet<String>,
        out: &mut Vec<SpecializedInterfaceContract<'a>>,
    ) {
        if !path.insert(interface.name.clone()) {
            return;
        }
        for parent_name in interface.extends.iter().chain(interface.implements.iter()) {
            let JavaType::Named { name, args } = crate::parser::parse_hierarchy_type(parent_name)
            else {
                continue;
            };
            let Some(parent) = find_type_decl_for_owner_context(program, &interface.name, &name)
            else {
                continue;
            };
            if parent.kind != TypeKind::Interface {
                continue;
            }
            let parent_substitutions = parent
                .type_params
                .iter()
                .enumerate()
                .map(|(index, type_param)| {
                    let actual = args.get(index).cloned().unwrap_or_else(|| JavaType::Named {
                        name: type_param.clone(),
                        args: Vec::new(),
                    });
                    (
                        type_param.clone(),
                        crate::resolve::substitute_method_type_variables(&actual, substitutions),
                    )
                })
                .collect::<HashMap<_, _>>();
            visit(
                program,
                parent,
                &parent_substitutions,
                depth + 1,
                include_defaults,
                path,
                out,
            );
        }
        for member in &interface.members {
            let Member::Method(method) = member else {
                continue;
            };
            if (!include_defaults && method.body.is_some())
                || method.modifiers.contains(&Modifier::Static)
            {
                continue;
            }
            let visible_substitutions = substitutions
                .iter()
                .filter(|(name, _)| !method.type_params.contains(name))
                .map(|(name, ty)| (name.clone(), ty.clone()))
                .collect::<HashMap<_, _>>();
            let return_type = crate::resolve::substitute_method_type_variables(
                &method.return_type,
                &visible_substitutions,
            );
            let param_types = method
                .params
                .iter()
                .map(|param| {
                    crate::resolve::substitute_method_type_variables(
                        &param.ty,
                        &visible_substitutions,
                    )
                })
                .collect::<Vec<_>>();
            let key = (
                method.name.as_str(),
                method
                    .params
                    .iter()
                    .zip(&param_types)
                    .map(|(param, ty)| {
                        crate::java_signature::erased_type_name(ty, param.is_varargs)
                    })
                    .collect::<Vec<_>>(),
            );
            let contract = SpecializedInterfaceContract {
                owner: interface,
                method,
                return_type,
                param_types,
                depth,
            };
            if let Some(index) = out.iter().position(|existing| {
                existing.method.name == key.0
                    && existing
                        .method
                        .params
                        .iter()
                        .zip(&existing.param_types)
                        .map(|(param, ty)| {
                            crate::java_signature::erased_type_name(ty, param.is_varargs)
                        })
                        .collect::<Vec<_>>()
                        == key.1
            }) {
                // Java's combined contract keeps a covariant return even
                // when the more general parent appeared first. A child
                // declaration with the same return replaces inherited copies.
                if interface_contract_return_refines(
                    program,
                    &contract.return_type,
                    &out[index].return_type,
                ) || (contract.return_type == out[index].return_type && depth < out[index].depth)
                {
                    out[index] = contract;
                }
            } else {
                out.push(contract);
            }
        }
        path.remove(&interface.name);
    }

    let mut contracts = Vec::new();
    visit(
        program,
        interface,
        &HashMap::new(),
        0,
        include_defaults,
        &mut HashSet::new(),
        &mut contracts,
    );
    contracts
}

pub(crate) fn zero_arg_closure_trait_impl(
    ty: &TypeDecl,
    trait_name: &str,
    facts: &FactSet,
    signatures: &SignaturePlan,
    exceptions: MethodExceptionPlan,
    representations: Option<&RepresentationPlan>,
) -> Option<String> {
    // A blanket `impl<P, F: Fn() -> P> Trait<P> for F` overlaps the generic
    // `Trait<P> for Box<T>` forwarding impl whenever `Box<T>` is callable.
    // Parameterized functional interfaces therefore use the nominal adapter
    // emitted at the conversion site; keep this blanket shortcut for the
    // coherence-safe non-generic case only.
    if !ty.type_params.is_empty() {
        return None;
    }
    let method = zero_arg_functional_method(ty)?;
    let rust_name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    let return_ty = if matches!(method.return_type, JavaType::Void) {
        RustType::Unit
    } else {
        let return_shape = signatures
            .required_method_signature(ty, method)
            .return_shape()
            .cloned()
            .unwrap_or_else(|| RustSlotShape::for_storage(method.return_type.clone()));
        if let Some(representations) = representations {
            crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                &return_shape,
                facts,
                representations,
            )
        } else {
            rust_type_for_slot_with_facts(&return_shape, facts)
        }
    };
    let return_ty = if exceptions.wrap_returns_in_result() {
        exception_result_type(return_ty)
    } else {
        return_ty
    };
    let return_ty = emit_type(&return_ty);
    let fn_bound = format!("Fn() -> {return_ty}");
    let method_return = format!(" -> {return_ty}");
    let method_body = "self()".to_string();
    let impl_params = trait_impl_lifetime_params(&ty.type_params);
    let impl_params = if impl_params.is_empty() {
        "__JavaAnonymous".to_string()
    } else {
        format!("{impl_params}, __JavaAnonymous")
    };
    Some(format!(
        "impl<{impl_params}> {trait_name} for __JavaAnonymous\nwhere\n    __JavaAnonymous: {fn_bound} + Clone + 'static,\n{{\n    fn {rust_name}(&self){method_return} {{ {method_body} }}\n    fn as_any(&self) -> &dyn std::any::Any {{ self }}\n    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}> {{ std::boxed::Box::new(std::clone::Clone::clone(self)) }}\n}}"
    ))
}

pub(crate) fn zero_arg_functional_method(ty: &TypeDecl) -> Option<&MethodDecl> {
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
    representations: &RepresentationPlan,
) -> Option<String> {
    let trait_name = rust_name_for_java_named_type(trait_name);
    let impl_type_name = rust_name_for_java_named_type(&ty.name);
    let impl_type = crate::item_assembly::rust_use_name_for_source_type(ty);
    let concrete_is_clone = crate::item_assembly::source_type_supports_struct_derive_clone(
        ty,
        signatures,
        facts,
        representations,
        program,
    );
    let mut impl_params =
        crate::item_assembly::rust_impl_params_for_source_type(ty, program, Some(representations));
    if concrete_is_clone {
        for (type_param, rendered) in ty.type_params.iter().zip(impl_params.iter_mut()) {
            let declares_param = rendered == type_param
                || rendered
                    .strip_prefix(type_param)
                    .is_some_and(|suffix| suffix.starts_with(':'));
            if declares_param && !rendered.split('+').any(|bound| bound.trim() == "Clone") {
                if rendered == type_param {
                    rendered.push_str(": Clone");
                } else {
                    rendered.push_str(" + Clone");
                }
            }
        }
    }
    let impl_generics = (!impl_params.is_empty())
        .then(|| format!("<{}>", impl_params.join(", ")))
        .unwrap_or_default();
    let placement = crate::storage_strategy::effective_storage_placement(
        facts.storage_placement_for(&ty.name),
        representations.storage_placement_for(&ty.name),
    );
    let shared_adapter =
        (placement.shared() && placement.interior_mutable()).then_some(placement.sync());
    let methods = specialized_interface_instance_contracts(program, interface)
        .into_iter()
        .filter_map(|contract| {
            let method_owner = contract.owner;
            let mut effective_method = contract.method.clone();
            effective_method.return_type = contract.return_type;
            for (param, ty) in effective_method.params.iter_mut().zip(contract.param_types) {
                param.ty = ty;
            }
            trait_impl_method(
                ty,
                method_owner,
                contract.method,
                &effective_method,
                program,
                // Receiver mutability belongs to the declared override slot.
                // The specialized clone substitutes generic parameter types
                // and is not a SignaturePlan identity; looking it up can choose
                // a different receiver than the emitted trait declaration.
                trait_method_receiver_plan(method_owner, contract.method, program, signatures),
                signatures,
                facts,
                trait_method_exception_plan(method_owner, &effective_method, program, exceptions),
                representations,
                shared_adapter,
            )
        })
        .collect::<Vec<_>>();
    let mut methods = methods;
    if let Some(sync) = shared_adapter {
        methods.extend(shared_trait_object_support_impl_methods(&trait_name));
        let trait_base = trait_name.split('<').next().unwrap_or(&trait_name);
        let adapter_name = format!("__JavaShared{impl_type_name}As{trait_base}");
        let type_args = (!ty.type_params.is_empty())
            .then(|| format!("<{}>", ty.type_params.join(", ")))
            .unwrap_or_default();
        let adapter_type = format!("{adapter_name}{type_args}");
        let handle = if sync {
            format!("std::sync::Arc<std::sync::Mutex<{impl_type}>>")
        } else {
            format!("std::rc::Rc<std::cell::RefCell<{impl_type}>>")
        };
        let direct_methods = methods
            .iter()
            .map(|method| method.replace("&self.0", "self").replace("self.0", "self"))
            .collect::<Vec<_>>()
            .join("\n");
        return Some(format!(
            "#[derive(Clone)]\npub struct {adapter_name}{impl_generics}(pub {handle});\n\nimpl{impl_generics} {trait_name} for {adapter_type} {{\n{}\n}}\n\nimpl{impl_generics} {trait_name} for {handle} {{\n{direct_methods}\n}}\n\nimpl{impl_generics} From<{handle}> for std::boxed::Box<dyn {trait_name}> {{\n    fn from(value: {handle}) -> Self {{ std::boxed::Box::new({adapter_name}(value)) }}\n}}",
            methods.join("\n")
        ));
    }
    methods.extend(trait_object_support_impl_methods(
        &trait_name,
        concrete_is_clone,
    ));
    let concrete_impl = format!(
        "impl{impl_generics} {trait_name} for {impl_type} {{\n{}\n}}",
        methods.join("\n")
    );
    if placement.shared() && !placement.interior_mutable() && placement.is_implemented() {
        let handle = if placement.sync() {
            "std::sync::Arc"
        } else {
            "std::rc::Rc"
        };
        let forwarded = methods
            .iter()
            .filter_map(|method| concrete_trait_impl_method_signature(method))
            .map(|signature| {
                forwarded_trait_method(&signature, SharedTraitHandle::Direct, Some(&trait_name))
            })
            .collect::<Option<Vec<_>>>()?;
        return Some(format!(
            "{concrete_impl}\n\nimpl{impl_generics} {trait_name} for {handle}<{impl_type}> {{\n{}\n}}",
            forwarded.join("\n")
        ));
    }
    Some(concrete_impl)
}

fn concrete_trait_impl_method_signature(method: &str) -> Option<String> {
    let (signature, _) = method.split_once(" {")?;
    Some(format!("{};", signature.trim_end()))
}

pub(crate) fn trait_object_support_method_signatures(trait_name: &str) -> Vec<String> {
    vec![
        "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
        format!("    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}>;"),
    ]
}

/// Java Object methods remain callable through an interface reference even
/// though they are not declared by the interface itself.  Source interfaces
/// lower to Rust traits, so these methods belong on the trait surface; emitting
/// an inherent `impl TraitName` is invalid Rust.  Keep this policy beside the
/// other trait-object support methods so every representation path shares it.
pub(crate) fn external_object_interface_trait_default_methods(
    ty: &TypeDecl,
    trait_name: &str,
    facts: Option<&FactSet>,
    program: Option<&Program>,
) -> Vec<String> {
    let declares_to_string = ty.members.iter().any(|member| {
        matches!(member, Member::Method(method)
            if method.params.is_empty()
                && matches!(method.name.as_str(), "toString" | "to_string"))
    });
    let has_source_interface_parent = program.is_some_and(|program| {
        ty.extends.iter().chain(&ty.implements).any(|parent| {
            let JavaType::Named { name, .. } = crate::parser::parse_hierarchy_type(parent) else {
                return false;
            };
            find_type_decl_for_owner_context(program, &ty.name, &name)
                .is_some_and(|parent| parent.kind == TypeKind::Interface)
        })
    });
    let context_free_inherits_to_string = program.is_none()
        && facts.is_some_and(|facts| {
            crate::inherited_members::external_inherited_instance_methods_for_type(ty, facts)
                .into_iter()
                .any(|method| {
                    method.arg_count() == 0
                        && (matches!(method.method_name(), "toString" | "to_string")
                            || matches!(method.rust_name(), "toString" | "to_string"))
                })
        });
    if declares_to_string
        || has_source_interface_parent
        || (program.is_none() && !context_free_inherits_to_string)
    {
        return Vec::new();
    }
    vec![format!(
        "    fn to_string(&self) -> String {{ format!(\"{{}}@0\", {trait_name:?}) }}"
    )]
}

fn trait_object_support_impl_methods(trait_name: &str, concrete_is_clone: bool) -> Vec<String> {
    let clone_box = if concrete_is_clone {
        format!(
            "    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}> {{ std::boxed::Box::new(std::clone::Clone::clone(self)) }}"
        )
    } else {
        format!(
            "    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}> {{ panic!(\"java2rust: clone_box for {trait_name} requires a cloneable Java object carrier\") }}"
        )
    };
    vec![
        "    fn as_any(&self) -> &dyn std::any::Any { self }".to_string(),
        clone_box,
    ]
}

fn shared_trait_object_support_impl_methods(trait_name: &str) -> Vec<String> {
    vec![
        // A Java interface value carries the runtime identity of the concrete
        // object, not the implementation detail used to adapt that object to
        // a Rust trait object.  Expose the shared handle stored in the adapter
        // so an explicit Java interface-to-class cast can recover it.  The
        // direct shared-handle impl below rewrites `&self.0` to `self`.
        "    fn as_any(&self) -> &dyn std::any::Any { &self.0 }".to_string(),
        format!(
            "    fn clone_box(&self) -> std::boxed::Box<dyn {trait_name}> {{ std::boxed::Box::new(std::clone::Clone::clone(self)) }}"
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

pub(crate) fn boxed_dyn_trait_clone_impl_with_generics(
    trait_path: &str,
    type_params: &[String],
) -> String {
    if type_params.is_empty() {
        return boxed_dyn_trait_clone_impl(trait_path);
    }
    // Cloning a trait object only delegates to its object-safe `clone_box`
    // method. It does not downcast through `Any` or construct a new concrete
    // implementor, so the trait's generic arguments need no `'static` bound.
    // Reusing the anonymous-object lifetime parameters here unnecessarily
    // made every struct containing `Box<dyn Trait<T>>` require `T: 'static`.
    let impl_params = type_params.join(", ");
    let value_path = trait_value_path(trait_path);
    format!(
        "impl<{impl_params}> Clone for std::boxed::Box<dyn {trait_path}> {{\n    fn clone(&self) -> Self {{ {value_path}::clone_box(&**self) }}\n}}"
    )
}

fn trait_value_path(trait_path: &str) -> String {
    trait_path.replacen('<', "::<", 1)
}

fn trait_impl_lifetime_params(type_params: &[String]) -> String {
    type_params
        .iter()
        .map(|name| format!("{name}: 'static"))
        .collect::<Vec<_>>()
        .join(", ")
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
#[cfg(test)]
pub(crate) fn shared_handle_forwarding_impl(
    trait_name: &str,
    signatures: &[String],
) -> Option<String> {
    shared_handle_forwarding_impl_with_policy(trait_name, signatures, false)
}

/// A `Send`-bound interface cannot be implemented by an `Rc` wrapper or an
/// unconstrained `Arc<T>`. Its shared forwarding uses only `Arc<Mutex<T>>`;
/// the trait bound itself ensures `T: Send` for that implementation.
pub(crate) fn shared_handle_forwarding_impl_with_policy(
    trait_name: &str,
    signatures: &[String],
    sync_trait: bool,
) -> Option<String> {
    shared_handle_forwarding_impl_with_policy_and_generics(trait_name, &[], signatures, sync_trait)
}

pub(crate) fn shared_handle_forwarding_impl_with_policy_and_generics(
    trait_name: &str,
    type_params: &[String],
    signatures: &[String],
    sync_trait: bool,
) -> Option<String> {
    shared_handle_forwarding_impl_with_policy_and_generics_and_cell_policy(
        trait_name,
        type_params,
        signatures,
        sync_trait,
        true,
    )
}

fn shared_handle_forwarding_impl_with_policy_and_generics_and_cell_policy(
    trait_name: &str,
    type_params: &[String],
    signatures: &[String],
    sync_trait: bool,
    emit_cell_handle_blankets: bool,
) -> Option<String> {
    let direct_methods = signatures
        .iter()
        .map(|signature| {
            forwarded_trait_method(signature, SharedTraitHandle::Direct, Some(trait_name))
        })
        .collect::<Option<Vec<_>>>();
    let local_methods = signatures
        .iter()
        .map(|signature| {
            forwarded_trait_method(signature, SharedTraitHandle::Local, Some(trait_name))
        })
        .collect::<Option<Vec<_>>>();
    let sync_methods = signatures
        .iter()
        .map(|signature| {
            forwarded_trait_method(signature, SharedTraitHandle::Sync, Some(trait_name))
        })
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
    // A parameterized trait admits downstream implementations such as
    // `Source<Local> for RefCell<T>` under Rust's orphan rules. Blanket impls
    // for both `Rc<T>` and `Rc<RefCell<T>>` therefore overlap. Keep mutable
    // cell handles on their authoritative representation and put direct
    // immutable handles behind a generated nominal carrier.
    let impl_prefix = if type_params.is_empty() {
        String::new()
    } else {
        format!("{}, ", trait_impl_lifetime_params(type_params))
    };
    let generic_direct_carriers = (!type_params.is_empty() && !sync_trait).then(|| {
        let trait_base = trait_name.split('<').next().unwrap_or(trait_name);
        [("Local", "std::rc::Rc"), ("Sync", "std::sync::Arc")]
            .into_iter()
            .map(|(suffix, handle)| {
                let carrier = format!("__JavaDirect{trait_base}{suffix}Carrier");
                format!(
                    "pub struct {carrier}<__JavaShared>({handle}<__JavaShared>);\n\nimpl<__JavaShared> Clone for {carrier}<__JavaShared> {{\n    fn clone(&self) -> Self {{ Self(std::clone::Clone::clone(&self.0)) }}\n}}\n\nimpl<__JavaShared> From<{handle}<__JavaShared>> for {carrier}<__JavaShared> {{\n    fn from(value: {handle}<__JavaShared>) -> Self {{ Self(value) }}\n}}"
                )
            })
            .collect::<Vec<_>>()
    });
    if let Some(carriers) = &generic_direct_carriers {
        implementations.extend(carriers.iter().cloned());
    }
    if let Some(methods) = direct_methods
        && !sync_trait
    {
        if !type_params.is_empty() {
            let trait_base = trait_name.split('<').next().unwrap_or(trait_name);
            for suffix in ["Local", "Sync"] {
                let carrier = format!("__JavaDirect{trait_base}{suffix}Carrier");
                let body = methods
                    .iter()
                    .map(|method| method.replace("&**self", "&*self.0"))
                    .collect::<Vec<_>>()
                    .join("\n");
                implementations.push(format!(
                    "impl<{impl_prefix}__JavaShared: {trait_name} + 'static> {trait_name} for {carrier}<__JavaShared> {{\n{body}\n}}"
                ));
            }
        }
    }
    let cell_handles = [
        (
            "std::rc::Rc<std::cell::RefCell<__JavaShared>>",
            if sync_trait { None } else { local_methods },
        ),
        (
            "std::sync::Arc<std::sync::Mutex<__JavaShared>>",
            sync_methods,
        ),
    ];
    for (handle, methods) in cell_handles {
        if !emit_cell_handle_blankets {
            continue;
        }
        if let Some(methods) = methods {
            implementations.push(format!(
                "impl<{impl_prefix}__JavaShared: {trait_name} + 'static> {trait_name} for {handle} {{\n{}\n}}",
                methods.join("\n")
            ));
        }
    }
    (!implementations.is_empty()).then(|| implementations.join("\n\n"))
}

/// Forwards a source interface through its canonical owned trait-object
/// carrier. This is intentionally generic over the boxed implementor so a
/// boxed subinterface also satisfies its source superinterface bounds.
pub(crate) fn boxed_trait_forwarding_impl_with_generics(
    trait_name: &str,
    type_params: &[String],
    signatures: &[String],
) -> Option<String> {
    let methods = signatures
        .iter()
        .map(|signature| {
            forwarded_trait_method(signature, SharedTraitHandle::Boxed, Some(trait_name))
        })
        .collect::<Option<Vec<_>>>()?;
    let impl_prefix = if type_params.is_empty() {
        String::new()
    } else {
        format!(
            "{}, ",
            type_params
                .iter()
                .map(|name| format!("{name}: 'static"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Some(format!(
        "impl<{impl_prefix}__JavaBoxed: {trait_name} + ?Sized> {trait_name} for std::boxed::Box<__JavaBoxed> {{\n{}\n}}",
        methods.join("\n")
    ))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SharedTraitHandle {
    Direct,
    /// An owned `Box<dyn Trait>` stored in a wrapper's first field. Unlike a
    /// shared Java-reference handle, this can forward both `&self` and
    /// `&mut self` parent-interface methods.
    OwnedField,
    Local,
    Sync,
    Boxed,
}

/// Turns one rendered trait-method signature into a forwarding implementation.
///
/// The signature is text this module just produced, so the parse is closed over
/// our own output rather than arbitrary Rust: `    fn name<generics>(&self, a: A) -> R;`
fn forwarded_trait_method(
    signature: &str,
    handle: SharedTraitHandle,
    trait_name: Option<&str>,
) -> Option<String> {
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
        if handle == SharedTraitHandle::Boxed {
            let trait_name = trait_value_path(trait_name?);
            return Some(format!(
                "    {signature_without_semicolon} {{ {trait_name}::clone_box(&**self) }}"
            ));
        }
        return Some(format!(
            "    {signature_without_semicolon} {{ std::boxed::Box::new(std::clone::Clone::clone(self)) }}"
        ));
    }
    if name == "as_any" && args.is_empty() {
        if handle == SharedTraitHandle::Boxed {
            let trait_name = trait_value_path(trait_name?);
            return Some(format!(
                "    {signature_without_semicolon} {{ {trait_name}::as_any(&**self) }}"
            ));
        }
        // The handle, not its payload, is the Java reference value. Returning
        // the payload here makes an interface downcast recover and clone `T`,
        // silently severing Java alias identity (and mismatching an `Rc<T>` /
        // `Arc<T>` destination). Cell handles already expose themselves for
        // the same reason; direct handles must do so as well.
        return Some(format!("    {signature_without_semicolon} {{ self }}"));
    }
    if (signature_return_is_borrowed(signature)
        && !matches!(
            handle,
            SharedTraitHandle::Direct | SharedTraitHandle::OwnedField | SharedTraitHandle::Boxed
        ))
        || (mutable_receiver && handle == SharedTraitHandle::Direct)
    {
        return None;
    }
    let receiver = match handle {
        SharedTraitHandle::Direct => "(**self)".to_string(),
        SharedTraitHandle::OwnedField => "(*self.0)".to_string(),
        SharedTraitHandle::Local if mutable_receiver => "self.borrow_mut()".to_string(),
        SharedTraitHandle::Local => "self.borrow()".to_string(),
        SharedTraitHandle::Sync if mutable_receiver => "self.lock().unwrap()".to_string(),
        SharedTraitHandle::Sync => "self.lock().unwrap()".to_string(),
        SharedTraitHandle::Boxed => "(**self)".to_string(),
    };
    if let Some(trait_name) = trait_name {
        let trait_name = trait_value_path(trait_name);
        let borrow = if mutable_receiver { "&mut *" } else { "&*" };
        let receiver = match handle {
            SharedTraitHandle::Direct
            | SharedTraitHandle::OwnedField
            | SharedTraitHandle::Boxed => {
                if mutable_receiver {
                    if handle == SharedTraitHandle::OwnedField {
                        "&mut *self.0".to_string()
                    } else {
                        "&mut **self".to_string()
                    }
                } else {
                    if handle == SharedTraitHandle::OwnedField {
                        "&*self.0".to_string()
                    } else {
                        "&**self".to_string()
                    }
                }
            }
            SharedTraitHandle::Local | SharedTraitHandle::Sync => {
                format!("{borrow}{receiver}")
            }
        };
        let arguments = std::iter::once(receiver)
            .chain(args)
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "    {signature_without_semicolon} {{ {trait_name}::{name}({arguments}) }}"
        ))
    } else {
        Some(format!(
            "    {signature_without_semicolon} {{ {receiver}.{name}({}) }}",
            args.join(", ")
        ))
    }
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

pub(crate) fn dyn_trait_anonymous_impl_with_generics(
    trait_path: &str,
    type_params: &[String],
) -> String {
    if type_params.is_empty() {
        return dyn_trait_anonymous_impl(trait_path);
    }
    let impl_params = trait_impl_lifetime_params(type_params);
    format!(
        "impl<{impl_params}> dyn {trait_path} {{\n    pub fn anonymous() -> std::boxed::Box<dyn {trait_path}> {{ panic!(\"java2rust: anonymous class body for {trait_path} requires object lowering\") }}\n}}"
    )
}

fn trait_impl_method(
    ty: &TypeDecl,
    interface: &TypeDecl,
    declared_interface_method: &MethodDecl,
    interface_method: &MethodDecl,
    program: &Program,
    trait_receiver: Option<ReceiverPlan>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    interface_exceptions: MethodExceptionPlan,
    representations: &RepresentationPlan,
    shared_adapter: Option<bool>,
) -> Option<String> {
    let mut trait_return_shape =
        trait_method_return_shape_plan(interface, declared_interface_method, program, signatures);
    if let Some(mut specialized_shape) = specialized_interface_trait_return_shape(
        ty,
        interface,
        declared_interface_method,
        interface_method,
        program,
        signatures,
    ) {
        if let Some(solved_trait_shape) = trait_return_shape.as_ref()
            && !matches!(solved_trait_shape.dispatch, DispatchShape::Concrete)
            && matches!(specialized_shape.dispatch, DispatchShape::Concrete)
        {
            // Specialization refines the declared Java type, but the emitted
            // trait signature remains authoritative for its Rust dispatch
            // carrier. Analysis/context copies do not necessarily carry the
            // emitted interface's boxed-trait decision.
            specialized_shape.dispatch = solved_trait_shape.dispatch.clone();
        }
        trait_return_shape = Some(specialized_shape);
    }
    if trait_return_shape.as_ref().is_some_and(|shape| {
        shape.java_ty != interface_method.return_type
            && shape.optional_element_type().as_ref() != Some(&interface_method.return_type)
            && !java_types_differ_only_by_nested_optionality(
                &shape.java_ty,
                &interface_method.return_type,
            )
    }) {
        let shape = trait_return_shape.as_mut().expect("checked above");
        if java_types_share_named_erasure(&shape.java_ty, &interface_method.return_type) {
            // Full-package analysis can reach the emitted interface through a
            // generic declaration copy (`Product<T>`) while the effective
            // contract has already been specialized (`Product<Value>`).
            // Substitute the Java type without discarding the solved boxed-
            // trait dispatch: the latter is the trait ABI we must implement.
            shape.java_ty = interface_method.return_type.clone();
        } else {
            trait_return_shape = Some(crate::shape_projection::storage_shape_for_java_type(
                representations,
                interface_method.return_type.clone(),
            ));
        }
    }
    let trait_return_shape_for_body = trait_return_shape.clone();
    let specialized_param_types = interface_method
        .params
        .iter()
        .map(|param| param.ty.clone())
        .collect::<Vec<_>>();
    let signature = trait_method_signature_with_receiver_and_return_shape_and_params(
        interface,
        declared_interface_method,
        signatures,
        facts,
        interface_exceptions,
        trait_receiver,
        trait_return_shape,
        program,
        Some(&specialized_param_types),
        Some(representations),
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
    let covariant_return = |candidate: &MethodDecl| {
        interface_contract_return_refines(
            program,
            &candidate.return_type,
            &interface_method.return_type,
        ) && trait_impl_method_compatibility_error(interface_method, candidate)
            == Some(TraitImplMethodCompatibilityError::ReturnTypeMismatch)
    };
    let compatible = candidates.iter().copied().find(|(_, candidate)| {
        trait_impl_method_compatibility_error(interface_method, candidate).is_none()
            || covariant_return(candidate)
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
            .filter(|_| !covariant_return(implementing_method))
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
    let interface_signature =
        signatures.required_method_signature(interface, declared_interface_method);
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
    let mut trait_param_shapes =
        trait_method_param_shape_plan(interface, declared_interface_method, program, signatures);
    for (index, param) in interface_method.params.iter().enumerate() {
        if trait_param_shapes
            .get(index)
            .is_some_and(|shape| !trait_shape_has_declared_java_type(shape, &param.ty))
        {
            trait_param_shapes[index] = RustSlotShape::for_storage(param.ty.clone());
        }
    }
    let args = interface_signature
        .params()
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let implementation_shape = implementation_signature
                .params()
                .get(index)
                .map(|param| param.shape());
            let trait_shape = trait_param_shapes.get(index).unwrap_or(&param.shape());
            trait_impl_forwarded_arg(param.name(), trait_shape, implementation_shape)
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
    let call = if let Some(sync) = shared_adapter {
        if forwarding_owner.name == ty.name {
            let comma_args = (!args.is_empty())
                .then(|| format!(", {args}"))
                .unwrap_or_default();
            format!(
                "{implementing_owner_name}::{rust_name}_with_outer(std::clone::Clone::clone(&self.0){comma_args})"
            )
        } else {
            let borrowed = if sync {
                "self.0.lock().unwrap()"
            } else if matches!(trait_receiver, Some(ReceiverPlan::Mutable)) {
                "self.0.borrow_mut()"
            } else {
                "self.0.borrow()"
            };
            let borrow = if matches!(trait_receiver, Some(ReceiverPlan::Mutable)) {
                "&mut *"
            } else {
                "&*"
            };
            let receiver = format!("{borrow}{borrowed}");
            let receiver_arg = if args.is_empty() {
                receiver
            } else {
                format!("{receiver}, {args}")
            };
            format!("{implementing_owner_name}::{rust_name}({receiver_arg})")
        }
    } else {
        format!("{implementing_owner_name}::{rust_name}({receiver_arg})")
    };
    let call = if covariant_return(implementing_method) {
        adapt_covariant_trait_impl_return(
            call,
            trait_return_shape_for_body
                .as_ref()
                .or(interface_signature.return_shape()),
            &implementation_signature.return_shape_cloned(),
            interface_exceptions,
            facts,
            representations,
            program,
        )
    } else {
        call
    };
    let call = if covariant_return(implementing_method) {
        call
    } else {
        adapt_trait_impl_forwarded_return(
            call,
            trait_return_shape_for_body
                .as_ref()
                .or(interface_signature.return_shape()),
            &implementation_signature.return_shape_cloned(),
            interface_exceptions,
        )
    };
    let body = if interface_exceptions.wrap_returns_in_result()
        || signatures
            .required_method_signature(interface, declared_interface_method)
            .return_shape()
            .is_some()
    {
        call
    } else {
        format!("{call};")
    };
    Some(format!("{} {{ {body} }}", signature.trim_end_matches(';')))
}

fn java_types_share_named_erasure(left: &JavaType, right: &JavaType) -> bool {
    match (left, right) {
        (
            JavaType::Named {
                name: left_name,
                args: left_args,
            },
            JavaType::Named {
                name: right_name,
                args: right_args,
            },
        ) => {
            left_args.len() == right_args.len()
                && crate::facts::external_type_name_matches(left_name, right_name)
        }
        _ => false,
    }
}

/// Specialize the already-solved generic trait ABI for one implementor.
///
/// Re-projecting directly from the Java declaration loses solver decisions
/// nested below collections (for example `List<Optional<List<O>>>`).  Keep the
/// solved shape and substitute only its declared interface parameters, using
/// the resolver's canonical recursive substitution.
fn specialized_interface_trait_return_shape(
    target: &TypeDecl,
    interface: &TypeDecl,
    declared_interface_method: &MethodDecl,
    effective_interface_method: &MethodDecl,
    program: &Program,
    signatures: &SignaturePlan,
) -> Option<RustSlotShape> {
    let original_interface =
        find_type_decl_for_owner_context(program, &target.name, &interface.name)?;
    let original_method = original_interface.members.iter().find_map(|member| {
        let Member::Method(method) = member else {
            return None;
        };
        (method.name == declared_interface_method.name
            && method.params.len() == declared_interface_method.params.len())
        .then_some(method)
    })?;
    let args = interface_type_arguments_in_target(program, target, original_interface)?;
    if original_interface.type_params.len() != args.len() {
        return None;
    }
    let substitutions = original_interface
        .type_params
        .iter()
        .cloned()
        .zip(args)
        .collect::<HashMap<_, _>>();
    let mut shape =
        trait_method_return_shape_plan(original_interface, original_method, program, signatures)?;
    shape.java_ty =
        crate::resolve::substitute_method_type_variables(&shape.java_ty, &substitutions);
    java_types_differ_only_by_nested_optionality(
        &shape.java_ty,
        &effective_interface_method.return_type,
    )
    .then_some(shape)
}

fn java_types_differ_only_by_nested_optionality(left: &JavaType, right: &JavaType) -> bool {
    if let Some(inner) = crate::types::optional_element_type(left) {
        return java_types_differ_only_by_nested_optionality(&inner, right);
    }
    if let Some(inner) = crate::types::optional_element_type(right) {
        return java_types_differ_only_by_nested_optionality(left, &inner);
    }
    match (left, right) {
        (JavaType::Array(left), JavaType::Array(right)) => {
            java_types_differ_only_by_nested_optionality(left, right)
        }
        (
            JavaType::Named {
                name: left_name,
                args: left_args,
            },
            JavaType::Named {
                name: right_name,
                args: right_args,
            },
        ) => {
            crate::facts::external_type_name_matches(left_name, right_name)
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| java_types_differ_only_by_nested_optionality(left, right))
        }
        (JavaType::Wildcard(left), JavaType::Wildcard(right)) => match (left, right) {
            (None, None) => true,
            (
                Some(crate::java_ir::WildcardBound::Extends(left)),
                Some(crate::java_ir::WildcardBound::Extends(right)),
            )
            | (
                Some(crate::java_ir::WildcardBound::Super(left)),
                Some(crate::java_ir::WildcardBound::Super(right)),
            ) => java_types_differ_only_by_nested_optionality(left, right),
            _ => false,
        },
        _ => left == right,
    }
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
    if let Some(trait_element_ty) = trait_shape.optional_element_type() {
        if let Some(implementation_shape) = implementation_return_shape
            && implementation_shape.optional_element_type().is_none()
            && implementation_shape.java_ty == trait_element_ty
        {
            return format!("Some({call})");
        }
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

fn adapt_covariant_trait_impl_return(
    call: String,
    trait_return_shape: Option<&RustSlotShape>,
    implementation_return_shape: &Option<RustSlotShape>,
    interface_exceptions: MethodExceptionPlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
    program: &Program,
) -> String {
    let Some(trait_shape) = trait_return_shape else {
        return call;
    };
    let projected_trait_shape = crate::shape_projection::project_java_value_shape(
        Some(representations),
        trait_shape.java_ty.clone(),
    );
    let trait_dispatch = if matches!(trait_shape.dispatch, DispatchShape::Concrete)
        && !matches!(projected_trait_shape.dispatch, DispatchShape::Concrete)
    {
        &projected_trait_shape.dispatch
    } else {
        &trait_shape.dispatch
    };
    let implementation_shared_adapter = implementation_return_shape
        .as_ref()
        .and_then(|shape| {
            shape
                .optional_element_type()
                .or_else(|| Some(shape.java_ty.clone()))
        })
        .and_then(|java_ty| match java_ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
            _ => None,
        })
        .is_some_and(|type_name| {
            let placement = representations.storage_placement_for(&type_name);
            placement.shared() && placement.interior_mutable() && placement.is_implemented()
        });
    let implementation_closed_carrier_variant = implementation_return_shape
        .as_ref()
        .and_then(|shape| {
            shape
                .optional_element_type()
                .or_else(|| Some(shape.java_ty.clone()))
        })
        .and_then(|java_ty| match java_ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => {
                representations.source_hierarchy_carrier_parts_for_variant(&name)
            }
            _ => None,
        });
    let conversion = |value: &str| match trait_dispatch {
        DispatchShape::BoxedDynTrait { trait_name } | DispatchShape::DynTrait { trait_name } => {
            // The dispatch name identifies the declaration that owns the
            // trait object.  Do not copy accidental arguments from a solved
            // use-site shape onto a declaration which is not generic (for
            // example `BlockData<Box<dyn BlockData>>`).  Source declaration
            // arity is authoritative; external traits without a declaration
            // retain the representation-aware fallback.
            let declared_trait_is_non_generic =
                source_declared_type_arity_by_emitted_name(program, trait_name) == Some(0);
            let trait_path = if declared_trait_is_non_generic {
                trait_name.clone()
            } else {
                crate::slot_render::parameterized_dyn_trait_path(
                    trait_name,
                    &trait_shape.java_ty,
                    &crate::effective_facts::EffectiveFacts::new(facts),
                    representations,
                )
            };
            if implementation_shared_adapter {
                // Shared source implementors expose a nominal adapter and a
                // matching `From<Handle>` implementation.  Reuse that
                // canonical boundary: boxing the Rc/RefCell handle directly
                // asks the handle itself to implement the source interface.
                format!("std::convert::Into::<std::boxed::Box<dyn {trait_path}>>::into({value})")
            } else {
                format!("std::boxed::Box::new({value}) as std::boxed::Box<dyn {trait_path}>")
            }
        }
        DispatchShape::ErasedObject => {
            format!("std::boxed::Box::new({value}) as std::boxed::Box<dyn std::any::Any>")
        }
        DispatchShape::ClosedEnum { enum_name } => {
            if let Some((implementation_carrier, variant)) =
                implementation_closed_carrier_variant.as_ref()
                && implementation_carrier == enum_name
            {
                format!("{enum_name}::{variant}({value})")
            } else {
                value.to_string()
            }
        }
        _ => value.to_string(),
    };
    let implementation_is_optional = implementation_return_shape
        .as_ref()
        .is_some_and(|shape| shape.optional_element_type().is_some());
    let trait_is_optional = trait_shape.optional_element_type().is_some();
    let adapt_value = |value: &str| match (implementation_is_optional, trait_is_optional) {
        (true, true) => format!(
            "{value}.map(|__java_covariant| {})",
            conversion("__java_covariant")
        ),
        (false, true) => format!("Some({})", conversion(value)),
        (true, false) => conversion(&format!("{value}.expect(\"Java null receiver\")")),
        (false, false) => conversion(value),
    };
    if interface_exceptions.wrap_returns_in_result() {
        format!("{call}.map(|__java_value| {})", adapt_value("__java_value"))
    } else {
        adapt_value(&call)
    }
}

/// Return a source declaration's generic arity even when focused-package
/// analysis carries multiple equivalent declaration copies. A uniqueness-only
/// lookup rejects those copies as ambiguous, but their common arity is still
/// authoritative for emitted Rust trait syntax.
fn source_declared_type_arity_by_emitted_name(
    program: &Program,
    emitted_name: &str,
) -> Option<usize> {
    fn collect(ty: &TypeDecl, emitted_name: &str, arities: &mut HashSet<usize>) {
        if crate::item_assembly::rust_use_name_for_source_type(ty) == emitted_name {
            arities.insert(ty.type_params.len());
        }
        for member in &ty.members {
            if let Member::Type(nested) = member {
                collect(nested, emitted_name, arities);
            }
        }
    }

    let mut arities = HashSet::new();
    for ty in &program.types {
        collect(ty, emitted_name, &mut arities);
    }
    (arities.len() == 1).then(|| *arities.iter().next().expect("one source type arity"))
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
                "&mut {{ let mut __java_super: {owner_name} = std::clone::Clone::clone(self).into(); __java_super }}"
            )
        }
        ReceiverPlan::Shared | ReceiverPlan::None => {
            format!(
                "&{{ let __java_super: {owner_name} = std::clone::Clone::clone(self).into(); __java_super }}"
            )
        }
    }
}

fn forwarded_arg_requires_owned_view_materialization(
    boundary_shape: &RustSlotShape,
    implementation_shape: &RustSlotShape,
) -> bool {
    boundary_shape.java_ty == implementation_shape.java_ty
        && boundary_shape.ownership == OwnershipShape::SharedBorrow
        && matches!(boundary_shape.view, ViewShape::Slice | ViewShape::Str)
        && implementation_shape.view == ViewShape::Native
        && matches!(
            implementation_shape.ownership,
            OwnershipShape::Owned | OwnershipShape::SharedBorrow
        )
}

fn trait_impl_forwarded_arg(
    arg_name: &str,
    trait_shape: &RustSlotShape,
    implementation_shape: Option<&RustSlotShape>,
) -> String {
    let arg_name = rust_identifier_token(arg_name);
    if implementation_shape.is_some_and(|shape| {
        trait_shape.java_ty == shape.java_ty
            && trait_shape.ownership == OwnershipShape::Owned
            && trait_shape.view == ViewShape::Native
            && shape.ownership == OwnershipShape::MutableBorrow
            && shape.view == ViewShape::Native
    }) {
        // The public hierarchy slot owns the argument, while a concrete
        // override may only mutate it for the duration of the call. Borrow a
        // moved temporary so the carrier parameter itself need not be declared
        // `mut`, and each match arm can apply the same adapter independently.
        return format!("&mut {{ {arg_name} }}");
    }
    if implementation_shape.is_some_and(|shape| {
        trait_shape.optional_element_type().is_some()
            && trait_shape.java_ty == shape.java_ty
            && trait_shape.ownership == OwnershipShape::Owned
            && trait_shape.view == ViewShape::Native
            && shape.ownership == OwnershipShape::SharedBorrow
            && matches!(shape.view, ViewShape::Slice | ViewShape::Str)
    }) {
        return format!("{arg_name}.as_deref()");
    }
    if implementation_shape.is_some_and(|shape| {
        trait_shape.ownership == OwnershipShape::Owned
            && trait_shape.view == ViewShape::Native
            && shape.ownership == OwnershipShape::SharedBorrow
            && shape.view == ViewShape::Str
            && shape.java_ty == trait_shape.java_ty
    }) {
        return format!("&{arg_name}");
    }
    if implementation_shape.is_some_and(|shape| {
        trait_shape.ownership == OwnershipShape::Owned
            && trait_shape.view == ViewShape::Native
            && shape.ownership == OwnershipShape::SharedBorrow
            && shape.view == ViewShape::Slice
            && shape.java_ty == trait_shape.java_ty
    }) {
        return format!("&{arg_name}");
    }
    if implementation_shape.is_some_and(|shape| {
        trait_shape.ownership == OwnershipShape::SharedBorrow
            && trait_shape.view == ViewShape::Slice
            && shape.ownership == OwnershipShape::SharedBorrow
            && shape.view == ViewShape::Native
            && shape.java_ty == trait_shape.java_ty
    }) {
        return format!("&{arg_name}.to_vec()");
    }
    if implementation_shape.is_some_and(|shape| {
        trait_shape.ownership == OwnershipShape::SharedBorrow
            && trait_shape.view == ViewShape::Str
            && shape.ownership == OwnershipShape::SharedBorrow
            && shape.view == ViewShape::Native
            && shape.java_ty == trait_shape.java_ty
    }) {
        return format!("&{arg_name}.to_string()");
    }
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
    arg_name
}

#[cfg(test)]
mod trait_impl_forwarded_arg_tests {
    use super::*;

    #[test]
    fn owned_trait_values_borrow_for_string_and_array_views() {
        let owned_string = RustSlotShape::for_storage(JavaType::String);
        let mut borrowed_string = owned_string.clone();
        borrowed_string.ownership = OwnershipShape::SharedBorrow;
        borrowed_string.view = ViewShape::Str;
        assert_eq!(
            trait_impl_forwarded_arg("sample", &owned_string, Some(&borrowed_string)),
            "&sample"
        );

        let array = JavaType::Array(Box::new(JavaType::Primitive(
            crate::java_ir::PrimitiveType::Int,
        )));
        let owned_array = RustSlotShape::for_storage(array);
        let mut borrowed_slice = owned_array.clone();
        borrowed_slice.ownership = OwnershipShape::SharedBorrow;
        borrowed_slice.view = ViewShape::Slice;
        assert_eq!(
            trait_impl_forwarded_arg("values", &owned_array, Some(&borrowed_slice)),
            "&values"
        );

        assert_eq!(
            trait_impl_forwarded_arg("values", &borrowed_slice, Some(&owned_array)),
            "values.to_vec()"
        );

        let owned_writer = RustSlotShape::for_storage(JavaType::Named {
            name: "Writer".to_string(),
            args: Vec::new(),
        });
        let mut mutably_borrowed_writer = owned_writer.clone();
        mutably_borrowed_writer.ownership = OwnershipShape::MutableBorrow;
        assert_eq!(
            trait_impl_forwarded_arg("writer", &owned_writer, Some(&mutably_borrowed_writer),),
            "&mut { writer }"
        );

        let mut borrowed_native_array = owned_array.clone();
        borrowed_native_array.ownership = OwnershipShape::SharedBorrow;
        assert_eq!(
            trait_impl_forwarded_arg("values", &borrowed_slice, Some(&borrowed_native_array)),
            "&values.to_vec()"
        );
    }
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
        .map(|param| rust_identifier_token(&param.name))
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
    trait_method_signature_with_receiver_and_return_shape_and_params(
        ty,
        method,
        signatures,
        facts,
        exceptions,
        receiver_override,
        return_shape_override,
        program,
        None,
        None,
    )
}

pub(crate) fn trait_method_signature_with_receiver_and_return_shape_and_params(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    receiver_override: Option<ReceiverPlan>,
    return_shape_override: Option<RustSlotShape>,
    program: &Program,
    specialized_param_types: Option<&[JavaType]>,
    representations: Option<&RepresentationPlan>,
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
        let specialized = specialized_param_types
            .and_then(|types| types.get(index))
            .filter(|ty| !trait_shape_has_declared_java_type(param.shape(), ty))
            .map(|ty| RustSlotShape::for_storage(ty.clone()));
        let shape = specialized
            .as_ref()
            .or_else(|| param_shapes.get(index))
            .unwrap_or(&param.shape());
        format!(
            "{}: {}",
            rust_identifier_token(param.name()),
            emit_type(&if let Some(representations) = representations {
                crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                    shape,
                    facts,
                    representations,
                )
            } else {
                rust_type_for_slot_with_facts(shape, facts)
            })
        )
    }));
    let return_type = trait_method_return_type_with_exception_plan(
        method,
        signatures,
        ty,
        facts,
        exceptions,
        return_shape_override,
        representations,
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
            let Some(Some(bound)) = method.type_param_bounds.get(index) else {
                return name.clone();
            };
            let bounds = crate::item_assembly::rust_bounds_for_java_type_param(bound, &[program]);
            if bounds.is_empty() {
                // Preserve the owner-aware fallback for ambiguous simple source
                // names; the central projector intentionally fails closed.
                if let JavaType::Named { name: bound, .. } = bound {
                    return match find_type_decl_for_owner_context(program, owner_name, bound) {
                        Some(ty) if matches!(ty.kind, TypeKind::Interface) => {
                            format!("{name}: {}", rust_name_for_java_named_type(&ty.name))
                        }
                        Some(ty) if matches!(ty.kind, TypeKind::Class) => {
                            format!("{name}: {}", source_class_bound_trait_name(&ty.name))
                        }
                        _ => name.clone(),
                    };
                }
                name.clone()
            } else {
                format!("{name}: {}", bounds.join(" + "))
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
    let interface_shape =
        trait_shape_with_declared_generic_contract(interface_shape, &interface_method.return_type);
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
            return Some(trait_shape_with_declared_generic_contract(
                implementation_shape,
                &interface_method.return_type,
            ));
        }
    }
    Some(interface_shape)
}

fn trait_shape_with_declared_generic_contract(
    mut shape: RustSlotShape,
    declared: &JavaType,
) -> RustSlotShape {
    fn restore(actual: &JavaType, declared: &JavaType) -> Option<JavaType> {
        if let Some(actual_element) = crate::types::optional_element_type(actual) {
            return restore(&actual_element, declared).map(crate::types::optional_java_type);
        }
        let (
            JavaType::Named {
                name: actual_name,
                args: actual_args,
            },
            JavaType::Named {
                name: declared_name,
                args: declared_args,
            },
        ) = (actual, declared)
        else {
            return None;
        };
        (actual_args.is_empty()
            && !declared_args.is_empty()
            && crate::facts::external_type_name_matches(actual_name, declared_name))
        .then(|| declared.clone())
    }

    if let Some(restored) = restore(&shape.java_ty, declared) {
        // Signature solving may strengthen nullability, ownership, view, or
        // dispatch from implementations and call sites. Those are valid ABI
        // decisions, but Java erasure must not discard type arguments that
        // are explicit in the interface declaration: every implementation of
        // `Iterator<T>` still shares the same `T` contract.
        shape.java_ty = restored;
    }
    shape
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
    // A bounded Java method type parameter is still a reference value.  Keep
    // that aliasing contract in the Rust trait ABI instead of consuming the
    // generic value.  Unbounded method parameters retain their existing owned
    // ABI (which is also used by identity-style generic methods).
    for (index, param) in interface_method.params.iter().enumerate() {
        let JavaType::Named { name, args } = &param.ty else {
            continue;
        };
        if !args.is_empty() {
            continue;
        }
        let Some(type_param_index) = interface_method
            .type_params
            .iter()
            .position(|type_param| type_param == name)
        else {
            continue;
        };
        if interface_method
            .type_param_bounds
            .get(type_param_index)
            .is_some_and(Option::is_some)
            && let Some(shape) = param_shapes.get_mut(index)
        {
            shape.ownership = OwnershipShape::SharedBorrow;
        }
    }
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

fn trait_shape_has_declared_java_type(shape: &RustSlotShape, declared: &JavaType) -> bool {
    shape.java_ty == *declared || shape.optional_element_type().as_ref() == Some(declared)
}

fn trait_param_shape_should_widen_to_implementation(
    interface_shape: &RustSlotShape,
    implementation_shape: &RustSlotShape,
) -> bool {
    if implementation_shape
        .optional_element_type()
        .is_some_and(|element_ty| element_ty == interface_shape.java_ty)
    {
        return true;
    }
    if interface_shape.java_ty == implementation_shape.java_ty
        && matches!(interface_shape.dispatch, DispatchShape::Concrete)
        && matches!(
            implementation_shape.dispatch,
            DispatchShape::ClosedEnum { .. }
        )
    {
        return true;
    }
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
    if interface_shape.java_ty == implementation_shape.java_ty
        && matches!(interface_shape.dispatch, DispatchShape::Concrete)
        && matches!(
            implementation_shape.dispatch,
            DispatchShape::ClosedEnum { .. }
        )
    {
        // The concrete implementation and the generated trait must name the
        // same Rust ABI. A source hierarchy carrier is not covariant metadata:
        // retaining the interface's stale Concrete tag renders the payload's
        // Rc/Arc storage type while the implementation returns the carrier.
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
    // Receiver capability is solved once for the whole batch. The signature
    // fixpoint joins override groups, generic interface implementations, and
    // equivalent qualified/nested owner aliases while deliberately widening
    // only the trait slot (not immutable concrete implementations). Repeating
    // those whole-program traversals during every trait emission is both
    // expensive and risks diverging from the call-site ABI.
    let _ = program;
    Some(
        signatures
            .required_method_signature(interface, interface_method)
            .receiver(),
    )
}

fn collect_type_decls_for_trait_receiver<'a>(ty: &'a TypeDecl, out: &mut Vec<&'a TypeDecl>) {
    out.push(ty);
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_decls_for_trait_receiver(nested, out);
        }
    }
}

/// Concrete source methods implementing one interface slot, using the same
/// generic-erasure compatibility rule as trait emission. Signature solving
/// also consumes this relation so calls through interface-typed fields see
/// the exact receiver ABI the emitted trait will expose.
pub(crate) fn source_interface_method_implementations<'a>(
    interface: &'a TypeDecl,
    interface_method: &'a MethodDecl,
    program: &'a Program,
) -> Vec<(&'a TypeDecl, &'a MethodDecl)> {
    let mut types = Vec::new();
    for ty in &program.types {
        collect_type_decls_for_trait_receiver(ty, &mut types);
    }
    let method_uses_type_param = interface.type_params.iter().any(|type_param| {
        java_type_mentions_named_type(&interface_method.return_type, type_param)
            || interface_method
                .params
                .iter()
                .any(|param| java_type_mentions_named_type(&param.ty, type_param))
    });
    types
        .into_iter()
        .filter(|ty| {
            ty.kind == TypeKind::Class
                && implemented_interfaces_for_type(program, ty)
                    .iter()
                    .any(|implemented| {
                        crate::facts::external_type_name_matches(implemented, &interface.name)
                    })
        })
        .filter_map(|implementor| {
            implementing_method_candidates_in_hierarchy(implementor, interface_method, program)
                .into_iter()
                .find(|candidate| {
                    trait_impl_method_compatibility_error(interface_method, candidate.1).is_none()
                        || (method_uses_type_param
                            && candidate.1.params.len() == interface_method.params.len()
                            && candidate
                                .1
                                .params
                                .iter()
                                .zip(&interface_method.params)
                                .all(|(actual, expected)| actual.is_varargs == expected.is_varargs))
                })
        })
        .collect()
}

fn java_type_mentions_named_type(ty: &JavaType, name: &str) -> bool {
    match ty {
        JavaType::Named {
            name: candidate,
            args,
        } => {
            candidate == name
                || args
                    .iter()
                    .any(|arg| java_type_mentions_named_type(arg, name))
        }
        JavaType::Array(element) => java_type_mentions_named_type(element, name),
        _ => false,
    }
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
    representations: Option<&RepresentationPlan>,
) -> Option<RustType> {
    let return_shape = return_shape_override
        .or_else(|| {
            signatures
                .required_method_signature(ty, method)
                .return_shape()
                .cloned()
        })
        .map(|shape| trait_shape_with_declared_generic_contract(shape, &method.return_type));
    let rust_return = return_shape.as_ref().map(|shape| {
        if let Some(representations) = representations {
            crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                shape,
                facts,
                representations,
            )
        } else {
            rust_type_for_slot_with_facts(shape, facts)
        }
    });
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
    let mut seen = HashSet::new();
    source_type_param_bounds(program)
        .into_iter()
        .filter_map(|bound| {
            find_type_decl_for_owner_context(program, &bound.owner_name, &bound.bound_name)
        })
        .filter(|bound| {
            matches!(bound.kind, TypeKind::Class)
                && (bound.name == ty.name || source_type_extends(program, ty, &bound.name))
                && seen.insert(bound.name.clone())
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
    fn collect_one(
        owner_name: &str,
        bound: &JavaType,
        seen: &mut HashSet<SourceTypeParamBound>,
        out: &mut Vec<SourceTypeParamBound>,
    ) {
        if let JavaType::Intersection(bounds) = bound {
            for bound in bounds {
                collect_one(owner_name, bound, seen, out);
            }
            return;
        }
        if let JavaType::Named { name, .. } = bound {
            let bound = SourceTypeParamBound {
                owner_name: owner_name.to_string(),
                bound_name: name.clone(),
            };
            if seen.insert(bound.clone()) {
                out.push(bound);
            }
        }
    }
    for bound in bounds.iter().flatten() {
        collect_one(owner_name, bound, seen, out);
    }
}

fn source_type_extends(program: &Program, ty: &TypeDecl, target_name: &str) -> bool {
    let mut next = ty.extends.as_deref();
    let mut owner_name = ty.name.as_str();
    while let Some(name) = next {
        let erased_name = name.split('<').next().unwrap_or(name).trim();
        if erased_name == target_name {
            return true;
        }
        let Some(super_ty) = find_type_decl_for_owner_context(program, owner_name, erased_name)
        else {
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
    let mut exact_matches = Vec::new();
    for ty in &program.types {
        collect_exact_type_decl_matches(ty, type_name, &mut exact_matches);
    }
    if let Some(exact) = unique_type_decl(exact_matches) {
        return Some(exact);
    }
    let mut matches = TypeDeclMatches::default();
    for ty in &program.types {
        collect_type_decl_matches(ty, type_name, program.package.as_deref(), &mut matches);
    }
    unique_type_decl(matches.exact)
        .or_else(|| unique_type_decl(matches.package_qualified_emitted_alias))
        .or_else(|| unique_type_decl(matches.source_qualified_alias))
        .or_else(|| unique_type_decl(matches.emitted_alias))
        .or_else(|| unique_type_decl(matches.simple))
}

fn find_type_decl_for_owner_context<'a>(
    program: &'a Program,
    owner_name: &str,
    type_name: &str,
) -> Option<&'a TypeDecl> {
    let mut exact_matches = Vec::new();
    for ty in &program.types {
        collect_exact_type_decl_matches(ty, type_name, &mut exact_matches);
    }
    if let Some(exact) = unique_type_decl(exact_matches) {
        return Some(exact);
    }
    let mut matches = TypeDeclMatches::default();
    for ty in &program.types {
        collect_type_decl_matches(ty, type_name, program.package.as_deref(), &mut matches);
    }
    unique_type_decl(matches.exact)
        .or_else(|| unique_type_decl(matches.package_qualified_emitted_alias))
        .or_else(|| unique_type_decl(matches.source_qualified_alias))
        .or_else(|| unique_same_package_type_decl(matches.simple.as_slice(), owner_name))
        .or_else(|| unique_type_decl(matches.emitted_alias))
        .or_else(|| unique_type_decl(matches.simple))
}

fn collect_exact_type_decl_matches<'a>(
    ty: &'a TypeDecl,
    type_name: &str,
    matches: &mut Vec<&'a TypeDecl>,
) {
    if ty.name == type_name {
        matches.push(ty);
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_exact_type_decl_matches(nested, type_name, matches);
        }
    }
}

#[derive(Default)]
struct TypeDeclMatches<'a> {
    exact: Vec<&'a TypeDecl>,
    package_qualified_emitted_alias: Vec<&'a TypeDecl>,
    source_qualified_alias: Vec<&'a TypeDecl>,
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
    package: Option<&str>,
    matches: &mut TypeDeclMatches<'a>,
) {
    let simple_name = ty.name.rsplit('.').next().unwrap_or(&ty.name);
    if ty.name == type_name {
        matches.exact.push(ty);
    } else if package.is_some_and(|package| {
        rust_name_for_java_named_type(&format!("{package}.{}", ty.name)) == type_name
    }) {
        matches.package_qualified_emitted_alias.push(ty);
    } else if type_name
        .strip_suffix(&ty.name)
        .is_some_and(|prefix| prefix.ends_with('.'))
    {
        matches.source_qualified_alias.push(ty);
    } else if rust_name_for_java_named_type(&ty.name) == type_name {
        matches.emitted_alias.push(ty);
    } else if simple_name == type_name || rust_name_for_java_named_type(simple_name) == type_name {
        matches.simple.push(ty);
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_decl_matches(nested, type_name, package, matches);
        }
    }
}

#[cfg(test)]
mod type_emission_tests {
    use super::{
        RepresentationTypeEmission, TraitImplMethodCompatibilityError,
        TraitImplMethodFallbackPolicy, TraitImplMethodMessagePolicy,
        adapt_covariant_trait_impl_return, interface_ancestor_instantiations,
        raw_type_item_for_with_facts, source_hierarchy_carrier_forwarding_method,
        source_hierarchy_carrier_item_for_type, source_hierarchy_carrier_receiver_plan,
        source_hierarchy_virtual_dispatch_trait_item_for_type,
        specialized_interface_abstract_contracts, trait_impl_emissions_for_type_with_facts,
        trait_impl_method_message_policy, trait_item_for_type_with_program,
        type_emission_for_program_with_facts_and_exceptions, type_emission_for_with_facts,
    };
    use crate::exception_plan::{MethodExceptionPlan, ProgramExceptionPlan};
    use crate::facts::{
        FactProvenance, FactSet, HierarchyOverride, HierarchyStrategy, TypeCapabilityOverride,
    };
    use crate::java_ir::{JavaType, Member};
    use crate::parser::parse_java_source;
    use crate::representation::{
        ObjectRepresentation, RepresentationPlan, RepresentationProvenance, TypeRepresentation,
        plan_representations,
    };
    use crate::resolve::resolve_program;
    use crate::semantic_analysis::analyze_program;
    use crate::signature::{ReceiverPlan, SignaturePlan, solve_signatures_with_facts};
    use crate::slot_shape::{DispatchShape, RustSlotShape};
    use crate::storage_strategy::TypeStorageFacts;
    use crate::usage::{analyze_usage, analyze_usage_with_facts};

    #[test]
    fn covariant_return_does_not_parameterize_a_non_generic_trait_declaration() {
        let parsed = parse_java_source(
            "interface BlockData {} class ConcreteBlockData implements BlockData {}",
        );
        let mut trait_shape = RustSlotShape::for_storage(JavaType::Named {
            name: "BlockData".to_string(),
            // Model a polluted solved use-site shape: declaration arity must
            // still decide whether the emitted trait path is parameterized.
            args: vec![JavaType::Named {
                name: "BlockData".to_string(),
                args: Vec::new(),
            }],
        });
        trait_shape.dispatch = DispatchShape::BoxedDynTrait {
            trait_name: "BlockData".to_string(),
        };
        let implementation_shape = Some(RustSlotShape::for_storage(JavaType::Named {
            name: "ConcreteBlockData".to_string(),
            args: Vec::new(),
        }));

        let rendered = adapt_covariant_trait_impl_return(
            "Self::make()".to_string(),
            Some(&trait_shape),
            &implementation_shape,
            MethodExceptionPlan::new(false),
            &FactSet::default(),
            &RepresentationPlan::default(),
            &parsed.program,
        );

        assert!(rendered.contains("Box<dyn BlockData>"), "{rendered}");
        assert!(!rendered.contains("BlockData<BlockData>"), "{rendered}");
    }

    #[test]
    fn hierarchy_carrier_field_accessor_uses_nested_carrier_representation() {
        let parsed = parse_java_source(
            r#"
class BlockOutputStream {}
class ParallelBlockOutputStream extends BlockOutputStream {}

class Serializer {
    BlockOutputStream stream;
    Serializer(BlockOutputStream stream) { this.stream = stream; }
}
class ParasiteSerializer extends Serializer {
    ParasiteSerializer(BlockOutputStream stream) { super(stream); }
}
class Use {
    BlockOutputStream anonymousOutput() {
        return new BlockOutputStream() {};
    }
    Serializer anonymousSerializer(BlockOutputStream stream) {
        return new Serializer(stream) {};
    }
    Serializer serializer(boolean parallel, BlockOutputStream stream) {
        return parallel ? new ParasiteSerializer(stream) : new Serializer(stream);
    }
    BlockOutputStream output(boolean parallel, BlockOutputStream stream) {
        return serializer(parallel, stream).stream;
    }
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
        let serializer = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Serializer")
            .expect("serializer type");
        let rendered = source_hierarchy_carrier_item_for_type(
            &representations,
            serializer,
            &signatures,
            &semantic.facts,
            &ProgramExceptionPlan::default(),
            &resolved.program,
        )
        .expect("serializer hierarchy carrier");

        assert!(
            rendered.contains("pub fn stream(&self) -> Option<BlockOutputStreamValue>"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("std::rc::Rc<BlockOutputStreamValue>"),
            "{rendered}"
        );
    }

    #[test]
    fn hierarchy_carrier_field_accessor_escapes_keyword_storage_names() {
        let parsed = parse_java_source(
            r#"
class Base {
    String type;
}
class Child extends Base {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let mut representations = RepresentationPlan::default();
        representations.insert_test_type_representation(TypeRepresentation::new(
            "Base",
            ObjectRepresentation::SourceHierarchyCarrier {
                carrier_name: "BaseValue".to_string(),
                variants: vec!["Base".to_string(), "Child".to_string()],
            },
            RepresentationProvenance::ParsedSource,
            vec!["Child".to_string()],
        ));
        representations.insert_test_type_representation(TypeRepresentation::new(
            "Child",
            ObjectRepresentation::ConcreteStruct,
            RepresentationProvenance::ParsedSource,
            Vec::new(),
        ));
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let base = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");
        let rendered = source_hierarchy_carrier_item_for_type(
            &representations,
            base,
            &signatures,
            &semantic.facts,
            &ProgramExceptionPlan::default(),
            &resolved.program,
        )
        .expect("base hierarchy carrier");

        assert!(rendered.contains("pub fn r#type(&self)"), "{rendered}");
        assert!(rendered.contains("value.r#type"), "{rendered}");
        assert!(!rendered.contains("value.type"), "{rendered}");
    }

    #[test]
    fn nested_hierarchy_carrier_exposes_flattened_inherited_fields() {
        let parsed = parse_java_source(
            r#"
class Base {
    private String key;
    private String value;
}
class Mid extends Base {}
final class Leaf extends Mid {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let mut representations = RepresentationPlan::default();
        representations.insert_test_type_representation(TypeRepresentation::new(
            "Mid",
            ObjectRepresentation::SourceHierarchyCarrier {
                carrier_name: "MidValue".to_string(),
                variants: vec!["Mid".to_string(), "Leaf".to_string()],
            },
            RepresentationProvenance::ParsedSource,
            vec!["Leaf".to_string()],
        ));
        for type_name in ["Base", "Leaf"] {
            representations.insert_test_type_representation(TypeRepresentation::new(
                type_name,
                ObjectRepresentation::ConcreteStruct,
                RepresentationProvenance::ParsedSource,
                Vec::new(),
            ));
        }
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let mid = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Mid")
            .expect("mid type");
        let rendered = source_hierarchy_carrier_item_for_type(
            &representations,
            mid,
            &signatures,
            &semantic.facts,
            &ProgramExceptionPlan::default(),
            &resolved.program,
        )
        .expect("mid hierarchy carrier");

        assert!(
            rendered.contains("pub fn key(&self) -> Option<String>")
                && rendered.contains("pub fn value(&self) -> Option<String>"),
            "{rendered}"
        );
        assert!(
            rendered.contains("Self::Mid(value) => std::clone::Clone::clone(&value.key)")
                && rendered.contains("Self::Leaf(value) => std::clone::Clone::clone(&value.key)"),
            "{rendered}"
        );
    }

    #[test]
    fn ancestor_instantiations_compose_type_arguments_across_interface_edges() {
        let parsed = parse_java_source(
            r#"
interface Source<T> { T value(); }
interface Relay<U> extends Source<U> {}
interface Joined extends Relay<String> {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let joined = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Joined")
            .unwrap();
        let ancestors = interface_ancestor_instantiations(&resolved.program, joined);
        assert_eq!(ancestors.len(), 2);
        assert_eq!(ancestors[0].interface.name, "Relay");
        assert_eq!(ancestors[0].args, vec![JavaType::String]);
        assert_eq!(ancestors[1].interface.name, "Source");
        assert_eq!(ancestors[1].args, vec![JavaType::String]);
    }

    #[test]
    fn inherited_generic_contracts_substitute_type_arguments_through_interface_chain() {
        let parsed = parse_java_source(
            r#"
interface Source<T> { T value(); T echo(T input); }
interface Relay<U> extends Source<U> {}
interface Named { String value(); }
interface Joined extends Relay<String>, Named {}
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
        for (name, type_param) in [("Source", "T"), ("Relay", "U")] {
            let interface = resolved
                .program
                .types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap();
            let rendered = trait_item_for_type_with_program(
                interface,
                &interface.name,
                &resolved.program,
                &signatures,
                &semantic.facts,
                &ProgramExceptionPlan::default(),
                &representations,
                false,
            );
            assert!(
                rendered.contains(&format!("pub trait {name}<{type_param}>")),
                "{rendered}"
            );
            assert!(
                rendered.contains(&format!("fn value(&self) -> {type_param};")),
                "{rendered}"
            );
            assert!(
                rendered.contains(&format!(
                    "fn echo(&self, input: {type_param}) -> {type_param};"
                )),
                "{rendered}"
            );
            assert!(
                rendered.contains(&format!("impl<{type_param}: 'static, __JavaShared:")),
                "{rendered}"
            );
        }
        let source_ty = JavaType::Named {
            name: "Source".to_string(),
            args: vec![JavaType::String],
        };
        let mut source_slot = crate::slot_shape::RustSlotShape::for_storage(source_ty);
        source_slot.view = crate::slot_shape::ViewShape::DynTraitView;
        source_slot.dispatch = crate::slot_shape::DispatchShape::BoxedDynTrait {
            trait_name: "Source".to_string(),
        };
        assert_eq!(
            crate::emit::emit_type(
                &crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                    &source_slot,
                    &semantic.facts,
                    &representations,
                )
            ),
            "Box<dyn Source<String>>"
        );
        let joined = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Joined")
            .unwrap();
        let contracts = specialized_interface_abstract_contracts(&resolved.program, joined);
        assert_eq!(contracts.len(), 2);
        let value = contracts
            .iter()
            .find(|contract| contract.method.name == "value")
            .unwrap();
        assert_eq!(value.return_type, JavaType::String);
        let echo = contracts
            .iter()
            .find(|contract| contract.method.name == "echo")
            .unwrap();
        assert_eq!(echo.return_type, JavaType::String);
        assert_eq!(echo.param_types, vec![JavaType::String]);

        let rendered = trait_item_for_type_with_program(
            joined,
            &joined.name,
            &resolved.program,
            &signatures,
            &semantic.facts,
            &ProgramExceptionPlan::default(),
            &representations,
            false,
        );
        assert!(
            rendered.contains("fn value(&self) -> String;"),
            "{rendered}"
        );
        assert!(
            rendered.contains("fn echo(&self, input: String) -> String;"),
            "{rendered}"
        );
        assert!(
            rendered.contains("impl Source<String> for __JavaInterfaceUpcastJoinedToSource"),
            "{rendered}"
        );
        assert!(
            rendered.contains("impl Relay<String> for __JavaInterfaceUpcastJoinedToRelay"),
            "{rendered}"
        );
    }

    #[test]
    fn interface_upcast_wrapper_forwards_mutable_parent_contract() {
        let parsed = parse_java_source(
            r#"
interface Parent { void mutate(); }
interface Child extends Parent {}
class Implementation implements Child {
    int value;
    public void mutate() { value++; }
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
        let child = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Child")
            .unwrap();

        let rendered = trait_item_for_type_with_program(
            child,
            &child.name,
            &resolved.program,
            &signatures,
            &semantic.facts,
            &ProgramExceptionPlan::default(),
            &representations,
            false,
        );

        assert!(
            rendered.contains("impl Parent for __JavaInterfaceUpcastChildToParent"),
            "{rendered}"
        );
        assert!(
            rendered.contains("fn mutate(&mut self)") && rendered.contains("(*self.0).mutate()"),
            "{rendered}"
        );
    }

    #[test]
    fn colliding_parent_contracts_choose_covariant_return_in_either_order() {
        let parsed = parse_java_source(
            r#"
interface General { Object value(); }
interface Specific { String value(); }
interface GeneralFirst extends General, Specific {}
interface SpecificFirst extends Specific, General {}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        for name in ["GeneralFirst", "SpecificFirst"] {
            let combined = resolved
                .program
                .types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap();
            let contracts = specialized_interface_abstract_contracts(&resolved.program, combined);
            assert_eq!(contracts.len(), 1, "{name}");
            assert_eq!(contracts[0].return_type, JavaType::String, "{name}");
        }
    }

    #[test]
    fn source_hierarchy_carrier_forwards_parameterized_instance_methods() {
        let parsed = parse_java_source("class Base { int value(int input) { return input; } }");
        let method = parsed.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .expect("base method");

        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string()],
            &crate::representation::RepresentationPlan::default(),
            &FactSet::default(),
            &SignaturePlan::default(),
            MethodExceptionPlan::new(false),
            &parsed.program.types[0],
            method,
            &parsed.program,
            &SignaturePlan::default(),
            None,
            false,
        )
        .expect("instance method forwards");

        assert!(
            rendered.contains("pub fn value(&self, input: i32) -> i32"),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "Self::Base(__java_carrier_payload) => __java_carrier_payload.value(input)"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn source_hierarchy_carrier_qualifies_java_to_string_on_shared_payload() {
        let parsed = parse_java_source(
            r#"
class Base {
    public String toString() { return "base"; }
}
class Use {
    boolean same(Base value) {
        Base alias = value;
        return alias == value;
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let representations = plan_representations(&resolved.program, &semantic.facts);
        assert!(representations.storage_placement_for("Base").shared());
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let base = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");
        let to_string = base
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "toString" => Some(method),
                _ => None,
            })
            .expect("toString method");

        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string()],
            &representations,
            &semantic.facts,
            &signatures,
            MethodExceptionPlan::new(false),
            base,
            to_string,
            &resolved.program,
            &signatures,
            None,
            false,
        )
        .expect("carrier method");

        assert!(
            rendered.contains(
                "Self::Base(__java_carrier_payload) => Base::to_string(&**__java_carrier_payload)"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn source_hierarchy_carrier_reuses_payload_overload_qualified_name() {
        let mut parsed = parse_java_source(
            r#"
class Base { void compute(int input) {} }
class Child extends Base { void compute(int input) {} }
"#,
        );
        let owner = &parsed.program.types[0];
        let root_method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method.clone()),
                _ => None,
            })
            .expect("base method");
        let child = parsed
            .program
            .types
            .iter_mut()
            .find(|ty| ty.name == "Child")
            .expect("child type");
        let child_method = child
            .members
            .iter_mut()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .expect("child method");
        child_method.rust_name = Some("compute_int".to_string());
        let owner = parsed.program.types[0].clone();

        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string(), "Child".to_string()],
            &crate::representation::RepresentationPlan::default(),
            &FactSet::default(),
            &SignaturePlan::default(),
            MethodExceptionPlan::new(false),
            &owner,
            &root_method,
            &parsed.program,
            &SignaturePlan::default(),
            None,
            false,
        )
        .expect("instance method forwards");

        assert!(
            rendered.contains("pub fn compute_int(&self, input: i32)")
                && rendered.contains(
                    "Self::Base(__java_carrier_payload) => __java_carrier_payload.compute(input)"
                )
                && rendered.contains(
                    "Self::Child(__java_carrier_payload) => __java_carrier_payload.compute_int(input)"
                ),
            "{rendered}"
        );
    }

    #[test]
    fn source_hierarchy_carrier_matches_generic_super_slot_after_substitution() {
        let mut parsed = parse_java_source(
            r#"
class Base<T> { void write(T value) {} }
class Child extends Base<String> { void write(String value) {} }
"#,
        );
        let child_method = parsed.program.types[1]
            .members
            .iter_mut()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .expect("child method");
        child_method.rust_name = Some("write_string".to_string());
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
        let owner = &resolved.program.types[0];
        let root_method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .expect("base method");

        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string(), "Child".to_string()],
            &representations,
            &semantic.facts,
            &signatures,
            MethodExceptionPlan::new(false),
            owner,
            root_method,
            &resolved.program,
            &signatures,
            None,
            false,
        )
        .expect("instance method forwards");

        assert!(
            rendered.contains("Self::Child(__java_carrier_payload)")
                && rendered.contains(
                    "write(&*(Box::new(value) as Box<dyn std::any::Any>).downcast::<String>()"
                ),
            "{rendered}"
        );
    }

    #[test]
    fn source_hierarchy_carrier_reuses_method_type_parameter_bounds() {
        let parsed = parse_java_source(
            r#"
class Observation {}
class Base {
    <O extends Observation> void consume(O value) {
        value.toString();
        value.toString();
    }
}
"#,
        );
        let owner = parsed
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "consume" => Some(method),
                _ => None,
            })
            .expect("generic method");

        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string()],
            &crate::representation::RepresentationPlan::default(),
            &FactSet::default(),
            &SignaturePlan::default(),
            MethodExceptionPlan::new(false),
            owner,
            method,
            &parsed.program,
            &SignaturePlan::default(),
            None,
            false,
        )
        .expect("carrier method");

        assert!(
            rendered.contains("pub fn consume<O: ObservationBound + Clone>(&self, value: O)"),
            "{rendered}"
        );

        let mut erased_summary_method = method.clone();
        erased_summary_method.type_param_bounds = vec![None];
        erased_summary_method.body = None;
        let rendered_from_erased_summary = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string()],
            &crate::representation::RepresentationPlan::default(),
            &FactSet::default(),
            &SignaturePlan::default(),
            MethodExceptionPlan::new(false),
            owner,
            &erased_summary_method,
            &parsed.program,
            &SignaturePlan::default(),
            None,
            false,
        )
        .expect("carrier method from erased summary");
        assert!(
            rendered_from_erased_summary
                .contains("pub fn consume<O: ObservationBound + Clone>(&self, value: O)"),
            "{rendered_from_erased_summary}"
        );

        // Focused-package analysis may keep the short emitted declaration as
        // a bodyless summary while the package-qualified copy retains the
        // source bound.  Both erase to the same carrier variant.
        let mut support = parsed.program.clone();
        let base_index = support
            .types
            .iter()
            .position(|ty| ty.name == "Base")
            .expect("base index");
        let qualified_base = support.types[base_index].clone();
        if let Some(Member::Method(summary_method)) = support.types[base_index]
            .members
            .iter_mut()
            .find(|member| matches!(member, Member::Method(method) if method.name == "consume"))
        {
            summary_method.type_param_bounds = vec![None];
            summary_method.body = None;
        }
        support.types.push(qualified_base);
        let summary_owner = &support.types[base_index];
        let summary_method = summary_owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "consume" => Some(method),
                _ => None,
            })
            .expect("summary method");
        let rendered_from_duplicate_summary = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string()],
            &crate::representation::RepresentationPlan::default(),
            &FactSet::default(),
            &SignaturePlan::default(),
            MethodExceptionPlan::new(false),
            summary_owner,
            summary_method,
            &support,
            &SignaturePlan::default(),
            None,
            false,
        )
        .expect("carrier method from duplicate summary");
        assert!(
            rendered_from_duplicate_summary
                .contains("pub fn consume<O: ObservationBound + Clone>(&self, value: O)"),
            "{rendered_from_duplicate_summary}"
        );
    }

    #[test]
    fn source_hierarchy_carrier_joins_clone_bounds_from_every_payload() {
        let parsed = parse_java_source(
            r#"
import java.util.List;

abstract class Base {
    abstract <T> T select(List<T> values);
}
class CloneUsingChild extends Base {
    <T> T select(List<T> values) { return values.get(0); }
}
class PlainChild extends Base {
    <T> T select(List<T> values) { return null; }
}
"#,
        );
        let owner = parsed
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "select" => Some(method),
                _ => None,
            })
            .expect("generic method");

        // Keep the non-cloning payload last. The old winner-takes-all policy
        // selected this body and lost the Clone obligation required by the
        // other match arm.
        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["CloneUsingChild".to_string(), "PlainChild".to_string()],
            &crate::representation::RepresentationPlan::default(),
            &FactSet::default(),
            &SignaturePlan::default(),
            MethodExceptionPlan::new(false),
            owner,
            method,
            &parsed.program,
            &SignaturePlan::default(),
            None,
            false,
        )
        .expect("carrier method");

        assert!(rendered.contains("pub fn select<T: Clone>"), "{rendered}");
    }

    #[test]
    fn source_hierarchy_carrier_joins_mutable_concrete_override_receiver() {
        let parsed = parse_java_source(
            r#"
abstract class Base {
    abstract int poll();
}
class Child extends Base {
    int count;
    int poll() { return count++; }
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
        let base = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .unwrap();
        let poll = base
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "poll" => Some(method),
                _ => None,
            })
            .unwrap();

        assert_eq!(
            source_hierarchy_carrier_receiver_plan(
                &["Child".to_string()],
                base,
                poll,
                &representations,
                &resolved.program,
                &SignaturePlan::default(),
                &signatures,
            ),
            ReceiverPlan::Mutable
        );
    }

    #[test]
    fn all_handle_source_hierarchy_carrier_forwards_mutation_through_shared_receiver() {
        let parsed = parse_java_source(
            r#"
class Base {
    int count;
    void bump() { count++; }
}
class Child extends Base {
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = analyze_program(&resolved.program, &resolved.facts, &FactSet::default());
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let mut representations = RepresentationPlan::default();
        representations.insert_test_type_representation(TypeRepresentation::new(
            "Base",
            ObjectRepresentation::SourceHierarchyCarrier {
                carrier_name: "BaseValue".to_string(),
                variants: vec!["Base".to_string(), "Child".to_string()],
            },
            RepresentationProvenance::ParsedSource,
            vec!["Child".to_string()],
        ));
        representations.insert_test_type_representation(TypeRepresentation::new(
            "Child",
            ObjectRepresentation::ConcreteStruct,
            RepresentationProvenance::ParsedSource,
            Vec::new(),
        ));
        for type_name in ["Base", "Child"] {
            representations.insert_test_storage_facts(
                type_name,
                TypeStorageFacts::default()
                    .with_locally_aliased(true)
                    .with_mutated(true),
            );
        }
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let base = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .unwrap();
        let bump = base
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "bump" => Some(method),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            signatures.method_receiver(base, bump),
            Some(ReceiverPlan::Mutable)
        );

        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Base".to_string(), "Child".to_string()],
            &representations,
            &semantic.facts,
            &signatures,
            MethodExceptionPlan::new(false),
            base,
            bump,
            &resolved.program,
            &signatures,
            None,
            false,
        )
        .expect("carrier bump forwarder");

        assert!(rendered.contains("pub fn bump(&self)"), "{rendered}");
        assert!(rendered.contains("bump_with_outer"), "{rendered}");
    }

    #[test]
    fn source_hierarchy_carrier_uses_the_payloads_post_solver_shared_param_abi() {
        let parsed = parse_java_source(
            r#"
class Record {
    int value;
}
abstract class Base {
    abstract int compare(Record record);
    int score(Record record) { return record.value; }
}
class Left extends Base {
    int compare(Record record) { record.value = 1; return score(record); }
}
class Right extends Base {
    int compare(Record record) { return score(record); }
}
class Use {
    int run(Base base, Record record) {
        Record alias = record;
        alias.value = 2;
        return base.compare(record);
    }
}
"#,
        );
        let resolved = resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = resolved.facts.clone();
        facts.hierarchy_overrides.push(HierarchyOverride {
            type_name: "Base".to_string(),
            strategy: HierarchyStrategy::ClosedEnum {
                enum_name: "BaseValue".to_string(),
                variants: vec!["Left".to_string(), "Right".to_string()],
            },
            provenance: FactProvenance::UserOverride,
        });
        let semantic = analyze_program(&resolved.program, &resolved.facts, &facts);
        let representations = plan_representations(&resolved.program, &semantic.facts);
        let usage = analyze_usage_with_facts(&resolved.program, &semantic.facts);
        let signatures = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &semantic.facts,
        );
        let base = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");
        let compare = base
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "compare" => Some(method),
                _ => None,
            })
            .expect("compare method");

        let rendered = source_hierarchy_carrier_forwarding_method(
            "BaseValue",
            &["Left".to_string(), "Right".to_string()],
            &representations,
            &semantic.facts,
            &signatures,
            MethodExceptionPlan::new(false),
            base,
            compare,
            &resolved.program,
            &signatures,
            None,
            false,
        )
        .expect("carrier method");

        assert!(
            rendered.contains(
                "pub fn compare(&self, record: std::rc::Rc<std::cell::RefCell<Record>>) -> i32"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains("__java_carrier_payload.compare(record)"),
            "{rendered}"
        );
    }

    #[test]
    fn source_hierarchy_carrier_does_not_forward_private_methods() {
        let parsed =
            parse_java_source("class Base { private int helper(int input) { return input; } }");
        let method = parsed.program.types[0]
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .expect("private method");

        assert!(
            source_hierarchy_carrier_forwarding_method(
                "BaseValue",
                &["Base".to_string()],
                &crate::representation::RepresentationPlan::default(),
                &FactSet::default(),
                &SignaturePlan::default(),
                MethodExceptionPlan::new(false),
                &parsed.program.types[0],
                method,
                &parsed.program,
                &SignaturePlan::default(),
                None,
                false,
            )
            .is_none()
        );
    }

    #[test]
    fn source_class_type_parameter_bound_does_not_expose_private_methods() {
        let generated = crate::translate_source(
            r#"
            class Base {
                private int hidden(int value) { return value; }
                public int visible(int value) { return hidden(value); }
            }
            class Use<T extends Base> {
                int apply(T value) { return value.visible(7); }
            }
            "#,
        )
        .rust;
        let bound_trait = generated
            .split("pub trait BaseBound")
            .nth(1)
            .and_then(|tail| tail.split("}\n").next())
            .expect("source class bound trait should be emitted");

        assert!(bound_trait.contains("fn visible("), "{generated}");
        assert!(
            !bound_trait.contains("fn hidden("),
            "private methods are statically bound and cannot be trait requirements: {generated}"
        );
    }

    #[test]
    fn bounded_source_type_parameter_nested_calls_use_bound_trait_dispatch() {
        let generated = crate::translate_source(
            r#"
            class Base {
                private Base peer;
                private int value;
                Base(Base peer, int value) { this.peer = peer; this.value = value; }
                public int getValue() { return value; }
                public void setValue(int value) { this.value = value; }
            }
            class Use<T extends Base> {
                boolean same(T left, T right) {
                    return left.getValue() == right.getValue();
                }
            }
            "#,
        )
        .rust;
        let use_impl = generated
            .split("impl<T: BaseBound")
            .nth(1)
            .expect("generic Use implementation should be emitted");

        assert!(use_impl.contains("left.get_value()"), "{generated}");
        assert!(use_impl.contains("right.get_value()"), "{generated}");
        assert!(
            !use_impl.contains("Base::get_value_with_outer"),
            "bounded generic receivers must not use a concrete carrier ABI: {generated}"
        );
    }

    #[test]
    fn anonymous_class_base_emits_object_safe_parameterized_dispatch_surface() {
        let parsed = parse_java_source(
            "class Base { int value(int input) { return input; } } class Use { Base make() { return new Base() {}; } }",
        );
        let facts = FactSet::default();
        let plan = plan_representations(&parsed.program, &facts);
        let base = parsed
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");

        let rendered = source_hierarchy_virtual_dispatch_trait_item_for_type(
            &plan,
            base,
            &SignaturePlan::default(),
            &facts,
            &ProgramExceptionPlan::default(),
            &parsed.program,
        )
        .expect("anonymous base gets a dispatch surface");
        assert!(
            rendered.contains("pub trait BaseAnonymousDispatchClone"),
            "{rendered}"
        );
        assert!(
            rendered.contains("impl Clone for Box<dyn BaseAnonymousDispatch>"),
            "{rendered}"
        );
        assert!(
            rendered.contains("pub trait BaseAnonymousDispatch"),
            "{rendered}"
        );
        assert!(
            rendered.contains("fn value(&self, input: i32) -> i32;"),
            "{rendered}"
        );
    }

    #[test]
    fn anonymous_carrier_dispatch_includes_same_source_superclass_method() {
        let parsed = parse_java_source(
            "class Parent { int inherited(int value) { return value; } } class Base extends Parent {} class Use { Base make() { return new Base() {}; } }",
        );
        let facts = FactSet::default();
        let plan = plan_representations(&parsed.program, &facts);
        let base = parsed
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Base")
            .expect("base type");
        let inherited = crate::inherited_members::inherited_instance_method_refs_for_type(
            base,
            &parsed.program,
        );
        assert!(
            inherited
                .iter()
                .any(|method| method.method().name == "inherited"),
            "{inherited:?}"
        );
        let rendered = source_hierarchy_virtual_dispatch_trait_item_for_type(
            &plan,
            base,
            &SignaturePlan::default(),
            &facts,
            &ProgramExceptionPlan::default(),
            &parsed.program,
        )
        .expect("anonymous base gets a dispatch surface");
        assert!(
            rendered.contains("fn inherited(&self, value: i32) -> i32;"),
            "{rendered}"
        );
    }

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
        assert!(enum_item.contains("impl JavaAppendValue for Holder_Type"));
        assert!(enum_item.contains("impl JavaAppendValue for &Holder_Type"));
    }

    #[test]
    fn representation_lowering_emits_dyn_trait_method_signatures_from_signature_plan() {
        let parsed = parse_java_source(
            r#"
interface Named {
    String name();
    int score(String prefix);
    Product create();
}
class Product {}

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
        assert!(item.contains("fn create(&self) -> Product;"), "{item}");
        assert!(!item.contains("fn create(&self) -> &Product;"), "{item}");
        assert!(item.contains("impl Clone for std::boxed::Box<dyn Named>"));
        assert!(!item.contains("Named for std::rc::Rc<__JavaShared>"));
        assert!(item.contains("Named for std::rc::Rc<std::cell::RefCell<__JavaShared>>"));
    }

    #[test]
    fn trait_contract_adopts_source_hierarchy_carrier_abi_from_implementation() {
        let parsed = parse_java_source(
            r#"
interface RootSource {
    Root get();
    void set(Root value);
    Root[] all();
    void replace(Root[] values);
}
class Root {
    static Root SHARED = new Root();
    Root alias;
    int value;
    Root() { alias = this; }
}
class RootSourceImpl implements RootSource {
    public Root get() { return new Root(); }
    public void set(Root value) {}
    public Root[] all() { return new Root[0]; }
    public void replace(Root[] values) {}
}
class Use {
    Root anonymousRoot() { return new Root() {}; }
    RootSource source() { return new RootSourceImpl(); }
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
        let source = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "RootSource")
            .unwrap();
        let RepresentationTypeEmission::RawRustItem(item) =
            type_emission_for_program_with_facts_and_exceptions(
                &representations,
                &resolved.program,
                source,
                &signatures,
                &semantic.facts,
                &ProgramExceptionPlan::default(),
            )
        else {
            panic!("interface should emit a raw trait item");
        };

        assert!(item.contains("fn get(&self) -> RootValue;"), "{item}");
        assert!(item.contains("fn set(&self, value: RootValue);"), "{item}");
        assert!(item.contains("Vec<Option<RootValue>>"), "{item}");
        assert!(!item.contains("Rc<Root>"), "{item}");
    }

    #[test]
    fn trait_contract_uses_source_hierarchy_carrier_without_visible_implementor() {
        let parsed = parse_java_source(
            r#"
interface RootSource {
    Root get(int index);
    void get(int index, Root target);
    Root getCopy(int index);
    Root[] all();
    void replace(Root[] values);
}
class Root {
    static Root SHARED = new Root();
    Root alias;
    int value;
    Root() { alias = this; }
}
class Child extends Root {
    Child alias;
    Child() { alias = this; }
    void bump() { value++; }
}
class Use {
    Root child() { return new Child(); }
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
        let source = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "RootSource")
            .unwrap();
        let RepresentationTypeEmission::RawRustItem(item) =
            type_emission_for_program_with_facts_and_exceptions(
                &representations,
                &resolved.program,
                source,
                &signatures,
                &semantic.facts,
                &ProgramExceptionPlan::default(),
            )
        else {
            panic!("interface should emit a raw trait item");
        };

        assert!(
            item.contains("fn get_int(&self, index: i32) -> RootValue;"),
            "{item}"
        );
        assert!(item.contains("Vec<Option<RootValue>>"), "{item}");
        assert!(!item.contains("Rc<Root>"), "{item}");
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
                "pub trait IdSource {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId>;\n    fn as_any(&self) -> &dyn std::any::Any;\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource>;\n}\n\nimpl dyn IdSource {\n    pub fn anonymous() -> std::boxed::Box<dyn IdSource> { panic!(\"java2rust: anonymous class body for IdSource requires object lowering\") }\n}\n\nimpl Clone for std::boxed::Box<dyn IdSource> {\n    fn clone(&self) -> Self { IdSource::clone_box(&**self) }\n}\n\nimpl<__JavaShared: IdSource + 'static> IdSource for std::rc::Rc<std::cell::RefCell<__JavaShared>> {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { IdSource::ids(&*self.borrow(), values) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { std::boxed::Box::new(std::clone::Clone::clone(self)) }\n}\n\nimpl<__JavaShared: IdSource + 'static> IdSource for std::sync::Arc<std::sync::Mutex<__JavaShared>> {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { IdSource::ids(&*self.lock().unwrap(), values) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { std::boxed::Box::new(std::clone::Clone::clone(self)) }\n}\n\nimpl<__JavaBoxed: IdSource + ?Sized> IdSource for std::boxed::Box<__JavaBoxed> {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { IdSource::ids(&**self, values) }\n    fn as_any(&self) -> &dyn std::any::Any { IdSource::as_any(&**self) }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { IdSource::clone_box(&**self) }\n}"
                    .to_string()
            )
        );
    }

    #[test]
    fn representation_lowering_clones_cloneable_direct_dyn_trait_implementors() {
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
                "impl Named for Dog {\n    fn name(&self) -> String { Dog::name(self) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn Named> { std::boxed::Box::new(std::clone::Clone::clone(self)) }\n}".to_string()
            ]
        );
    }

    #[test]
    fn immutable_interface_implementor_owns_its_shared_handle_abi() {
        let source = r#"
interface BlockData {
    byte[] data();
}

class CompressedBlock implements BlockData {
    final byte[] bytes;
    CompressedBlock(byte[] bytes) { this.bytes = bytes; }
    public byte[] data() { return bytes; }
}

class BlockFactory {
    static BlockData make() { return new CompressedBlock(new byte[1]); }
    static CompressedBlock recover(BlockData value) {
        return (CompressedBlock) value;
    }
}
"#;
        let parsed = parse_java_source(source);
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
        let concrete = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "CompressedBlock")
            .unwrap();
        let impls = trait_impl_emissions_for_type_with_facts(
            &representations,
            &resolved.program,
            concrete,
            &signatures,
            &semantic.facts,
        );
        let rendered = impls.join("\n");

        assert!(
            rendered.contains("impl BlockData for std::rc::Rc<CompressedBlock>"),
            "{rendered}"
        );
        assert!(
            rendered.contains("fn as_any(&self) -> &dyn std::any::Any { self }"),
            "{rendered}"
        );

        let translation = crate::translate_source(source).rust;
        assert!(
            translation.contains("downcast_ref::<std::rc::Rc<CompressedBlock>>()"),
            "{translation}"
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
    fn trait_impl_forwarding_wraps_nonnull_return_for_nullable_trait_contract() {
        let parsed = parse_java_source(
            r#"
interface Bytes {
    byte[] data();
}

class MaybeBytes implements Bytes {
    byte[] value;
    public byte[] data() { return value; }
}

class PresentBytes implements Bytes {
    byte[] value;
    PresentBytes() { value = new byte[1]; }
    public byte[] data() { return value; }
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
        let present = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "PresentBytes")
            .unwrap();

        let impls = trait_impl_emissions_for_type_with_facts(
            &representations,
            &resolved.program,
            present,
            &signatures,
            &semantic.facts,
        );

        assert_eq!(impls.len(), 1);
        assert!(
            impls[0].contains("fn data(&self) -> Option<Vec<i8>> { PresentBytes::data(self) }"),
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
                "impl IdSource for Source {\n    fn ids(&self, values: Vec<ExternalId>) -> Vec<ExternalId> { Source::ids(self, values) }\n    fn as_any(&self) -> &dyn std::any::Any { self }\n    fn clone_box(&self) -> std::boxed::Box<dyn IdSource> { std::boxed::Box::new(std::clone::Clone::clone(self)) }\n}"
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
                "fn score(&self, prefix: String) -> i32 { OverloadedNamed::score_string(self, &prefix) }"
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
        // method must remain callable through both cell-backed handle forms.
        // Direct handles are emitted with each concrete implementation so the
        // ABI survives per-file support stripping without coherence overlap.
        for expected in [
            "pub trait Mapper {\n    fn id<T>(&self, value: T) -> T where Self: Sized;",
            "Mapper for std::rc::Rc<std::cell::RefCell<__JavaShared>>",
            "Mapper for std::sync::Arc<std::sync::Mutex<__JavaShared>>",
            "fn id<T>(&self, value: T) -> T where Self: Sized { Mapper::id(&*self.borrow(), value) }",
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
    fn find_type_decl_maps_package_qualified_nested_summary_identity_to_emitted_decl() {
        let parsed = crate::parser::parse_java_source(
            "package example.identity; class Outer { class Nested {} }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);

        assert_eq!(
            find_type_decl_for_owner_context(
                &resolved.program,
                "Outer",
                "example.identity.Outer.Nested",
            )
            .map(crate::item_assembly::rust_use_name_for_source_type),
            Some("Outer_Nested".to_string())
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
    use super::{boxed_trait_forwarding_impl_with_generics, shared_handle_forwarding_impl};

    #[test]
    fn forwards_a_read_only_surface_through_cell_handles() {
        let rendered = shared_handle_forwarding_impl(
            "Named",
            &[
                "    fn id(&self) -> String;".to_string(),
                "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
            ],
        )
        .expect("a &self-only surface forwards");
        assert!(rendered.contains(
            "impl<__JavaShared: Named + 'static> Named for std::rc::Rc<std::cell::RefCell<__JavaShared>>"
        ));
        assert!(rendered.contains("fn id(&self) -> String { Named::id(&*self.borrow()) }"));
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
                "fn clone_box(&self) -> std::boxed::Box<dyn Named> { std::boxed::Box::new(std::clone::Clone::clone(self)) }"
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
            rendered.contains("{ Sink::put(&*self.borrow(), key, value) }"),
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
        assert!(rendered.contains("fn size(&self) -> i32 { Shape::size(&*self.borrow()) }"));
        assert!(rendered.contains("fn grow(&mut self) { Shape::grow(&mut *self.borrow_mut()) }"));
        assert!(rendered.contains("Shape for std::sync::Arc<std::sync::Mutex<__JavaShared>>"));
        assert!(
            rendered.contains("fn grow(&mut self) { Shape::grow(&mut *self.lock().unwrap()) }")
        );
        assert!(!rendered.contains("Shape for std::rc::Rc<__JavaShared>"));
    }

    #[test]
    fn forwards_fallible_interface_surface_with_trait_arguments_and_returns() {
        let rendered = shared_handle_forwarding_impl(
            "ParallelCompressor",
            &[
                "    fn wrap_and_write_header(&self, stream: OutputStream) -> Result<BlockOutputStreamValue, JavaException>;".to_string(),
                "    fn update_checksum_pre_compression(&mut self, current: Option<UncompressedBlockData>) -> Result<(), JavaException>;".to_string(),
                "    fn compress(&self, previous: Option<UncompressedBlockData>, current: Option<UncompressedBlockData>) -> Result<Option<Box<dyn BlockData>>, JavaException>;".to_string(),
                "    fn update_checksum_post_compression(&mut self, current: &dyn BlockData);".to_string(),
                "    fn write_trailer(&self, stream: BlockOutputStreamValue) -> Result<(), JavaException>;".to_string(),
                "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
                "    fn clone_box(&self) -> std::boxed::Box<dyn ParallelCompressor>;".to_string(),
            ],
        )
        .expect("the complete owned-return surface forwards through cell handles");
        assert!(
            rendered
                .contains("ParallelCompressor for std::rc::Rc<std::cell::RefCell<__JavaShared>>"),
            "{rendered}"
        );
    }

    #[test]
    fn borrowed_returns_require_a_nominal_direct_handle_impl() {
        assert!(
            shared_handle_forwarding_impl(
                "LabelSource",
                &[
                    "    fn label(&self) -> &str;".to_string(),
                    "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
                ],
            )
            .is_none()
        );
    }

    #[test]
    fn boxed_trait_carrier_forwards_parameterized_surface() {
        let rendered = boxed_trait_forwarding_impl_with_generics(
            "Product<T>",
            &["T".to_string()],
            &[
                "    fn value(&self, index: i32) -> T;".to_string(),
                "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
                "    fn clone_box(&self) -> std::boxed::Box<dyn Product<T>>;".to_string(),
            ],
        )
        .expect("boxed owned interface carrier forwards its trait surface");
        assert!(
            rendered.contains(
                "impl<T: 'static, __JavaBoxed: Product<T> + ?Sized> Product<T> for std::boxed::Box<__JavaBoxed>"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains("Product::<T>::value(&**self, index)"),
            "{rendered}"
        );
    }

    #[test]
    fn boxed_trait_carrier_forwards_mutating_multi_method_surface() {
        let rendered = boxed_trait_forwarding_impl_with_generics(
            "Opdf<O>",
            &["O".to_string()],
            &[
                "    fn log_probability(&mut self, o: O) -> Result<f64, JavaException>;".to_string(),
                "    fn generate(&mut self) -> Result<O, JavaException>;".to_string(),
                "    fn fit_o_array_double_array(&mut self, o: Vec<Option<O>>, weights: Vec<f64>) -> Result<(), JavaException>;".to_string(),
                "    fn clone(&self) -> Result<Box<dyn Opdf<O>>, JavaException>;".to_string(),
                "    fn as_any(&self) -> &dyn std::any::Any;".to_string(),
                "    fn clone_box(&self) -> std::boxed::Box<dyn Opdf<O>>;".to_string(),
            ],
        )
        .expect("boxed mutating interface carrier forwards its complete surface");
        assert!(
            rendered.contains("Opdf::<O>::fit_o_array_double_array(&mut **self, o, weights)"),
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
        assert!(rendered.contains("__JavaShared: Mapper"), "{rendered}");
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
