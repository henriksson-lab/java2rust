//! Physical qualifiers of source generic arguments, projected from solved
//! declaration slots. A binder is identified by its source owner and index;
//! an unrelated method's `T` never supplies an owner-level requirement.
use std::collections::HashMap;

use crate::java_ir::{JavaType, Member, Program, TypeDecl};
use crate::java_signature::JavaOwnerKey;
use crate::signature::SignatureDependencyNode;
use crate::signature::SignaturePlan;
use crate::slot_shape::RustSlotShape;
use crate::storage_strategy::StoragePlacement;
use crate::types::type_parameter_paths as binder_paths;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SourceGenericAllocationPlan {
    arguments: HashMap<JavaOwnerKey, Vec<RustSlotShape>>,
    aliases: HashMap<String, Option<JavaOwnerKey>>,
}

/// A declaration/use slot already represented by the signature solver.
#[derive(Debug, Clone)]
pub(crate) struct GenericAllocationSlot {
    pub node: SignatureDependencyNode,
    pub owner: String,
    pub ty: JavaType,
    pub shadows: Vec<String>,
    pub declaration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenericAllocationEdge {
    pub source: SignatureDependencyNode,
    pub source_path: Vec<usize>,
    pub source_argument: Option<JavaType>,
    pub target: SignatureDependencyNode,
    pub target_path: Vec<usize>,
    /// None denotes a symbolic binder occurrence; concrete uses retain the
    /// planner's native eligibility check at the substituted child path.
    pub target_argument: Option<JavaType>,
}

/// Index once, then let the ordinary dependency worklist saturate qualifiers.
/// Each scoped owner/binder has an explicit solver anchor, including owners
/// with no direct declaration occurrence. Star edges avoid a quadratic clique
/// between occurrences and instantiations.
pub(crate) fn constraint_edges(
    program: &Program,
    slots: &[GenericAllocationSlot],
) -> Vec<GenericAllocationEdge> {
    let types = source_types(program);
    let identities = SourceGenericAllocationPlan::for_declarations(program, &types);
    let mut groups =
        HashMap::<(JavaOwnerKey, usize), Vec<(SignatureDependencyNode, Vec<usize>)>>::new();
    for slot in slots.iter().filter(|slot| slot.declaration) {
        let Some(owner) = identities.owner_key(&slot.owner) else {
            continue;
        };
        let params = identities.arguments[&owner]
            .iter()
            .map(|shape| match &shape.java_ty {
                JavaType::Named { name, .. } => name.clone(),
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        for (index, path) in binder_paths(&slot.ty, &params, &slot.shadows) {
            groups
                .entry((owner.clone(), index))
                .or_default()
                .push((slot.node.clone(), path));
        }
    }
    // A binder is a solver slot even when its owner declares no field or
    // method mentioning it (e.g. Alias<A> extends Function<A,A>). Choosing a
    // declaration occurrence as the only anchor loses such ancestor paths.
    let anchors = identities
        .arguments
        .iter()
        .flat_map(|(owner, arguments)| {
            arguments.iter().enumerate().map(move |(index, _)| {
                (
                    (owner.clone(), index),
                    (
                        SignatureDependencyNode::GenericParameter {
                            owner: owner.clone(),
                            index,
                        },
                        vec![],
                    ),
                )
            })
        })
        .collect::<HashMap<_, _>>();
    let mut edges = Vec::new();
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left, _), (right, _)| {
        (left.0.as_str(), left.1).cmp(&(right.0.as_str(), right.1))
    });
    for (key, occurrences) in groups {
        let anchor = anchors[&key].clone();
        for occurrence in occurrences {
            for (from, to) in [(&anchor, &occurrence), (&occurrence, &anchor)] {
                edges.push(GenericAllocationEdge {
                    source: from.0.clone(),
                    source_path: from.1.clone(),
                    source_argument: None,
                    target: to.0.clone(),
                    target_path: to.1.clone(),
                    target_argument: None,
                });
            }
        }
    }
    for (child, parent, substitutions) in generic_ancestor_edges(program, &types) {
        let child_key = JavaOwnerKey::source_type_in_program(program, child);
        for (index, parameter) in identities.arguments[&parent].iter().enumerate() {
            let JavaType::Named { name, .. } = &parameter.java_ty else {
                continue;
            };
            let (Some(actual), Some(from)) = (
                substitutions.get(name),
                anchors.get(&(parent.clone(), index)),
            ) else {
                continue;
            };
            for (child_index, path) in binder_paths(actual, &child.type_params, &[]) {
                if let Some(to) = anchors.get(&(child_key.clone(), child_index)) {
                    let mut source_path = from.1.clone();
                    source_path.extend(path.clone());
                    edges.push(GenericAllocationEdge {
                        source: from.0.clone(),
                        source_path,
                        source_argument: None,
                        target: to.0.clone(),
                        target_path: to.1.clone(),
                        target_argument: None,
                    });
                    let mut target_path = from.1.clone();
                    target_path.extend(path.clone());
                    edges.push(GenericAllocationEdge {
                        source: to.0.clone(),
                        source_path: to.1.clone(),
                        source_argument: None,
                        target: from.0.clone(),
                        target_path,
                        target_argument: None,
                    });
                }
            }
        }
    }
    fn uses(
        slot: &GenericAllocationSlot,
        ty: &JavaType,
        prefix: &mut Vec<usize>,
        identities: &SourceGenericAllocationPlan,
        anchors: &HashMap<(JavaOwnerKey, usize), (SignatureDependencyNode, Vec<usize>)>,
        edges: &mut Vec<GenericAllocationEdge>,
    ) {
        let ty = crate::types::optional_element_type(ty).unwrap_or_else(|| ty.clone());
        match ty {
            JavaType::Named { name, args } => {
                let owner = identities.owner_key(&name);
                for (index, arg) in args.iter().enumerate() {
                    prefix.push(index);
                    if let Some(from) = owner
                        .as_ref()
                        .and_then(|owner| anchors.get(&(owner.clone(), index)))
                    {
                        edges.push(GenericAllocationEdge {
                            source: from.0.clone(),
                            source_path: from.1.clone(),
                            source_argument: None,
                            target: slot.node.clone(),
                            target_path: prefix.clone(),
                            target_argument: Some(arg.clone()),
                        });
                        edges.push(GenericAllocationEdge {
                            source: slot.node.clone(),
                            source_path: prefix.clone(),
                            source_argument: Some(arg.clone()),
                            target: from.0.clone(),
                            target_path: from.1.clone(),
                            target_argument: None,
                        });
                    }
                    uses(slot, arg, prefix, identities, anchors, edges);
                    prefix.pop();
                }
            }
            JavaType::Array(child) => {
                prefix.push(0);
                uses(slot, &child, prefix, identities, anchors, edges);
                prefix.pop();
            }
            _ => {}
        }
    }
    for slot in slots {
        uses(
            slot,
            &slot.ty,
            &mut Vec::new(),
            &identities,
            &anchors,
            &mut edges,
        );
    }
    edges
}

/// Scoped symbolic occurrence paths; method binders shadow owner binders.
/// A qualifier below symbolic T may only become structural after instantiation.
pub(crate) fn symbolic_occurrence_paths(
    program: &Program,
    slots: &[GenericAllocationSlot],
) -> Vec<crate::plan_contracts::NativeSlotKey> {
    let identities = SourceGenericAllocationPlan::for_declarations(program, &source_types(program));
    let mut result = Vec::new();
    for slot in slots {
        let params = identities
            .owner_key(&slot.owner)
            .map(|owner| {
                identities.arguments[&owner]
                    .iter()
                    .filter_map(|shape| match &shape.java_ty {
                        JavaType::Named { name, .. } => Some(name.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        // Owner binders and method binders are both symbolic at their exact
        // occurrence. Method binders shadow owner binders but must not become
        // concrete merely because the enclosing class has no type parameters.
        for (_, path) in binder_paths(&slot.ty, &params, &slot.shadows)
            .into_iter()
            .chain(binder_paths(&slot.ty, &slot.shadows, &[]))
        {
            result.push(crate::plan_contracts::NativeSlotKey {
                node: slot.node.clone(),
                path,
            });
        }
    }
    result
}

impl SourceGenericAllocationPlan {
    /// Pure artifact construction from the signature solver's binder slots.
    /// No ancestor fixed point or new qualifier inference belongs here.
    pub(crate) fn from_solved_binders(
        program: &Program,
        mut qualifiers: impl FnMut(&JavaOwnerKey, usize) -> Vec<(Vec<usize>, StoragePlacement)>,
    ) -> Self {
        let mut plan = Self::for_declarations(program, &source_types(program));
        for (owner, arguments) in &mut plan.arguments {
            for (index, argument) in arguments.iter_mut().enumerate() {
                for (path, placement) in qualifiers(owner, index) {
                    argument.require_native_allocation_at(&path, placement);
                }
            }
        }
        plan
    }

    #[cfg(test)]
    pub(crate) fn build(program: &Program, signatures: &SignaturePlan) -> Self {
        let types = source_types(program);
        let mut plan = Self::for_declarations(program, &types);
        for owner in types
            .iter()
            .copied()
            .filter(|ty| !ty.type_params.is_empty())
        {
            let key = JavaOwnerKey::source_type_in_program(program, owner);
            let mut arguments = owner
                .type_params
                .iter()
                .map(|name| {
                    RustSlotShape::for_storage(JavaType::Named {
                        name: name.clone(),
                        args: vec![],
                    })
                })
                .collect::<Vec<_>>();
            for member in &owner.members {
                match member {
                    Member::Method(method) => {
                        let Some(signature) = signatures.method_signature(owner, method) else {
                            continue;
                        };
                        for (param, solved) in method.params.iter().zip(signature.params()) {
                            project_binders(
                                &param.ty,
                                solved.shape(),
                                &owner.type_params,
                                &method.type_params,
                                &mut arguments,
                            );
                        }
                        if let Some(solved) = signature.return_shape() {
                            project_binders(
                                &method.return_type,
                                solved,
                                &owner.type_params,
                                &method.type_params,
                                &mut arguments,
                            );
                        }
                    }
                    Member::Field(field) => {
                        if let Some(solved) =
                            signatures.field_shape_for_declared_field(owner, field)
                        {
                            project_binders(
                                &field.ty,
                                solved,
                                &owner.type_params,
                                &[],
                                &mut arguments,
                            );
                        }
                    }
                    Member::Constructor(ctor) => {
                        if let Some(signature) = signatures.constructor_signature(owner, ctor) {
                            for (param, solved) in ctor.params.iter().zip(signature.params()) {
                                project_binders(
                                    &param.ty,
                                    solved.shape(),
                                    &owner.type_params,
                                    &ctor.type_params,
                                    &mut arguments,
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            plan.arguments.insert(key, arguments);
        }
        let edges = generic_ancestor_edges(program, &types);
        loop {
            let mut changed = false;
            for (child, parent_key, substitution) in &edges {
                let parent = plan.arguments[parent_key].clone();
                let child_key = JavaOwnerKey::source_type_in_program(program, child);
                let target = plan.arguments.get_mut(&child_key).unwrap();
                let before = target.clone();
                for argument in parent {
                    let JavaType::Named { name, .. } = &argument.java_ty else {
                        continue;
                    };
                    if let Some(actual) = substitution.get(name) {
                        project_binders(actual, &argument, &child.type_params, &[], target);
                    }
                }
                changed |= before != *target;
            }
            if !changed {
                break;
            }
        }
        plan
    }

    fn for_declarations(program: &Program, types: &[&TypeDecl]) -> Self {
        let mut plan = Self::default();
        for owner in types
            .iter()
            .copied()
            .filter(|owner| !owner.type_params.is_empty())
        {
            let key = JavaOwnerKey::source_type_in_program(program, owner);
            for alias in [owner.name.as_str(), owner.name.rsplit('.').next().unwrap()] {
                plan.aliases
                    .entry(alias.into())
                    .and_modify(|prior| {
                        if prior.as_ref() != Some(&key) {
                            *prior = None;
                        }
                    })
                    .or_insert(Some(key.clone()));
            }
            plan.arguments.insert(
                key,
                owner
                    .type_params
                    .iter()
                    .map(|name| {
                        RustSlotShape::for_storage(JavaType::Named {
                            name: name.clone(),
                            args: vec![],
                        })
                    })
                    .collect(),
            );
        }
        plan
    }

    fn owner_key(&self, owner: &str) -> Option<JavaOwnerKey> {
        let key = JavaOwnerKey::resolved_source(owner);
        if self.arguments.contains_key(&key) {
            Some(key)
        } else {
            self.aliases.get(owner)?.clone()
        }
    }

    pub(crate) fn argument(&self, owner: &str, index: usize) -> Option<&RustSlotShape> {
        self.arguments.get(&self.owner_key(owner)?)?.get(index)
    }

    /// Annotate an instantiated source type at its structural child paths.
    /// Qualifiers are attached only where substitution produces a native
    /// value; a String/class argument does not become an array allocation.
    pub(crate) fn project(&self, shape: &mut RustSlotShape) {
        fn visit(
            plan: &SourceGenericAllocationPlan,
            ty: &JavaType,
            prefix: &mut Vec<usize>,
            out: &mut Vec<(Vec<usize>, StoragePlacement)>,
        ) {
            let ty = crate::types::optional_element_type(ty).unwrap_or_else(|| ty.clone());
            match ty {
                JavaType::Named { name, args } => {
                    for (index, arg) in args.iter().enumerate() {
                        prefix.push(index);
                        if let Some(required) = plan.argument(&name, index) {
                            for (path, placement) in allocation_paths(required) {
                                if native_argument_allocation_target(arg, &path) {
                                    let mut path_at_root = prefix.clone();
                                    path_at_root.extend(path);
                                    out.push((path_at_root, placement));
                                }
                            }
                        }
                        visit(plan, arg, prefix, out);
                        prefix.pop();
                    }
                }
                JavaType::Array(element) => {
                    prefix.push(0);
                    visit(plan, &element, prefix, out);
                    prefix.pop();
                }
                _ => {}
            }
        }
        let mut requirements = Vec::new();
        visit(self, &shape.java_ty, &mut Vec::new(), &mut requirements);
        for (path, placement) in requirements {
            shape.require_native_allocation_at(&path, placement);
        }
    }
}

fn allocation_paths(shape: &RustSlotShape) -> Vec<(Vec<usize>, StoragePlacement)> {
    let mut paths = shape.nested_native_allocations.clone();
    if shape.native_allocation != StoragePlacement::default() {
        paths.push((vec![], shape.native_allocation));
    }
    paths
}

fn project_binders(
    declared: &JavaType,
    solved: &RustSlotShape,
    params: &[String],
    shadows: &[String],
    arguments: &mut [RustSlotShape],
) {
    let requirements = allocation_paths(solved);
    for (index, prefix) in binder_paths(declared, params, shadows) {
        for (path, placement) in &requirements {
            if let Some(suffix) = path.strip_prefix(prefix.as_slice()) {
                arguments[index].require_native_allocation_at(suffix, *placement);
            }
        }
    }
}

pub(crate) fn native_argument_allocation_target(ty: &JavaType, path: &[usize]) -> bool {
    RustSlotShape::native_child_type_at_path(ty, path).is_some_and(|ty| {
        let ty = crate::types::optional_element_type(&ty).unwrap_or(ty);
        matches!(ty, JavaType::Array(_))
            || crate::stdlib::java_collection_kind_for_type(&ty).is_some()
    })
}

fn source_types(program: &Program) -> Vec<&TypeDecl> {
    fn collect<'a>(types: &'a [TypeDecl], out: &mut Vec<&'a TypeDecl>) {
        for ty in types {
            out.push(ty);
            for member in &ty.members {
                if let Member::Type(nested) = member {
                    collect(std::slice::from_ref(nested), out);
                }
            }
        }
    }
    let mut out = Vec::new();
    collect(&program.types, &mut out);
    out
}

fn generic_ancestor_edges<'a>(
    program: &'a Program,
    types: &[&'a TypeDecl],
) -> Vec<(&'a TypeDecl, JavaOwnerKey, HashMap<String, JavaType>)> {
    let generic = types
        .iter()
        .copied()
        .filter(|ty| !ty.type_params.is_empty())
        .collect::<Vec<_>>();
    let mut edges = Vec::new();
    for &child in &generic {
        for parent_ref in child.extends.iter().chain(&child.implements) {
            let JavaType::Named { name, .. } = crate::parser::parse_hierarchy_type(parent_ref)
            else {
                continue;
            };
            let Some(parent) = crate::inherited_members::find_type_decl_for_owner_context(
                program,
                &child.name,
                &name,
            ) else {
                continue;
            };
            if parent.type_params.is_empty() {
                continue;
            }
            let Some(substitution) =
                crate::inherited_members::source_ancestor_type_arguments(program, child, parent)
            else {
                continue;
            };
            edges.push((
                child,
                JavaOwnerKey::source_type_in_program(program, parent),
                substitution,
            ));
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraint_index_is_linear_and_keeps_native_eligibility() {
        let fields = (0..100)
            .map(|index| format!("T value{index};"))
            .collect::<String>();
        let parsed = crate::parser::parse_java_source(&format!(
            "class Domain<T> {{ {fields} }} class Consumer {{ Domain<int[]> array; Domain<String> text; }}"
        ));
        let mut slots = Vec::new();
        for owner in &parsed.program.types {
            for member in &owner.members {
                if let Member::Field(field) = member {
                    slots.push(GenericAllocationSlot {
                        node: SignatureDependencyNode::Field {
                            field: crate::signature::FieldKey::source_field(owner, field),
                        },
                        owner: owner.name.clone(),
                        ty: field.ty.clone(),
                        shadows: vec![],
                        declaration: true,
                    });
                }
            }
        }
        let edges = constraint_edges(&parsed.program, &slots);
        assert!(
            edges.len() <= slots.len() * 3,
            "{} edges for {} slots",
            edges.len(),
            slots.len()
        );
        let concrete = edges
            .iter()
            .filter_map(|edge| edge.target_argument.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(concrete.len(), 2);
        assert!(
            concrete
                .iter()
                .any(|ty| native_argument_allocation_target(ty, &[]))
        );
        assert!(
            concrete
                .iter()
                .any(|ty| !native_argument_allocation_target(ty, &[]))
        );
    }

    #[test]
    fn declaration_projection_respects_shadowing_paths_and_unrelated_binders() {
        let parse = crate::parser::parse_hierarchy_type;
        let mut solved = RustSlotShape::for_storage(parse("List<T>"));
        let local = StoragePlacement::new(true, true, false);
        let sync = StoragePlacement::new(true, true, true);
        solved.require_native_allocation_at(&[0], local);
        solved.require_native_allocation_at(&[0, 1], sync);
        let params = vec!["T".into(), "U".into()];
        let empty = vec![
            RustSlotShape::for_storage(parse("T")),
            RustSlotShape::for_storage(parse("U")),
        ];
        let mut projected = empty.clone();
        project_binders(
            &parse("List<T>"),
            &solved,
            &params,
            &["T".into()],
            &mut projected,
        );
        assert_eq!(projected, empty, "method T shadows the owner's T");
        project_binders(&parse("List<T>"), &solved, &params, &[], &mut projected);
        assert_eq!(projected[0].native_allocation, local);
        assert_eq!(
            projected[0].nested_native_allocations,
            vec![(vec![1], sync)]
        );
        assert_eq!(projected[1], empty[1]);
        assert_eq!(projected[0].java_ty, parse("T"));
    }

    #[test]
    fn instantiations_preserve_optionality_and_reject_ambiguous_owner_aliases() {
        let parse = crate::parser::parse_hierarchy_type;
        let mut required = RustSlotShape::for_storage(parse("T"));
        let sync = StoragePlacement::new(true, true, true);
        required.native_allocation = sync;
        let mut plan = SourceGenericAllocationPlan::default();
        for owner in ["a.Value", "b.Value"] {
            plan.arguments
                .insert(JavaOwnerKey::resolved_source(owner), vec![required.clone()]);
        }
        plan.aliases.insert("Value".into(), None);
        assert!(plan.argument("Value", 0).is_none());
        assert!(plan.argument("missing.Value", 0).is_none());
        let ty = crate::types::optional_java_type(parse("a.Value<int[]>"));
        let mut shape = RustSlotShape::for_storage(ty.clone());
        plan.project(&mut shape);
        assert_eq!(shape.java_ty, ty);
        assert_eq!(shape.native_allocation, StoragePlacement::default());
        assert_eq!(shape.nested_native_allocations, vec![(vec![0], sync)]);
        let mut concrete = RustSlotShape::for_storage(parse("a.Value<String>"));
        plan.project(&mut concrete);
        assert!(concrete.nested_native_allocations.is_empty());
    }
}
