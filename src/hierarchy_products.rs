//! Typed output contracts for finite, fixed-product source hierarchies.
//! The plan is a read-only projection of solved signatures, never a new solver.
use std::collections::BTreeMap;

use crate::java_ir::{JavaType, Member, MethodDecl, Modifier, Program, TypeDecl};
use crate::representation::{ObjectRepresentation, RepresentationPlan};
use crate::signature::SignaturePlan;
use crate::slot_shape::{OwnershipShape, RustSlotShape};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HierarchyProductPlan {
    pub roots: BTreeMap<String, HierarchyProductRoot>,
    pub sources: BTreeMap<String, Vec<(String, String, usize)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HierarchyProductRoot {
    pub parameter: String,
    /// Stable method order also defines converter fields following the handle.
    pub methods: Vec<String>,
    pub variants: BTreeMap<String, Vec<RustSlotShape>>,
}

fn mentions(ty: &JavaType, parameter: &str) -> bool {
    crate::item_assembly::java_type_contains_type_param(ty, parameter)
}

fn direct(ty: &JavaType, parameter: &str) -> bool {
    matches!(ty, JavaType::Named { name, args } if name == parameter && args.is_empty())
        || matches!(ty, JavaType::Unknown(name) if name == parameter)
}

fn find<'a>(program: &'a Program, owner: &str, name: &str) -> Option<&'a TypeDecl> {
    crate::inherited_members::find_type_decl_for_owner_context(program, owner, name)
}

fn methods(owner: &TypeDecl, program: &Program) -> Vec<MethodDecl> {
    let mut methods = owner
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(method.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for inherited in
        crate::inherited_members::inherited_instance_method_refs_for_type(owner, program)
    {
        if let Some(bindings) = crate::inherited_members::source_ancestor_type_arguments(
            program,
            owner,
            inherited.declaring_ty(),
        ) {
            methods.push(crate::java_type_substitution::specialized_method(
                inherited.method(),
                &bindings,
            ));
        }
    }
    methods
}

impl HierarchyProductPlan {
    pub fn from_signatures(
        program: &Program,
        representations: &RepresentationPlan,
        signatures: &SignaturePlan,
    ) -> Self {
        let mut plan = Self::default();
        for entry in representations.type_representations() {
            let ObjectRepresentation::SourceHierarchyCarrier { variants, .. } =
                entry.representation()
            else {
                continue;
            };
            let Some(owner) = find(program, entry.java_name(), entry.java_name()) else {
                continue;
            };
            if let Some(root) = plan_root(owner, variants, program, representations, signatures) {
                plan.roots.insert(owner.name.clone(), root);
            }
        }
        for (name, root) in &plan.roots {
            for (variant, products) in &root.variants {
                for (index, product) in products.iter().enumerate() {
                    if let JavaType::Named { name: source, .. } = &product.java_ty
                        && let Some(source) = find(program, name, source)
                    {
                        plan.sources.entry(source.name.clone()).or_default().push((
                            name.clone(),
                            variant.clone(),
                            index,
                        ));
                    }
                }
            }
        }
        plan
    }
}

fn plan_root(
    owner: &TypeDecl,
    variants: &[String],
    program: &Program,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
) -> Option<HierarchyProductRoot> {
    let [parameter] = owner.type_params.as_slice() else {
        return None;
    };
    if representations.anonymous_class_base_requires_virtual_carrier(&owner.name) {
        return None;
    }
    // Stored or consumed binders are invariant contracts. In particular a
    // returned Writer<P> is not a scalar output P and cannot be retagged.
    if owner.members.iter().any(|member| match member {
        Member::Field(field) => {
            !field.modifiers.contains(&Modifier::Static) && mentions(&field.ty, parameter)
        }
        Member::Constructor(constructor) => {
            !constructor.type_params.contains(parameter)
                && constructor
                    .params
                    .iter()
                    .any(|p| mentions(&p.ty, parameter))
        }
        _ => false,
    }) {
        return None;
    }
    for inherited in
        crate::inherited_members::inherited_instance_field_refs_for_type(owner, program)
    {
        let bindings = crate::inherited_members::source_ancestor_type_arguments(
            program,
            owner,
            find(program, &owner.name, inherited.declaring_type())?,
        )?;
        if mentions(
            &crate::resolve::substitute_method_type_variables(&inherited.field().ty, &bindings),
            parameter,
        ) {
            return None;
        }
    }
    if owner
        .type_param_bounds
        .iter()
        .flatten()
        .any(|bound| mentions(bound, parameter))
    {
        return None;
    }
    let mut outputs = Vec::new();
    for method in methods(owner, program) {
        if method.modifiers.contains(&Modifier::Static) || method.type_params.contains(parameter) {
            continue;
        }
        if method
            .type_param_bounds
            .iter()
            .flatten()
            .any(|bound| mentions(bound, parameter))
            || method.params.iter().any(|p| mentions(&p.ty, parameter))
        {
            return None;
        }
        if !mentions(&method.return_type, parameter) {
            continue;
        }
        if !direct(&method.return_type, parameter) || !method.type_params.is_empty() {
            return None;
        }
        let name = crate::java_signature::source_method_rust_name(&method);
        let target = signatures.method_return_shape_by_rust_name(&owner.name, &name)?;
        // Nullable/borrowed producers require a different lifting contract.
        // Never invent a null check, copy a payload, or erase a loan here.
        if target.optional_element_type().is_some() || !direct(&target.java_ty, parameter) {
            return None;
        }
        if !outputs.contains(&name) {
            outputs.push(name);
        }
    }
    if outputs.is_empty() {
        return None;
    }
    outputs.sort();
    let mut products = BTreeMap::new();
    for variant in variants {
        let payload = find(program, &owner.name, variant)?;
        if !payload.type_params.is_empty() {
            return None;
        }
        let bindings =
            crate::inherited_members::source_ancestor_type_arguments(program, payload, owner)?;
        if mentions(bindings.get(parameter)?, parameter) {
            return None;
        }
        let mut values = Vec::new();
        for name in &outputs {
            let shape = signatures
                .method_return_shape_by_rust_name(&payload.name, name)
                .or_else(|| {
                    crate::inherited_members::inherited_instance_method_refs_for_type(
                        payload, program,
                    )
                    .into_iter()
                    .find(|method| {
                        crate::java_signature::source_method_rust_name(method.method()) == *name
                    })
                    .and_then(|method| {
                        signatures
                            .method_return_shape_by_rust_name(&method.declaring_ty().name, name)
                    })
                })?;
            if crate::inherited_members::java_type_contains_synthesized_source_wildcard(
                &shape.java_ty,
            ) || mentions(&shape.java_ty, parameter)
                || shape.optional_element_type().is_some()
                || !matches!(
                    shape.ownership,
                    OwnershipShape::Owned | OwnershipShape::CopyValue
                )
            {
                return None;
            }
            values.push(shape);
        }
        products.insert(variant.clone(), values);
    }
    Some(HierarchyProductRoot {
        parameter: parameter.clone(),
        methods: outputs,
        variants: products,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan(source: &str) -> HierarchyProductPlan {
        let parsed = crate::parser::parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &crate::facts::FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        HierarchyProductPlan::from_signatures(&resolved.program, &representations, &signatures)
    }
    #[test]
    fn fixed_owned_products_are_planned_without_retagging_stored_values() {
        let plan = plan(concat!(
            include_str!("../tests/fixtures/ReaderProduct.java"),
            "\n",
            include_str!("../tests/fixtures/CovariantReaderProductProbe.java"),
        ));
        let root = plan
            .roots
            .get("ProductReader")
            .expect("owned product reader");
        assert_eq!(root.parameter, "P");
        assert_eq!(root.methods, ["read"]);
        assert_eq!(root.variants.len(), 2);
        for values in root.variants.values() {
            assert_eq!(values.len(), 1);
        }
    }
    #[test]
    fn consumed_or_invariant_nested_products_are_not_output_adapters() {
        for member in [
            "void accept(P value) {}",
            "P stored;",
            "abstract java.util.List<P> nested();",
            "<X extends P> void accept(X value) {}",
        ] {
            let source = format!(
                "class Product {{}} abstract class Reader<P> {{ abstract P read(); {member} }} class Concrete extends Reader<Product> {{ Product read() {{ return new Product(); }} java.util.List<Product> nested() {{ return new java.util.ArrayList<>(); }} }} class Use {{ Reader<? extends Product> get() {{ return new Concrete(); }} }}"
            );
            assert!(!plan(&source).roots.contains_key("Reader"), "{member}");
        }
    }

    #[test]
    fn nullable_products_keep_the_existing_contract_until_a_nullable_lift_is_proved() {
        let source = "class Product {} abstract class Reader<P> { abstract P read(); } class Concrete extends Reader<Product> { Product read() { return null; } } class Use { Reader<? extends Product> get() { return new Concrete(); } }";
        assert!(!plan(source).roots.contains_key("Reader"));
    }
}
