//! Method-scoped native upper-bound contracts. These describe a generic
//! carrier's children without assigning a concrete owner representation to M.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MethodGenericBound {
    pub name: String,
    pub java_bound: JavaType,
    pub return_occurrence: bool,
    pub constraints: Vec<SignatureConstraint>,
}

pub(super) fn collect(
    program: &Program,
    facts: &FactSet,
) -> HashMap<(MethodKey, usize), MethodGenericBound> {
    fn visit(
        ty: &TypeDecl,
        facts: &FactSet,
        result: &mut HashMap<(MethodKey, usize), MethodGenericBound>,
    ) {
        for member in &ty.members {
            match member {
                Member::Method(method) => {
                    for (index, (name, bound)) in method
                        .type_params
                        .iter()
                        .zip(&method.type_param_bounds)
                        .enumerate()
                    {
                        let Some(bound) = bound else {
                            continue;
                        };
                        if EffectiveFacts::new(facts)
                            .retained_native_map_kind(bound)
                            .is_none()
                        {
                            continue;
                        }
                        result.insert((MethodKey::source_method(ty, method), index), MethodGenericBound {
                            name: name.clone(), java_bound: bound.clone(),
                            return_occurrence: matches!(&method.return_type, JavaType::Named { name: returned, args } if returned == name && args.is_empty()),
                            constraints: Vec::new(),
                        });
                    }
                }
                Member::Type(nested) => visit(nested, facts, result),
                _ => {}
            }
        }
    }
    let mut result = HashMap::new();
    for ty in &program.types {
        visit(ty, facts, &mut result);
    }
    result
}

pub(super) fn remap_storage_node(
    state: &SignatureConstraintState,
    node: SignatureDependencyNode,
) -> SignatureDependencyNode {
    let (method, java_ty, returned) = match &node {
        SignatureDependencyNode::MethodParam { method, index, .. } => (
            method,
            state
                .methods
                .get(method)
                .and_then(|set| set.param_java_types.get(*index)),
            false,
        ),
        SignatureDependencyNode::MethodLocal {
            method,
            binding: Some(binding),
            ..
        } => (
            method,
            state
                .methods
                .get(method)
                .and_then(|set| set.local_by_binding(binding))
                .map(|local| &local.java_ty),
            false,
        ),
        SignatureDependencyNode::MethodReturn { method } => (method, None, true),
        _ => return node,
    };
    let found = state.method_generic_bounds.iter().find_map(|((owner, index), bound)| {
        (owner == method && if returned { bound.return_occurrence } else {
            matches!(java_ty, Some(JavaType::Named { name, args }) if name == &bound.name && args.is_empty())
        }).then_some(*index)
    });
    found
        .map(|index| SignatureDependencyNode::MethodGenericBound {
            method: method.clone(),
            index,
        })
        .unwrap_or(node)
}

pub(super) fn endpoint(
    state: &SignatureConstraintState,
    node: SignatureDependencyNode,
    slot: &crate::usage::NativeCollectionStorageSlot,
) -> NativeCollectionStorageEndpoint {
    let node = remap_storage_node(state, node);
    if let SignatureDependencyNode::MethodGenericBound { method, index } = &node
        && let Some(bound) = state.method_generic_bounds.get(&(method.clone(), *index))
        && let Some(child) = RustSlotShape::native_child_type_at_path(&bound.java_bound, &slot.path)
    {
        return (node, slot.path.clone(), child, bound.java_bound.clone());
    }
    (
        node,
        slot.path.clone(),
        slot.ty.clone(),
        slot.root_ty.clone(),
    )
}

fn node_type(state: &SignatureConstraintState, node: &SignatureDependencyNode) -> Option<JavaType> {
    match node {
        SignatureDependencyNode::MethodGenericBound { method, index } => state
            .method_generic_bounds
            .get(&(method.clone(), *index))
            .map(|bound| bound.java_bound.clone()),
        SignatureDependencyNode::MethodParam { method, index, .. } => state
            .methods
            .get(method)?
            .param_java_types
            .get(*index)
            .cloned(),
        SignatureDependencyNode::MethodReturn { method } => {
            state.methods.get(method)?.return_java_type.clone()
        }
        SignatureDependencyNode::MethodLocal {
            method,
            binding: Some(binding),
            ..
        } => state
            .methods
            .get(method)?
            .local_by_binding(binding)
            .map(|local| local.java_ty.clone()),
        SignatureDependencyNode::Field { field } => state.field_java_types.get(field).cloned(),
        _ => None,
    }
}

pub(super) fn edges(
    state: &SignatureConstraintState,
    usage: &UsageFacts,
) -> Vec<NativeCollectionStorageEdge> {
    let mut result = Vec::new();
    let mut add = |left: SignatureDependencyNode, right: SignatureDependencyNode, retain| {
        let left = remap_storage_node(state, left);
        let right = remap_storage_node(state, right);
        if !matches!(left, SignatureDependencyNode::MethodGenericBound { .. })
            && !matches!(right, SignatureDependencyNode::MethodGenericBound { .. })
        {
            return;
        }
        let (Some(left_ty), Some(right_ty)) = (node_type(state, &left), node_type(state, &right))
        else {
            return;
        };
        if crate::stdlib::java_map_type_args(&left_ty).is_none()
            || crate::stdlib::java_map_type_args(&right_ty).is_none()
        {
            return;
        }
        result.push((
            (left, vec![], left_ty.clone(), left_ty),
            (right, vec![], right_ty.clone(), right_ty),
            retain,
            CollectionStorageProjection::Alias,
        ));
    };
    for (key, caller) in usage.method_entries() {
        let Some(caller_key) = method_key_for_usage(state, key) else {
            continue;
        };
        for call in caller.call_sites() {
            let Some(callee) = call
                .resolved()
                .and_then(|call| method_key_for_resolved_call(state, call))
            else {
                continue;
            };
            let Some(parameters) = state.methods.get(&callee) else {
                continue;
            };
            for (index, binding) in call.argument_bindings() {
                let Some(name) = parameters.param_names.get(*index) else {
                    continue;
                };
                let Some(binding) = caller.binding_facts_for_declaration_id(binding) else {
                    continue;
                };
                let Some(source) = method_binding_fact_dependency_node(state, &caller_key, binding)
                else {
                    continue;
                };
                add(
                    source,
                    SignatureDependencyNode::MethodParam {
                        method: callee.clone(),
                        name: name.clone(),
                        index: *index,
                    },
                    true,
                );
            }
            for (index, origin) in call.collection_argument_origins() {
                let Some(name) = parameters.param_names.get(*index) else {
                    continue;
                };
                let source = match origin {
                    crate::usage::CollectionArgumentOrigin::MethodReturn(call) => {
                        method_key_for_resolved_call(state, call)
                            .map(|method| SignatureDependencyNode::MethodReturn { method })
                    }
                    crate::usage::CollectionArgumentOrigin::Field { owner, name } => {
                        field_key_for_resolved_source(state, owner, name)
                            .map(|field| SignatureDependencyNode::Field { field })
                    }
                };
                if let Some(source) = source {
                    add(
                        source,
                        SignatureDependencyNode::MethodParam {
                            method: callee.clone(),
                            name: name.clone(),
                            index: *index,
                        },
                        true,
                    );
                }
            }
        }
    }
    for (left, right) in call_result_storage_edges(state, usage) {
        add(left, right, false);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_method_bound_contracts_survive_separate_primary_emission() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/corpus/csv/GenericMapInsertContracts.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let representations =
            crate::representation::plan_representations(&resolved.program, &resolved.facts);
        let usage = crate::usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let primary = Program::default();
        let whole = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &resolved.facts,
        );
        let plan = solve_signatures_with_facts_and_support_context(
            &primary,
            &representations,
            &usage,
            &resolved.facts,
            &resolved.program,
        );
        let owner = &resolved.program.types[0];
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method)
                if matches!(method.name.as_str(), "putIn" | "forward" | "putViaHeaders") =>
            {
                Some(method)
            }
            _ => None,
        }) {
            let bound = plan.method_generic_bound_shape(owner, method, 0).unwrap();
            let (_, value) = crate::stdlib::java_map_type_args(&bound.java_ty).unwrap();
            assert!(
                crate::types::optional_element_type(value).is_some(),
                "{:?}",
                bound
            );
            assert_eq!(
                bound,
                whole.method_generic_bound_shape(owner, method, 0).unwrap(),
                "cross-file bound {}",
                method.name
            );
        }
    }

    #[test]
    fn method_map_bound_children_reach_forwarders_and_caller_storage() {
        let parsed = crate::parser::parse_java_source(include_str!(
            "../tests/corpus/csv/GenericMapInsertContracts.java"
        ));
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let representations =
            crate::representation::plan_representations(&resolved.program, &resolved.facts);
        let usage = crate::usage::analyze_usage_with_facts(&resolved.program, &resolved.facts);
        let plan = solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &resolved.facts,
        );
        let ty = &resolved.program.types[0];
        for name in ["putIn", "forward"] {
            let method = ty
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == name => Some(method),
                    _ => None,
                })
                .unwrap();
            let bound = plan.method_generic_bound_shape(ty, method, 0).unwrap();
            let (_, value) = crate::stdlib::java_map_type_args(&bound.java_ty).unwrap();
            assert!(
                crate::types::optional_element_type(value).is_some(),
                "{name}: {bound:?}; diagnostics={:?}",
                plan.diagnostics()
            );
            assert!(
                !bound.native_allocation.shared(),
                "generic owner remains caller-specific"
            );
        }
        let run = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "run" => Some(method),
                _ => None,
            })
            .unwrap();
        let target = plan
            .method_signature(ty, run)
            .unwrap()
            .locals()
            .iter()
            .find(|local| local.name() == "target")
            .unwrap();
        let (_, value) = crate::stdlib::java_map_type_args(&target.shape().java_ty).unwrap();
        assert!(
            crate::types::optional_element_type(value).is_some(),
            "caller: {:?}",
            target.shape()
        );
        let unrelated = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "unrelated" => Some(method),
                _ => None,
            })
            .unwrap();
        let bound = plan.method_generic_bound_shape(ty, unrelated, 0).unwrap();
        let (key, value) = crate::stdlib::java_map_type_args(&bound.java_ty).unwrap();
        assert!(
            crate::types::optional_element_type(key).is_none()
                && crate::types::optional_element_type(value).is_none(),
            "unrelated M must retain its own contract: {bound:?}"
        );
    }

    #[test]
    fn source_owned_map_bound_is_not_a_native_method_bound() {
        let parsed = crate::parser::parse_java_source(
            "class Map<K,V> {} class Use { <M extends Map<String,String>> M keep(M value) { return value; } }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(collect(&resolved.program, &resolved.facts).is_empty());
    }
}
