//! Admission proof for source-interface references with shared receivers.
//!
//! Every mutable implementation must already dispatch through its canonical
//! Java handle. Copying the outer Rc/Arc can then copy a reference without
//! borrowing a payload or allocating another interface wrapper.
use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, Member, Modifier, Program, TypeDecl, TypeKind};
use crate::representation::RepresentationPlan;
use std::collections::{HashMap, HashSet};

fn types<'a>(items: &'a [TypeDecl], out: &mut Vec<&'a TypeDecl>) {
    for ty in items {
        out.push(ty);
        for member in &ty.members {
            if let Member::Type(child) = member {
                types(std::slice::from_ref(child), out);
            }
        }
    }
}
fn name(ty: &JavaType) -> Option<&str> {
    match ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        _ => None,
    }
}

pub(crate) fn plan(
    program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
) -> HashMap<String, bool> {
    let mut declarations = Vec::new();
    types(&program.types, &mut declarations);
    // Reuse one instantiated ancestry inventory for admission and exclusions.
    let ancestry = declarations
        .iter()
        .map(|ty| {
            (
                ty.name.clone(),
                crate::representation_lowering::implemented_interfaces_for_type(program, ty),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut anonymous = HashSet::new();
    crate::semantic_analysis::walk_program_exprs(program, &mut |expr| {
        if let Expr::New {
            ty, body: Some(_), ..
        } = expr
            && let Some(name) = name(ty)
        {
            anonymous.insert(name.to_string());
        }
    });
    let mut result = HashMap::new();
    for interface in declarations
        .iter()
        .copied()
        .filter(|ty| ty.kind == TypeKind::Interface)
    {
        if !matches!(
            representations.representation_for(&JavaType::Named {
                name: interface.name.clone(),
                args: vec![]
            }),
            crate::object_model::ObjectRepresentation::DynTrait { .. }
        ) || representations
            .existential_interface_carrier_for_interface(&interface.name)
            .is_some()
            || crate::effective_facts::EffectiveFacts::new(facts)
                .native_functional_carrier(&JavaType::Named {
                    name: interface.name.clone(),
                    args: vec![],
                })
                .is_some()
        {
            continue;
        }
        // Anonymous objects/SAMs and source-interface inheritance require their
        // own admission proof. Marker ancestors do not add a receiver contract.
        let contracts = crate::representation_lowering::specialized_interface_abstract_contracts(
            program, interface,
        );
        if contracts.len() <= 1
            || declarations.iter().any(|other| {
                other.kind == TypeKind::Interface
                    && other.name != interface.name
                    && (ancestry[&other.name]
                        .iter()
                        .any(|n| crate::facts::external_type_name_matches(n, &interface.name))
                        || ancestry[&interface.name]
                            .iter()
                            .any(|n| crate::facts::external_type_name_matches(n, &other.name)))
            })
        {
            continue;
        }
        let implementors = declarations
            .iter()
            .copied()
            .filter(|ty| {
                ty.kind != TypeKind::Interface
                    && !ty.modifiers.contains(&Modifier::Abstract)
                    && ancestry[&ty.name]
                        .iter()
                        .any(|n| crate::facts::external_type_name_matches(n, &interface.name))
            })
            .collect::<Vec<_>>();
        if implementors.is_empty()
            || implementors.iter().any(|ty| ty.kind != TypeKind::Class)
            || anonymous.iter().any(|n| {
                crate::facts::external_type_name_matches(n, &interface.name)
                    || crate::inherited_members::find_type_decl_for_owner_context(
                        program,
                        &interface.name,
                        n,
                    )
                    .is_some_and(|base| {
                        ancestry.get(&base.name).is_some_and(|interfaces| {
                            interfaces.iter().any(|n| {
                                crate::facts::external_type_name_matches(n, &interface.name)
                            })
                        })
                    })
            })
        {
            continue;
        }
        if !implementors.iter().all(|ty| {
            let placement = crate::storage_strategy::effective_storage_placement(
                facts.storage_placement_for(&ty.name),
                representations.storage_placement_for(&ty.name),
            );
            placement.shared() && placement.is_implemented()
        }) {
            continue;
        }
        let sync = crate::representation_lowering::source_interface_requires_sync(
            program, interface, facts,
        ) || crate::representation_lowering::source_interface_requires_send(
            program,
            interface,
            representations,
            facts,
        );
        if sync
            && !implementors.iter().all(|ty| {
                crate::storage_strategy::effective_storage_placement(
                    facts.storage_placement_for(&ty.name),
                    representations.storage_placement_for(&ty.name),
                )
                .sync()
            })
        {
            continue;
        }
        result.insert(interface.name.clone(), sync);
    }
    result
}

/// A bounded generic value stored behind a source-interface reference needs
/// the same Java-object lifetime capability as every other owned heap escape.
pub(crate) fn capture_requires_any(
    expr: &Expr,
    target: &JavaType,
    binder: &str,
    types: &crate::types::TypeEnv,
    representations: &RepresentationPlan,
) -> bool {
    match expr {
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            return [then_expr, else_expr].iter().any(|branch| {
                capture_requires_any(branch, target, binder, types, representations)
            });
        }
        Expr::Switch { cases, .. } => {
            return cases.iter().any(|case| {
                capture_requires_any(&case.value, target, binder, types, representations)
            });
        }
        _ => {}
    }
    let target = crate::types::optional_element_type(target).unwrap_or_else(|| target.clone());
    if !matches!(
        representations.dispatch_for(&target),
        crate::slot_shape::DispatchShape::ReferenceDynTrait { .. }
    ) {
        return false;
    }
    let actual = crate::types::infer_expr_type(expr, types);
    let actual = crate::types::optional_element_type(&actual).unwrap_or(actual);
    matches!(&actual, JavaType::Named { name, args } if name == binder && args.is_empty())
        || matches!(&actual, JavaType::Unknown(name) if name == binder)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn representation(source: &str) -> RepresentationPlan {
        let parsed = crate::parser::parse_java_source(source);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        crate::representation::plan_representations(&resolved.program, &semantic.facts)
    }
    #[test]
    fn mutable_canonical_implementations_admit_local_references() {
        let plan = representation(include_str!(
            "../tests/fixtures/SharedInterfaceReferenceProbe.java"
        ));
        assert_eq!(
            plan.source_interface_reference_sync("RefCounter"),
            Some(false)
        );
        assert!(matches!(
            plan.dispatch_for(&JavaType::Named {
                name: "RefCounter".into(),
                args: vec![JavaType::String]
            }),
            crate::slot_shape::DispatchShape::ReferenceDynTrait { sync: false, .. }
        ));
    }
    #[test]
    fn static_canonical_implementations_require_sync_reference_dispatch() {
        let plan = representation(include_str!(
            "../tests/fixtures/SharedInterfaceSyncReferenceProbe.java"
        ));
        assert_eq!(
            plan.source_interface_reference_sync("SyncReferenceCounter"),
            Some(true)
        );
    }
    #[test]
    fn anonymous_subclasses_and_source_interface_inheritance_retain_existing_dispatch() {
        for extra in [
            "class Extra { RefCounter<String> create() { return new RefCounterImpl() { public int value() { return 77; } }; } }",
            "interface SubCounter extends RefCounter<String> {}",
            "enum OtherCounter implements RefCounter<String> { ONLY; public int value() {return 0;} public void add(int delta) {} public int reenter(RefCounter<String> alias) {return 0;} public RefCounter<String> clone() {return this;} public String item() {return null;} public void item(String value) {} }",
        ] {
            let source = format!(
                "{}\n{extra}",
                include_str!("../tests/fixtures/SharedInterfaceReferenceProbe.java")
            );
            assert_eq!(
                representation(&source).source_interface_reference_sync("RefCounter"),
                None,
                "{extra}"
            );
        }
    }
}
