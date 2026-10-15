use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

use crate::coercion::{
    function_call_projection, owner_associated_path_projection,
    recover_parseable_raw_operand_projection, rust_string_literal_projection,
    self_associated_path_projection,
};
use crate::exception_lowering::exception_result_type;
use crate::exception_plan::MethodExceptionPlan;
use crate::facts::FactSet;
use crate::java_ir::{
    BinaryOp, Block, ConstructorDecl, Expr, FieldDecl, JavaType, Literal, Member, MethodDecl,
    Modifier, PrimitiveType, Program, Stmt, TypeDecl, TypeKind,
};
use crate::java_naming::rust_name_for_java_named_type;
use crate::representation::RepresentationPlan;
use crate::rust_ir::{
    RustAssociatedConst, RustExpr, RustField, RustFunction, RustImpl, RustItem, RustParam,
    RustReceiver, RustStmt, RustStruct, RustType, rust_type_arg_projection_or_raw,
};
use crate::signature::{ReceiverPlan, SignaturePlan};
use crate::slot_render::{
    default_expr_for_slot_with_facts, rust_type_for_java_type_with_facts,
    rust_type_for_slot_with_facts, rust_type_for_slot_with_facts_and_representations,
};
use crate::slot_shape::{OwnershipShape, RustSlotShape};
use crate::types::{
    TypeEnv, infer_expr_type, optional_element_type, raw_rust_box_dyn_trait,
    raw_rust_box_sized_inner, raw_rust_generic_args, raw_rust_option_inner, raw_rust_vec_inner,
    sequence_element_type, two_dimensional_array_element_type,
};

#[cfg(test)]
pub(crate) fn struct_item_for_type_with_facts(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> RustItem {
    struct_item_for_type_with_facts_and_support_context(ty, signatures, facts, &Program::default())
}

#[cfg(test)]
fn struct_item_for_type_with_facts_and_support_context(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
) -> RustItem {
    let fields = struct_fields_for_type(ty, signatures, facts, None, support_program);
    RustItem::Struct(RustStruct {
        name: rust_decl_name_for_source_type(ty),
        derives: struct_derives_for_type(ty, &fields, signatures, facts, support_program),
        fields,
        is_unit: false,
    })
}

pub(crate) fn struct_item_for_type_with_facts_representations_and_support_context(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
    support_program: &Program,
) -> RustItem {
    let fields = struct_fields_for_type(
        ty,
        signatures,
        facts,
        Some(representations),
        support_program,
    );
    RustItem::Struct(RustStruct {
        name: rust_decl_name_for_source_type(ty),
        derives: struct_derives_for_type(ty, &fields, signatures, facts, support_program),
        fields,
        is_unit: false,
    })
}

/// Emit a conditional `Default` implementation for a generic source type
/// used as a constructor placeholder.
///
/// Constructor lowering currently creates the Rust struct before replaying
/// Java field assignments.  A field assigned later therefore needs a
/// temporary default value.  Generic structs cannot safely use
/// `#[derive(Default)]`: derive can impose unnecessary `T: Default` bounds,
/// while carriers such as `Vec<T>`, `HashMap<T, U>`, and `PhantomData<T>` are
/// defaultable for every `T`.  Expressing the obligation on the emitted field
/// types lets Rust prove precisely the construction that lowering performs.
pub(crate) fn source_constructor_placeholder_default_impl_for_type(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
    support_program: &Program,
) -> Option<String> {
    if ty.type_params.is_empty()
        || !program_needs_constructor_placeholder_default_for_type(support_program, ty)
    {
        return None;
    }
    let fields = struct_fields_for_type(
        ty,
        signatures,
        facts,
        Some(representations),
        support_program,
    );
    if struct_derives_for_type(ty, &fields, signatures, facts, support_program)
        .iter()
        .any(|derive| derive == "Default")
    {
        return None;
    }
    let mut field_types = fields
        .iter()
        .map(|field| crate::emit::emit_type(&field.ty))
        .collect::<Vec<_>>();
    field_types.sort();
    field_types.dedup();
    let where_clause = if field_types.is_empty() {
        String::new()
    } else {
        format!(
            " where {}",
            field_types
                .iter()
                .map(|field_ty| format!("{field_ty}: Default"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let initializers = fields
        .iter()
        .map(|field| format!("{}: Default::default()", field.name))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "impl<{}> Default for {}{where_clause} {{\n    fn default() -> Self {{ Self {{ {initializers} }} }}\n}}",
        ty.type_params.join(", "),
        rust_use_name_for_source_type(ty),
    ))
}

#[cfg(test)]
pub(crate) fn impl_item_for_type(ty: &TypeDecl, functions: Vec<RustFunction>) -> RustItem {
    impl_item_for_type_with_support_context(ty, functions, &Program::default())
}

#[cfg(test)]
pub(crate) fn impl_item_for_type_with_support_context(
    ty: &TypeDecl,
    functions: Vec<RustFunction>,
    support_program: &Program,
) -> RustItem {
    impl_item_for_type_with_support_context_and_representations(
        ty,
        functions,
        support_program,
        None,
    )
}

#[cfg(test)]
pub(crate) fn impl_item_for_type_with_support_context_and_representations(
    ty: &TypeDecl,
    functions: Vec<RustFunction>,
    support_program: &Program,
    representations: Option<&RepresentationPlan>,
) -> RustItem {
    RustItem::Impl(RustImpl {
        target: rust_impl_target_for_source_type(ty, support_program, representations),
        trait_name: None,
        associated_consts: associated_consts_for_type(ty),
        functions,
    })
}

/// Assemble inherent source methods without imposing instance-only Java
/// generic bounds on static members.
///
/// Java forbids a static member from referring to its declaring class's type
/// parameters.  Such a member therefore belongs on an unbounded Rust impl;
/// putting it on the bounded instance impl makes otherwise valid calls require
/// an unrelated bound (for example `Reader<O: Distribution<?>>::parse`).
pub(crate) fn impl_items_for_type_with_static_partition(
    ty: &TypeDecl,
    instance_functions: Vec<RustFunction>,
    static_functions: Vec<RustFunction>,
    support_program: &Program,
    representations: Option<&RepresentationPlan>,
) -> Vec<RustItem> {
    let associated_consts = associated_consts_for_type(ty);
    if ty.type_params.is_empty() {
        let mut functions = instance_functions;
        functions.extend(static_functions);
        return vec![RustItem::Impl(RustImpl {
            target: rust_impl_target_for_source_type(ty, support_program, representations),
            trait_name: None,
            associated_consts,
            functions,
        })];
    }

    let mut items = Vec::new();
    if !static_functions.is_empty() || !associated_consts.is_empty() {
        items.push(RustItem::Impl(RustImpl {
            target: rust_unbounded_impl_target_for_source_type(ty),
            trait_name: None,
            associated_consts,
            functions: static_functions,
        }));
    }
    if !instance_functions.is_empty() {
        items.push(RustItem::Impl(RustImpl {
            target: rust_impl_target_for_source_type(ty, support_program, representations),
            trait_name: None,
            associated_consts: Vec::new(),
            functions: instance_functions,
        }));
    }
    items
}

pub(crate) fn rust_decl_name_for_source_type(ty: &TypeDecl) -> String {
    let name = rust_name_for_java_named_type(&ty.name);
    if ty.type_params.is_empty() {
        name
    } else {
        format!("{}<{}>", name, ty.type_params.join(", "))
    }
}

pub(crate) fn rust_use_name_for_source_type(ty: &TypeDecl) -> String {
    rust_decl_name_for_source_type(ty)
}

pub(crate) fn rust_impl_target_for_source_type(
    ty: &TypeDecl,
    support_program: &Program,
    representations: Option<&RepresentationPlan>,
) -> String {
    let use_name = rust_use_name_for_source_type(ty);
    let impl_params = rust_impl_params_for_source_type(ty, support_program, representations);
    if impl_params.is_empty() {
        use_name
    } else {
        format!("<{}> {}", impl_params.join(", "), use_name)
    }
}

fn rust_unbounded_impl_target_for_source_type(ty: &TypeDecl) -> String {
    let use_name = rust_use_name_for_source_type(ty);
    if ty.type_params.is_empty() {
        use_name
    } else {
        format!("<{}> {use_name}", ty.type_params.join(", "))
    }
}

/// Rust bounds for one declared Java type-parameter bound.
///
/// The single owner of that mapping. It previously existed twice — once for
/// class parameters and once for method/constructor parameters — and the two
/// copies drifted: only the class copy learned the JDK case, so a
/// `Comparable` bound on a method was silently dropped.
pub(crate) fn rust_bounds_for_java_type_param(
    bound: &JavaType,
    support_programs: &[&Program],
) -> Vec<String> {
    if let JavaType::Intersection(bounds) = bound {
        let mut projected = Vec::new();
        for bound in bounds {
            for rust_bound in rust_bounds_for_java_type_param(bound, support_programs) {
                if !projected.contains(&rust_bound) {
                    projected.push(rust_bound);
                }
            }
        }
        return projected;
    }
    let JavaType::Named {
        name: bound_name,
        args,
    } = bound
    else {
        return Vec::new();
    };
    if source_type_bound_is_trait_like(bound_name, support_programs) {
        let trait_name = rust_name_for_java_named_type(bound_name);
        if args.is_empty() {
            return vec![trait_name];
        }
        let args = args
            .iter()
            .map(rust_type_for_java_type_param_bound_arg)
            .collect::<Vec<_>>()
            .join(", ");
        return vec![format!("{trait_name}<{args}>")];
    }
    if source_type_bound_is_class_like(bound_name, support_programs) {
        let trait_name = crate::representation_lowering::source_class_bound_trait_name(bound_name);
        if args.is_empty() {
            return vec![trait_name];
        }
        let args = args
            .iter()
            .map(rust_type_for_java_type_param_bound_arg)
            .collect::<Vec<_>>()
            .join(", ");
        return vec![format!("{trait_name}<{args}>")];
    }
    // The bound names a library type rather than a source one. Dropping it left
    // the parameter with no capability at all, so `T extends Comparable<T>`
    // lost `compareTo`.
    let projected = jdk_type_param_bound_rust_bounds(bound_name, args);
    if !projected.is_empty() {
        return projected;
    }
    if source_type_bound_has_any_candidate(bound_name, support_programs) {
        // Ambiguous simple source names deliberately remain unresolved here.
        // Owner-aware emitters get a chance to select the same-package type;
        // treating the simple spelling as an external trait would preempt
        // that stronger source identity.
        return Vec::new();
    }

    // An unavailable external declaration is still a Java capability
    // contract when it appears as an upper bound. A Rust struct cannot be
    // used in this position, and dropping the bound makes every receiver call
    // on the type parameter ill-typed. The external-support planner emits a
    // matching demand-driven trait and moves its observed instance surface
    // onto that trait.
    let trait_name = rust_name_for_java_named_type(bound_name);
    if args.is_empty() {
        vec![trait_name]
    } else {
        let args = args
            .iter()
            .map(rust_type_for_java_type_param_bound_arg)
            .collect::<Vec<_>>()
            .join(", ");
        vec![format!("{trait_name}<{args}>")]
    }
}

fn primary_named_type_param_bound(bound: &JavaType) -> Option<&str> {
    match bound {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => Some(name),
        JavaType::Intersection(bounds) => bounds.first().and_then(primary_named_type_param_bound),
        _ => None,
    }
}

/// Projects an argument that occurs inside a Java type-parameter bound.
///
/// Ordinary wildcard *values* may need erasure because Rust has no direct
/// existential generic argument. A bound such as
/// `T extends Factory<? super T>` is different: erasing its argument to
/// `Any` also erases the only capability the bound exists to provide. The
/// generated Rust parameter is already the storage carrier substituted for
/// `T`, so projecting the wildcard's declared bound preserves one coherent
/// trait contract (`Factory<T>`) for declaration, implementation, and calls.
fn rust_type_for_java_type_param_bound_arg(arg: &JavaType) -> String {
    let effective = match arg {
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound)))
        | JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => bound.as_ref(),
        _ => arg,
    };
    crate::emit::emit_type(&rust_type_for_java_type_with_facts(
        effective,
        &FactSet::default(),
    ))
}

/// Whether any declared type-parameter bound in the program projects to a Rust
/// bound that needs the `JavaComparable` support trait.
///
/// Asks the projection itself rather than re-scanning the IR for `Comparable`.
/// A separate scan is a second source of truth, and the previous one walked only
/// class parameters — so a `Comparable` bound on a method projected to `Ord`
/// while the trait providing `compare_to` was never emitted.
pub(crate) fn program_projects_java_comparable_bound(
    program: &Program,
    support_programs: &[&Program],
) -> bool {
    fn bound_needs_it(bound: &Option<JavaType>, support_programs: &[&Program]) -> bool {
        bound.as_ref().is_some_and(|bound| {
            rust_bounds_for_java_type_param(bound, support_programs)
                .iter()
                .any(|rust_bound| rust_bound == "Ord")
        })
    }
    fn walk(ty: &TypeDecl, support_programs: &[&Program]) -> bool {
        ty.type_param_bounds
            .iter()
            .any(|bound| bound_needs_it(bound, support_programs))
            || ty.members.iter().any(|member| match member {
                Member::Method(method) => method
                    .type_param_bounds
                    .iter()
                    .any(|bound| bound_needs_it(bound, support_programs)),
                Member::Constructor(constructor) => constructor
                    .type_param_bounds
                    .iter()
                    .any(|bound| bound_needs_it(bound, support_programs)),
                Member::Type(nested) => walk(nested, support_programs),
                _ => false,
            })
    }
    program.types.iter().any(|ty| walk(ty, support_programs))
}

/// Whether a declared type-parameter bound needs the shared Java-enum
/// capability. Keep this coupled to `rust_bounds_for_java_type_param`: the
/// projection, rather than another spelling-based scan, is the source of
/// truth for both the generic signature and its support item.
pub(crate) fn program_projects_java_enum_bound(
    program: &Program,
    support_programs: &[&Program],
) -> bool {
    fn bound_needs_it(bound: &Option<JavaType>, support_programs: &[&Program]) -> bool {
        bound.as_ref().is_some_and(|bound| {
            rust_bounds_for_java_type_param(bound, support_programs)
                .iter()
                .any(|rust_bound| rust_bound == "JavaEnum")
        })
    }
    fn walk(ty: &TypeDecl, support_programs: &[&Program]) -> bool {
        ty.type_param_bounds
            .iter()
            .any(|bound| bound_needs_it(bound, support_programs))
            || ty.members.iter().any(|member| match member {
                Member::Method(method) => method
                    .type_param_bounds
                    .iter()
                    .any(|bound| bound_needs_it(bound, support_programs)),
                Member::Constructor(constructor) => constructor
                    .type_param_bounds
                    .iter()
                    .any(|bound| bound_needs_it(bound, support_programs)),
                Member::Type(nested) => walk(nested, support_programs),
                _ => false,
            })
    }
    program.types.iter().any(|ty| walk(ty, support_programs))
}

/// `Comparable` maps to `Ord`; the generated support library blanket-implements
/// `JavaComparable` (and therefore `compare_to`) for every `T: Ord`.
fn jdk_type_param_bound_rust_bounds(bound: &str, args: &[JavaType]) -> Vec<String> {
    match bound.rsplit('.').next().unwrap_or(bound) {
        "Comparable" => vec!["Ord".to_string()],
        "Cloneable" => vec!["Clone".to_string()],
        "Map" if args.len() == 2 => vec![format!(
            "JavaMapInsert<{}, {}>",
            rust_type_for_java_type_param_bound_arg(&args[0]),
            rust_type_for_java_type_param_bound_arg(&args[1])
        )],
        // Every Java enum has stable identity/value semantics and exposes its
        // declaration ordinal. One capability trait carries all of those
        // guarantees instead of rediscovering Clone/Eq/Hash at each use.
        "Enum" => vec!["JavaEnum".to_string()],
        _ => Vec::new(),
    }
}

pub(crate) fn rust_impl_params_for_source_type(
    ty: &TypeDecl,
    support_program: &Program,
    representations: Option<&RepresentationPlan>,
) -> Vec<String> {
    let field_types = source_field_types(ty);
    ty.type_params
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let mut bounds = Vec::new();
            if let Some(Some(bound)) = ty.type_param_bounds.get(index) {
                if !unexpressible_existential_bound_is_unused(ty, name, bound, support_program) {
                    bounds.extend(rust_bounds_for_java_type_param(bound, &[support_program]));
                }
            }
            // Java type arguments are heap objects with runtime class identity.
            // Generic source bodies may stringify a nullable T through the
            // erased-object display dispatcher, which needs Rust Any.
            bounds.push("std::any::Any".to_string());
            if field_types.values().any(|field_ty| {
                java_type_requires_hash_capability_for_type_param(
                    field_ty,
                    name,
                    &[support_program],
                    &mut HashSet::new(),
                )
            }) {
                // Java Object supplies equality/hashCode to every reference,
                // but that is frequently identity rather than Rust structural
                // Eq + Hash. The semantic collection boundary carries the
                // Java contract explicitly instead of inventing derives for
                // translated payload structs.
                bounds.push("JavaMapKey".to_string());
            }
            if let Some(bound) = ty
                .type_param_bounds
                .get(index)
                .and_then(Option::as_ref)
                .and_then(primary_named_type_param_bound)
            {
                let into_bounds = source_type_param_into_supertype_bounds(
                    bound,
                    support_program,
                    representations,
                );
                if !into_bounds.is_empty() {
                    bounds.extend(into_bounds);
                    bounds.push("'static".to_string());
                }
            }
            if bounds.is_empty() {
                name.clone()
            } else {
                format!("{name}: {}", bounds.join(" + "))
            }
        })
        .collect()
}

/// Java's `T extends Interface<?>` is an existential capability, not the
/// invariant Rust bound `T: Interface<Box<dyn Any>>`.  When the declaring
/// class only stores, accepts, or returns `T`, no interface operation needs
/// that capability in the inherent/carrier impl and emitting the false bound
/// rejects every legitimate concrete instantiation.  Keep the bound whenever
/// a body uses a `T` value as a method receiver or passes one across another
/// call boundary; those cases need a future erased-capability adapter rather
/// than silent weakening.
fn unexpressible_existential_bound_is_unused(
    ty: &TypeDecl,
    type_param: &str,
    bound: &JavaType,
    support_program: &Program,
) -> bool {
    let JavaType::Named { name, args } = bound else {
        return false;
    };
    if args.is_empty()
        || !args
            .iter()
            .any(|arg| matches!(arg, JavaType::Wildcard(None)))
    {
        return false;
    }

    if crate::inherited_members::owner_wildcard_bound_is_fully_input_reified(
        ty,
        type_param,
        bound,
        &[support_program],
    ) {
        return true;
    }

    if !source_type_bound_is_trait_like(name, &[support_program]) {
        return false;
    }

    !ty.members.iter().any(|member| {
        let (params, body) = match member {
            Member::Method(method) => (&method.params, method.body.as_ref()),
            Member::Constructor(constructor) => (&constructor.params, constructor.body.as_ref()),
            _ => return false,
        };
        let Some(body) = body else {
            return false;
        };
        let mut env = source_callable_type_env(ty, params);
        for stmt in &body.statements {
            env.learn_stmt(stmt);
        }
        let mut used = false;
        crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
            if used {
                return;
            }
            let value_uses_capability = |value: &Expr| {
                java_type_contains_type_param(&infer_expr_type(value, &env), type_param)
            };
            used = match expr {
                Expr::Call { receiver, args, .. } => {
                    receiver.as_deref().is_some_and(&value_uses_capability)
                        || args.iter().any(&value_uses_capability)
                }
                Expr::New { args, .. } => args.iter().any(&value_uses_capability),
                _ => false,
            };
        });
        used
    })
}

fn java_type_uses_type_param_as_hash_key(ty: &JavaType, type_param: &str) -> bool {
    let JavaType::Named { name, args } = ty else {
        return false;
    };
    let simple = name.rsplit('.').next().unwrap_or(name);
    let directly_hashed = match simple {
        "Map" | "HashMap" | "Hashtable" | "ConcurrentHashMap" => args
            .first()
            .is_some_and(|key| java_type_contains_type_param(key, type_param)),
        "Set" | "HashSet" => args
            .first()
            .is_some_and(|element| java_type_contains_type_param(element, type_param)),
        _ => false,
    };
    directly_hashed
        || args
            .iter()
            .any(|arg| java_type_uses_type_param_as_hash_key(arg, type_param))
}

fn java_type_requires_hash_capability_for_type_param(
    ty: &JavaType,
    type_param: &str,
    support_programs: &[&Program],
    visiting: &mut HashSet<(String, String)>,
) -> bool {
    if java_type_uses_type_param_as_hash_key(ty, type_param) {
        return true;
    }
    let JavaType::Named { name, args } = ty else {
        return false;
    };
    let Some(owner) = source_type_bound_decl(name, support_programs) else {
        return false;
    };
    owner
        .type_params
        .iter()
        .enumerate()
        .any(|(index, nested_param)| {
            args.get(index)
                .is_some_and(|arg| java_type_contains_type_param(arg, type_param))
                && visiting.insert((owner.name.clone(), nested_param.clone()))
                && {
                    let fields = source_field_types(owner);
                    let requires = fields.values().any(|field_ty| {
                        java_type_requires_hash_capability_for_type_param(
                            field_ty,
                            nested_param,
                            support_programs,
                            visiting,
                        )
                    });
                    visiting.remove(&(owner.name.clone(), nested_param.clone()));
                    requires
                }
        })
}

fn source_type_param_into_supertype_bounds(
    bound: &str,
    support_program: &Program,
    representations: Option<&RepresentationPlan>,
) -> Vec<String> {
    let mut bounds = Vec::new();
    let mut current =
        source_type_bound_decl(bound, &[support_program]).and_then(|ty| ty.extends.as_deref());
    let mut visited = HashSet::new();
    while let Some(super_name) = current {
        let Some(super_ty) = source_type_bound_decl(super_name, &[support_program]) else {
            break;
        };
        if !visited.insert(super_ty.name.clone()) {
            break;
        }
        if matches!(super_ty.kind, crate::java_ir::TypeKind::Class) {
            bounds.push(format!(
                "Into<{}>",
                source_type_param_into_supertype_bound_target(&super_ty.name, representations)
            ));
        }
        current = super_ty.extends.as_deref();
    }
    bounds
}

fn source_type_param_into_supertype_bound_target(
    super_name: &str,
    representations: Option<&RepresentationPlan>,
) -> String {
    let Some(representations) = representations else {
        return rust_name_for_java_named_type(super_name);
    };
    let Some(representation) = representations.type_representation(super_name) else {
        return rust_name_for_java_named_type(super_name);
    };
    match representation.representation() {
        crate::representation::ObjectRepresentation::ClosedEnum { enum_name, .. }
        | crate::representation::ObjectRepresentation::SourceHierarchyCarrier {
            carrier_name: enum_name,
            ..
        } => rust_name_for_java_named_type(enum_name),
        _ => rust_name_for_java_named_type(super_name),
    }
}

/// Whether a *method or constructor* type parameter needs a `Clone` bound.
///
/// The analogue of the class rule below: there, a parameter stored in a field
/// and flowing through a method may be cloned. Here, a parameter that arrives as
/// an argument and is read more than once in the body is what makes clone
/// recovery insert `.clone()`, which then needs the bound to compile.
pub(crate) fn callable_type_param_needs_clone_bound(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&crate::java_ir::Block>,
) -> bool {
    let Some(body) = body else {
        return false;
    };
    let typed_params = params
        .iter()
        .filter(|param| java_type_contains_type_param(&param.ty, type_param))
        .map(|param| param.name.as_str())
        .collect::<Vec<_>>();
    // Several lowering obligations arise from locals and enhanced-for
    // bindings, not only from formal parameters.  This is especially common
    // for class type parameters: a method can copy `T` values between local
    // collections without accepting a `T`-typed argument.  Evaluate these
    // whole-body capability analyses before the parameter-only fast path.
    if callable_type_param_repeating_region_needs_clone_bound(type_param, params, body)
        || callable_type_param_collection_get_needs_clone_bound(type_param, params, body)
        || callable_type_param_collection_bulk_copy_needs_clone_bound(type_param, params, body)
        || callable_type_param_iterator_method_needs_clone_bound(type_param, params, body)
        || callable_type_param_trait_object_cast_needs_clone_bound(type_param, body)
    {
        return true;
    }
    if typed_params.is_empty() {
        return false;
    }
    let mut counts = std::collections::HashMap::new();
    for stmt in &body.statements {
        crate::lower::collect_stmt_read_counts_for_liveness(stmt, &mut counts);
    }
    typed_params
        .iter()
        .any(|name| counts.get(*name).copied().unwrap_or(0) > 1)
}

fn callable_type_param_trait_object_cast_needs_clone_bound(
    type_param: &str,
    body: &crate::java_ir::Block,
) -> bool {
    let mut needs_clone = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if matches!(expr,
            Expr::Cast {
                ty: JavaType::Named { name, args },
                ..
            } if name == type_param && args.is_empty()
        ) {
            needs_clone = true;
        }
    });
    needs_clone
}

/// The current Java iterator facade owns a snapshot and exposes its methods
/// from an `impl<T: Clone>` block. Propagate that actual facade capability to
/// source callables instead of discovering the missing bound at rustc time.
fn callable_type_param_iterator_method_needs_clone_bound(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: &crate::java_ir::Block,
) -> bool {
    let mut types = TypeEnv::new();
    for param in params {
        types.insert_param(param.source_name.clone(), param.ty.clone());
        if param.name != param.source_name {
            types.insert_param(param.name.clone(), param.ty.clone());
        }
    }
    for stmt in &body.statements {
        types.learn_stmt(stmt);
    }
    let mut needs_clone = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if needs_clone {
            return;
        }
        let Expr::Call {
            receiver: Some(receiver),
            ..
        } = expr
        else {
            return;
        };
        let JavaType::Named { name, args } = infer_expr_type(receiver, &types) else {
            return;
        };
        let simple = name.rsplit('.').next().unwrap_or(&name);
        needs_clone = matches!(simple, "Iterator" | "JavaIterator")
            && args
                .iter()
                .any(|arg| java_type_contains_type_param(arg, type_param));
    });
    needs_clone
}

/// A textual read in a loop is dynamically repeated. If the binding carries a
/// callable type parameter, owned-argument lowering may have to clone it on
/// the back edge even though a source-level read counter sees it only once.
/// This complements the ordinary `count > 1` rule above and is deliberately
/// shared by signature planning and move recovery.
fn callable_type_param_repeating_region_needs_clone_bound(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: &crate::java_ir::Block,
) -> bool {
    let mut typed_names = params
        .iter()
        .filter(|param| java_type_contains_type_param(&param.ty, type_param))
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    collect_type_param_local_names(body, type_param, &mut typed_names);
    block_repeating_region_reads_any(body, &typed_names)
}

fn collect_type_param_local_names(block: &Block, type_param: &str, names: &mut HashSet<String>) {
    for stmt in &block.statements {
        match stmt {
            Stmt::VarDecl { ty, name, .. } => {
                if java_type_contains_type_param(ty, type_param) {
                    names.insert(name.clone());
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_type_param_local_names(then_block, type_param, names);
                if let Some(else_block) = else_block {
                    collect_type_param_local_names(else_block, type_param, names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_type_param_local_names(body, type_param, names);
            }
            Stmt::For { init, body, .. } => {
                collect_type_param_local_names(
                    &Block {
                        statements: init.clone(),
                    },
                    type_param,
                    names,
                );
                collect_type_param_local_names(body, type_param, names);
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                body,
                ..
            } => {
                if java_type_contains_type_param(var_type, type_param) {
                    names.insert(var_name.clone());
                }
                collect_type_param_local_names(body, type_param, names);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_type_param_local_names(
                        &Block {
                            statements: case.statements.clone(),
                        },
                        type_param,
                        names,
                    );
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_type_param_local_names(
                    &Block {
                        statements: resources.clone(),
                    },
                    type_param,
                    names,
                );
                collect_type_param_local_names(body, type_param, names);
                for catch in catches {
                    collect_type_param_local_names(&catch.body, type_param, names);
                }
                if let Some(finally_block) = finally_block {
                    collect_type_param_local_names(finally_block, type_param, names);
                }
            }
            Stmt::Assign { .. }
            | Stmt::Expr(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::LocalType(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn block_repeating_region_reads_any(block: &Block, typed_names: &HashSet<String>) -> bool {
    fn counts_read_any(counts: &HashMap<String, usize>, names: &HashSet<String>) -> bool {
        names
            .iter()
            .any(|name| counts.get(name).copied().unwrap_or(0) > 0)
    }
    fn block_reads_any(block: &Block, names: &HashSet<String>) -> bool {
        let mut counts = HashMap::new();
        for stmt in &block.statements {
            crate::lower::collect_stmt_read_counts_for_liveness(stmt, &mut counts);
        }
        counts_read_any(&counts, names)
    }
    fn expr_reads_any(expr: &Expr, names: &HashSet<String>) -> bool {
        let mut counts = HashMap::new();
        crate::lower::collect_expr_read_counts_for_liveness(expr, &mut counts);
        counts_read_any(&counts, names)
    }

    block.statements.iter().any(|stmt| match stmt {
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_reads_any(condition, typed_names)
                || block_reads_any(body, typed_names)
                || block_repeating_region_reads_any(body, typed_names)
        }
        Stmt::For {
            condition,
            update,
            body,
            ..
        } => {
            condition
                .as_ref()
                .is_some_and(|condition| expr_reads_any(condition, typed_names))
                || update
                    .iter()
                    .any(|update| expr_reads_any(update, typed_names))
                || block_reads_any(body, typed_names)
                || block_repeating_region_reads_any(body, typed_names)
        }
        Stmt::EnhancedFor { body, .. } => {
            block_reads_any(body, typed_names)
                || block_repeating_region_reads_any(body, typed_names)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_repeating_region_reads_any(then_block, typed_names)
                || else_block.as_ref().is_some_and(|else_block| {
                    block_repeating_region_reads_any(else_block, typed_names)
                })
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_repeating_region_reads_any(block, typed_names)
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            block_repeating_region_reads_any(
                &Block {
                    statements: case.statements.clone(),
                },
                typed_names,
            )
        }),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            block_repeating_region_reads_any(
                &Block {
                    statements: resources.clone(),
                },
                typed_names,
            ) || block_repeating_region_reads_any(body, typed_names)
                || catches
                    .iter()
                    .any(|catch| block_repeating_region_reads_any(&catch.body, typed_names))
                || finally_block.as_ref().is_some_and(|finally_block| {
                    block_repeating_region_reads_any(finally_block, typed_names)
                })
        }
        Stmt::VarDecl { .. }
        | Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(_)
        | Stmt::Throw(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::LocalType(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    })
}

pub(crate) fn callable_type_param_needs_clone_bound_with_support_contexts(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&crate::java_ir::Block>,
    support_programs: &[&Program],
) -> bool {
    let mut search = CloneRequirementSearch::default();
    callable_type_param_needs_clone_bound_with_support_contexts_inner(
        type_param,
        params,
        body,
        support_programs,
        &mut search,
    )
}

type CloneRequirementKey = (String, String, String);

#[derive(Debug, Default)]
struct CloneRequirementSearch {
    visiting: HashSet<CloneRequirementKey>,
    method_visiting: Vec<(String, MethodDecl, String)>,
    method_memo: Vec<(String, MethodDecl, String, bool)>,
    inherited_fields: HashMap<String, HashMap<String, JavaType>>,
    inherited_dispatch: HashMap<(String, String, usize), Option<(String, String, usize, usize)>>,
}

/// Translation-scoped memoization for inferred `Clone` capabilities.
///
/// A single source method can be queried while rendering its signature and
/// many more times while lowering moves in its body. In inheritance-heavy
/// generic code those queries recursively inspect the same call graph. Keep
/// the traversal state local to one translation, while sharing completed
/// answers across every emitted method in that translation.
#[derive(Debug, Default)]
pub(crate) struct CloneRequirementCache {
    search: RefCell<CloneRequirementSearch>,
}

impl CloneRequirementSearch {
    fn insert(&mut self, key: CloneRequirementKey) -> bool {
        self.visiting.insert(key)
    }

    fn remove(&mut self, key: &CloneRequirementKey) {
        self.visiting.remove(key);
    }

    fn inherited_fields(
        &mut self,
        ty: &TypeDecl,
        support_programs: &[&Program],
    ) -> HashMap<String, JavaType> {
        if let Some(fields) = self.inherited_fields.get(&ty.name) {
            return fields.clone();
        }
        let fields = source_field_types_with_inherited(ty, support_programs);
        self.inherited_fields
            .insert(ty.name.clone(), fields.clone());
        fields
    }
}

/// Whether the Rust signature emitted for this source method guarantees that
/// `type_param` implements `Clone`.
///
/// Move recovery asks this before cloning an owned generic value.  Keeping the
/// question here makes lowering consume the exact same declared-bound and
/// inferred-bound policy that renders the method signature, rather than
/// maintaining a second approximation of which Java type parameters are safe
/// to clone.
pub(crate) fn source_method_emits_clone_bound_for_type_param_with_cache(
    ty: &TypeDecl,
    method: &MethodDecl,
    type_param: &str,
    representations: Option<&RepresentationPlan>,
    support_programs: &[&Program],
    cache: &CloneRequirementCache,
) -> bool {
    if let Some(index) = method
        .type_params
        .iter()
        .position(|candidate| candidate == type_param)
    {
        let declared = method
            .type_param_bounds
            .get(index)
            .and_then(Option::as_ref)
            .is_some_and(|bound| {
                rust_bounds_for_java_type_param(bound, support_programs)
                    .iter()
                    .any(|bound| bound == "Clone")
            });
        return declared
            || callable_type_param_needs_clone_bound_with_support_contexts_inner(
                type_param,
                &method.params,
                method.body.as_ref(),
                support_programs,
                &mut cache.search.borrow_mut(),
            );
    }

    let Some(index) = ty
        .type_params
        .iter()
        .position(|candidate| candidate == type_param)
    else {
        return false;
    };
    let declared = ty
        .type_param_bounds
        .get(index)
        .and_then(Option::as_ref)
        .is_some_and(|bound| {
            rust_bounds_for_java_type_param(bound, support_programs)
                .iter()
                .any(|bound| bound == "Clone")
        });
    if declared {
        return true;
    }

    let uses_shared_storage = representations.is_some_and(|representations| {
        let placement = representations.storage_placement_for(&ty.name);
        placement.shared() && placement.is_implemented()
    });
    let mut search = cache.search.borrow_mut();
    let fields = search.inherited_fields(ty, support_programs);
    source_method_class_type_param_needs_clone_bound(
        ty,
        method,
        type_param,
        uses_shared_storage,
        &fields,
        support_programs,
        &mut search,
    )
}

/// Constructor counterpart of
/// [`source_method_emits_clone_bound_for_type_param_with_cache`].
pub(crate) fn source_constructor_emits_clone_bound_for_type_param(
    ty: &TypeDecl,
    constructor: &ConstructorDecl,
    type_param: &str,
    support_programs: &[&Program],
) -> bool {
    if let Some(index) = constructor
        .type_params
        .iter()
        .position(|candidate| candidate == type_param)
    {
        let declared = constructor
            .type_param_bounds
            .get(index)
            .and_then(Option::as_ref)
            .is_some_and(|bound| {
                rust_bounds_for_java_type_param(bound, support_programs)
                    .iter()
                    .any(|bound| bound == "Clone")
            });
        return declared
            || callable_type_param_needs_clone_bound_with_support_contexts(
                type_param,
                &constructor.params,
                constructor.body.as_ref(),
                support_programs,
            );
    }

    let Some(index) = ty
        .type_params
        .iter()
        .position(|candidate| candidate == type_param)
    else {
        return false;
    };
    let declared = ty
        .type_param_bounds
        .get(index)
        .and_then(Option::as_ref)
        .is_some_and(|bound| {
            rust_bounds_for_java_type_param(bound, support_programs)
                .iter()
                .any(|bound| bound == "Clone")
        });
    let mut search = CloneRequirementSearch::default();
    declared
        || source_constructor_class_type_param_needs_clone_bound(
            ty,
            constructor,
            type_param,
            support_programs,
            &mut search,
        )
}

fn callable_type_param_needs_clone_bound_with_support_contexts_inner(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&crate::java_ir::Block>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    if callable_type_param_needs_clone_bound(type_param, params, body) {
        return true;
    }
    let Some(body) = body else {
        return false;
    };
    let typed_params = params
        .iter()
        .filter(|param| java_type_contains_type_param(&param.ty, type_param))
        .map(|param| param.name.as_str())
        .collect::<HashSet<_>>();
    let mut callable_types = TypeEnv::new();
    for param in params {
        callable_types.insert_param(param.source_name.clone(), param.ty.clone());
        if param.name != param.source_name {
            callable_types.insert_param(param.name.clone(), param.ty.clone());
        }
    }
    for stmt in &body.statements {
        callable_types.learn_stmt(stmt);
    }
    let mut constructed_owner_requires_clone = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if !constructed_owner_requires_clone {
            constructed_owner_requires_clone = expr_constructs_clone_requiring_generic_owner(
                expr,
                type_param,
                &callable_types,
                support_programs,
                visiting,
            );
        }
    });
    constructed_owner_requires_clone
        || (!typed_params.is_empty()
            && block_delegates_type_param_to_clone_requiring_constructor(
                body,
                &typed_params,
                support_programs,
                visiting,
            ))
}

fn block_delegates_type_param_to_clone_requiring_constructor(
    block: &Block,
    typed_params: &HashSet<&str>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    block.statements.iter().any(|stmt| {
        stmt_delegates_type_param_to_clone_requiring_constructor(
            stmt,
            typed_params,
            support_programs,
            visiting,
        )
    })
}

fn stmt_delegates_type_param_to_clone_requiring_constructor(
    stmt: &Stmt,
    typed_params: &HashSet<&str>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer.as_ref().is_some_and(|expr| {
            expr_delegates_type_param_to_clone_requiring_method(
                expr,
                typed_params,
                support_programs,
                visiting,
            )
        }),
        Stmt::Assign { target, value } => {
            expr_delegates_type_param_to_clone_requiring_method(
                target,
                typed_params,
                support_programs,
                visiting,
            ) || expr_delegates_type_param_to_clone_requiring_method(
                value,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_delegates_type_param_to_clone_requiring_method(
                expr,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Stmt::ConstructorInvocation {
            args,
            resolved: Some(resolved),
            ..
        } => resolved_constructor_forwards_clone_requirement(
            resolved,
            args,
            typed_params,
            support_programs,
            visiting,
        ),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_delegates_type_param_to_clone_requiring_method(
                condition,
                typed_params,
                support_programs,
                visiting,
            ) || block_delegates_type_param_to_clone_requiring_constructor(
                then_block,
                typed_params,
                support_programs,
                visiting,
            ) || else_block.as_ref().is_some_and(|else_block| {
                block_delegates_type_param_to_clone_requiring_constructor(
                    else_block,
                    typed_params,
                    support_programs,
                    visiting,
                )
            })
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_delegates_type_param_to_clone_requiring_method(
                condition,
                typed_params,
                support_programs,
                visiting,
            ) || block_delegates_type_param_to_clone_requiring_constructor(
                body,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_delegates_type_param_to_clone_requiring_method(
                iterable,
                typed_params,
                support_programs,
                visiting,
            ) || block_delegates_type_param_to_clone_requiring_constructor(
                body,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_delegates_type_param_to_clone_requiring_constructor(
                body,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter().any(|stmt| {
                stmt_delegates_type_param_to_clone_requiring_constructor(
                    stmt,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || condition.as_ref().is_some_and(|expr| {
                expr_delegates_type_param_to_clone_requiring_method(
                    expr,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || update.iter().any(|expr| {
                expr_delegates_type_param_to_clone_requiring_method(
                    expr,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || block_delegates_type_param_to_clone_requiring_constructor(
                body,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Stmt::Switch { selector, cases } => {
            expr_delegates_type_param_to_clone_requiring_method(
                selector,
                typed_params,
                support_programs,
                visiting,
            ) || cases.iter().any(|case| {
                case.statements.iter().any(|stmt| {
                    stmt_delegates_type_param_to_clone_requiring_constructor(
                        stmt,
                        typed_params,
                        support_programs,
                        visiting,
                    )
                })
            })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(|stmt| {
                stmt_delegates_type_param_to_clone_requiring_constructor(
                    stmt,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || block_delegates_type_param_to_clone_requiring_constructor(
                body,
                typed_params,
                support_programs,
                visiting,
            ) || catches.iter().any(|catch| {
                block_delegates_type_param_to_clone_requiring_constructor(
                    &catch.body,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || finally_block.as_ref().is_some_and(|finally_block| {
                block_delegates_type_param_to_clone_requiring_constructor(
                    finally_block,
                    typed_params,
                    support_programs,
                    visiting,
                )
            })
        }
        _ => false,
    }
}

fn expr_delegates_type_param_to_clone_requiring_method(
    expr: &Expr,
    typed_params: &HashSet<&str>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    match expr {
        Expr::Call {
            receiver,
            args,
            resolved,
            ..
        } => {
            resolved.as_ref().is_some_and(|resolved| {
                resolved_method_forwards_clone_requirement(
                    resolved,
                    receiver.as_deref(),
                    args,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || receiver.as_ref().is_some_and(|receiver| {
                expr_delegates_type_param_to_clone_requiring_method(
                    receiver,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || args.iter().any(|arg| {
                expr_delegates_type_param_to_clone_requiring_method(
                    arg,
                    typed_params,
                    support_programs,
                    visiting,
                )
            })
        }
        Expr::New {
            qualifier,
            args,
            resolved,
            ty,
            ..
        } => {
            source_constructor_for_capability_requirement(
                resolved.as_ref(),
                ty,
                args.len(),
                support_programs,
            )
            .is_some_and(|(owner, constructor, rust_name)| {
                source_constructor_forwards_clone_requirement(
                    owner,
                    constructor,
                    &rust_name,
                    args,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || qualifier.as_ref().is_some_and(|qualifier| {
                expr_delegates_type_param_to_clone_requiring_method(
                    qualifier,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || args.iter().any(|arg| {
                expr_delegates_type_param_to_clone_requiring_method(
                    arg,
                    typed_params,
                    support_programs,
                    visiting,
                )
            })
        }
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            expr_delegates_type_param_to_clone_requiring_method(
                receiver,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            expr_delegates_type_param_to_clone_requiring_method(
                array,
                typed_params,
                support_programs,
                visiting,
            ) || expr_delegates_type_param_to_clone_requiring_method(
                index,
                typed_params,
                support_programs,
                visiting,
            )
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(|dimension| {
                expr_delegates_type_param_to_clone_requiring_method(
                    dimension,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }) || initializer.as_ref().is_some_and(|items| {
                items.iter().any(|item| {
                    expr_delegates_type_param_to_clone_requiring_method(
                        item,
                        typed_params,
                        support_programs,
                        visiting,
                    )
                })
            })
        }
        Expr::ArrayInitializer(items) => items.iter().any(|item| {
            expr_delegates_type_param_to_clone_requiring_method(
                item,
                typed_params,
                support_programs,
                visiting,
            )
        }),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => [condition.as_ref(), then_expr.as_ref(), else_expr.as_ref()]
            .into_iter()
            .any(|item| {
                expr_delegates_type_param_to_clone_requiring_method(
                    item,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }),
        Expr::Switch { selector, cases } => {
            expr_delegates_type_param_to_clone_requiring_method(
                selector,
                typed_params,
                support_programs,
                visiting,
            ) || cases.iter().any(|case| {
                expr_delegates_type_param_to_clone_requiring_method(
                    &case.value,
                    typed_params,
                    support_programs,
                    visiting,
                )
            })
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                expr_delegates_type_param_to_clone_requiring_method(
                    expr,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }
            crate::java_ir::LambdaBody::Block(block) => {
                block_delegates_type_param_to_clone_requiring_constructor(
                    block,
                    typed_params,
                    support_programs,
                    visiting,
                )
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => false,
    }
}

fn resolved_method_forwards_clone_requirement(
    resolved: &crate::java_ir::ResolvedCall,
    receiver: Option<&Expr>,
    args: &[Expr],
    typed_params: &HashSet<&str>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    let Some(owner) = source_type_bound_decl(&resolved.declaring_type, support_programs) else {
        return false;
    };
    let Some(method) = owner.members.iter().find_map(|member| match member {
        Member::Method(method)
            if crate::java_signature::source_method_rust_name(method) == resolved.rust_name
                && method.params.len() == args.len() =>
        {
            Some(method)
        }
        _ => None,
    }) else {
        return false;
    };
    let argument_requires_clone = args.iter().enumerate().any(|(arg_index, arg)| {
        if !expr_reads_named_type_param_param(arg, typed_params) {
            return false;
        }
        let Some(target_param) = method.params.get(arg_index) else {
            return false;
        };
        let method_type_param_needs_clone = method
            .type_params
            .iter()
            .filter(|callee_type_param| {
                java_type_contains_type_param(&target_param.ty, callee_type_param)
            })
            .any(|callee_type_param| {
                let key = (
                    owner.name.clone(),
                    resolved.rust_name.clone(),
                    callee_type_param.clone(),
                );
                if !visiting.insert(key.clone()) {
                    return false;
                }
                let needs_clone = callable_type_param_needs_clone_bound_with_support_contexts_inner(
                    callee_type_param,
                    &method.params,
                    method.body.as_ref(),
                    support_programs,
                    visiting,
                );
                visiting.remove(&key);
                needs_clone
            });
        if method_type_param_needs_clone {
            return true;
        }
        let fields = source_field_types(owner);
        owner
            .type_params
            .iter()
            .filter(|callee_type_param| {
                java_type_contains_type_param(&target_param.ty, callee_type_param)
            })
            .any(|callee_type_param| {
                let key = (
                    owner.name.clone(),
                    resolved.rust_name.clone(),
                    callee_type_param.clone(),
                );
                if !visiting.insert(key.clone()) {
                    return false;
                }
                let needs_clone = source_method_class_type_param_needs_clone_bound(
                    owner,
                    method,
                    callee_type_param,
                    false,
                    &fields,
                    support_programs,
                    visiting,
                );
                visiting.remove(&key);
                needs_clone
            })
    });
    if argument_requires_clone {
        return true;
    }
    if !receiver.is_some_and(|receiver| expr_reads_named_type_param_param(receiver, typed_params)) {
        return false;
    }
    // The resolved call identifies the generic receiver declaration but not
    // its instantiated type arguments. Reaching the call through a parameter
    // already proven to contain the caller's type parameter is sufficient to
    // propagate any receiver-class capability required by the selected
    // method. This remains selective at the caller: unrelated receiver
    // parameters do not participate.
    let fields = source_field_types(owner);
    owner.type_params.iter().any(|callee_type_param| {
        let key = (
            owner.name.clone(),
            resolved.rust_name.clone(),
            callee_type_param.clone(),
        );
        if !visiting.insert(key.clone()) {
            return false;
        }
        let needs_clone = source_method_class_type_param_needs_clone_bound(
            owner,
            method,
            callee_type_param,
            false,
            &fields,
            support_programs,
            visiting,
        );
        visiting.remove(&key);
        needs_clone
    })
}

fn resolved_constructor_forwards_clone_requirement(
    resolved: &crate::java_ir::ResolvedConstructor,
    args: &[Expr],
    typed_params: &HashSet<&str>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    let Some(owner) = source_type_bound_decl(&resolved.declaring_type, support_programs) else {
        return false;
    };
    let Some(constructor) = owner.members.iter().find_map(|member| match member {
        Member::Constructor(constructor)
            if constructor.rust_name.as_deref().unwrap_or("new") == resolved.rust_name
                && constructor.params.len() == args.len() =>
        {
            Some(constructor)
        }
        _ => None,
    }) else {
        return false;
    };
    source_constructor_forwards_clone_requirement(
        owner,
        constructor,
        &resolved.rust_name,
        args,
        typed_params,
        support_programs,
        visiting,
    )
}

fn source_constructor_forwards_clone_requirement(
    owner: &TypeDecl,
    constructor: &ConstructorDecl,
    rust_name: &str,
    args: &[Expr],
    typed_params: &HashSet<&str>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    args.iter().enumerate().any(|(arg_index, arg)| {
        if !expr_reads_named_type_param_param(arg, typed_params) {
            return false;
        }
        let Some(target_param) = constructor.params.get(arg_index) else {
            return false;
        };
        let constructor_type_param_needs_clone = constructor
            .type_params
            .iter()
            .filter(|callee_type_param| {
                java_type_contains_type_param(&target_param.ty, callee_type_param)
            })
            .any(|callee_type_param| {
                let key = (
                    owner.name.clone(),
                    rust_name.to_string(),
                    callee_type_param.clone(),
                );
                if !visiting.insert(key.clone()) {
                    return false;
                }
                let needs_clone = callable_type_param_needs_clone_bound_with_support_contexts_inner(
                    callee_type_param,
                    &constructor.params,
                    constructor.body.as_ref(),
                    support_programs,
                    visiting,
                );
                visiting.remove(&key);
                needs_clone
            });
        if constructor_type_param_needs_clone {
            return true;
        }
        owner
            .type_params
            .iter()
            .filter(|callee_type_param| {
                java_type_contains_type_param(&target_param.ty, callee_type_param)
            })
            .any(|callee_type_param| {
                let key = (
                    owner.name.clone(),
                    rust_name.to_string(),
                    callee_type_param.clone(),
                );
                if !visiting.insert(key.clone()) {
                    return false;
                }
                let needs_clone = source_constructor_class_type_param_needs_clone_bound(
                    owner,
                    constructor,
                    callee_type_param,
                    support_programs,
                    visiting,
                );
                visiting.remove(&key);
                needs_clone
            })
    })
}

fn callable_type_param_collection_get_needs_clone_bound(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: &crate::java_ir::Block,
) -> bool {
    let mut locals = params
        .iter()
        .map(|param| (param.name.clone(), param.ty.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    block_type_param_collection_get_needs_clone_bound(type_param, body, &mut locals)
}

/// Java `Collection.addAll` copies object references from its source. Native
/// Rust collections represent those references as cloneable carriers, so a
/// generic element copied from a borrowed collection needs the same `Clone`
/// capability as a generic `List.get` result recovered into owned storage.
fn callable_type_param_collection_bulk_copy_needs_clone_bound(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: &crate::java_ir::Block,
) -> bool {
    let mut locals = params
        .iter()
        .map(|param| (param.name.clone(), param.ty.clone()))
        .collect::<std::collections::HashMap<_, _>>();
    block_type_param_collection_bulk_copy_needs_clone_bound(type_param, body, &mut locals)
}

fn block_type_param_collection_bulk_copy_needs_clone_bound(
    type_param: &str,
    block: &crate::java_ir::Block,
    locals: &mut std::collections::HashMap<String, JavaType>,
) -> bool {
    block.statements.iter().any(|stmt| {
        stmt_type_param_collection_bulk_copy_needs_clone_bound(type_param, stmt, locals)
    })
}

fn stmt_type_param_collection_bulk_copy_needs_clone_bound(
    type_param: &str,
    stmt: &Stmt,
    locals: &mut std::collections::HashMap<String, JavaType>,
) -> bool {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
            ..
        } => {
            let needs_bound = initializer.as_ref().is_some_and(|expr| {
                expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, locals)
            });
            locals.insert(name.clone(), ty.clone());
            needs_bound
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, locals)
        }
        Stmt::Assign { target, value } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, target, locals)
                || expr_type_param_collection_bulk_copy_needs_clone(type_param, value, locals)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, condition, locals)
                || block_type_param_collection_bulk_copy_needs_clone_bound(
                    type_param,
                    then_block,
                    &mut locals.clone(),
                )
                || else_block.as_ref().is_some_and(|block| {
                    block_type_param_collection_bulk_copy_needs_clone_bound(
                        type_param,
                        block,
                        &mut locals.clone(),
                    )
                })
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, condition, locals)
                || block_type_param_collection_bulk_copy_needs_clone_bound(
                    type_param,
                    body,
                    &mut locals.clone(),
                )
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => {
            if expr_type_param_collection_bulk_copy_needs_clone(type_param, iterable, locals) {
                return true;
            }
            let mut scoped = locals.clone();
            scoped.insert(var_name.clone(), var_type.clone());
            block_type_param_collection_bulk_copy_needs_clone_bound(type_param, body, &mut scoped)
        }
        Stmt::Block(body) | Stmt::LabeledBlock { body, .. } => {
            block_type_param_collection_bulk_copy_needs_clone_bound(
                type_param,
                body,
                &mut locals.clone(),
            )
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut scoped = locals.clone();
            init.iter().any(|stmt| {
                stmt_type_param_collection_bulk_copy_needs_clone_bound(
                    type_param,
                    stmt,
                    &mut scoped,
                )
            }) || condition.as_ref().is_some_and(|expr| {
                expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, &scoped)
            }) || update.iter().any(|expr| {
                expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, &scoped)
            }) || block_type_param_collection_bulk_copy_needs_clone_bound(
                type_param,
                body,
                &mut scoped,
            )
        }
        Stmt::Switch { selector, cases } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, selector, locals)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_type_param_collection_bulk_copy_needs_clone(
                                type_param, expr, locals,
                            )
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || {
                        let mut scoped = locals.clone();
                        case.statements.iter().any(|stmt| {
                            stmt_type_param_collection_bulk_copy_needs_clone_bound(
                                type_param,
                                stmt,
                                &mut scoped,
                            )
                        })
                    }
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut scoped = locals.clone();
            resources.iter().any(|stmt| {
                stmt_type_param_collection_bulk_copy_needs_clone_bound(
                    type_param,
                    stmt,
                    &mut scoped,
                )
            }) || block_type_param_collection_bulk_copy_needs_clone_bound(
                type_param,
                body,
                &mut scoped,
            ) || catches.iter().any(|catch| {
                block_type_param_collection_bulk_copy_needs_clone_bound(
                    type_param,
                    &catch.body,
                    &mut locals.clone(),
                )
            }) || finally_block.as_ref().is_some_and(|block| {
                block_type_param_collection_bulk_copy_needs_clone_bound(
                    type_param,
                    block,
                    &mut locals.clone(),
                )
            })
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, locals)),
        Stmt::Return(None)
        | Stmt::LocalType(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
}

fn expr_type_param_collection_bulk_copy_needs_clone(
    type_param: &str,
    expr: &Expr,
    locals: &std::collections::HashMap<String, JavaType>,
) -> bool {
    match expr {
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } => {
            let this_call_needs_clone = matches!(name.as_str(), "addAll" | "add_all")
                && args.len() == 1
                && match &args[0] {
                    Expr::Var(name) => locals.get(name),
                    Expr::Cast { ty, .. } => Some(ty),
                    _ => None,
                }
                .and_then(crate::types::sequence_element_type)
                .is_some_and(|element| java_type_contains_type_param(&element, type_param));
            this_call_needs_clone
                || receiver.as_ref().is_some_and(|receiver| {
                    expr_type_param_collection_bulk_copy_needs_clone(type_param, receiver, locals)
                })
                || args.iter().any(|arg| {
                    expr_type_param_collection_bulk_copy_needs_clone(type_param, arg, locals)
                })
        }
        Expr::FieldAccess(receiver, _) | Expr::Unary { expr: receiver, .. } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, receiver, locals)
        }
        Expr::ArrayAccess(array, index)
        | Expr::Binary {
            left: array,
            right: index,
            ..
        } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, array, locals)
                || expr_type_param_collection_bulk_copy_needs_clone(type_param, index, locals)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(|expr| {
                expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, locals)
            }) || initializer.as_ref().is_some_and(|items| {
                items.iter().any(|expr| {
                    expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, locals)
                })
            })
        }
        Expr::ArrayInitializer(items) => items
            .iter()
            .any(|expr| expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, locals)),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, condition, locals)
                || expr_type_param_collection_bulk_copy_needs_clone(type_param, then_expr, locals)
                || expr_type_param_collection_bulk_copy_needs_clone(type_param, else_expr, locals)
        }
        Expr::Switch { selector, cases } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, selector, locals)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_type_param_collection_bulk_copy_needs_clone(
                                type_param, expr, locals,
                            )
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || expr_type_param_collection_bulk_copy_needs_clone(
                        type_param,
                        &case.value,
                        locals,
                    )
                })
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier.as_ref().is_some_and(|qualifier| {
                expr_type_param_collection_bulk_copy_needs_clone(type_param, qualifier, locals)
            }) || args.iter().any(|arg| {
                expr_type_param_collection_bulk_copy_needs_clone(type_param, arg, locals)
            }) || body.as_ref().is_some_and(|members| {
                members.iter().any(|member| match member {
                    Member::Method(method) => method.body.as_ref().is_some_and(|body| {
                        block_type_param_collection_bulk_copy_needs_clone_bound(
                            type_param,
                            body,
                            &mut locals.clone(),
                        )
                    }),
                    Member::Constructor(constructor) => {
                        constructor.body.as_ref().is_some_and(|body| {
                            block_type_param_collection_bulk_copy_needs_clone_bound(
                                type_param,
                                body,
                                &mut locals.clone(),
                            )
                        })
                    }
                    _ => false,
                })
            })
        }
        Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, locals)
        }
        Expr::Lambda { params, body } => {
            let mut scoped = locals.clone();
            for param in params {
                if let Some(ty) = &param.ty {
                    scoped.insert(param.name.clone(), ty.clone());
                } else {
                    scoped.remove(&param.name);
                }
            }
            match body {
                crate::java_ir::LambdaBody::Expr(expr) => {
                    expr_type_param_collection_bulk_copy_needs_clone(type_param, expr, &scoped)
                }
                crate::java_ir::LambdaBody::Block(block) => {
                    block_type_param_collection_bulk_copy_needs_clone_bound(
                        type_param,
                        block,
                        &mut scoped,
                    )
                }
            }
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => false,
    }
}

fn block_type_param_collection_get_needs_clone_bound(
    type_param: &str,
    block: &crate::java_ir::Block,
    locals: &mut std::collections::HashMap<String, JavaType>,
) -> bool {
    for stmt in &block.statements {
        if stmt_type_param_collection_get_needs_clone_bound(type_param, stmt, locals) {
            return true;
        }
    }
    false
}

fn stmt_type_param_collection_get_needs_clone_bound(
    type_param: &str,
    stmt: &Stmt,
    locals: &mut std::collections::HashMap<String, JavaType>,
) -> bool {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
            ..
        } => {
            let needs_bound = initializer.as_ref().is_some_and(|initializer| {
                java_type_contains_type_param(ty, type_param)
                    && expr_is_type_param_sequence_get(type_param, initializer, locals)
            });
            locals.insert(name.clone(), ty.clone());
            needs_bound
        }
        Stmt::Return(Some(expr)) => expr_is_type_param_sequence_get(type_param, expr, locals),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            let mut then_locals = locals.clone();
            let mut else_locals = locals.clone();
            block_type_param_collection_get_needs_clone_bound(
                type_param,
                then_block,
                &mut then_locals,
            ) || else_block.as_ref().is_some_and(|else_block| {
                block_type_param_collection_get_needs_clone_bound(
                    type_param,
                    else_block,
                    &mut else_locals,
                )
            })
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Block(body) => {
            let mut scoped = locals.clone();
            block_type_param_collection_get_needs_clone_bound(type_param, body, &mut scoped)
        }
        Stmt::For { init, body, .. } => {
            let mut scoped = locals.clone();
            for init_stmt in init {
                if stmt_type_param_collection_get_needs_clone_bound(
                    type_param,
                    init_stmt,
                    &mut scoped,
                ) {
                    return true;
                }
            }
            block_type_param_collection_get_needs_clone_bound(type_param, body, &mut scoped)
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            body,
            ..
        } => {
            let mut scoped = locals.clone();
            scoped.insert(var_name.clone(), var_type.clone());
            block_type_param_collection_get_needs_clone_bound(type_param, body, &mut scoped)
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            let mut scoped = locals.clone();
            case.statements.iter().any(|stmt| {
                stmt_type_param_collection_get_needs_clone_bound(type_param, stmt, &mut scoped)
            })
        }),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut scoped = locals.clone();
            for resource in resources {
                if stmt_type_param_collection_get_needs_clone_bound(
                    type_param,
                    resource,
                    &mut scoped,
                ) {
                    return true;
                }
            }
            block_type_param_collection_get_needs_clone_bound(type_param, body, &mut scoped)
                || catches.iter().any(|catch| {
                    let mut catch_locals = locals.clone();
                    block_type_param_collection_get_needs_clone_bound(
                        type_param,
                        &catch.body,
                        &mut catch_locals,
                    )
                })
                || finally_block.as_ref().is_some_and(|finally_block| {
                    let mut finally_locals = locals.clone();
                    block_type_param_collection_get_needs_clone_bound(
                        type_param,
                        finally_block,
                        &mut finally_locals,
                    )
                })
        }
        Stmt::LabeledBlock { body, .. } => {
            let mut scoped = locals.clone();
            block_type_param_collection_get_needs_clone_bound(type_param, body, &mut scoped)
        }
        Stmt::Assign { .. }
        | Stmt::Expr(_)
        | Stmt::Return(None)
        | Stmt::Throw(_)
        | Stmt::LocalType(_)
        | Stmt::ConstructorInvocation { .. }
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
}

fn expr_is_type_param_sequence_get(
    type_param: &str,
    expr: &Expr,
    locals: &std::collections::HashMap<String, JavaType>,
) -> bool {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return false;
    };
    if name != "get" || args.len() != 1 {
        return false;
    }
    let Expr::Var(receiver_name) = receiver.as_ref() else {
        return false;
    };
    let Some(receiver_ty) = locals.get(receiver_name) else {
        return false;
    };
    sequence_element_type(receiver_ty)
        .is_some_and(|element_ty| java_type_contains_type_param(&element_ty, type_param))
}

fn block_returns_this(block: &Block) -> bool {
    block.statements.iter().any(stmt_returns_this)
}

fn stmt_returns_this(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(Some(Expr::This)) => true,
        Stmt::If {
            then_block,
            else_block,
            ..
        } => block_returns_this(then_block) || else_block.as_ref().is_some_and(block_returns_this),
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::EnhancedFor { body, .. }
        | Stmt::Block(body)
        | Stmt::LabeledBlock { body, .. } => block_returns_this(body),
        Stmt::For { init, body, .. } => {
            init.iter().any(stmt_returns_this) || block_returns_this(body)
        }
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_returns_this)),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_returns_this)
                || block_returns_this(body)
                || catches.iter().any(|catch| block_returns_this(&catch.body))
                || finally_block.as_ref().is_some_and(block_returns_this)
        }
        _ => false,
    }
}

fn source_method_clones_class_type_param_param_to_owned_local(
    method: &MethodDecl,
    type_param: &str,
) -> bool {
    let param_names = method
        .params
        .iter()
        .filter(|param| java_type_contains_type_param(&param.ty, type_param))
        .map(|param| param.name.as_str())
        .collect::<HashSet<_>>();
    if param_names.is_empty() {
        return false;
    }
    method.body.as_ref().is_some_and(|body| {
        block_clones_class_type_param_param_to_owned_local(body, type_param, &param_names)
    })
}

fn block_clones_class_type_param_param_to_owned_local(
    block: &Block,
    type_param: &str,
    param_names: &HashSet<&str>,
) -> bool {
    block_clones_class_type_param_param_to_owned_local_before_later_read(
        block,
        type_param,
        param_names,
        false,
    )
}

fn block_clones_class_type_param_param_to_owned_local_before_later_read(
    block: &Block,
    type_param: &str,
    param_names: &HashSet<&str>,
    mut later_param_read: bool,
) -> bool {
    for stmt in block.statements.iter().rev() {
        if stmt_clones_class_type_param_param_to_owned_local_before_later_read(
            stmt,
            type_param,
            param_names,
            later_param_read,
        ) {
            return true;
        }
        later_param_read |= stmt_reads_named_type_param_param(stmt, param_names);
    }
    false
}

fn stmt_clones_class_type_param_param_to_owned_local_before_later_read(
    stmt: &Stmt,
    type_param: &str,
    param_names: &HashSet<&str>,
    later_param_read: bool,
) -> bool {
    match stmt {
        Stmt::VarDecl {
            ty,
            initializer: Some(initializer),
            ..
        } => {
            // Class type parameters are passed by shared reference.  Producing
            // an owned local of the same type therefore clones the parameter
            // even when this is its final source-level read (for example,
            // `T value = present ? line : null`).  Liveness only decides
            // whether an owned parameter may be moved; it cannot turn `&T`
            // into `T`.
            java_type_contains_type_param(ty, type_param)
                && expr_reads_named_type_param_param(initializer, param_names)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_clones_class_type_param_param_to_owned_local_before_later_read(
                then_block,
                type_param,
                param_names,
                later_param_read,
            ) || else_block.as_ref().is_some_and(|else_block| {
                block_clones_class_type_param_param_to_owned_local_before_later_read(
                    else_block,
                    type_param,
                    param_names,
                    later_param_read,
                )
            })
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } | Stmt::Block(body) => {
            block_clones_class_type_param_param_to_owned_local_before_later_read(
                body,
                type_param,
                param_names,
                true,
            )
        }
        Stmt::For { init, body, .. } => {
            init.iter().any(|stmt| {
                stmt_clones_class_type_param_param_to_owned_local_before_later_read(
                    stmt,
                    type_param,
                    param_names,
                    true,
                )
            }) || block_clones_class_type_param_param_to_owned_local_before_later_read(
                body,
                type_param,
                param_names,
                true,
            )
        }
        Stmt::EnhancedFor { body, .. } => {
            block_clones_class_type_param_param_to_owned_local_before_later_read(
                body,
                type_param,
                param_names,
                true,
            )
        }
        Stmt::Switch { cases, .. } => cases.iter().any(|case| {
            case.statements.iter().any(|stmt| {
                stmt_clones_class_type_param_param_to_owned_local_before_later_read(
                    stmt,
                    type_param,
                    param_names,
                    later_param_read,
                )
            })
        }),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(|stmt| {
                stmt_clones_class_type_param_param_to_owned_local_before_later_read(
                    stmt,
                    type_param,
                    param_names,
                    true,
                )
            }) || block_clones_class_type_param_param_to_owned_local_before_later_read(
                body,
                type_param,
                param_names,
                later_param_read,
            ) || catches.iter().any(|catch| {
                block_clones_class_type_param_param_to_owned_local_before_later_read(
                    &catch.body,
                    type_param,
                    param_names,
                    later_param_read,
                )
            }) || finally_block.as_ref().is_some_and(|finally_block| {
                block_clones_class_type_param_param_to_owned_local_before_later_read(
                    finally_block,
                    type_param,
                    param_names,
                    later_param_read,
                )
            })
        }
        _ => false,
    }
}

fn stmt_reads_named_type_param_param(stmt: &Stmt, param_names: &HashSet<&str>) -> bool {
    match stmt {
        Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_reads_named_type_param_param(expr, param_names)),
        Stmt::Assign { target, value } => {
            expr_reads_named_type_param_param(target, param_names)
                || expr_reads_named_type_param_param(value, param_names)
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            expr_reads_named_type_param_param(expr, param_names)
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_reads_named_type_param_param(condition, param_names)
                || block_reads_named_type_param_param(then_block, param_names)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_reads_named_type_param_param(block, param_names))
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_reads_named_type_param_param(condition, param_names)
                || block_reads_named_type_param_param(body, param_names)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_reads_named_type_param_param(stmt, param_names))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_reads_named_type_param_param(expr, param_names))
                || update
                    .iter()
                    .any(|expr| expr_reads_named_type_param_param(expr, param_names))
                || block_reads_named_type_param_param(body, param_names)
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            expr_reads_named_type_param_param(iterable, param_names)
                || block_reads_named_type_param_param(body, param_names)
        }
        Stmt::Switch { selector, cases } => {
            expr_reads_named_type_param_param(selector, param_names)
                || cases.iter().any(|case| {
                    case.statements
                        .iter()
                        .any(|stmt| stmt_reads_named_type_param_param(stmt, param_names))
                })
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|stmt| stmt_reads_named_type_param_param(stmt, param_names))
                || block_reads_named_type_param_param(body, param_names)
                || catches
                    .iter()
                    .any(|catch| block_reads_named_type_param_param(&catch.body, param_names))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_reads_named_type_param_param(block, param_names))
        }
        Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_reads_named_type_param_param(expr, param_names)),
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_reads_named_type_param_param(block, param_names)
        }
        Stmt::Return(None)
        | Stmt::LocalType(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Unsupported(_) => false,
    }
}

fn block_reads_named_type_param_param(block: &Block, param_names: &HashSet<&str>) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_reads_named_type_param_param(stmt, param_names))
}

fn expr_reads_named_type_param_param(expr: &Expr, param_names: &HashSet<&str>) -> bool {
    match expr {
        Expr::Var(name) => param_names.contains(name.as_str()),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            expr_reads_named_type_param_param(then_expr, param_names)
                || expr_reads_named_type_param_param(else_expr, param_names)
        }
        Expr::Cast { expr, .. } | Expr::Unary { expr, .. } => {
            expr_reads_named_type_param_param(expr, param_names)
        }
        _ => false,
    }
}

fn source_field_types(ty: &TypeDecl) -> HashMap<String, JavaType> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some((field.name.clone(), field.ty.clone())),
            _ => None,
        })
        .collect()
}

fn source_field_types_with_inherited(
    ty: &TypeDecl,
    support_programs: &[&Program],
) -> HashMap<String, JavaType> {
    let mut fields = source_field_types(ty);
    for program in support_programs {
        for inherited in
            crate::inherited_members::inherited_instance_field_refs_for_type(ty, program)
        {
            let Some(declaring_ty) =
                source_type_bound_decl(inherited.declaring_type(), support_programs)
            else {
                continue;
            };
            let substitutions = crate::inherited_members::source_class_ancestor_type_arguments(
                program,
                ty,
                declaring_ty,
            )
            .unwrap_or_default();
            let field_ty = crate::resolve::substitute_method_type_variables(
                &inherited.field().ty,
                &substitutions,
            );
            fields
                .entry(inherited.field().name.clone())
                .or_insert(field_ty);
        }
    }
    fields
}

fn source_method_needs_class_type_param_clone_bound(
    method: &MethodDecl,
    type_param: &str,
    fields: &HashMap<String, JavaType>,
) -> bool {
    let Some(body) = &method.body else {
        return false;
    };
    let generic_owner_locals = body
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::VarDecl { ty, name, .. } if java_type_contains_type_param(ty, type_param) => {
                Some(name.as_str())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();
    let mut reads_generic_field_through_local = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if let Expr::FieldAccess(receiver, field) = expr
            && matches!(receiver.as_ref(), Expr::Var(name) if generic_owner_locals.contains(name.as_str()))
            && fields
                .get(field)
                .is_some_and(|field_ty| java_type_contains_type_param(field_ty, type_param))
        {
            reads_generic_field_through_local = true;
        }
    });
    if reads_generic_field_through_local {
        // Representation lowering accesses a generic field through the local
        // hierarchy/reference carrier's generated getter. That getter returns
        // an owned Java value and therefore clones generic contents.
        return true;
    }
    if body.statements.iter().any(|stmt| {
        matches!(stmt,
            Stmt::VarDecl { ty, initializer: Some(initializer), .. }
                if java_type_contains_type_param(ty, type_param)
                    && (expr_java_type(initializer, fields).is_some_and(|field_ty| {
                            java_type_contains_type_param(&field_ty, type_param)
                        })
                        || expr_recovers_owned_class_type_param_from_field(
                            initializer,
                            type_param,
                            fields,
                        ))
        )
    }) {
        // Reading a generic field into an owned local cannot move the field;
        // the emitted `.cloned()` requires this capability on the class impl.
        return true;
    }
    if !java_type_contains_type_param(&method.return_type, type_param) {
        return false;
    }
    body.statements.iter().any(|stmt| {
        method_return_expr(stmt).is_some_and(|expr| {
            expr_recovers_owned_class_type_param_from_field(expr, type_param, fields)
        })
    })
}

fn method_return_expr(stmt: &Stmt) -> Option<&Expr> {
    match stmt {
        Stmt::Return(Some(expr)) => Some(expr),
        _ => None,
    }
}

fn expr_recovers_owned_class_type_param_from_field(
    expr: &Expr,
    type_param: &str,
    fields: &HashMap<String, JavaType>,
) -> bool {
    match expr {
        Expr::Var(name) => fields
            .get(name)
            .is_some_and(|field_ty| java_type_contains_type_param(field_ty, type_param)),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => fields
            .get(field)
            .is_some_and(|field_ty| java_type_contains_type_param(field_ty, type_param)),
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == "get" && args.len() == 1 => {
            expr_java_type(receiver, fields).is_some_and(|receiver_ty| {
                collection_lookup_value_type(&receiver_ty)
                    .is_some_and(|value_ty| java_type_contains_type_param(&value_ty, type_param))
            })
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == "iterator" && args.is_empty() => {
            expr_java_type(receiver, fields).is_some_and(|receiver_ty| {
                sequence_element_type(&receiver_ty).is_some_and(|element_ty| {
                    java_type_contains_type_param(&element_ty, type_param)
                })
            })
        }
        Expr::Cast { expr, .. } => {
            expr_recovers_owned_class_type_param_from_field(expr, type_param, fields)
        }
        _ => false,
    }
}

fn expr_java_type(expr: &Expr, fields: &HashMap<String, JavaType>) -> Option<JavaType> {
    match expr {
        Expr::Var(name) => fields.get(name).cloned(),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            fields.get(field).cloned()
        }
        _ => None,
    }
}

fn collection_lookup_value_type(java_ty: &JavaType) -> Option<JavaType> {
    if let Some((_, value_ty)) = crate::stdlib::java_map_type_args(java_ty) {
        return Some(value_ty.clone());
    }
    sequence_element_type(java_ty)
}

pub(crate) fn java_type_contains_type_param(ty: &JavaType, type_param: &str) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            (args.is_empty() && name == type_param)
                || args
                    .iter()
                    .any(|arg| java_type_contains_type_param(arg, type_param))
        }
        JavaType::Unknown(name) => name == type_param,
        JavaType::Array(element) => java_type_contains_type_param(element, type_param),
        JavaType::Intersection(bounds) => bounds
            .iter()
            .any(|bound| java_type_contains_type_param(bound, type_param)),
        JavaType::Wildcard(Some(bound)) => match bound {
            crate::java_ir::WildcardBound::Extends(inner)
            | crate::java_ir::WildcardBound::Super(inner) => {
                java_type_contains_type_param(inner, type_param)
            }
        },
        JavaType::Void | JavaType::Primitive(_) | JavaType::String | JavaType::Wildcard(None) => {
            false
        }
    }
}

fn source_type_bound_decl<'a>(
    bound: &str,
    support_programs: &[&'a Program],
) -> Option<&'a TypeDecl> {
    unique_source_type_bound_decl_by_match_kind(
        bound,
        support_programs,
        SourceTypeBoundMatch::Exact,
    )
    .or_else(|| {
        unique_source_type_bound_decl_by_match_kind(
            bound,
            support_programs,
            SourceTypeBoundMatch::RustAlias,
        )
    })
    .or_else(|| {
        unique_source_type_bound_decl_by_match_kind(
            bound,
            support_programs,
            SourceTypeBoundMatch::Simple,
        )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceTypeBoundMatch {
    Exact,
    RustAlias,
    Simple,
}

fn unique_source_type_bound_decl_by_match_kind<'a>(
    bound: &str,
    support_programs: &[&'a Program],
    kind: SourceTypeBoundMatch,
) -> Option<&'a TypeDecl> {
    let mut found: Option<&'a TypeDecl> = None;
    let mut ambiguous = false;
    for program in support_programs {
        collect_source_type_bound_decls_in_program_by_match_kind(bound, program, kind, &mut |ty| {
            if let Some(existing) = found {
                if source_type_bound_duplicate_decl_is_same_owner(existing, ty) {
                    return true;
                }
                ambiguous = true;
                return false;
            }
            found = Some(ty);
            true
        });
        if ambiguous {
            return None;
        }
    }
    found
}

fn source_type_bound_duplicate_decl_is_same_owner(left: &TypeDecl, right: &TypeDecl) -> bool {
    left.name == right.name
        && left.kind == right.kind
        && left.extends == right.extends
        && left.implements == right.implements
        && left.type_params == right.type_params
        && left.type_param_bounds == right.type_param_bounds
}

fn collect_source_type_bound_decls_in_program_by_match_kind<'a>(
    bound: &str,
    program: &'a Program,
    kind: SourceTypeBoundMatch,
    visit: &mut impl FnMut(&'a TypeDecl) -> bool,
) {
    for ty in &program.types {
        if !collect_source_type_bound_decls_in_type_by_match_kind(bound, ty, kind, visit) {
            break;
        }
    }
}

fn collect_source_type_bound_decls_in_type_by_match_kind<'a>(
    bound: &str,
    ty: &'a TypeDecl,
    kind: SourceTypeBoundMatch,
    visit: &mut impl FnMut(&'a TypeDecl) -> bool,
) -> bool {
    if source_type_bound_matches(ty, bound, kind) && !visit(ty) {
        return false;
    }
    ty.members.iter().all(|member| match member {
        Member::Type(nested) => {
            collect_source_type_bound_decls_in_type_by_match_kind(bound, nested, kind, visit)
        }
        _ => true,
    })
}

fn source_type_bound_matches(ty: &TypeDecl, bound: &str, kind: SourceTypeBoundMatch) -> bool {
    let simple_name = ty.name.rsplit('.').next().unwrap_or(&ty.name);
    match kind {
        SourceTypeBoundMatch::Exact => ty.name == bound,
        SourceTypeBoundMatch::RustAlias => rust_name_for_java_named_type(&ty.name) == bound,
        SourceTypeBoundMatch::Simple => {
            simple_name == bound || rust_name_for_java_named_type(simple_name) == bound
        }
    }
}

fn source_type_bound_is_trait_like(bound: &str, support_programs: &[&Program]) -> bool {
    source_type_bound_decl(bound, support_programs)
        .is_some_and(|ty| matches!(ty.kind, crate::java_ir::TypeKind::Interface))
}

fn source_type_bound_is_class_like(bound: &str, support_programs: &[&Program]) -> bool {
    source_type_bound_decl(bound, support_programs)
        .is_some_and(|ty| matches!(ty.kind, crate::java_ir::TypeKind::Class))
}

fn source_type_bound_has_any_candidate(bound: &str, support_programs: &[&Program]) -> bool {
    for kind in [
        SourceTypeBoundMatch::Exact,
        SourceTypeBoundMatch::RustAlias,
        SourceTypeBoundMatch::Simple,
    ] {
        for program in support_programs {
            let mut found = false;
            collect_source_type_bound_decls_in_program_by_match_kind(
                bound,
                program,
                kind,
                &mut |_| {
                    found = true;
                    false
                },
            );
            if found {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
pub(crate) fn constructor_function_shell_with_facts(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
) -> RustFunction {
    constructor_function_shell_with_facts_and_support_context(
        ty,
        ctor,
        signatures,
        body,
        facts,
        &Program::default(),
    )
}

#[cfg(test)]
fn constructor_function_shell_with_facts_and_support_context(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
    support_program: &Program,
) -> RustFunction {
    constructor_function_shell_with_facts_and_support_contexts(
        ty,
        ctor,
        signatures,
        body,
        facts,
        &[support_program],
    )
}

pub(crate) fn constructor_function_shell_with_facts_and_support_contexts(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
    support_programs: &[&Program],
) -> RustFunction {
    RustFunction {
        name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        type_params: function_type_params_with_optional_lifetime_and_support_contexts(
            None,
            &ctor.type_params,
            &ctor.type_param_bounds,
            false,
            support_programs,
            &ctor.params,
            ctor.body.as_ref(),
        ),
        params: constructor_params(ty, ctor, signatures, facts),
        return_type: Some(constructor_return_type(ty, ctor, signatures)),
        where_bounds: source_constructor_class_type_param_where_bounds(ty, ctor, support_programs),
        body,
        is_pub: true,
        is_pub_crate: false,
        receiver: None,
    }
}

pub(crate) fn constructor_return_body(
    ty: &TypeDecl,
    fields: Vec<(String, RustExpr)>,
) -> Vec<RustStmt> {
    vec![RustStmt::Return(Some(RustExpr::StructLit {
        name: rust_use_name_for_source_type(ty),
        fields: reorder_struct_literal_moves_after_reads(recover_struct_literal_field_exprs(
            with_type_param_marker_field_values(ty, fields),
        )),
    }))]
}

pub(crate) fn with_type_param_marker_field_values(
    ty: &TypeDecl,
    mut fields: Vec<(String, RustExpr)>,
) -> Vec<(String, RustExpr)> {
    if source_type_needs_type_param_marker_field(ty)
        && fields.iter().all(|(name, _)| name != "__marker")
    {
        fields.push((
            "__marker".to_string(),
            RustExpr::path(["std", "marker", "PhantomData"]),
        ));
    }
    fields
}

pub(crate) fn function_type_params_with_optional_lifetime_and_support_contexts(
    callable_name: Option<&str>,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
    needs_lifetime: bool,
    support_programs: &[&Program],
    callable_params: &[crate::java_ir::Param],
    callable_body: Option<&crate::java_ir::Block>,
) -> Vec<String> {
    function_type_params_with_required_clone_bounds_and_support_contexts(
        callable_name,
        type_params,
        type_param_bounds,
        needs_lifetime,
        support_programs,
        callable_params,
        callable_body,
        &[],
    )
}

/// Render callable generics while joining Rust-only `Clone` obligations from
/// every implementation that a synthetic forwarding surface can dispatch to.
/// Ordinary source callables pass an empty slice and retain their body-local
/// inference; hierarchy carriers pass the per-parameter union for all payloads.
pub(crate) fn function_type_params_with_required_clone_bounds_and_support_contexts(
    callable_name: Option<&str>,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
    needs_lifetime: bool,
    support_programs: &[&Program],
    callable_params: &[crate::java_ir::Param],
    callable_body: Option<&crate::java_ir::Block>,
    required_clone_bounds: &[bool],
) -> Vec<String> {
    let mut params = Vec::new();
    if needs_lifetime {
        params.push("'a".to_string());
    }
    params.extend(type_params.iter().enumerate().map(|(index, name)| {
        let mut bounds = type_param_bounds
            .get(index)
            .and_then(|bound| bound.as_ref())
            .map(|bound| rust_bounds_for_java_type_param(bound, support_programs))
            .unwrap_or_default();
        if callable_type_param_needs_clone_bound_with_support_contexts(
            name,
            callable_params,
            callable_body,
            support_programs,
        ) || required_clone_bounds.get(index).copied().unwrap_or(false)
        {
            bounds.retain(|bound| bound != "Clone");
            bounds.push("Clone".to_string());
        }
        if callable_body
            .is_some_and(|body| callable_type_param_trait_object_cast_needs_clone_bound(name, body))
            && !bounds.iter().any(|bound| bound == "std::any::Any")
        {
            // `Any::downcast_ref::<T>` is the runtime check used for an
            // explicit Java interface-to-type-parameter cast.  Carry its
            // `'static` requirement in the same Java-object capability bound
            // used elsewhere for reified generic values.
            bounds.push("std::any::Any".to_string());
        }
        if crate::inherited_members::is_synthesized_source_wildcard_type_param(name)
            && !bounds.iter().any(|bound| bound == "std::any::Any")
        {
            // A Java wildcard capture is still an owned heap-object type.
            // Its Rust reification can escape behind a `Box<dyn Trait>`, whose
            // default object lifetime is `'static`; `Any` expresses the same
            // runtime-object lifetime contract used by source class generics.
            bounds.push("std::any::Any".to_string());
        }
        if callable_type_param_flows_through_source_class_argument(
            name,
            callable_params,
            support_programs,
        ) && !bounds.iter().any(|bound| bound == "std::any::Any")
        {
            // Source class inherent impls use `Any` for Java runtime-class
            // identity. A callable receiving `Model<T>` must therefore carry
            // the same obligation before it can invoke even a bound-neutral
            // method through that class or its hierarchy carrier.
            bounds.push("std::any::Any".to_string());
        }
        if callable_type_param_reaches_hash_backed_source_type(
            name,
            callable_params,
            callable_body,
            support_programs,
        ) {
            // A Java method type parameter can instantiate a source generic
            // whose semantic map/set field needs Java key equality even when
            // the method itself never mentions the collection. Propagate that
            // capability through the constructor/method boundary.
            if !bounds.iter().any(|bound| bound == "std::any::Any") {
                bounds.push("std::any::Any".to_string());
            }
            if !bounds.iter().any(|bound| bound == "JavaMapKey") {
                bounds.push("JavaMapKey".to_string());
            }
        }
        if bounds.is_empty() {
            name.clone()
        } else {
            format!("{name}: {}", bounds.join(" + "))
        }
    }));
    if callable_params
        .iter()
        .any(|param| submit_callable_param_needs_synthetic_return_type(callable_name, param))
    {
        params.push(submit_callable_synthetic_return_type_param(type_params));
    }
    params
}

fn callable_type_param_reaches_hash_backed_source_type(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&crate::java_ir::Block>,
    support_programs: &[&Program],
) -> bool {
    let Some(body) = body else {
        return false;
    };
    let mut types = TypeEnv::new();
    for param in params {
        types.insert_param(param.source_name.clone(), param.ty.clone());
        if param.name != param.source_name {
            types.insert_param(param.name.clone(), param.ty.clone());
        }
    }
    for stmt in &body.statements {
        types.learn_stmt(stmt);
    }

    let mut requires_hash = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if requires_hash {
            return;
        }
        let instantiated = match expr {
            Expr::New { ty, .. } => Some(ty.clone()),
            Expr::Call {
                receiver: Some(receiver),
                ..
            } => Some(infer_expr_type(receiver, &types)),
            _ => None,
        };
        requires_hash = instantiated.as_ref().is_some_and(|ty| {
            source_type_instantiation_requires_hash_capability(ty, type_param, support_programs)
        });
    });
    requires_hash
}

fn source_type_instantiation_requires_hash_capability(
    ty: &JavaType,
    callable_type_param: &str,
    support_programs: &[&Program],
) -> bool {
    let JavaType::Named { name, args } = ty else {
        return false;
    };
    let Some(owner) = source_type_bound_decl(name, support_programs) else {
        return false;
    };
    owner
        .type_params
        .iter()
        .enumerate()
        .any(|(index, owner_param)| {
            args.get(index).is_some_and(|arg| {
                java_type_contains_type_param(arg, callable_type_param)
                    && source_field_types(owner).values().any(|field_ty| {
                        java_type_requires_hash_capability_for_type_param(
                            field_ty,
                            owner_param,
                            support_programs,
                            &mut HashSet::new(),
                        )
                    })
            })
        })
}

fn callable_type_param_flows_through_source_class_argument(
    type_param: &str,
    params: &[crate::java_ir::Param],
    support_programs: &[&Program],
) -> bool {
    params.iter().any(|param| {
        let JavaType::Named { name, args } = &param.ty else {
            return false;
        };
        args.iter()
            .any(|arg| java_type_contains_type_param(arg, type_param))
            && source_type_bound_decl(name, support_programs).is_some_and(|ty| {
                matches!(
                    ty.kind,
                    crate::java_ir::TypeKind::Class | crate::java_ir::TypeKind::Enum
                )
            })
    })
}

fn submit_callable_synthetic_return_type_param(type_params: &[String]) -> String {
    let base = "__JavaCallableReturn";
    if !type_params.iter().any(|param| param == base) {
        return base.to_string();
    }
    let mut index = 0usize;
    loop {
        let candidate = format!("{base}{index}");
        if !type_params.iter().any(|param| param == &candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn reorder_struct_literal_moves_after_reads(
    fields: Vec<(String, RustExpr)>,
) -> Vec<(String, RustExpr)> {
    let mut deferred = Vec::new();
    let mut ordered = Vec::new();
    for (index, (field_name, expr)) in fields.iter().cloned().enumerate() {
        if let Some(var_name) = rust_expr_direct_move_var(&expr)
            && fields
                .iter()
                .enumerate()
                .filter(|(other_index, _)| *other_index != index)
                .any(|(_, (_, other_expr))| rust_expr_reads_var(other_expr, &var_name))
        {
            deferred.push((field_name, expr));
            continue;
        }
        ordered.push((field_name, expr));
    }
    ordered.extend(deferred);
    ordered
}

fn recover_struct_literal_field_exprs(fields: Vec<(String, RustExpr)>) -> Vec<(String, RustExpr)> {
    fields
        .into_iter()
        .map(|(name, expr)| (name, recover_struct_literal_field_expr(expr)))
        .collect()
}

fn recover_struct_literal_field_expr(expr: RustExpr) -> RustExpr {
    match expr {
        RustExpr::Raw(_) => recover_parseable_raw_operand_projection(expr),
        RustExpr::Block { stmts, tail } => RustExpr::Block {
            stmts: stmts
                .into_iter()
                .map(recover_struct_literal_field_stmt)
                .collect(),
            tail: tail.map(|tail| Box::new(recover_struct_literal_field_expr(*tail))),
        },
        RustExpr::Grouped(expr) => {
            RustExpr::Grouped(Box::new(recover_struct_literal_field_expr(*expr)))
        }
        RustExpr::Shaped {
            shape,
            actual,
            expr,
        } => RustExpr::Shaped {
            shape,
            actual,
            expr: Box::new(recover_struct_literal_field_expr(*expr)),
        },
        RustExpr::MarkedBlock { marker, expr } => RustExpr::MarkedBlock {
            marker,
            expr: Box::new(recover_struct_literal_field_expr(*expr)),
        },
        RustExpr::PostfixComment { expr, comment } => RustExpr::PostfixComment {
            expr: Box::new(recover_struct_literal_field_expr(*expr)),
            comment,
        },
        other => other,
    }
}

fn recover_struct_literal_field_stmt(stmt: RustStmt) -> RustStmt {
    match stmt {
        RustStmt::Raw(text) => recover_struct_literal_raw_stmt(text),
        RustStmt::Let {
            mutable,
            name,
            ty,
            value,
        } => RustStmt::Let {
            mutable,
            name,
            ty,
            value: value.map(recover_struct_literal_field_expr),
        },
        RustStmt::Assign { target, value } => RustStmt::Assign {
            target: recover_struct_literal_field_expr(target),
            value: recover_struct_literal_field_expr(value),
        },
        RustStmt::AssignOp { target, op, value } => RustStmt::AssignOp {
            target: recover_struct_literal_field_expr(target),
            op,
            value: recover_struct_literal_field_expr(value),
        },
        RustStmt::Expr(expr) => RustStmt::Expr(recover_struct_literal_field_expr(expr)),
        RustStmt::Return(Some(expr)) => {
            RustStmt::Return(Some(recover_struct_literal_field_expr(expr)))
        }
        other => other,
    }
}

fn recover_struct_literal_raw_stmt(text: String) -> RustStmt {
    raw_structured_stmt_projection(&text).unwrap_or_else(|| RustStmt::raw_leaf(text))
}

fn raw_structured_stmt_projection(text: &str) -> Option<RustStmt> {
    let trimmed = text.trim();
    let expr_text = trimmed
        .strip_prefix("return ")
        .and_then(|tail| tail.strip_suffix(';'));
    if let Some(expr_text) = expr_text {
        return recover_structured_raw_stmt_expr(expr_text)
            .map(|expr| RustStmt::Return(Some(expr)));
    }
    trimmed
        .strip_suffix(';')
        .and_then(recover_structured_raw_stmt_expr)
        .map(RustStmt::Expr)
}

fn recover_structured_raw_stmt_expr(expr_text: &str) -> Option<RustExpr> {
    let recovered = recover_parseable_raw_operand_projection(RustExpr::raw_leaf(expr_text));
    (!recovered.is_raw_leaf()).then_some(recovered)
}

fn rust_expr_direct_move_var(expr: &RustExpr) -> Option<String> {
    match expr {
        RustExpr::Var(name) if movable_local_identifier(name) => Some(name.clone()),
        _ => None,
    }
}

fn movable_local_identifier(name: &str) -> bool {
    !matches!(name, "self" | "super" | "crate" | "Self")
}

fn rust_expr_reads_var(expr: &RustExpr, var_name: &str) -> bool {
    match expr {
        RustExpr::Shaped { expr, .. } => rust_expr_reads_var(expr, var_name),
        RustExpr::Grouped(expr) => rust_expr_reads_var(expr, var_name),
        RustExpr::Var(name) => name == var_name,
        RustExpr::Field(receiver, _) => rust_expr_reads_var(receiver, var_name),
        RustExpr::Index(target, index) => {
            rust_expr_reads_var(target, var_name) || rust_expr_reads_var(index, var_name)
        }
        RustExpr::Unary { expr, .. } => rust_expr_reads_var(expr, var_name),
        RustExpr::Cast { expr, .. } => rust_expr_reads_var(expr, var_name),
        RustExpr::Try(expr) => rust_expr_reads_var(expr, var_name),
        RustExpr::Binary { left, right, .. } => {
            rust_expr_reads_var(left, var_name) || rust_expr_reads_var(right, var_name)
        }
        RustExpr::Range { start, end, .. } => {
            rust_expr_reads_var(start, var_name) || rust_expr_reads_var(end, var_name)
        }
        RustExpr::If {
            condition,
            then_expr,
            else_expr,
        } => {
            rust_expr_reads_var(condition, var_name)
                || rust_expr_reads_var(then_expr, var_name)
                || rust_expr_reads_var(else_expr, var_name)
        }
        RustExpr::Tuple(items) | RustExpr::ArrayLiteral(items) => {
            items.iter().any(|item| rust_expr_reads_var(item, var_name))
        }
        RustExpr::VecLiteral(items) => items.iter().any(|item| rust_expr_reads_var(item, var_name)),
        RustExpr::VecRepeat { value, len } => {
            rust_expr_reads_var(value, var_name) || rust_expr_reads_var(len, var_name)
        }
        RustExpr::UnsupportedJavaArrayCreation { dimensions, .. } => dimensions
            .iter()
            .any(|dimension| rust_expr_reads_var(dimension, var_name)),
        RustExpr::Call { function, args } => {
            rust_expr_reads_var(function, var_name)
                || args.iter().any(|arg| rust_expr_reads_var(arg, var_name))
        }
        RustExpr::MacroCall { args, .. } => {
            args.iter().any(|arg| rust_expr_reads_var(arg, var_name))
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            method == var_name
                || rust_expr_reads_var(receiver, var_name)
                || args.iter().any(|arg| rust_expr_reads_var(arg, var_name))
        }
        RustExpr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, field_expr)| rust_expr_reads_var(field_expr, var_name)),
        RustExpr::MarkedBlock { expr, .. } => rust_expr_reads_var(expr, var_name),
        RustExpr::PostfixComment { expr, .. } => rust_expr_reads_var(expr, var_name),
        RustExpr::Closure { params, body, .. } => {
            if params.iter().any(|param| param.pattern == var_name) {
                return false;
            }
            match body {
                crate::rust_ir::RustClosureBody::Expr(expr) => rust_expr_reads_var(expr, var_name),
            }
        }
        RustExpr::Match { expr, arms } => {
            rust_expr_reads_var(expr, var_name)
                || arms.iter().any(|arm| {
                    !arm.pattern
                        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                        .any(|token| token == var_name)
                        && rust_expr_reads_var(&arm.body, var_name)
                })
        }
        RustExpr::Block { stmts, tail } => {
            stmts.iter().any(|stmt| rust_stmt_reads_var(stmt, var_name))
                || tail
                    .as_ref()
                    .is_some_and(|tail| rust_expr_reads_var(tail, var_name))
        }
        RustExpr::Path(_)
        | RustExpr::GenericPath { .. }
        | RustExpr::TypeAssociatedPath { .. }
        | RustExpr::TraitAssociatedPath { .. }
        | RustExpr::Literal(_)
        | RustExpr::Raw(_)
        | RustExpr::Pattern(_) => false,
    }
}

fn rust_stmt_reads_var(stmt: &RustStmt, var_name: &str) -> bool {
    match stmt {
        RustStmt::Let { name, value, .. } => {
            value
                .as_ref()
                .is_some_and(|value| rust_expr_reads_var(value, var_name))
                && name != var_name
        }
        RustStmt::Assign { target, value } | RustStmt::AssignOp { target, value, .. } => {
            rust_expr_reads_var(target, var_name) || rust_expr_reads_var(value, var_name)
        }
        RustStmt::Expr(expr) | RustStmt::Return(Some(expr)) => rust_expr_reads_var(expr, var_name),
        RustStmt::If {
            condition,
            then_body,
            else_body,
        } => {
            rust_expr_reads_var(condition, var_name)
                || then_body
                    .iter()
                    .any(|stmt| rust_stmt_reads_var(stmt, var_name))
                || else_body
                    .iter()
                    .any(|stmt| rust_stmt_reads_var(stmt, var_name))
        }
        RustStmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => {
            rust_expr_reads_var(expr, var_name)
                || (pattern
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .all(|token| token != var_name)
                    && (then_body
                        .iter()
                        .any(|stmt| rust_stmt_reads_var(stmt, var_name))
                        || else_body
                            .iter()
                            .any(|stmt| rust_stmt_reads_var(stmt, var_name))))
        }
        RustStmt::While { condition, body } => {
            rust_expr_reads_var(condition, var_name)
                || body.iter().any(|stmt| rust_stmt_reads_var(stmt, var_name))
        }
        RustStmt::WhileLet {
            pattern,
            expr,
            body,
            ..
        } => {
            rust_expr_reads_var(expr, var_name)
                || (pattern
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .all(|token| token != var_name)
                    && body.iter().any(|stmt| rust_stmt_reads_var(stmt, var_name)))
        }
        RustStmt::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            rust_expr_reads_var(iterable, var_name)
                || (pattern
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .all(|token| token != var_name)
                    && body.iter().any(|stmt| rust_stmt_reads_var(stmt, var_name)))
        }
        RustStmt::Loop { body, .. } => body.iter().any(|stmt| rust_stmt_reads_var(stmt, var_name)),
        RustStmt::BreakValue { value, .. } => rust_expr_reads_var(value, var_name),
        RustStmt::Raw(_) => false,
        RustStmt::Return(None) | RustStmt::Break(_) | RustStmt::Continue(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_type_decl(name: &str, kind: TypeKind, extends: Option<&str>) -> TypeDecl {
        TypeDecl {
            kind,
            name: name.to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: extends.map(str::to_string),
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            static_init_blocks: Vec::new(),
            span: crate::diagnostics::Span::default(),
        }
    }

    fn support_program(types: Vec<TypeDecl>) -> Program {
        Program {
            types,
            ..Program::default()
        }
    }

    fn qualified_new_expr(qualifier: Expr, args: Vec<Expr>) -> Expr {
        Expr::New {
            ty: JavaType::Unknown("Inner".to_string()),
            qualifier: Some(Box::new(qualifier)),
            args,
            body: None,
            resolved: None,
        }
    }

    #[test]
    fn constructor_self_read_scan_sees_qualified_new_receiver() {
        assert!(constructor_expr_reads_self(&qualified_new_expr(
            Expr::FieldAccess(Box::new(Expr::This), "outer".to_string()),
            Vec::new(),
        )));
    }

    #[test]
    fn mutable_array_target_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(
            Expr::Unary {
                op: crate::java_ir::UnaryOp::PostInc,
                expr: Box::new(Expr::ArrayAccess(
                    Box::new(Expr::Var("values".to_string())),
                    Box::new(Expr::Literal(Literal::Int(0))),
                )),
            },
            Vec::new(),
        );
        let mut names = HashSet::new();

        collect_mutable_array_target_params_in_expr(&expr, &mut names);

        assert_eq!(names, HashSet::from(["values".to_string()]));
    }

    #[test]
    fn callback_reference_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(Expr::Var("callback".to_string()), Vec::new());

        assert!(expr_references_callback_param(&expr, "callback"));
    }

    #[test]
    fn priority_queue_callback_storage_scan_sees_qualified_new_receiver() {
        let expr = qualified_new_expr(
            Expr::New {
                ty: JavaType::Named {
                    name: "PriorityQueue".to_string(),
                    args: Vec::new(),
                },
                qualifier: None,
                args: vec![Expr::Var("comparator".to_string())],
                body: None,
                resolved: None,
            },
            Vec::new(),
        );

        assert!(expr_stores_priority_queue_comparator_callback(
            &expr,
            "comparator"
        ));
    }

    #[test]
    fn collections_sort_consumes_comparator_param_as_callback() {
        let method = MethodDecl {
            name: "sortValues".to_string(),
            rust_name: None,
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            return_type: JavaType::Void,
            throws: Vec::new(),
            params: vec![crate::java_ir::Param {
                name: "comparator".to_string(),
                source_name: "comparator".to_string(),
                ty: JavaType::Named {
                    name: "Comparator".to_string(),
                    args: vec![JavaType::String],
                },
                is_varargs: false,
                binding: None,
            }],
            modifiers: Vec::new(),
            body: Some(Block {
                statements: vec![Stmt::Expr(Expr::Call {
                    receiver: Some(Box::new(Expr::Var("Collections".to_string()))),
                    name: "sort".to_string(),
                    args: vec![
                        Expr::Var("values".to_string()),
                        Expr::Var("comparator".to_string()),
                    ],
                    resolved: None,
                })],
            }),
            span: crate::diagnostics::Span::default(),
        };

        assert!(method_param_is_nonescaping_functional_callback(
            &method,
            "comparator"
        ));
        assert_eq!(
            functional_callback_param_type(&method.params[0].ty, &FactSet::default()),
            Some(RustType::ImplFnTrait {
                trait_name: "FnMut".to_string(),
                params: vec![RustType::String, RustType::String],
                return_type: Some(Box::new(RustType::I32)),
            })
        );
    }

    #[test]
    fn method_params_render_collections_sort_comparator_param_as_callback() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;

class SortUse {
    void sortValues(ArrayList<String> values, Comparator<String> comparator) {
        Collections.sort(values, comparator);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let representations =
            crate::representation::plan_representations(&resolved.program, &resolved.facts);
        let usage = crate::usage::analyze_usage(&resolved.program);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &resolved.facts,
        );
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "sortValues" => Some(method),
                _ => None,
            })
            .unwrap();

        let params = method_params(ty, method, &signatures, &resolved.facts);

        assert_eq!(
            params
                .iter()
                .find(|param| param.name.contains("comparator"))
                .map(|param| &param.ty),
            Some(&RustType::ImplFnTrait {
                trait_name: "FnMut".to_string(),
                params: vec![RustType::String, RustType::String],
                return_type: Some(Box::new(RustType::I32)),
            })
        );
    }

    #[test]
    fn method_params_render_priority_queue_comparator_param_as_callback() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.Comparator;
import java.util.PriorityQueue;

class QueueUse {
    static PriorityQueue<Integer> queue(Comparator<Integer> comparator) {
        return new PriorityQueue<Integer>(comparator);
    }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let representations =
            crate::representation::plan_representations(&resolved.program, &resolved.facts);
        let usage = crate::usage::analyze_usage(&resolved.program);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &resolved.facts,
        );
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) if method.name == "queue" => Some(method),
                _ => None,
            })
            .unwrap();

        let params = method_params(ty, method, &signatures, &resolved.facts);

        assert_eq!(
            params
                .iter()
                .find(|param| param.name.contains("comparator"))
                .map(|param| &param.ty),
            Some(&RustType::Raw(
                "impl FnMut(i32, i32) -> i32 + 'static".to_string()
            ))
        );
    }

    #[test]
    fn source_type_bound_decl_refuses_ambiguous_simple_package_peer() {
        let support = support_program(vec![
            test_type_decl("a.Base", TypeKind::Interface, None),
            test_type_decl("b.Base", TypeKind::Class, None),
        ]);
        let bound = JavaType::Named {
            name: "Base".to_string(),
            args: Vec::new(),
        };

        assert_eq!(
            rust_bounds_for_java_type_param(&bound, &[&support]),
            Vec::<String>::new()
        );
        assert_eq!(
            rust_bounds_for_java_type_param(
                &JavaType::Named {
                    name: "a.Base".to_string(),
                    args: Vec::new(),
                },
                &[&support],
            ),
            vec![rust_name_for_java_named_type("a.Base")]
        );

        let support = support_program(vec![
            test_type_decl("a.B.C", TypeKind::Interface, None),
            test_type_decl("AB.C", TypeKind::Class, None),
        ]);
        let rust_alias = rust_name_for_java_named_type("a.B.C");
        assert_eq!(rust_alias, rust_name_for_java_named_type("AB.C"));
        assert_eq!(
            rust_bounds_for_java_type_param(
                &JavaType::Named {
                    name: rust_alias,
                    args: Vec::new(),
                },
                &[&support],
            ),
            Vec::<String>::new()
        );
    }

    #[test]
    fn source_type_bound_decl_accepts_duplicate_view_of_same_owner() {
        let left = support_program(vec![test_type_decl("a.Base", TypeKind::Interface, None)]);
        let right = support_program(vec![test_type_decl("a.Base", TypeKind::Interface, None)]);
        let bound = JavaType::Named {
            name: "a.Base".to_string(),
            args: Vec::new(),
        };

        assert_eq!(
            rust_bounds_for_java_type_param(&bound, &[&left, &right]),
            vec![rust_name_for_java_named_type("a.Base")]
        );
    }

    #[test]
    fn source_type_f_bound_projects_lower_bounded_wildcard_to_type_parameter() {
        let support = support_program(vec![test_type_decl("Factory", TypeKind::Interface, None)]);
        let bound = JavaType::Named {
            name: "Factory".to_string(),
            args: vec![JavaType::Wildcard(Some(
                crate::java_ir::WildcardBound::Super(Box::new(JavaType::Named {
                    name: "T".to_string(),
                    args: Vec::new(),
                })),
            ))],
        };

        assert_eq!(
            rust_bounds_for_java_type_param(&bound, &[&support]),
            vec!["Factory<T>".to_string()]
        );
    }

    #[test]
    fn intersection_type_param_bound_projects_every_declared_capability() {
        let parsed = crate::parser::parse_java_source(
            r#"
interface Observation {}
interface CentroidFactory<T> {}
class Holder {
    <T extends Observation & CentroidFactory<? super T>> T keep(T value) {
        return value;
    }
}
"#,
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let holder = parsed
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Holder")
            .unwrap();
        let method = holder
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        let bound = method.type_param_bounds[0].as_ref().unwrap();

        assert_eq!(
            rust_bounds_for_java_type_param(bound, &[&parsed.program]),
            vec!["Observation", "CentroidFactory<T>"]
        );
    }

    #[test]
    fn generic_hash_container_fields_add_rust_key_capabilities_to_impl() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.HashMap;
import java.util.HashSet;

class HashBacked<K, V> {
    HashMap<K, V> values;
    HashSet<V> seen;
}

class HashBackedOwner<T> {
    HashBacked<T, String> nested;
}
"#,
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let ty = &parsed.program.types[0];

        assert_eq!(
            rust_impl_params_for_source_type(ty, &parsed.program, None),
            vec![
                "K: std::any::Any + JavaMapKey".to_string(),
                "V: std::any::Any + JavaMapKey".to_string(),
            ]
        );
        let owner = &parsed.program.types[1];
        assert_eq!(
            rust_impl_params_for_source_type(owner, &parsed.program, None),
            vec!["T: std::any::Any + JavaMapKey".to_string()]
        );
    }

    #[test]
    fn source_type_bound_decl_refuses_duplicate_exact_owner() {
        let left = support_program(vec![test_type_decl("a.Base", TypeKind::Interface, None)]);
        let right = support_program(vec![test_type_decl("a.Base", TypeKind::Class, None)]);
        let bound = JavaType::Named {
            name: "a.Base".to_string(),
            args: Vec::new(),
        };

        assert_eq!(
            rust_bounds_for_java_type_param(&bound, &[&left, &right]),
            Vec::<String>::new(),
            "duplicate exact source type-bound owners must fail closed"
        );
    }

    #[test]
    fn source_type_param_into_supertype_bounds_refuses_ambiguous_simple_package_peer() {
        let support = support_program(vec![
            test_type_decl("a.Parent", TypeKind::Class, None),
            test_type_decl("a.Base", TypeKind::Class, Some("a.Parent")),
            test_type_decl("b.Parent", TypeKind::Class, None),
            test_type_decl("b.Base", TypeKind::Class, Some("b.Parent")),
        ]);

        assert!(source_type_param_into_supertype_bounds("Base", &support, None).is_empty());
        assert_eq!(
            source_type_param_into_supertype_bounds("a.Base", &support, None),
            vec![format!(
                "Into<{}>",
                rust_name_for_java_named_type("a.Parent")
            )]
        );

        let support = support_program(vec![
            test_type_decl("a.Parent", TypeKind::Class, None),
            test_type_decl("a.B.C", TypeKind::Class, Some("a.Parent")),
            test_type_decl("AB.Parent", TypeKind::Class, None),
            test_type_decl("AB.C", TypeKind::Class, Some("AB.Parent")),
        ]);
        let rust_alias = rust_name_for_java_named_type("a.B.C");
        assert_eq!(rust_alias, rust_name_for_java_named_type("AB.C"));
        assert!(source_type_param_into_supertype_bounds(&rust_alias, &support, None).is_empty());
    }

    #[test]
    fn source_type_param_into_supertype_bounds_refuses_duplicate_exact_owner() {
        let support = support_program(vec![
            test_type_decl("a.Parent", TypeKind::Class, None),
            test_type_decl("a.Base", TypeKind::Class, Some("a.Parent")),
            test_type_decl("b.Parent", TypeKind::Class, None),
            test_type_decl("a.Base", TypeKind::Class, Some("b.Parent")),
        ]);

        assert!(
            source_type_param_into_supertype_bounds("a.Base", &support, None).is_empty(),
            "duplicate exact source type-bound owners must not choose the first supertype"
        );
    }

    #[test]
    fn method_shell_exception_plan_is_internal_and_wraps_return_type() {
        let parsed = crate::parser::parse_java_source(
            r#"
class ExternalShell {
    private int len(ExternalLabel value) {
        return value.length();
    }
}

class ExternalLabel {}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = resolved.facts.clone();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "ExternalLabel".to_string(),
                copy: None,
                clone: None,
                string_like: Some(true),
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: None,
                object_safe_trait: None,
                provenance: crate::facts::FactProvenance::UserOverride,
            });
        let representations =
            crate::representation::plan_representations(&resolved.program, &facts);
        let usage = crate::usage::analyze_usage(&resolved.program);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &facts,
        );
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        let body = vec![RustStmt::Return(Some(RustExpr::Literal("0".to_string())))];

        let default_shell =
            method_function_shell_with_facts(ty, method, &signatures, body.clone(), &facts);
        let planned_shell = method_function_shell_with_facts_and_exception_plan(
            ty,
            method,
            &signatures,
            body,
            &facts,
            MethodExceptionPlan::new(false),
        );
        assert_eq!(planned_shell.return_type, default_shell.return_type);

        let result_shell = method_function_shell_with_facts_and_exception_plan(
            ty,
            method,
            &signatures,
            Vec::new(),
            &facts,
            MethodExceptionPlan::new(true),
        );
        assert_eq!(
            result_shell.return_type,
            Some(exception_result_type(default_shell.return_type.unwrap()))
        );
    }

    #[test]
    fn item_assembly_uses_fact_aware_slot_rendering_for_function_shells() {
        let parsed = crate::parser::parse_java_source(
            r#"
class ExternalShell {
    private int len(ExternalLabel value) {
        return value.length();
    }
}

class ExternalLabel {}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let mut facts = resolved.facts.clone();
        facts
            .type_capability_overrides
            .push(crate::facts::TypeCapabilityOverride {
                type_name: "ExternalLabel".to_string(),
                copy: None,
                clone: None,
                string_like: Some(true),
                sequence_like: None,
                implements: Vec::new(),
                extends: None,
                final_type: None,
                object_safe_trait: None,
                provenance: crate::facts::FactProvenance::UserOverride,
            });
        let representations =
            crate::representation::plan_representations(&resolved.program, &facts);
        let usage = crate::usage::analyze_usage(&resolved.program);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &facts,
        );
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        let body = vec![RustStmt::Return(Some(RustExpr::Literal("0".to_string())))];

        let default_facts = FactSet::default();
        let default_shell =
            method_function_shell_with_facts(ty, method, &signatures, body.clone(), &default_facts);
        let fact_shell = method_function_shell_with_facts(ty, method, &signatures, body, &facts);

        assert_eq!(
            default_shell.params[0].ty,
            RustType::Ref {
                mutable: false,
                lifetime: None,
                inner: Box::new(RustType::Named("ExternalLabel".to_string())),
            }
        );
        assert_eq!(
            fact_shell.params[0].ty,
            RustType::Ref {
                mutable: false,
                lifetime: None,
                inner: Box::new(RustType::Str),
            }
        );
    }

    #[test]
    fn delegating_constructor_return_body_calls_self_associated_constructor() {
        assert_eq!(
            delegating_constructor_return_body(
                "new_with_name",
                vec![RustExpr::Var("name".to_string())],
            ),
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::Path(vec![
                    "Self".to_string(),
                    "new_with_name".to_string(),
                ])),
                args: vec![RustExpr::Var("name".to_string())],
            }))]
        );
    }

    #[test]
    fn item_assembly_owns_struct_impl_and_function_shells_from_signatures() {
        let parsed = crate::parser::parse_java_source(
            r#"
class Holder {
    String name;
    Holder(String name) { this.name = name; }
    public String echo(String value) { return value; }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let representations =
            crate::representation::plan_representations(&resolved.program, &resolved.facts);
        let usage = crate::usage::analyze_usage(&resolved.program);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &usage,
            &FactSet::default(),
        );
        let ty = &resolved.program.types[0];
        let method = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        let ctor = ty
            .members
            .iter()
            .find_map(|member| match member {
                Member::Constructor(ctor) => Some(ctor),
                _ => None,
            })
            .unwrap();
        let default_facts = FactSet::default();

        let struct_item = struct_item_for_type_with_facts(ty, &signatures, &default_facts);
        let RustItem::Struct(rust_struct) = struct_item else {
            panic!("type shell should be assembled as a struct item");
        };
        assert_eq!(rust_struct.name, "Holder");
        assert_eq!(rust_struct.fields.len(), 1);
        assert_eq!(rust_struct.fields[0].name, "name");
        assert_eq!(
            rust_struct.fields[0].ty,
            crate::slot_render::rust_type_for_slot(
                signatures.field_shape("Holder", "name").unwrap()
            )
        );

        let ctor_body = vec![RustStmt::Return(None)];
        let ctor_fn = constructor_function_shell_with_facts(
            ty,
            ctor,
            &signatures,
            ctor_body.clone(),
            &default_facts,
        );
        assert_eq!(ctor_fn.name, "new");
        assert_eq!(ctor_fn.params.len(), 1);
        assert_eq!(ctor_fn.params[0].name, "name");
        assert_eq!(
            ctor_fn.params[0].ty,
            crate::slot_render::rust_type_for_slot(
                signatures.constructor_param_plans(ty, ctor).unwrap()[0].shape()
            )
        );
        assert_eq!(
            ctor_fn.return_type,
            Some(RustType::Named("Holder".to_string()))
        );
        assert_eq!(ctor_fn.receiver, None);
        assert_eq!(ctor_fn.body, ctor_body);

        assert_eq!(
            constructor_return_body(
                ty,
                vec![("name".to_string(), RustExpr::Var("name".to_string()))]
            ),
            vec![RustStmt::Return(Some(RustExpr::StructLit {
                name: "Holder".to_string(),
                fields: vec![("name".to_string(), RustExpr::Var("name".to_string()))],
            }))]
        );
        assert_eq!(
            constructor_return_body(
                ty,
                vec![
                    ("reader".to_string(), RustExpr::Var("reader".to_string())),
                    (
                        "query".to_string(),
                        RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("reader".to_string())),
                            method: "query_3".to_string(),
                            args: Vec::new(),
                        },
                    ),
                ],
            ),
            vec![RustStmt::Return(Some(RustExpr::StructLit {
                name: "Holder".to_string(),
                fields: vec![
                    (
                        "query".to_string(),
                        RustExpr::MethodCall {
                            receiver: Box::new(RustExpr::Var("reader".to_string())),
                            method: "query_3".to_string(),
                            args: Vec::new(),
                        },
                    ),
                    ("reader".to_string(), RustExpr::Var("reader".to_string())),
                ],
            }))]
        );
        assert_eq!(
            constructor_return_body(
                ty,
                vec![
                    ("reader".to_string(), RustExpr::Raw("reader".to_string())),
                    (
                        "query".to_string(),
                        RustExpr::Raw("Some(reader.query_3(contig, start, end))".to_string()),
                    ),
                ],
            ),
            vec![RustStmt::Return(Some(RustExpr::StructLit {
                name: "Holder".to_string(),
                fields: vec![
                    (
                        "query".to_string(),
                        RustExpr::Shaped {
                            shape: crate::rust_ir::RustExprShape::OptionalConstructed,
                            actual: None,
                            expr: Box::new(RustExpr::Call {
                                function: Box::new(RustExpr::Path(vec!["Some".to_string()])),
                                args: vec![RustExpr::MethodCall {
                                    receiver: Box::new(RustExpr::Var("reader".to_string())),
                                    method: "query_3".to_string(),
                                    args: vec![
                                        RustExpr::Var("contig".to_string()),
                                        RustExpr::Var("start".to_string()),
                                        RustExpr::Var("end".to_string()),
                                    ],
                                }],
                            }),
                        },
                    ),
                    ("reader".to_string(), RustExpr::Var("reader".to_string())),
                ],
            }))]
        );
        assert_eq!(
            constructor_default_field_value_with_facts(
                "count".to_string(),
                RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
                &default_facts,
            ),
            (
                "count".to_string(),
                default_expr_for_slot_with_facts(
                    &RustSlotShape::for_storage(JavaType::Primitive(PrimitiveType::Int)),
                    &default_facts
                )
            )
        );

        let method_body = vec![RustStmt::Return(Some(RustExpr::Var("value".to_string())))];
        let method_fn = method_function_shell_with_facts(
            ty,
            method,
            &signatures,
            method_body.clone(),
            &default_facts,
        );
        assert_eq!(method_fn.name, "echo");
        assert_eq!(method_fn.params.len(), 1);
        assert_eq!(method_fn.params[0].name, "value");
        assert_eq!(
            method_fn.return_type,
            signatures
                .method_return_shape(ty, method)
                .map(|method_shape| crate::slot_render::rust_type_for_slot(&method_shape))
        );
        assert_eq!(method_fn.receiver, Some(RustReceiver::Shared));
        assert_eq!(method_fn.body, method_body);

        let impl_item = impl_item_for_type(ty, vec![method_fn.clone()]);
        let RustItem::Impl(rust_impl) = impl_item else {
            panic!("impl shell should be assembled as an impl item");
        };
        assert_eq!(rust_impl.target, "Holder");
        assert_eq!(rust_impl.functions, vec![method_fn]);
    }

    #[test]
    fn derive_support_type_lookup_refuses_ambiguous_emitted_rust_name() {
        let support = support_program(vec![
            test_type_decl("a.B.C", TypeKind::Class, None),
            test_type_decl("AB.C", TypeKind::Interface, None),
        ]);
        let rust_alias = rust_name_for_java_named_type("a.B.C");
        assert_eq!(rust_alias, rust_name_for_java_named_type("AB.C"));

        assert!(
            find_type_decl_by_rust_name(&support, &rust_alias).is_none(),
            "derive support must not recover a Java owner from a colliding emitted Rust name"
        );
        assert!(!rust_named_type_supports_struct_derive_default(
            &rust_alias,
            &HashSet::new(),
            &SignaturePlan::default(),
            &FactSet::default(),
            &support,
            &mut HashSet::new()
        ));
    }

    #[test]
    fn raw_shared_handle_clone_support_requires_complete_generic_type() {
        let support = Program::default();
        let type_params = HashSet::new();
        let signatures = SignaturePlan::default();
        let facts = FactSet::default();

        for raw in [
            "std::rc::Rc<Unmodelled>",
            "Rc<Unmodelled>",
            "std::sync::Arc<Unmodelled>",
            "Arc<Unmodelled>",
            "Vec<std::sync::Arc<Unmodelled>>",
        ] {
            assert!(
                rust_type_supports_struct_derive_clone(
                    &RustType::Raw(raw.to_string()),
                    &type_params,
                    &signatures,
                    &facts,
                    &support,
                    &mut HashSet::new(),
                ),
                "well-formed shared handle should support Clone derive: {raw}"
            );
        }

        for raw in [
            "std::rc::Rc<Unmodelled",
            "std::sync::Arc<Unmodelled",
            "Vec<std::rc::Rc<Unmodelled>",
            "std::rc::Rc<Left, Right>",
            "Rc<Left, Right>",
            "std::sync::Arc<Left, Right>",
            "Arc<Left, Right>",
        ] {
            assert!(
                !rust_type_supports_struct_derive_clone(
                    &RustType::Raw(raw.to_string()),
                    &type_params,
                    &signatures,
                    &facts,
                    &support,
                    &mut HashSet::new(),
                ),
                "malformed shared handle text should fail closed: {raw}"
            );
        }
    }

    #[test]
    fn raw_clone_support_recovers_simple_type_atoms_before_string_parsing() {
        let support = Program::default();
        let mut type_params = HashSet::new();
        type_params.insert("T".to_string());
        let signatures = SignaturePlan::default();
        let facts = FactSet::default();

        for raw in ["String", "i32", "T", "Vec<Option<T>>"] {
            assert!(
                rust_type_supports_struct_derive_clone(
                    &RustType::Raw(raw.to_string()),
                    &type_params,
                    &signatures,
                    &facts,
                    &support,
                    &mut HashSet::new(),
                ),
                "simple raw clone support should match structured RustType handling: {raw}"
            );
        }
    }

    #[test]
    fn raw_clone_support_fails_closed_for_unrecovered_type_args() {
        let support = Program::default();
        let type_params = HashSet::new();
        let signatures = SignaturePlan::default();
        let facts = FactSet::default();

        assert!(
            !rust_type_supports_struct_derive_clone(
                &RustType::Raw("Vec<dyn Fn(i32)>".to_string()),
                &type_params,
                &signatures,
                &facts,
                &support,
                &mut HashSet::new(),
            ),
            "unrecovered raw generic args must not be treated as semantic type names"
        );
    }

    #[test]
    fn structured_method_calls_count_method_name_reads() {
        let call = RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("self".to_string())),
            method: "get_default_tool_v_c_f_header_lines".to_string(),
            args: Vec::new(),
        };

        assert!(rust_expr_reads_var(
            &call,
            "get_default_tool_v_c_f_header_lines"
        ));
        assert!(!rust_expr_reads_var(&call, "unrelated_method"));
    }

    #[test]
    fn raw_direct_move_vars_require_structured_recovery() {
        assert_eq!(
            rust_expr_direct_move_var(&RustExpr::Var("value".to_string())),
            Some("value".to_string())
        );
        assert_eq!(
            rust_expr_direct_move_var(&RustExpr::Raw("value".to_string())),
            None
        );
        for pseudo in ["self", "super", "crate", "Self"] {
            assert_eq!(
                rust_expr_direct_move_var(&RustExpr::Var(pseudo.to_string())),
                None
            );
        }
        assert_eq!(
            rust_expr_direct_move_var(&RustExpr::Raw("value +".to_string())),
            None
        );
    }

    #[test]
    fn constructor_return_field_recovery_happens_before_move_reordering() {
        let ty = TypeDecl {
            kind: TypeKind::Class,
            name: "Owner".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            static_init_blocks: Vec::new(),
            span: Default::default(),
        };
        let body = constructor_return_body(
            &ty,
            vec![
                ("first".to_string(), RustExpr::Raw("value".to_string())),
                (
                    "second".to_string(),
                    RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("value".to_string())),
                        method: "clone".to_string(),
                        args: Vec::new(),
                    },
                ),
            ],
        );

        let [RustStmt::Return(Some(RustExpr::StructLit { fields, .. }))] = body.as_slice() else {
            panic!("constructor return body should be a struct literal return: {body:?}");
        };
        assert_eq!(fields[0].0, "second");
        assert_eq!(fields[1].0, "first");
        assert!(matches!(fields[1].1, RustExpr::Var(ref name) if name == "value"));
    }

    #[test]
    fn constructor_return_block_field_recovery_happens_before_move_reordering() {
        let ty = TypeDecl {
            kind: TypeKind::Class,
            name: "Owner".to_string(),
            type_params: Vec::new(),
            type_param_bounds: Vec::new(),
            modifiers: Vec::new(),
            extends: None,
            implements: Vec::new(),
            permits: Vec::new(),
            members: Vec::new(),
            static_init_blocks: Vec::new(),
            span: Default::default(),
        };
        let body = constructor_return_body(
            &ty,
            vec![
                ("first".to_string(), RustExpr::Var("value".to_string())),
                (
                    "second".to_string(),
                    RustExpr::Block {
                        stmts: vec![RustStmt::Raw("helper(value);".to_string())],
                        tail: None,
                    },
                ),
            ],
        );

        let [RustStmt::Return(Some(RustExpr::StructLit { fields, .. }))] = body.as_slice() else {
            panic!("constructor return body should be a struct literal return: {body:?}");
        };
        assert_eq!(fields[0].0, "second");
        assert_eq!(fields[1].0, "first");
        let RustExpr::Block { stmts, .. } = &fields[0].1 else {
            panic!(
                "second field should stay block-valued after recovery: {:?}",
                fields[0].1
            );
        };
        assert!(matches!(
            stmts.as_slice(),
            [RustStmt::Expr(RustExpr::Call { args, .. })]
                if matches!(args.as_slice(), [RustExpr::Var(name)] if name == "value")
        ));
    }

    #[test]
    fn raw_reads_require_structured_recovery() {
        let structured = RustExpr::Binary {
            op: "+".to_string(),
            left: Box::new(RustExpr::Var("left".to_string())),
            right: Box::new(RustExpr::Var("right".to_string())),
        };
        assert!(rust_expr_reads_var(&structured, "left"));
        assert!(!rust_expr_reads_var(
            &RustExpr::Raw("left + right".to_string()),
            "left"
        ));
        assert!(!rust_expr_reads_var(
            &RustExpr::Raw("\"left\"".to_string()),
            "left"
        ));
        assert!(!rust_expr_reads_var(
            &RustExpr::Raw("left +".to_string()),
            "left"
        ));
        assert!(!rust_stmt_reads_var(
            &RustStmt::Raw("return left + right;".to_string()),
            "right"
        ));
        assert!(!rust_stmt_reads_var(
            &RustStmt::Raw("return left +;".to_string()),
            "left"
        ));
    }
}

pub(crate) fn constructor_default_field_value_with_facts(
    field_name: String,
    shape: RustSlotShape,
    facts: &FactSet,
) -> (String, RustExpr) {
    (field_name, default_expr_for_slot_with_facts(&shape, facts))
}

pub(crate) fn static_storage_initial_fallback_value(
    shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    default_expr_for_slot_with_facts(shape, facts)
}

pub(crate) fn delegating_constructor_return_body(
    rust_name: &str,
    args: Vec<RustExpr>,
) -> Vec<RustStmt> {
    vec![RustStmt::Return(Some(function_call_projection(
        self_associated_path_projection(rust_name),
        args,
    )))]
}

#[cfg(test)]
pub(crate) fn method_function_shell_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
) -> RustFunction {
    method_function_shell_with_facts_and_exception_plan(
        ty,
        method,
        signatures,
        body,
        facts,
        MethodExceptionPlan::new(false),
    )
}

#[cfg(test)]
fn method_function_shell_with_facts_and_exception_plan(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
) -> RustFunction {
    method_function_shell_with_facts_exception_plan_and_support_context(
        ty,
        method,
        signatures,
        body,
        facts,
        exceptions,
        &Program::default(),
    )
}

#[cfg(test)]
fn method_function_shell_with_facts_exception_plan_and_support_context(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    support_program: &Program,
) -> RustFunction {
    method_function_shell_with_facts_exception_plan_and_support_contexts(
        ty,
        method,
        signatures,
        body,
        facts,
        exceptions,
        &[support_program],
        None,
        None,
    )
}

pub(crate) fn method_function_shell_with_facts_exception_plan_and_support_contexts(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    support_programs: &[&Program],
    representations: Option<&RepresentationPlan>,
    clone_requirement_cache: Option<&CloneRequirementCache>,
) -> RustFunction {
    let needs_lifetime = method_needs_borrowed_return_lifetime(ty, method, signatures, facts);
    let retained_type_param_indices = method
        .type_params
        .iter()
        .enumerate()
        .filter_map(|(index, type_param)| {
            (!method_type_param_is_checked_exception_only(method, index, type_param))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let retained_type_params = retained_type_param_indices
        .iter()
        .map(|index| method.type_params[*index].clone())
        .collect::<Vec<_>>();
    let retained_type_param_bounds = retained_type_param_indices
        .iter()
        .map(|index| {
            method
                .type_param_bounds
                .get(*index)
                .cloned()
                .unwrap_or(None)
        })
        .collect::<Vec<_>>();
    let name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    RustFunction {
        name,
        type_params: function_type_params_with_optional_lifetime_and_support_contexts(
            Some(&method.name),
            &retained_type_params,
            &retained_type_param_bounds,
            needs_lifetime,
            support_programs,
            &method.params,
            method.body.as_ref(),
        ),
        params: method_params_with_optional_lifetime(
            ty,
            method,
            signatures,
            facts,
            needs_lifetime,
            representations,
        ),
        return_type: method_return_type_with_optional_lifetime(
            ty,
            method,
            signatures,
            facts,
            exceptions,
            needs_lifetime,
            representations,
        ),
        where_bounds: clone_requirement_cache.map_or_else(
            || {
                source_method_class_type_param_where_bounds(
                    ty,
                    method,
                    representations,
                    support_programs,
                )
            },
            |cache| {
                source_method_class_type_param_where_bounds_with_cache(
                    ty,
                    method,
                    representations,
                    support_programs,
                    cache,
                )
            },
        ),
        body,
        is_pub: true,
        is_pub_crate: false,
        receiver: lower_receiver(method_receiver(ty, method, signatures)),
    }
}

fn method_type_param_is_checked_exception_only(
    method: &MethodDecl,
    index: usize,
    type_param: &str,
) -> bool {
    let bound_is_throwable = method
        .type_param_bounds
        .get(index)
        .and_then(|bound| bound.as_ref())
        .is_some_and(|bound| {
            matches!(
                bound,
                JavaType::Named { name, .. }
                    if crate::stdlib::canonical_external_receiver_type_key(name)
                        .rsplit('.')
                        .next()
                        == Some("Throwable")
            )
        });
    bound_is_throwable
        && !java_type_contains_type_param(&method.return_type, type_param)
        && method
            .params
            .iter()
            .all(|param| !java_type_contains_type_param(&param.ty, type_param))
}

pub(crate) fn source_method_class_type_param_where_bounds(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: Option<&RepresentationPlan>,
    support_programs: &[&Program],
) -> Vec<String> {
    let cache = CloneRequirementCache::default();
    source_method_class_type_param_where_bounds_with_cache(
        ty,
        method,
        representations,
        support_programs,
        &cache,
    )
}

pub(crate) fn source_method_class_type_param_where_bounds_with_cache(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: Option<&RepresentationPlan>,
    support_programs: &[&Program],
    cache: &CloneRequirementCache,
) -> Vec<String> {
    let uses_shared_storage = representations.is_some_and(|representations| {
        let placement = representations.storage_placement_for(&ty.name);
        placement.shared() && placement.is_implemented()
    });
    let mut visiting = cache.search.borrow_mut();
    let fields = visiting.inherited_fields(ty, support_programs);
    ty.type_params
        .iter()
        .filter(|type_param| {
            source_method_class_type_param_needs_clone_bound(
                ty,
                method,
                type_param,
                uses_shared_storage,
                &fields,
                support_programs,
                &mut visiting,
            )
        })
        .map(|type_param| format!("{type_param}: Clone"))
        .collect()
}

fn source_method_class_type_param_needs_clone_bound(
    ty: &TypeDecl,
    method: &MethodDecl,
    type_param: &str,
    uses_shared_storage: bool,
    fields: &HashMap<String, JavaType>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    if let Some((_, _, _, needs_clone)) =
        visiting
            .method_memo
            .iter()
            .find(|(owner, candidate, candidate_type_param, _)| {
                owner == &ty.name && candidate == method && candidate_type_param == type_param
            })
    {
        return *needs_clone;
    }
    if visiting
        .method_visiting
        .iter()
        .any(|(owner, candidate, candidate_type_param)| {
            owner == &ty.name && candidate == method && candidate_type_param == type_param
        })
    {
        return false;
    }
    visiting
        .method_visiting
        .push((ty.name.clone(), method.clone(), type_param.to_string()));
    let needs_clone = callable_type_param_needs_clone_bound_with_support_contexts_inner(
        type_param,
        &method.params,
        method.body.as_ref(),
        support_programs,
        visiting,
    ) || source_callable_delegates_class_type_param_clone_requirement(
        ty,
        &method.params,
        method.body.as_ref(),
        type_param,
        support_programs,
        visiting,
    ) || (!uses_shared_storage
        && java_type_contains_type_param(&method.return_type, type_param)
        && method.body.as_ref().is_some_and(block_returns_this))
        || source_method_clones_class_type_param_param_to_owned_local(method, type_param)
        || (fields
            .values()
            .any(|field_ty| java_type_contains_type_param(field_ty, type_param))
            && source_method_needs_class_type_param_clone_bound(method, type_param, fields));
    visiting.method_visiting.pop();
    visiting.method_memo.push((
        ty.name.clone(),
        method.clone(),
        type_param.to_string(),
        needs_clone,
    ));
    needs_clone
}

fn expr_constructs_clone_requiring_generic_owner(
    expr: &Expr,
    caller_type_param: &str,
    caller_types: &TypeEnv,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    let Expr::New {
        args,
        resolved,
        ty: constructed_type,
        ..
    } = expr
    else {
        return false;
    };
    let Some((owner, constructor, constructor_rust_name)) =
        source_constructor_for_capability_requirement(
            resolved.as_ref(),
            constructed_type,
            args.len(),
            support_programs,
        )
    else {
        return false;
    };
    owner.type_params.iter().any(|callee_type_param| {
        let receives_caller_type_param =
            args.iter()
                .zip(&constructor.params)
                .any(|(argument, target_param)| {
                    java_type_contains_type_param(
                        &infer_expr_type(argument, caller_types),
                        caller_type_param,
                    ) && java_type_contains_type_param(&target_param.ty, callee_type_param)
                });
        if !receives_caller_type_param {
            return false;
        }
        let key = (
            owner.name.clone(),
            constructor_rust_name.clone(),
            callee_type_param.clone(),
        );
        if !visiting.insert(key.clone()) {
            return false;
        }
        let needs_clone = source_constructor_class_type_param_needs_clone_bound(
            owner,
            constructor,
            callee_type_param,
            support_programs,
            visiting,
        );
        visiting.remove(&key);
        needs_clone
    })
}

fn source_callable_delegates_class_type_param_clone_requirement(
    ty: &TypeDecl,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    type_param: &str,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    let Some(body) = body else {
        return false;
    };
    let Some(type_param_index) = ty.type_params.iter().position(|name| name == type_param) else {
        return false;
    };
    let caller_types = source_callable_type_env(ty, params);
    let mut needs_clone = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if needs_clone {
            return;
        }
        if expr_constructs_clone_requiring_generic_owner(
            expr,
            type_param,
            &caller_types,
            support_programs,
            visiting,
        ) {
            needs_clone = true;
            return;
        }
        let Expr::Call {
            receiver,
            args,
            name,
            resolved,
            ..
        } = expr
        else {
            return;
        };
        let Some((mut owner, mut callee, callee_rust_name)) =
            source_call_for_capability_requirement(
                resolved.as_ref(),
                receiver.as_deref(),
                name,
                args.len(),
                &caller_types,
                support_programs,
            )
        else {
            return;
        };
        let current_receiver = receiver
            .as_deref()
            .is_none_or(|receiver| matches!(receiver, Expr::This));
        if current_receiver {
            // A source body inherited from a superclass still dispatches
            // virtual calls through the concrete child.  Analyze the method
            // that will actually occupy that child slot, rather than only the
            // declaration selected while parsing the superclass body.
            if let Some((dispatch_owner, dispatch_method)) =
                effective_inherited_current_receiver_method(
                    ty,
                    &callee_rust_name,
                    args.len(),
                    support_programs,
                    visiting,
                )
            {
                owner = dispatch_owner;
                callee = dispatch_method;
            }
        }
        let callee_type_params = if current_receiver && owner.name == ty.name {
            owner
                .type_params
                .get(type_param_index)
                .into_iter()
                .collect::<Vec<_>>()
        } else if current_receiver {
            // An unqualified call may resolve to a method materialized from a
            // generic source superclass. Map the child's type parameter
            // through the declared ancestor arguments before asking which
            // capability the inherited method requires. Treating this as an
            // unrelated receiver drops bounds such as `C: Clone` from
            // `Child<S, C>::set_query -> Base<S, C>::reset`.
            let substitutions = support_programs.iter().find_map(|program| {
                crate::inherited_members::source_class_ancestor_type_arguments(program, ty, owner)
            });
            let Some(substitutions) = substitutions else {
                return;
            };
            owner
                .type_params
                .iter()
                .filter(|callee_type_param| {
                    substitutions
                        .get(*callee_type_param)
                        .is_some_and(|argument| java_type_contains_type_param(argument, type_param))
                })
                .collect::<Vec<_>>()
        } else {
            let Some(receiver) = receiver.as_deref() else {
                return;
            };
            let receiver_ty = infer_expr_type(receiver, &caller_types);
            let JavaType::Named { name, args } = receiver_ty else {
                return;
            };
            let Some(receiver_owner) = source_type_bound_decl(&name, support_programs) else {
                return;
            };
            if receiver_owner.name != owner.name {
                return;
            }
            owner
                .type_params
                .iter()
                .zip(args.iter())
                .filter_map(|(callee_type_param, argument)| {
                    java_type_contains_type_param(argument, type_param).then_some(callee_type_param)
                })
                .collect::<Vec<_>>()
        };
        let fields = visiting.inherited_fields(owner, support_programs);
        needs_clone = callee_type_params.into_iter().any(|callee_type_param| {
            let analyze_concrete_receiver = current_receiver && owner.name != ty.name;
            if analyze_concrete_receiver && callee_type_param == type_param {
                // With the same source type-parameter name, the concrete
                // hierarchy analysis subsumes the lexical-owner analysis and
                // additionally follows deeper virtual dispatch. Avoid
                // traversing both equivalent call trees: large inherited
                // templates otherwise branch exponentially.
                let concrete_key = (
                    ty.name.clone(),
                    callee_rust_name.clone(),
                    type_param.to_string(),
                );
                if !visiting.insert(concrete_key.clone()) {
                    return false;
                }
                let concrete_fields = visiting.inherited_fields(ty, support_programs);
                let needs_clone = source_method_class_type_param_needs_clone_bound(
                    ty,
                    callee,
                    type_param,
                    false,
                    &concrete_fields,
                    support_programs,
                    visiting,
                );
                visiting.remove(&concrete_key);
                return needs_clone;
            }
            let key = (
                owner.name.clone(),
                callee_rust_name.clone(),
                callee_type_param.clone(),
            );
            if !visiting.insert(key.clone()) {
                return false;
            }
            let needs_clone = source_method_class_type_param_needs_clone_bound(
                owner,
                callee,
                callee_type_param,
                false,
                &fields,
                support_programs,
                visiting,
            );
            visiting.remove(&key);
            if needs_clone {
                return true;
            }
            if !analyze_concrete_receiver {
                return false;
            }

            // The selected implementation can itself contain virtual calls.
            // Analyze that copied body once more in the concrete receiver's
            // hierarchy so requirements discovered in a deeper override flow
            // through every inherited caller. Keep the lexical-owner pass
            // above as well: its source type-parameter names may differ from
            // the concrete child's names.
            let concrete_key = (
                ty.name.clone(),
                callee_rust_name.clone(),
                type_param.to_string(),
            );
            if !visiting.insert(concrete_key.clone()) {
                return false;
            }
            let concrete_fields = visiting.inherited_fields(ty, support_programs);
            let needs_clone = source_method_class_type_param_needs_clone_bound(
                ty,
                callee,
                type_param,
                false,
                &concrete_fields,
                support_programs,
                visiting,
            );
            visiting.remove(&concrete_key);
            needs_clone
        });
    });
    needs_clone
}

fn effective_inherited_current_receiver_method<'a>(
    ty: &'a TypeDecl,
    rust_name: &str,
    arg_count: usize,
    support_programs: &[&'a Program],
    search: &mut CloneRequirementSearch,
) -> Option<(&'a TypeDecl, &'a MethodDecl)> {
    if let Some(method) = ty.members.iter().find_map(|member| match member {
        Member::Method(method)
            if crate::java_signature::source_method_rust_name(method) == rust_name
                && method.params.len() == arg_count =>
        {
            Some(method)
        }
        _ => None,
    }) {
        return Some((ty, method));
    }
    let key = (ty.name.clone(), rust_name.to_string(), arg_count);
    if let Some(cached) = search.inherited_dispatch.get(&key).cloned() {
        return cached.and_then(|(owner_name, method_name, span_start, span_end)| {
            let owner = source_type_bound_decl(&owner_name, support_programs)?;
            let method = owner.members.iter().find_map(|member| match member {
                Member::Method(method)
                    if method.name == method_name
                        && method.span.start() == span_start
                        && method.span.end() == span_end =>
                {
                    Some(method)
                }
                _ => None,
            })?;
            Some((owner, method))
        });
    }
    let resolved = support_programs.iter().find_map(|program| {
        crate::inherited_members::inherited_instance_method_refs_for_type(ty, program)
            .into_iter()
            .find(|inherited| {
                inherited.rust_name() == rust_name && inherited.method().params.len() == arg_count
            })
            .map(|inherited| (inherited.declaring_ty(), inherited.method()))
    });
    search.inherited_dispatch.insert(
        key,
        resolved.map(|(owner, method)| {
            (
                owner.name.clone(),
                method.name.clone(),
                method.span.start(),
                method.span.end(),
            )
        }),
    );
    resolved
}

fn source_call_for_capability_requirement<'a>(
    resolved: Option<&crate::java_ir::ResolvedCall>,
    receiver: Option<&Expr>,
    source_name: &str,
    arg_count: usize,
    caller_types: &TypeEnv,
    support_programs: &[&'a Program],
) -> Option<(&'a TypeDecl, &'a MethodDecl, String)> {
    if let Some(resolved) = resolved {
        let owner = source_type_bound_decl(&resolved.declaring_type, support_programs)?;
        let method = owner.members.iter().find_map(|member| match member {
            Member::Method(method)
                if crate::java_signature::source_method_rust_name(method) == resolved.rust_name
                    && method.params.len() == arg_count =>
            {
                Some(method)
            }
            _ => None,
        })?;
        return Some((owner, method, resolved.rust_name.clone()));
    }

    // Resolution can deliberately remain conservative for a complex generic
    // receiver even though lowering can still use a unique source member.  Use
    // the same canonical Java-to-Rust method-name policy here and refuse
    // overload ambiguity rather than inventing a second name mapping.
    let receiver_ty = infer_expr_type(receiver?, caller_types);
    let JavaType::Named { name, .. } = receiver_ty else {
        return None;
    };
    let owner = source_type_bound_decl(&name, support_programs)?;
    let mut candidates = owner.members.iter().filter_map(|member| match member {
        Member::Method(method)
            if method.params.len() == arg_count
                && (method.name == source_name
                    || crate::java_signature::source_method_rust_name(method) == source_name) =>
        {
            Some(method)
        }
        _ => None,
    });
    let method = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let rust_name = crate::java_signature::source_method_rust_name(method);
    Some((owner, method, rust_name))
}

fn source_constructor_for_capability_requirement<'a>(
    resolved: Option<&crate::java_ir::ResolvedConstructor>,
    constructed_type: &JavaType,
    arg_count: usize,
    support_programs: &[&'a Program],
) -> Option<(&'a TypeDecl, &'a ConstructorDecl, String)> {
    if let Some(resolved) = resolved {
        let owner = source_type_bound_decl(&resolved.declaring_type, support_programs)?;
        let constructor = owner.members.iter().find_map(|member| match member {
            Member::Constructor(constructor)
                if constructor.rust_name.as_deref().unwrap_or("new") == resolved.rust_name
                    && constructor.params.len() == arg_count =>
            {
                Some(constructor)
            }
            _ => None,
        })?;
        return Some((owner, constructor, resolved.rust_name.clone()));
    }

    let JavaType::Named { name, .. } = constructed_type else {
        return None;
    };
    let owner = source_type_bound_decl(name, support_programs)?;
    let mut candidates = owner.members.iter().filter_map(|member| match member {
        Member::Constructor(constructor) if constructor.params.len() == arg_count => {
            Some(constructor)
        }
        _ => None,
    });
    let constructor = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let rust_name = constructor
        .rust_name
        .clone()
        .unwrap_or_else(|| "new".to_string());
    Some((owner, constructor, rust_name))
}

/// Builds the lexical type environment used when class-level capability
/// obligations follow a resolved call through its receiver.  Receiver typing
/// belongs to the same `TypeEnv`/`infer_expr_type` path as ordinary semantic
/// analysis; keeping a second field-only lookup here made parameter receivers
/// and renamed parameters disagree with the resolver.
fn source_callable_type_env(ty: &TypeDecl, params: &[crate::java_ir::Param]) -> TypeEnv {
    let mut env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
        }
    }
    for param in params {
        env.insert_param(param.source_name.clone(), param.ty.clone());
        if param.name != param.source_name {
            env.insert_param(param.name.clone(), param.ty.clone());
        }
    }
    env
}

fn source_constructor_class_type_param_where_bounds(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    support_programs: &[&Program],
) -> Vec<String> {
    let mut visiting = CloneRequirementSearch::default();
    ty.type_params
        .iter()
        .filter(|type_param| {
            source_constructor_class_type_param_needs_clone_bound(
                ty,
                ctor,
                type_param,
                support_programs,
                &mut visiting,
            )
        })
        .map(|type_param| format!("{type_param}: Clone"))
        .collect()
}

fn source_constructor_class_type_param_needs_clone_bound(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    type_param: &str,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    callable_type_param_needs_clone_bound_with_support_contexts_inner(
        type_param,
        &ctor.params,
        ctor.body.as_ref(),
        support_programs,
        visiting,
    ) || source_callable_delegates_class_type_param_clone_requirement(
        ty,
        &ctor.params,
        ctor.body.as_ref(),
        type_param,
        support_programs,
        visiting,
    )
}

fn method_needs_borrowed_return_lifetime(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> bool {
    if !method_return_type_with_facts(ty, method, signatures, facts)
        .as_ref()
        .is_some_and(rust_type_contains_ref)
        || !method_params(ty, method, signatures, facts)
            .iter()
            .any(|param| rust_type_contains_ref(&param.ty))
    {
        return false;
    }
    method.modifiers.contains(&Modifier::Static) || method_returns_source_param(method)
}

fn method_returns_source_param(method: &MethodDecl) -> bool {
    let param_names = method
        .params
        .iter()
        .flat_map(|param| [param.name.as_str(), param.source_name.as_str()])
        .collect::<std::collections::HashSet<_>>();
    method
        .body
        .as_ref()
        .is_some_and(|body| block_returns_any_param(body, &param_names))
}

fn block_returns_any_param(block: &Block, param_names: &std::collections::HashSet<&str>) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_returns_any_param(stmt, param_names))
}

fn stmt_returns_any_param(stmt: &Stmt, param_names: &std::collections::HashSet<&str>) -> bool {
    match stmt {
        Stmt::Return(Some(expr)) => expr_returns_any_param(expr, param_names),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_returns_any_param(then_block, param_names)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_returns_any_param(block, param_names))
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_returns_any_param(block, param_names)
        }
        Stmt::While { body, .. } | Stmt::DoWhile { body, .. } => {
            block_returns_any_param(body, param_names)
        }
        Stmt::For { init, body, .. } => {
            init.iter()
                .any(|stmt| stmt_returns_any_param(stmt, param_names))
                || block_returns_any_param(body, param_names)
        }
        _ => false,
    }
}

fn expr_returns_any_param(expr: &Expr, param_names: &std::collections::HashSet<&str>) -> bool {
    match expr {
        Expr::Var(name) => param_names.contains(name.as_str()),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            expr_returns_any_param(then_expr, param_names)
                || expr_returns_any_param(else_expr, param_names)
        }
        Expr::Cast { expr, .. } => expr_returns_any_param(expr, param_names),
        _ => false,
    }
}

fn method_return_shape(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
) -> Option<RustSlotShape> {
    signatures
        .required_method_signature(ty, method)
        .return_shape()
        .cloned()
}

fn method_return_type_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Option<RustType> {
    if let Some(ty) = rust_crate_mapped_external_rust_type(&method.return_type, facts) {
        return Some(ty);
    }
    let return_shape = method_return_shape(ty, method, signatures);
    if let Some(shape) = return_shape.as_ref() {
        return Some(rust_type_for_slot_with_facts(shape, facts));
    }
    if rust_crate_mapped_external_java_type(&method.return_type, facts) {
        return Some(rust_type_for_java_type_with_facts(
            &method.return_type,
            facts,
        ));
    }
    None
}

pub(crate) fn rust_crate_mapped_external_rust_type(
    java_ty: &JavaType,
    facts: &FactSet,
) -> Option<RustType> {
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = java_ty else {
        return None;
    };
    let rust_type_name = facts.external_crate_rust_type_name_for(name)?;
    let pack = facts.external_library_packs.iter().rev().find(|pack| {
        pack.mode == crate::facts::ExternalLibraryMode::RustCrateMapping
            && (pack.package_prefix == *name
                || crate::facts::external_type_name_matches(&pack.package_prefix, name))
    })?;
    let crate_name = pack.rust_target.as_deref()?;
    if let JavaType::Named { args, .. } = java_ty
        && !args.is_empty()
    {
        let args = args
            .iter()
            .map(|arg| rust_type_for_java_type_with_facts(arg, facts))
            .collect();
        return Some(crate::slot_render::rust_generic_type(
            format!("{crate_name}::{rust_type_name}"),
            args,
        ));
    }
    Some(RustType::Named(format!("{crate_name}::{rust_type_name}")))
}

fn rust_crate_mapped_external_java_type(java_ty: &JavaType, facts: &FactSet) -> bool {
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = java_ty else {
        return false;
    };
    facts
        .external_library_pack_for_type(name)
        .is_some_and(|pack| pack.mode == crate::facts::ExternalLibraryMode::RustCrateMapping)
}

fn struct_fields_for_type(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
    support_program: &Program,
) -> Vec<RustField> {
    let mut fields = Vec::new();
    // A Java monitor belongs to each object, not to the class.  Keep it out of
    // ordinary source-field planning: it is runtime metadata that is seeded by
    // every constructor below, and `JavaMonitor`'s trait implementations make
    // it neutral to the structural derives used by generated source structs.
    if type_declares_synchronized_instance_method(ty) {
        fields.push(RustField {
            name: "__java_monitor".to_string(),
            ty: RustType::Raw("std::sync::Arc<JavaMonitor>".to_string()),
            is_pub: false,
            is_pub_crate: false,
        });
    }
    if let Some(outer_type) = facts.captures.outer_this_type_for_type(&ty.name) {
        let outer_carrier = representations.and_then(|representations| {
            representations
                .type_representation(outer_type)
                .and_then(|representation| match representation.representation() {
                    crate::representation::ObjectRepresentation::SourceHierarchyCarrier {
                        carrier_name,
                        ..
                    } if carrier_name != &rust_name_for_java_named_type(outer_type) => {
                        Some(RustType::Named(carrier_name.clone()))
                    }
                    _ => None,
                })
        });
        let outer_rust_type = outer_carrier.unwrap_or_else(|| {
            let outer_rust_name = rust_name_for_java_named_type(outer_type);
            find_type_decl_by_rust_name(support_program, &outer_rust_name)
                .filter(|outer| !outer.type_params.is_empty())
                .map(|outer| {
                    crate::slot_render::rust_generic_type(
                        outer_rust_name.clone(),
                        outer
                            .type_params
                            .iter()
                            .cloned()
                            .map(RustType::Named)
                            .collect(),
                    )
                })
                .unwrap_or_else(|| RustType::Named(outer_rust_name))
        });
        let outer_rust_type = if representations
            .and_then(|representations| representations.type_representation(outer_type))
            .is_some_and(|representation| {
                matches!(
                    representation.representation(),
                    crate::representation::ObjectRepresentation::SourceHierarchyCarrier {
                        carrier_name,
                        ..
                    } if carrier_name != &rust_name_for_java_named_type(outer_type)
                )
            }) {
            outer_rust_type
        } else {
            shared_outer_handle_type(facts.storage_placement_for(outer_type), outer_rust_type)
        };
        fields.push(RustField {
            name: "__outer".to_string(),
            ty: outer_rust_type,
            is_pub: false,
            is_pub_crate: false,
        });
    }
    let upcast_visible_fields = source_type_has_context_subtypes(ty, support_program);
    let carrier_visible_fields = representations.is_some_and(|representations| {
        let emitted_name = rust_name_for_java_named_type(&ty.name);
        representations.type_representations().any(|entry| {
            matches!(
                entry.representation(),
                crate::representation::ObjectRepresentation::SourceHierarchyCarrier {
                    variants,
                    ..
                } if variants.iter().any(|variant| {
                    variant == &ty.name
                        || rust_name_for_java_named_type(variant) == emitted_name
                })
            )
        })
    });
    // Java nested classes are nestmates: they may access private fields of
    // their enclosing class. Flattening nested declarations into sibling Rust
    // structs therefore needs crate visibility for the enclosing storage.
    // `pub(crate)` preserves external encapsulation while allowing the access
    // Java's type checker already validated inside this generated crate.
    let nestmate_visible_fields = ty
        .members
        .iter()
        .any(|member| matches!(member, Member::Type(_)));
    let source_field_rust_names = crate::java_naming::source_instance_field_rust_names(ty);
    fields.extend(ty.members.iter().filter_map(|member| {
        match member {
            Member::Field(field) if !is_static_field(field) => Some(RustField {
                name: source_field_rust_names
                    .get(&field.name)
                    .cloned()
                    .unwrap_or_else(|| field.name.clone()),
                ty: rust_type_for_instance_field(
                    ty,
                    &field.name,
                    &signatures.required_declared_field_shape(ty, field),
                    facts,
                    representations,
                ),
                is_pub: source_field_is_public(field),
                is_pub_crate: !source_field_is_public(field)
                    && (!source_field_is_private(field)
                        || upcast_visible_fields
                        || carrier_visible_fields
                        || nestmate_visible_fields),
            }),
            _ => None,
        }
    }));
    fields.extend(
        crate::inherited_members::inherited_instance_fields_for_type(ty, support_program)
            .into_iter()
            .map(|inherited| RustField {
                name: inherited.rust_name().to_string(),
                ty: rust_type_for_slot_maybe_with_representations(
                    &signatures.required_field_shape(&ty.name, inherited.rust_name()),
                    facts,
                    representations,
                ),
                is_pub: false,
                is_pub_crate: upcast_visible_fields || carrier_visible_fields,
            }),
    );
    fields.extend(
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
            .into_iter()
            .map(|inherited| RustField {
                name: inherited.rust_name().to_string(),
                ty: {
                    let shape = signatures.required_field_shape(&ty.name, inherited.rust_name());
                    if inherited.rust_name() == "__java_super" {
                        // The carrier is an exact base-class subobject.  Do
                        // not re-project it through the open-hierarchy
                        // representation chosen for ordinary Java references.
                        match inherited.field_type() {
                            JavaType::Named { name, .. } | JavaType::Unknown(name) => {
                                RustType::Named(rust_name_for_java_named_type(name))
                            }
                            _ => rust_type_for_slot_with_facts(&shape, facts),
                        }
                    } else {
                        rust_type_for_slot_maybe_with_representations(
                            &shape,
                            facts,
                            representations,
                        )
                    }
                },
                is_pub: false,
                // External-super storage is synthetic object-model state.
                // Conversion impls are package support and may be hoisted to
                // the crate root during multi-file assembly, so they need
                // crate visibility even when there are no source subtypes in
                // the current analysis unit.
                is_pub_crate: true,
            }),
    );
    let mut seen = HashSet::new();
    fields.retain(|field| seen.insert(field.name.clone()));
    if source_type_needs_type_param_marker_field(ty)
        && fields.iter().all(|field| field.name != "__marker")
    {
        fields.push(RustField {
            name: "__marker".to_string(),
            ty: type_param_marker_type(&ty.type_params),
            is_pub: false,
            is_pub_crate: false,
        });
    }
    fields
}

pub(crate) fn type_declares_synchronized_instance_method(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Method(method) => {
            (!method.modifiers.contains(&Modifier::Static)
                && method.modifiers.contains(&Modifier::Synchronized))
                || method
                    .body
                    .as_ref()
                    .is_some_and(block_contains_synchronized_this)
        }
        Member::Constructor(constructor) => constructor
            .body
            .as_ref()
            .is_some_and(block_contains_synchronized_this),
        _ => false,
    })
}

pub(crate) fn block_contains_synchronized_this(block: &Block) -> bool {
    block.statements.iter().any(stmt_contains_synchronized_this)
}

fn stmt_contains_synchronized_this(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::LabeledBlock { label, .. } if label == "__java_synchronized_this" => true,
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_contains_synchronized_this(block)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            block_contains_synchronized_this(then_block)
                || else_block
                    .as_ref()
                    .is_some_and(block_contains_synchronized_this)
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::EnhancedFor { body, .. } => block_contains_synchronized_this(body),
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_contains_synchronized_this)),
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources.iter().any(stmt_contains_synchronized_this)
                || block_contains_synchronized_this(body)
                || catches
                    .iter()
                    .any(|catch| block_contains_synchronized_this(&catch.body))
                || finally_block
                    .as_ref()
                    .is_some_and(block_contains_synchronized_this)
        }
        _ => false,
    }
}

pub(crate) fn type_declares_synchronized_static_method(ty: &TypeDecl) -> bool {
    ty.members.iter().any(|member| {
        matches!(member, Member::Method(method)
            if method.modifiers.contains(&Modifier::Synchronized)
                && method.modifiers.contains(&Modifier::Static))
    })
}

fn source_type_needs_type_param_marker_field(ty: &TypeDecl) -> bool {
    !ty.type_params.is_empty() && matches!(ty.kind, TypeKind::Class | TypeKind::Record)
}

fn type_param_marker_type(type_params: &[String]) -> RustType {
    let return_type = if type_params.len() == 1 {
        RustType::Named(type_params[0].clone())
    } else {
        RustType::Tuple(
            type_params
                .iter()
                .map(|type_param| RustType::Named(type_param.clone()))
                .collect(),
        )
    };
    crate::slot_render::rust_generic_type(
        "std::marker::PhantomData",
        vec![RustType::FnPointer {
            params: Vec::new(),
            return_type: Box::new(return_type),
        }],
    )
}

fn rust_type_for_instance_field(
    owner: &TypeDecl,
    field_name: &str,
    shape: &RustSlotShape,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> RustType {
    let rust_ty = rust_type_for_slot_maybe_with_representations(shape, facts, representations);
    let rust_ty = if let Some(semantic_base) =
        java_semantic_hash_collection_for_source_field(owner, field_name)
    {
        java_semantic_hash_collection_field_rust_type(rust_ty, semantic_base).unwrap_or_else(|| {
            panic!(
                "Java semantic hash field must render as a supported collection: {}::{field_name}",
                owner.name
            )
        })
    } else if representations.is_some_and(|representations| {
        representations.field_needs_live_native_map(&owner.name, field_name)
    }) {
        live_native_map_field_rust_type(
            rust_ty,
            representations.is_some_and(|representations| {
                representations.live_native_map_field_has_nullable_key(&owner.name, field_name)
            }),
            representations.is_some_and(|representations| {
                representations.live_native_map_field_has_nullable_value(&owner.name, field_name)
            }),
        )
        .unwrap_or_else(|| {
            panic!(
                "live native-map field must render as a supported concrete map: {}::{field_name}",
                owner.name
            )
        })
    } else {
        rust_ty
    };
    if instance_field_is_self_referential(owner, field_name, shape)
        && !field_type_is_shared_handle(shape, facts)
    {
        return box_recursive_field_type(rust_ty);
    }
    rust_ty
}

/// Selects the Java-equality collection representation when a hash key is an
/// owner type parameter. Such a parameter may be instantiated with a source
/// object whose inherited `Object.equals` is identity; requiring Rust `Eq +
/// Hash` would either reject the legal Java program or silently change it to
/// structural equality.
pub(crate) fn java_semantic_hash_collection_for_source_field(
    owner: &TypeDecl,
    field_name: &str,
) -> Option<&'static str> {
    let field = owner.members.iter().find_map(|member| match member {
        Member::Field(field)
            if field.name == field_name
                || crate::parser::to_snake_case(&field.name) == field_name =>
        {
            Some(field)
        }
        _ => None,
    })?;
    let JavaType::Named { name, args } = &field.ty else {
        return None;
    };
    let simple = name.rsplit('.').next().unwrap_or(name);
    let key = match simple {
        "Map" | "HashMap" | "Hashtable" | "ConcurrentHashMap" => args.first()?,
        "Set" | "HashSet" => args.first()?,
        _ => return None,
    };
    if !owner
        .type_params
        .iter()
        .any(|param| java_type_contains_type_param(key, param))
    {
        return None;
    }
    Some(match simple {
        "Set" | "HashSet" => "JavaSemanticHashSet",
        _ => "JavaSemanticHashMap",
    })
}

fn java_semantic_hash_collection_field_rust_type(
    base: RustType,
    semantic_base: &str,
) -> Option<RustType> {
    let RustType::Generic { args, .. } = base else {
        return None;
    };
    Some(crate::slot_render::rust_generic_type(semantic_base, args))
}

fn live_native_map_field_rust_type(
    base: RustType,
    nullable_key: bool,
    nullable_value: bool,
) -> Option<RustType> {
    let RustType::Generic { base, args } = base else {
        return None;
    };
    if args.len() != 2 {
        return None;
    }
    let tracked_base = if base.ends_with("HashMap") && !base.ends_with("LinkedHashMap") {
        "JavaTrackedHashMap"
    } else if base.ends_with("BTreeMap") {
        "JavaTrackedTreeMap"
    } else if base.ends_with("JavaLinkedHashMap") {
        "JavaLinkedHashMap"
    } else {
        return None;
    };
    let mut args = args;
    if nullable_key && !matches!(args[0], RustType::Option(_)) {
        args[0] = RustType::Option(Box::new(args[0].clone()));
    }
    if nullable_value && !matches!(args[1], RustType::Option(_)) {
        args[1] = RustType::Option(Box::new(args[1].clone()));
    }
    Some(crate::slot_render::rust_generic_type(
        "std::sync::Arc",
        vec![crate::slot_render::rust_generic_type(
            "std::sync::RwLock",
            vec![crate::slot_render::rust_generic_type(tracked_base, args)],
        )],
    ))
}

fn rust_type_for_slot_maybe_with_representations(
    shape: &RustSlotShape,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> RustType {
    representations
        .map(|representations| {
            rust_type_for_slot_with_facts_and_representations(shape, facts, representations)
        })
        .unwrap_or_else(|| rust_type_for_slot_with_facts(shape, facts))
}

/// A handle is already an indirection, so boxing one gives `Option<Box<Rc<T>>>`.
fn field_type_is_shared_handle(shape: &RustSlotShape, facts: &FactSet) -> bool {
    let java_ty = shape
        .optional_element_type()
        .unwrap_or_else(|| shape.java_ty.clone());
    let JavaType::Named { name, .. } = java_ty else {
        return false;
    };
    let placement = facts.storage_placement_for(&name);
    placement.shared() && placement.is_implemented()
}

fn instance_field_is_self_referential(
    owner: &TypeDecl,
    field_name: &str,
    shape: &RustSlotShape,
) -> bool {
    owner.members.iter().any(|member| {
        let Member::Field(field) = member else {
            return false;
        };
        field.name == field_name
            && !is_static_field(field)
            && java_type_names_same_after_optional(
                &shape.java_ty,
                &JavaType::Named {
                    name: owner.name.clone(),
                    args: Vec::new(),
                },
            )
    })
}

fn java_type_names_same_after_optional(left: &JavaType, right: &JavaType) -> bool {
    let left = optional_element_type(left).unwrap_or_else(|| left.clone());
    let right = optional_element_type(right).unwrap_or_else(|| right.clone());
    match (&left, &right) {
        (JavaType::Named { name: left, .. }, JavaType::Named { name: right, .. })
        | (JavaType::Unknown(left), JavaType::Named { name: right, .. })
        | (JavaType::Named { name: left, .. }, JavaType::Unknown(right))
        | (JavaType::Unknown(left), JavaType::Unknown(right)) => {
            left == right
                || left.rsplit('.').next() == right.rsplit('.').next()
                || rust_name_for_java_named_type(left) == rust_name_for_java_named_type(right)
        }
        _ => false,
    }
}

fn box_recursive_field_type(rust_ty: RustType) -> RustType {
    match rust_ty {
        RustType::Option(inner) => RustType::Option(Box::new(
            crate::slot_render::rust_generic_type("Box", vec![*inner]),
        )),
        other => crate::slot_render::rust_generic_type("Box", vec![other]),
    }
}

fn shared_outer_handle_type(
    placement: crate::storage_strategy::StoragePlacement,
    inner: RustType,
) -> RustType {
    let celled = placement
        .cell_path()
        .map(|cell| crate::slot_render::rust_generic_type(cell, vec![inner.clone()]))
        .unwrap_or(inner);
    placement
        .handle_path()
        .map(|handle| crate::slot_render::rust_generic_type(handle, vec![celled.clone()]))
        .unwrap_or(celled)
}

fn source_field_is_public(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Public)
}

fn source_field_is_private(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Private)
}

fn source_type_has_context_subtypes(ty: &TypeDecl, support_program: &Program) -> bool {
    support_program
        .types
        .iter()
        .any(|candidate| source_type_extends(candidate, &ty.name, support_program))
}

fn source_type_extends(candidate: &TypeDecl, super_name: &str, support_program: &Program) -> bool {
    let Some(extends) = candidate.extends.as_deref() else {
        return false;
    };
    if java_type_names_same_after_optional(
        &JavaType::Named {
            name: extends.to_string(),
            args: Vec::new(),
        },
        &JavaType::Named {
            name: super_name.to_string(),
            args: Vec::new(),
        },
    ) {
        return true;
    }
    support_program
        .types
        .iter()
        .find(|ty| {
            java_type_names_same_after_optional(
                &JavaType::Named {
                    name: ty.name.clone(),
                    args: Vec::new(),
                },
                &JavaType::Named {
                    name: extends.to_string(),
                    args: Vec::new(),
                },
            )
        })
        .is_some_and(|parent| source_type_extends(parent, super_name, support_program))
}

fn struct_derives_for_type(
    ty: &TypeDecl,
    fields: &[RustField],
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
) -> Vec<String> {
    let mut derives = Vec::new();
    let type_params: HashSet<String> = ty.type_params.iter().cloned().collect();
    if source_struct_fields_support_derive_clone(ty, fields, signatures, facts, support_program) {
        derives.push("Clone".to_string());
    }
    if fields
        .iter()
        .all(|field| rust_type_supports_struct_derive_partial_eq(&field.ty))
    {
        derives.push("PartialEq".to_string());
    }
    // `Default` is added only for a type some `new T[n]` needs, not for every
    // struct that could support it. Java allocates `n` null slots; the closest
    // thing the value representation has is `n` default values, and that needs
    // the derive. Restricting it to the types that need it keeps the change off
    // every other struct's derive line — the "derive obligation" 0034 asks for,
    // the same shape as the `Clone` obligation 0002a needed.
    if (crate::array_default_obligations::program_allocates_object_array_of(support_program, ty)
        || program_needs_constructor_placeholder_default_for_type(support_program, ty))
        && ty.type_params.is_empty()
        && fields.iter().all(|field| {
            rust_type_supports_struct_derive_default(
                &field.ty,
                &type_params,
                signatures,
                facts,
                support_program,
                &mut HashSet::new(),
            )
        })
    {
        derives.push("Default".to_string());
    }
    if source_type_declares_java_hash_equality(ty)
        && fields
            .iter()
            .all(|field| rust_type_supports_struct_derive_eq_hash(&field.ty))
    {
        if !derives.iter().any(|derive| derive == "PartialEq") {
            derives.push("PartialEq".to_string());
        }
        derives.push("Eq".to_string());
        derives.push("Hash".to_string());
    }
    derives
}

/// Whether the concrete Rust struct emitted for a source declaration derives
/// `Clone`.
///
/// Consumers that synthesize code containing `self.clone()` must use the same
/// structural proof as the derive itself. Keeping that decision here prevents
/// trait-object adapters and the concrete item from drifting apart.
pub(crate) fn source_type_supports_struct_derive_clone(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
    support_program: &Program,
) -> bool {
    let fields = struct_fields_for_type(
        ty,
        signatures,
        facts,
        Some(representations),
        support_program,
    );
    source_struct_fields_support_derive_clone(ty, &fields, signatures, facts, support_program)
}

fn source_struct_fields_support_derive_clone(
    ty: &TypeDecl,
    fields: &[RustField],
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
) -> bool {
    let type_params: HashSet<String> = ty.type_params.iter().cloned().collect();
    fields.iter().all(|field| {
        rust_type_supports_struct_derive_clone(
            &field.ty,
            &type_params,
            signatures,
            facts,
            support_program,
            &mut HashSet::new(),
        )
    })
}

pub(crate) fn program_needs_constructor_placeholder_default_for_type(
    support_program: &Program,
    target: &TypeDecl,
) -> bool {
    support_program.types.iter().any(|owner| {
        let fields = owner
            .members
            .iter()
            .filter_map(|member| match member {
                Member::Field(field) => Some(field),
                _ => None,
            })
            .collect::<Vec<_>>();
        owner.members.iter().any(|member| {
            let Member::Constructor(ctor) = member else {
                return false;
            };
            ctor.body.as_ref().is_some_and(|body| {
                body.statements.iter().any(|stmt| {
                    constructor_stmt_assigns_placeholder_field_of_type(stmt, &fields, target)
                })
            })
        })
    })
}

fn constructor_stmt_assigns_placeholder_field_of_type(
    stmt: &Stmt,
    fields: &[&FieldDecl],
    target: &TypeDecl,
) -> bool {
    match stmt {
        Stmt::Assign {
            target: lhs,
            value: _,
        } => {
            let Some(field_name) = assigned_instance_field_name(lhs) else {
                return false;
            };
            fields.iter().any(|field| {
                field.name == field_name
                    && java_type_names_same_after_optional(
                        &field.ty,
                        &JavaType::Named {
                            name: target.name.clone(),
                            args: Vec::new(),
                        },
                    )
            })
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            then_block.statements.iter().any(|stmt| {
                constructor_stmt_assigns_placeholder_field_of_type(stmt, fields, target)
            }) || else_block.as_ref().is_some_and(|block| {
                block.statements.iter().any(|stmt| {
                    constructor_stmt_assigns_placeholder_field_of_type(stmt, fields, target)
                })
            })
        }
        _ => false,
    }
}

fn assigned_instance_field_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(field) => Some(field),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            Some(field)
        }
        _ => None,
    }
}

#[cfg(test)]
fn constructor_expr_reads_self(expr: &Expr) -> bool {
    match expr {
        Expr::This | Expr::Super => true,
        Expr::FieldAccess(receiver, _) => constructor_expr_reads_self(receiver),
        Expr::Call { receiver, args, .. } => {
            receiver.as_deref().is_some_and(constructor_expr_reads_self)
                || args.iter().any(constructor_expr_reads_self)
        }
        Expr::New {
            qualifier, args, ..
        } => {
            qualifier
                .as_deref()
                .is_some_and(constructor_expr_reads_self)
                || args.iter().any(constructor_expr_reads_self)
        }
        Expr::ArrayInitializer(args) => args.iter().any(constructor_expr_reads_self),
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions.iter().any(constructor_expr_reads_self)
                || initializer
                    .as_ref()
                    .is_some_and(|items| items.iter().any(constructor_expr_reads_self))
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            constructor_expr_reads_self(expr)
        }
        Expr::ArrayAccess(array, index) => {
            constructor_expr_reads_self(array) || constructor_expr_reads_self(index)
        }
        Expr::Binary { left, right, .. } => {
            constructor_expr_reads_self(left) || constructor_expr_reads_self(right)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            constructor_expr_reads_self(condition)
                || constructor_expr_reads_self(then_expr)
                || constructor_expr_reads_self(else_expr)
        }
        Expr::Switch { selector, cases } => {
            constructor_expr_reads_self(selector)
                || cases
                    .iter()
                    .any(|case| constructor_expr_reads_self(&case.value))
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn source_type_declares_java_hash_equality(ty: &TypeDecl) -> bool {
    let mut has_equals = false;
    let mut has_hash_code = false;
    for member in &ty.members {
        let Member::Method(method) = member else {
            continue;
        };
        match method.name.as_str() {
            "equals" if method.params.len() == 1 => has_equals = true,
            "hashCode" | "hash_code" if method.params.is_empty() => has_hash_code = true,
            _ => {}
        }
    }
    has_equals && has_hash_code
}

fn rust_type_supports_struct_derive_clone(
    ty: &RustType,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    match ty {
        RustType::Unit
        | RustType::Bool
        | RustType::I8
        | RustType::I16
        | RustType::I32
        | RustType::I64
        | RustType::F32
        | RustType::F64
        | RustType::Char
        | RustType::String => true,
        RustType::Vec(inner) | RustType::Array2(inner) | RustType::Option(inner) => {
            rust_type_supports_struct_derive_clone(
                inner,
                type_params,
                signatures,
                facts,
                support_program,
                visiting,
            )
        }
        RustType::Generic { base, args }
            if base == "std::rc::Rc"
                || base == "Rc"
                || base == "std::sync::Arc"
                || base == "Arc"
                || base == "std::marker::PhantomData"
                || base == "PhantomData" =>
        {
            args.len() == 1
        }
        RustType::Tuple(items) => items.iter().all(|item| {
            rust_type_supports_struct_derive_clone(
                item,
                type_params,
                signatures,
                facts,
                support_program,
                visiting,
            )
        }),
        RustType::FnPointer { .. } => true,
        RustType::ImplFnTrait { .. } => false,
        RustType::Generic { base, args } if base == "Box" || base == "std::boxed::Box" => {
            let Some(inner) = args.first() else {
                return false;
            };
            if args.len() != 1 {
                return false;
            }
            match inner {
                RustType::DynTrait(trait_name) => {
                    raw_dyn_trait_supports_box_clone_adapter(trait_name)
                }
                other => rust_type_supports_struct_derive_clone(
                    other,
                    type_params,
                    signatures,
                    facts,
                    support_program,
                    visiting,
                ),
            }
        }
        RustType::Generic { args, .. } => args.iter().all(|arg| {
            rust_type_supports_struct_derive_clone(
                arg,
                type_params,
                signatures,
                facts,
                support_program,
                visiting,
            )
        }),
        RustType::Named(name) if type_params.contains(name) => true,
        RustType::Named(name) => rust_named_type_supports_struct_derive_clone(
            name,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        ),
        RustType::Raw(raw)
            if raw_rust_shared_handle_type_inner(raw).is_some()
                || raw_rust_type_supports_derive_clone(
                    raw,
                    type_params,
                    signatures,
                    facts,
                    support_program,
                    visiting,
                ) =>
        {
            true
        }
        RustType::DynTrait(_)
        | RustType::ImplTrait(_)
        | RustType::Slice(_)
        | RustType::Ref { .. }
        | RustType::Str
        | RustType::Raw(_) => false,
    }
}

fn raw_rust_type_supports_derive_clone(
    raw: &str,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    let raw = raw.trim();
    if let Some(supports_clone) = simple_raw_rust_type_supports_derive_clone(
        raw,
        type_params,
        signatures,
        facts,
        support_program,
        visiting,
    ) {
        return supports_clone;
    }
    if raw_rust_generic_args(raw, "std::marker::PhantomData")
        .or_else(|| raw_rust_generic_args(raw, "PhantomData"))
        .is_some()
    {
        return true;
    }
    if raw_rust_box_dyn_trait(raw).is_some_and(raw_dyn_trait_supports_box_clone_adapter) {
        return true;
    }
    if let Some(inner) = raw_rust_option_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if let Some(inner) = raw_rust_vec_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if let Some(inner) = raw_rust_box_sized_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if let Some(args) = raw_rust_generic_args(raw, "std::collections::HashMap")
        .or_else(|| raw_rust_generic_args(raw, "HashMap"))
        && args.len() == 2
    {
        return args.iter().all(|arg| {
            raw_type_arg_supports_derive_clone(
                arg,
                type_params,
                signatures,
                facts,
                support_program,
                visiting,
            )
        });
    }
    if let Some(args) = raw_rust_generic_args(raw, "std::collections::HashSet")
        .or_else(|| raw_rust_generic_args(raw, "HashSet"))
        && args.len() == 1
    {
        return raw_type_arg_supports_derive_clone(
            args[0],
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    false
}

fn raw_type_arg_supports_derive_clone(
    arg: &str,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    let raw = arg.trim();
    if let Some(supports_clone) = simple_raw_rust_type_supports_derive_clone(
        raw,
        type_params,
        signatures,
        facts,
        support_program,
        visiting,
    ) {
        return supports_clone;
    }
    // A shared handle is `Clone` whatever it points at -- cloning it bumps a
    // refcount. The top-level arm already knows this; a handle nested inside a
    // collection element reaches here instead, and without this a
    // `HashSet<Rc<T>>` field silently cost its struct the `Clone` derive
    // (claude_audits/0002b).
    if raw_rust_shared_handle_type_inner(raw).is_some() {
        return true;
    }
    if let Some(inner) = raw_rust_option_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if let Some(inner) = raw_rust_vec_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if raw_rust_box_dyn_trait(raw).is_some_and(raw_dyn_trait_supports_box_clone_adapter) {
        return true;
    }
    if let Some(inner) = raw_rust_box_sized_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if raw.contains('<') {
        return raw_rust_type_supports_derive_clone(
            raw,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    false
}

fn simple_raw_rust_type_supports_derive_clone(
    raw: &str,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> Option<bool> {
    match rust_type_arg_projection_or_raw(raw) {
        RustType::Raw(_) => None,
        ty => Some(rust_type_supports_struct_derive_clone(
            &ty,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        )),
    }
}

fn raw_rust_shared_handle_type_inner(raw: &str) -> Option<&str> {
    raw_rust_generic_args(raw, "std::rc::Rc")
        .or_else(|| raw_rust_generic_args(raw, "Rc"))
        .or_else(|| raw_rust_generic_args(raw, "std::sync::Arc"))
        .or_else(|| raw_rust_generic_args(raw, "Arc"))
        .and_then(|args| {
            if args.len() == 1 {
                args.first().copied().map(str::trim)
            } else {
                None
            }
        })
}

fn raw_dyn_trait_supports_box_clone_adapter(trait_name: &str) -> bool {
    !matches!(trait_name.trim(), "Any" | "std::any::Any")
}

/// Whether a struct field's Rust type can participate in `#[derive(Default)]`.
///
/// Mirrors [`rust_type_supports_struct_derive_clone`], with one deliberate
/// difference: an *unknown* named type answers **false** rather than true.
/// Clone can assume an unmodelled facade is `Clone` because getting it wrong
/// costs a compile error at a site that was already using `.clone()`. Default is
/// asked speculatively — to decide whether `new T[n]` can allocate `n` slots —
/// so a wrong "yes" would turn a loud fallback into a broken build. A wrong
/// "no" only keeps today's diagnostic.
fn rust_type_supports_struct_derive_default(
    ty: &RustType,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    match ty {
        RustType::Unit
        | RustType::Bool
        | RustType::I8
        | RustType::I16
        | RustType::I32
        | RustType::I64
        | RustType::F32
        | RustType::F64
        | RustType::Char
        | RustType::String => true,
        // `Option` is `Default` (as `None`) whatever it wraps, which is what
        // makes a recursive `Node { Node next; }` work: the field lowers to
        // `Option<Box<Node>>` and defaults to `None` without needing `Node`
        // itself to be constructible.
        RustType::Option(_) => true,
        RustType::Vec(_) | RustType::Array2(_) => true,
        RustType::Named(name) if type_params.contains(name) => false,
        RustType::Named(name) => rust_named_type_supports_struct_derive_default(
            name,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        ),
        RustType::Tuple(items) => items.iter().all(|item| {
            rust_type_supports_struct_derive_default(
                item,
                type_params,
                signatures,
                facts,
                support_program,
                visiting,
            )
        }),
        RustType::FnPointer { .. } => true,
        RustType::ImplFnTrait { .. }
        | RustType::Generic { .. }
        | RustType::DynTrait(_)
        | RustType::ImplTrait(_)
        | RustType::Slice(_)
        | RustType::Ref { .. }
        | RustType::Str
        | RustType::Raw(_) => false,
    }
}

/// Whether a named source type can derive `Default`.
///
/// Unknown names answer `false` — see the note on
/// [`rust_type_supports_struct_derive_default`].
fn rust_named_type_supports_struct_derive_default(
    name: &str,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    if type_params.contains(name) {
        return false;
    }
    let Some(ty) = find_type_decl_by_rust_name(support_program, name) else {
        return false;
    };
    if !source_type_is_plain_struct(ty) {
        return false;
    }
    source_type_decl_supports_struct_derive_default(
        ty,
        type_params,
        signatures,
        facts,
        support_program,
        visiting,
    )
}

pub(crate) fn source_type_decl_supports_struct_derive_default(
    ty: &TypeDecl,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    if type_params.contains(&ty.name) || !source_type_is_plain_struct(ty) {
        return false;
    }
    // Coinductive: a type reached through its own field graph is assumed to
    // work, exactly as the `Clone` walk does, so a self-referential type does
    // not recurse forever.
    if !visiting.insert(ty.name.clone()) {
        return true;
    }
    let nested_type_params: HashSet<String> = ty.type_params.iter().cloned().collect();
    let fields = struct_fields_for_type(ty, signatures, facts, None, support_program);
    // A generic type has no single zero value, so it is excluded rather than
    // guessed at.
    let supports_default = ty.type_params.is_empty()
        && fields.iter().all(|field| {
            rust_type_supports_struct_derive_default(
                &field.ty,
                &nested_type_params,
                signatures,
                facts,
                support_program,
                visiting,
            )
        });
    visiting.remove(&ty.name);
    supports_default
}

/// `Default` is only meaningful for a type lowered as a plain struct: an
/// interface, an enum or an erased carrier has no zero value to derive.
fn source_type_is_plain_struct(ty: &TypeDecl) -> bool {
    matches!(ty.kind, crate::java_ir::TypeKind::Class)
}

fn rust_named_type_supports_struct_derive_clone(
    name: &str,
    type_params: &HashSet<String>,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    if type_params.contains(name) {
        return true;
    }
    let Some(ty) = find_type_decl_by_rust_name(support_program, name) else {
        return true;
    };
    if !visiting.insert(ty.name.clone()) {
        return true;
    }
    let fields = struct_fields_for_type(ty, signatures, facts, None, support_program);
    let supports_clone = fields.iter().all(|field| {
        rust_type_supports_struct_derive_clone(
            &field.ty,
            type_params,
            signatures,
            facts,
            support_program,
            visiting,
        )
    });
    visiting.remove(&ty.name);
    supports_clone
}

fn find_type_decl_by_rust_name<'a>(program: &'a Program, rust_name: &str) -> Option<&'a TypeDecl> {
    let mut matches = Vec::new();
    for ty in &program.types {
        collect_type_decls_by_rust_name(ty, rust_name, &mut matches);
    }
    unique_type_decl_match(&matches)
}

fn collect_type_decls_by_rust_name<'a>(
    ty: &'a TypeDecl,
    rust_name: &str,
    out: &mut Vec<&'a TypeDecl>,
) {
    if rust_name_for_java_named_type(&ty.name) == rust_name {
        out.push(ty);
    }
    for member in &ty.members {
        if let Member::Type(nested) = member {
            collect_type_decls_by_rust_name(nested, rust_name, out);
        }
    }
}

fn unique_type_decl_match<'a>(matches: &[&'a TypeDecl]) -> Option<&'a TypeDecl> {
    let first = *matches.first()?;
    if matches.iter().all(|ty| ty.name == first.name) {
        Some(first)
    } else {
        None
    }
}

fn rust_type_supports_struct_derive_partial_eq(ty: &RustType) -> bool {
    match ty {
        RustType::Unit
        | RustType::Bool
        | RustType::I8
        | RustType::I16
        | RustType::I32
        | RustType::I64
        | RustType::F32
        | RustType::F64
        | RustType::Char
        | RustType::String => true,
        RustType::Vec(inner) | RustType::Array2(inner) | RustType::Option(inner) => {
            rust_type_supports_struct_derive_partial_eq(inner)
        }
        RustType::Tuple(items) => items
            .iter()
            .all(rust_type_supports_struct_derive_partial_eq),
        RustType::FnPointer { .. } => true,
        RustType::ImplFnTrait { .. }
        | RustType::Generic { .. }
        | RustType::DynTrait(_)
        | RustType::ImplTrait(_)
        | RustType::Named(_) => false,
        RustType::Slice(_) | RustType::Ref { .. } | RustType::Str | RustType::Raw(_) => false,
    }
}

fn rust_type_supports_struct_derive_eq_hash(ty: &RustType) -> bool {
    match ty {
        RustType::Unit
        | RustType::Bool
        | RustType::I8
        | RustType::I16
        | RustType::I32
        | RustType::I64
        | RustType::Char
        | RustType::String => true,
        RustType::Vec(inner) | RustType::Array2(inner) | RustType::Option(inner) => {
            rust_type_supports_struct_derive_eq_hash(inner)
        }
        RustType::Tuple(items) => items.iter().all(rust_type_supports_struct_derive_eq_hash),
        RustType::FnPointer { .. } => true,
        RustType::ImplFnTrait { .. }
        | RustType::F32
        | RustType::F64
        | RustType::Generic { .. }
        | RustType::DynTrait(_)
        | RustType::ImplTrait(_)
        | RustType::Named(_)
        | RustType::Slice(_)
        | RustType::Ref { .. }
        | RustType::Str
        | RustType::Raw(_) => false,
    }
}

pub(crate) fn is_static_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static)
}

fn is_static_final_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static) && field.modifiers.contains(&Modifier::Final)
}

fn associated_consts_for_type(ty: &TypeDecl) -> Vec<RustAssociatedConst> {
    let const_infos = static_final_const_infos(ty);
    let source_static_field_rust_names = crate::java_naming::source_static_field_rust_names(ty);
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if is_static_final_field(field) => associated_const_for_field(
                field,
                source_static_field_rust_names
                    .get(&field.name)
                    .map(String::as_str)
                    .unwrap_or(&field.name),
                &const_infos,
            ),
            _ => None,
        })
        .collect()
}

pub(crate) fn static_final_field_has_associated_const(ty: &TypeDecl, field: &FieldDecl) -> bool {
    if !is_static_final_field(field) {
        return false;
    }
    let const_infos = static_final_const_infos(ty);
    associated_const_for_field(field, &field.name, &const_infos).is_some()
}

#[derive(Debug, Clone)]
struct StaticFinalConstInfo {
    normalized: String,
    original: String,
    array_len: Option<usize>,
    has_associated_const: bool,
    string_value: Option<String>,
}

fn static_final_const_infos(ty: &TypeDecl) -> Vec<StaticFinalConstInfo> {
    let mut infos = Vec::new();
    for member in &ty.members {
        let Member::Field(field) = member else {
            continue;
        };
        if is_static_final_field(field) {
            let string_value = field.initializer.as_ref().and_then(|initializer| {
                const_string_expr_value(initializer, &infos, Some(&field.ty))
            });
            let has_associated_const = field
                .initializer
                .as_ref()
                .and_then(|initializer| const_type_and_value(&field.ty, initializer, &infos))
                .or_else(|| non_literal_static_final_const_type_and_value(field))
                .is_some();
            infos.push(StaticFinalConstInfo {
                normalized: crate::parser::to_snake_case(&field.name),
                original: field.name.clone(),
                array_len: static_final_array_initializer_len(field),
                has_associated_const,
                string_value,
            });
        }
    }
    infos
}

fn static_final_array_initializer_len(field: &FieldDecl) -> Option<usize> {
    if !matches!(field.ty, JavaType::Array(_)) {
        return None;
    }
    match field.initializer.as_ref()? {
        Expr::ArrayInitializer(elements) => Some(elements.len()),
        Expr::ArrayCreation {
            initializer: Some(elements),
            ..
        } => Some(elements.len()),
        _ => None,
    }
}

fn associated_const_for_field(
    field: &FieldDecl,
    rust_name: &str,
    const_infos: &[StaticFinalConstInfo],
) -> Option<RustAssociatedConst> {
    let (ty, value) = field
        .initializer
        .as_ref()
        .and_then(|initializer| const_type_and_value(&field.ty, initializer, const_infos))
        .or_else(|| non_literal_static_final_const_type_and_value(field))?;
    Some(RustAssociatedConst {
        name: rust_name.to_string(),
        ty,
        value,
        is_pub: !field.modifiers.contains(&Modifier::Private),
    })
}

fn non_literal_static_final_const_type_and_value(
    field: &FieldDecl,
) -> Option<(RustType, RustExpr)> {
    if !is_static_final_field(field) || field.initializer.is_none() {
        return None;
    }
    let name = match &field.ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    let rust_name = rust_name_for_java_named_type(name);
    if matches!(rust_name.as_str(), "Pattern" | "JavaUtilRegexPattern")
        && let Some((value, flags)) =
            static_final_pattern_compile_literal(field.initializer.as_ref()?)
    {
        let (constructor, args) = if let Some(flags) = flags {
            (
                "compile_static_with_flags",
                vec![rust_string_literal_projection(&value), flags],
            )
        } else {
            (
                "compile_static",
                vec![rust_string_literal_projection(&value)],
            )
        };
        return Some((
            RustType::Named(rust_name),
            function_call_projection(
                owner_associated_path_projection("Pattern", constructor),
                args,
            ),
        ));
    }
    // An arbitrary Java object initializer is not a Rust const expression.
    // It must use the static-storage path so construction happens once and
    // subsequent reads preserve Java object identity.
    None
}

fn static_final_pattern_compile_literal(initializer: &Expr) -> Option<(String, Option<RustExpr>)> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = initializer
    else {
        return None;
    };
    if !matches!(
        name.as_str(),
        "compile" | "compile_1" | "compile_static" | "Pattern::compile"
    ) {
        return None;
    }
    let Expr::Var(receiver_name) = receiver.as_ref() else {
        return None;
    };
    let receiver_name = rust_name_for_java_named_type(receiver_name);
    if !matches!(
        receiver_name.as_str(),
        "Pattern" | "JavaUtilRegexPattern" | "pattern" | "java_util_regex_pattern"
    ) {
        return None;
    }
    let (pattern, flags) = match args.as_slice() {
        [pattern] => (pattern, None),
        [pattern, flags] => (
            pattern,
            Some(const_expr_value(
                flags,
                &[],
                Some(&JavaType::Primitive(PrimitiveType::Int)),
            )?),
        ),
        _ => return None,
    };
    let Expr::Literal(Literal::String(value)) = pattern else {
        return None;
    };
    Some((value.clone(), flags))
}

fn const_type_and_value(
    ty: &JavaType,
    initializer: &Expr,
    const_infos: &[StaticFinalConstInfo],
) -> Option<(RustType, RustExpr)> {
    // A non-null compile-time boxed primitive constant has the same executable
    // carrier as its unboxed value. Emitting the facade marker (`Double`) loses
    // both the literal and Java's automatic unboxing at primitive uses.
    let unboxed_ty = match ty {
        JavaType::Named { name, args } if args.is_empty() => {
            crate::stdlib::boxed_primitive_type(name).map(JavaType::Primitive)
        }
        _ => None,
    };
    let effective_ty = unboxed_ty.as_ref().unwrap_or(ty);
    let rust_ty = const_rust_type_for_java_type(effective_ty)?;
    let value = const_expr_value(initializer, const_infos, Some(effective_ty))?;
    Some((rust_ty, value))
}

fn const_rust_type_for_java_type(ty: &JavaType) -> Option<RustType> {
    match ty {
        JavaType::Array(element) => {
            let element_ty = const_rust_type_for_java_type(element)?;
            Some(RustType::Ref {
                mutable: false,
                lifetime: Some("'static".to_string()),
                inner: Box::new(RustType::Slice(Box::new(element_ty))),
            })
        }
        JavaType::String => Some(RustType::Ref {
            mutable: false,
            lifetime: Some("'static".to_string()),
            inner: Box::new(RustType::Str),
        }),
        JavaType::Primitive(PrimitiveType::Boolean) => Some(RustType::Bool),
        JavaType::Primitive(PrimitiveType::Byte) => Some(RustType::I8),
        JavaType::Primitive(PrimitiveType::Short) => Some(RustType::I16),
        JavaType::Primitive(PrimitiveType::Int) => Some(RustType::I32),
        JavaType::Primitive(PrimitiveType::Long) => Some(RustType::I64),
        JavaType::Primitive(PrimitiveType::Float) => Some(RustType::F32),
        JavaType::Primitive(PrimitiveType::Double) => Some(RustType::F64),
        JavaType::Primitive(PrimitiveType::Char) => Some(RustType::Char),
        _ => None,
    }
}

fn const_expr_value(
    expr: &Expr,
    const_infos: &[StaticFinalConstInfo],
    expected_ty: Option<&JavaType>,
) -> Option<RustExpr> {
    match expr {
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Add,
            ..
        } if matches!(expected_ty, Some(JavaType::String)) => {
            const_string_expr_value(expr, const_infos, expected_ty)
                .map(|value| rust_string_literal_projection(&value))
        }
        Expr::Literal(literal) => const_literal_expr_value(literal, expected_ty),
        Expr::Var(name) => {
            let reference = static_const_reference_info(name, const_infos)?;
            if reference.has_associated_const {
                Some(self_associated_path_projection(&reference.original))
            } else {
                None
            }
        }
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            let reference = static_const_reference_info(field, const_infos)?;
            if reference.has_associated_const {
                Some(self_associated_path_projection(&reference.original))
            } else {
                None
            }
        }
        Expr::FieldAccess(receiver, field)
            if field == "length" && matches!(expected_ty, Some(JavaType::Primitive(_))) =>
        {
            let Expr::Var(name) = receiver.as_ref() else {
                return None;
            };
            static_const_array_len(name, const_infos).map(|len| RustExpr::Literal(len.to_string()))
        }
        Expr::FieldAccess(receiver, field) => {
            let Expr::Var(owner) = receiver.as_ref() else {
                return None;
            };
            let owner = rust_name_for_java_named_type(owner);
            let member = const_reference_name(field, const_infos);
            Some(owner_associated_path_projection(&owner, &member))
        }
        Expr::Unary {
            op: crate::java_ir::UnaryOp::Neg,
            expr,
        } => Some(RustExpr::Unary {
            op: "-".to_string(),
            expr: Box::new(const_expr_value(expr, const_infos, expected_ty)?),
        }),
        Expr::Unary {
            op: crate::java_ir::UnaryOp::Pos,
            expr,
        } => const_expr_value(expr, const_infos, expected_ty),
        Expr::Binary { op, left, right } if const_binary_op_is_allowed(*op) => {
            Some(RustExpr::Binary {
                op: crate::operator_lowering::binary_op_to_rust(*op).to_string(),
                left: Box::new(const_expr_value(left, const_infos, expected_ty)?),
                right: Box::new(const_expr_value(right, const_infos, expected_ty)?),
            })
        }
        Expr::Cast { expr, ty } if const_rust_type_for_java_type(ty).is_some() => {
            Some(RustExpr::Cast {
                expr: Box::new(const_expr_value(expr, const_infos, Some(ty))?),
                ty: crate::emit::emit_type(&const_rust_type_for_java_type(ty)?),
            })
        }
        Expr::ArrayInitializer(elements) => {
            let element_ty = expected_array_element_type(expected_ty)?;
            const_array_expr_value(elements, const_infos, element_ty)
        }
        Expr::ArrayCreation {
            element_type,
            initializer: Some(elements),
            ..
        } => {
            let element_ty = expected_array_element_type(expected_ty).unwrap_or(element_type);
            const_array_expr_value(elements, const_infos, element_ty)
        }
        _ => None,
    }
}

fn const_string_expr_value(
    expr: &Expr,
    const_infos: &[StaticFinalConstInfo],
    expected_ty: Option<&JavaType>,
) -> Option<String> {
    if !matches!(expected_ty, Some(JavaType::String)) {
        return None;
    }
    match expr {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        Expr::Var(name) => static_const_string_value(name, const_infos),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            static_const_string_value(field, const_infos)
        }
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Add,
            left,
            right,
        } => {
            let left = const_string_expr_value(left, const_infos, expected_ty)?;
            let right = const_string_expr_value(right, const_infos, expected_ty)?;
            Some(format!("{left}{right}"))
        }
        _ => None,
    }
}

fn static_const_string_value(name: &str, const_infos: &[StaticFinalConstInfo]) -> Option<String> {
    let normalized = crate::parser::to_snake_case(name);
    const_infos
        .iter()
        .find(|info| info.original == name || info.normalized == normalized)
        .and_then(|info| info.string_value.clone())
}

fn const_array_expr_value(
    elements: &[Expr],
    const_infos: &[StaticFinalConstInfo],
    element_ty: &JavaType,
) -> Option<RustExpr> {
    let values = elements
        .iter()
        .map(|element| const_expr_value(element, const_infos, Some(element_ty)))
        .collect::<Option<Vec<_>>>()?;
    Some(RustExpr::Unary {
        op: "&".to_string(),
        expr: Box::new(RustExpr::ArrayLiteral(values)),
    })
}

fn expected_array_element_type(expected_ty: Option<&JavaType>) -> Option<&JavaType> {
    match expected_ty {
        Some(JavaType::Array(element)) => Some(element.as_ref()),
        _ => None,
    }
}

fn const_literal_expr_value(literal: &Literal, expected_ty: Option<&JavaType>) -> Option<RustExpr> {
    match (literal, expected_ty) {
        (Literal::String(value), _) => Some(rust_string_literal_projection(value)),
        (Literal::Int(value), Some(JavaType::Primitive(PrimitiveType::Char))) => {
            u32::try_from(*value)
                .ok()
                .and_then(char::from_u32)
                .map(|value| RustExpr::Literal(format!("{value:?}")))
        }
        (Literal::Int(value), Some(JavaType::Primitive(PrimitiveType::Long))) => {
            Some(RustExpr::Literal(format!("{value}i64")))
        }
        (Literal::Int(value), Some(JavaType::Primitive(PrimitiveType::Float))) => {
            Some(RustExpr::Literal(format!("{value}f32")))
        }
        (Literal::Int(value), Some(JavaType::Primitive(PrimitiveType::Double))) => {
            Some(RustExpr::Literal(format!("{value}f64")))
        }
        (Literal::Long(value), Some(JavaType::Primitive(PrimitiveType::Float))) => {
            Some(RustExpr::Cast {
                expr: Box::new(RustExpr::Literal(format!("{value}i64"))),
                ty: "f32".to_string(),
            })
        }
        (Literal::Long(value), Some(JavaType::Primitive(PrimitiveType::Double))) => {
            Some(RustExpr::Cast {
                expr: Box::new(RustExpr::Literal(format!("{value}i64"))),
                ty: "f64".to_string(),
            })
        }
        (Literal::Float(value), Some(JavaType::Primitive(PrimitiveType::Double))) => {
            Some(RustExpr::Literal(format!("{value}f64")))
        }
        _ => Some(crate::expr_render::literal_expr(literal)),
    }
}

fn const_reference_name<'a>(name: &'a str, const_infos: &'a [StaticFinalConstInfo]) -> &'a str {
    static_const_reference_info(name, const_infos)
        .map(|info| info.original.as_str())
        .unwrap_or(name)
}

fn static_const_reference_info<'a>(
    name: &str,
    const_infos: &'a [StaticFinalConstInfo],
) -> Option<&'a StaticFinalConstInfo> {
    const_infos
        .iter()
        .find(|info| name == info.normalized || name == info.original)
}

fn static_const_array_len(name: &str, const_infos: &[StaticFinalConstInfo]) -> Option<usize> {
    const_infos
        .iter()
        .find(|info| name == info.normalized || name == info.original)
        .and_then(|info| info.array_len)
}

fn const_binary_op_is_allowed(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Rem
            | BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr
    )
}

fn constructor_params(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Vec<RustParam> {
    let signature = signatures.required_constructor_signature(ty, ctor);
    let access_names = crate::java_naming::source_binding_rust_names(
        ctor.params.iter().map(|param| param.source_name.as_str()),
    );
    signature
        .params()
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let name = ctor
                .params
                .get(index)
                .and_then(|source_param| access_names.get(&source_param.source_name))
                .cloned()
                .unwrap_or_else(|| param.name().to_string());
            RustParam {
                name,
                ty: rust_type_for_slot_with_facts(&param.shape(), facts),
            }
        })
        .collect()
}

fn constructor_return_type(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
) -> RustType {
    let _ = signatures.required_constructor_signature(ty, ctor);
    RustType::Named(rust_use_name_for_source_type(ty))
}

fn method_params(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Vec<RustParam> {
    method_params_with_optional_lifetime(ty, method, signatures, facts, false, None)
}

fn method_params_with_optional_lifetime(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    lifetime: bool,
    representations: Option<&RepresentationPlan>,
) -> Vec<RustParam> {
    let signature = signatures.required_method_signature(ty, method);
    let mutable_array_target_params = mutable_array_target_params(method);
    let access_names = crate::java_naming::source_binding_rust_names(
        method.params.iter().map(|param| param.source_name.as_str()),
    );
    signature
        .params()
        .iter()
        .enumerate()
        .map(|(index, param)| {
            let source_param_name = method
                .params
                .get(index)
                .map(|source_param| source_param.name.as_str())
                .unwrap_or_else(|| param.name());
            let callback_param =
                method_param_is_nonescaping_functional_callback(method, source_param_name);
            let submit_callable_param =
                method_param_is_submit_callable_callback(method, source_param_name);
            let mutable_optional_sequence_param = mutable_array_target_params
                .contains(param.name())
                && optional_sequence_slot(&param.shape());
            let mutable_owned_binding_param = param.binding_mutable()
                && matches!(
                    param.shape().ownership,
                    OwnershipShape::Owned | OwnershipShape::CopyValue
                );
            let name = method
                .params
                .get(index)
                .and_then(|source_param| access_names.get(&source_param.source_name))
                .cloned()
                .unwrap_or_else(|| param.name().to_string());
            RustParam {
                name: if callback_param
                    || submit_callable_param
                    || mutable_optional_sequence_param
                    || mutable_owned_binding_param
                {
                    format!("mut {name}")
                } else {
                    name
                },
                ty: if callback_param || submit_callable_param {
                    let source_java_ty = method
                        .params
                        .get(index)
                        .map(|source_param| &source_param.ty)
                        .unwrap_or_else(|| param.java_ty());
                    let callback_ty = if submit_callable_param {
                        submit_callable_callback_param_type(method, source_java_ty, facts)
                    } else {
                        functional_callback_param_type(source_java_ty, facts)
                    }
                    .unwrap_or_else(|| {
                        rust_type_for_slot_with_optional_representations(
                            &param.shape(),
                            facts,
                            representations,
                        )
                    });
                    let callback_ty = maybe_add_lifetime_to_refs(callback_ty, lifetime);
                    if callback_param
                        && method_stores_priority_queue_comparator_callback(method, param.name())
                    {
                        RustType::Raw(format!(
                            "{} + 'static",
                            crate::emit::emit_type(&callback_ty)
                        ))
                    } else {
                        callback_ty
                    }
                } else {
                    let rust_ty = method
                        .params
                        .get(index)
                        .and_then(|source_param| {
                            source_java_class_token_param_type(
                                &source_param.ty,
                                &param.shape(),
                                facts,
                            )
                        })
                        .unwrap_or_else(|| {
                            rust_type_for_slot_with_optional_representations(
                                &param.shape(),
                                facts,
                                representations,
                            )
                        });
                    maybe_add_lifetime_to_refs(rust_ty, lifetime)
                },
            }
        })
        .collect()
}

fn source_java_class_token_param_type(
    ty: &JavaType,
    solved_shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<RustType> {
    let JavaType::Named { name, .. } = ty else {
        return None;
    };
    if crate::stdlib::canonical_external_receiver_type_key(name) != "Class" {
        return None;
    }
    let token = rust_type_for_java_type_with_facts(ty, facts);
    Some(if solved_shape.optional_element_type().is_some() {
        RustType::Option(Box::new(token))
    } else {
        token
    })
}

fn optional_sequence_slot(shape: &RustSlotShape) -> bool {
    optional_element_type(&shape.java_ty).is_some_and(|inner| {
        sequence_element_type(&inner).is_some()
            || two_dimensional_array_element_type(&inner).is_some()
    })
}

fn mutable_array_target_params(method: &MethodDecl) -> HashSet<String> {
    let mut names = HashSet::new();
    if let Some(body) = &method.body {
        collect_mutable_array_target_params_in_stmts(&body.statements, &mut names);
    }
    names
}

fn collect_mutable_array_target_params_in_stmts(statements: &[Stmt], names: &mut HashSet<String>) {
    for stmt in statements {
        match stmt {
            Stmt::Assign { target, value } => {
                collect_array_target_root_name(target, names);
                collect_mutable_array_target_params_in_expr(value, names);
            }
            Stmt::VarDecl { initializer, .. } => {
                if let Some(initializer) = initializer {
                    collect_mutable_array_target_params_in_expr(initializer, names);
                }
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_mutable_array_target_params_in_expr(expr, names);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_mutable_array_target_params_in_expr(condition, names);
                collect_mutable_array_target_params_in_stmts(&then_block.statements, names);
                if let Some(else_block) = else_block {
                    collect_mutable_array_target_params_in_stmts(&else_block.statements, names);
                }
            }
            Stmt::While {
                condition, body, ..
            }
            | Stmt::DoWhile {
                condition, body, ..
            } => {
                collect_mutable_array_target_params_in_expr(condition, names);
                collect_mutable_array_target_params_in_stmts(&body.statements, names);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_mutable_array_target_params_in_stmts(init, names);
                if let Some(condition) = condition {
                    collect_mutable_array_target_params_in_expr(condition, names);
                }
                for expr in update {
                    collect_mutable_array_target_params_in_expr(expr, names);
                }
                collect_mutable_array_target_params_in_stmts(&body.statements, names);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                collect_mutable_array_target_params_in_expr(iterable, names);
                collect_mutable_array_target_params_in_stmts(&body.statements, names);
            }
            Stmt::Switch { selector, cases } => {
                collect_mutable_array_target_params_in_expr(selector, names);
                for case in cases {
                    collect_mutable_array_target_params_in_stmts(&case.statements, names);
                }
            }
            Stmt::Try {
                resources,
                body,
                catches,
                finally_block,
            } => {
                collect_mutable_array_target_params_in_stmts(resources, names);
                collect_mutable_array_target_params_in_stmts(&body.statements, names);
                for catch in catches {
                    collect_mutable_array_target_params_in_stmts(&catch.body.statements, names);
                }
                if let Some(finally_block) = finally_block {
                    collect_mutable_array_target_params_in_stmts(&finally_block.statements, names);
                }
            }
            Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
                collect_mutable_array_target_params_in_stmts(&block.statements, names);
            }
            Stmt::ConstructorInvocation { args, .. } => {
                for arg in args {
                    collect_mutable_array_target_params_in_expr(arg, names);
                }
            }
            Stmt::Return(None)
            | Stmt::LocalType(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_mutable_array_target_params_in_expr(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Binary {
            op: crate::java_ir::BinaryOp::Assign,
            left,
            right,
        } => {
            collect_array_target_root_name(left, names);
            collect_mutable_array_target_params_in_expr(right, names);
        }
        Expr::Unary {
            op:
                crate::java_ir::UnaryOp::PostInc
                | crate::java_ir::UnaryOp::PostDec
                | crate::java_ir::UnaryOp::PreInc
                | crate::java_ir::UnaryOp::PreDec,
            expr,
        } => {
            collect_array_target_root_name(expr, names);
        }
        Expr::FieldAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            collect_mutable_array_target_params_in_expr(receiver, names);
        }
        Expr::Binary { left, right, .. } => {
            collect_mutable_array_target_params_in_expr(left, names);
            collect_mutable_array_target_params_in_expr(right, names);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_mutable_array_target_params_in_expr(condition, names);
            collect_mutable_array_target_params_in_expr(then_expr, names);
            collect_mutable_array_target_params_in_expr(else_expr, names);
        }
        Expr::Switch { selector, cases } => {
            collect_mutable_array_target_params_in_expr(selector, names);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        collect_mutable_array_target_params_in_expr(expr, names);
                    }
                }
                collect_mutable_array_target_params_in_expr(&case.value, names);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_mutable_array_target_params_in_expr(receiver, names);
            }
            for arg in args {
                collect_mutable_array_target_params_in_expr(arg, names);
            }
        }
        Expr::New {
            qualifier, args, ..
        } => {
            if let Some(qualifier) = qualifier {
                collect_mutable_array_target_params_in_expr(qualifier, names);
            }
            for arg in args {
                collect_mutable_array_target_params_in_expr(arg, names);
            }
        }
        Expr::ArrayInitializer(args) => {
            for arg in args {
                collect_mutable_array_target_params_in_expr(arg, names);
            }
        }
        Expr::ArrayAccess(array, index) => {
            collect_mutable_array_target_params_in_expr(array, names);
            collect_mutable_array_target_params_in_expr(index, names);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_mutable_array_target_params_in_expr(dimension, names);
            }
            if let Some(initializer) = initializer {
                for element in initializer {
                    collect_mutable_array_target_params_in_expr(element, names);
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                collect_mutable_array_target_params_in_expr(expr, names);
            }
            crate::java_ir::LambdaBody::Block(block) => {
                collect_mutable_array_target_params_in_stmts(&block.statements, names);
            }
        },
        Expr::MethodReference { .. }
        | Expr::Var(_)
        | Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn collect_array_target_root_name(expr: &Expr, names: &mut HashSet<String>) {
    let Expr::ArrayAccess(array, _) = expr else {
        return;
    };
    if let Some(name) = array_access_root_var(array) {
        names.insert(name.to_string());
    }
}

fn array_access_root_var(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name),
        Expr::ArrayAccess(array, _) => array_access_root_var(array),
        _ => None,
    }
}

/// The Rust closure type a functional-interface parameter lowers to.
///
/// Arity and result both come from one [`crate::stdlib::functional_interface_shape`]
/// lookup. They used to be two hand-written matches that had to agree and did
/// not: `Supplier` had an arity arm and no result arm, so `Supplier<String>`
/// became `impl FnMut()` and its `-> String` vanished (`E0308`), and every name
/// missing from the shorter match stayed an opaque struct with no methods —
/// `Comparator<T>` reached `.compare(..)` on `pub struct Comparator;` (`E0599`).
fn functional_callback_param_type(java_ty: &JavaType, facts: &FactSet) -> Option<RustType> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(name.as_str());
    let shape = crate::stdlib::functional_interface_shape(simple)?;

    let source_ty = |source: &crate::stdlib::FunctionalTypeSource| match source {
        crate::stdlib::FunctionalTypeSource::TypeArgument(index) => args
            .get(*index)
            .map(|ty| rust_type_for_java_type_with_facts(ty, facts)),
        crate::stdlib::FunctionalTypeSource::Primitive(primitive) => Some(
            rust_type_for_java_type_with_facts(&JavaType::Primitive(*primitive), facts),
        ),
    };

    let inputs = shape
        .inputs()
        .iter()
        .map(source_ty)
        .collect::<Option<Vec<_>>>()?;
    // A missing result *type argument* means the source did not spell the
    // generic out; keep the previous behaviour rather than guess.
    let return_type = match shape.returns().as_ref().map(source_ty) {
        Some(Some(result)) => Some(Box::new(result)),
        Some(None) => return None,
        None => None,
    };
    Some(RustType::ImplFnTrait {
        trait_name: "FnMut".to_string(),
        params: inputs,
        return_type,
    })
}

fn method_param_is_submit_callable_callback(method: &MethodDecl, param_name: &str) -> bool {
    crate::parser::to_snake_case(&method.name) == "submit"
        && method
            .params
            .iter()
            .any(|param| param.name == param_name && java_type_is_callable(&param.ty))
}

fn submit_callable_param_needs_synthetic_return_type(
    callable_name: Option<&str>,
    param: &crate::java_ir::Param,
) -> bool {
    callable_name.is_some_and(|name| crate::parser::to_snake_case(name) == "submit")
        && java_type_is_raw_callable(&param.ty)
}

fn submit_callable_callback_param_type(
    method: &MethodDecl,
    java_ty: &JavaType,
    facts: &FactSet,
) -> Option<RustType> {
    if !java_type_is_callable(java_ty) {
        return None;
    }
    let return_type = match java_ty {
        JavaType::Named { args, .. } => args
            .first()
            .map(|ty| rust_type_for_java_type_with_facts(ty, facts))
            .unwrap_or_else(|| {
                RustType::Named(submit_callable_synthetic_return_type_param(
                    &method.type_params,
                ))
            }),
        _ => return None,
    };
    Some(RustType::ImplFnTrait {
        trait_name: "FnMut".to_string(),
        params: Vec::new(),
        return_type: Some(Box::new(return_type)),
    })
}

fn java_type_is_raw_callable(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Named { name, args } if args.is_empty() && java_type_name_is_callable(name))
}

fn java_type_is_callable(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Named { name, .. } if java_type_name_is_callable(name))
}

fn java_type_name_is_callable(name: &str) -> bool {
    crate::stdlib::canonical_external_receiver_type_key(name)
        .rsplit('.')
        .next()
        .is_some_and(|simple| simple == "Callable")
}

pub(crate) fn method_param_is_nonescaping_functional_callback(
    method: &MethodDecl,
    param_name: &str,
) -> bool {
    let Some(callback_method) = method.params.iter().find_map(|param| {
        (param.name == param_name
            && functional_callback_param_type(&param.ty, &FactSet::default()).is_some())
        .then(|| functional_interface_method_name(&param.ty))
        .flatten()
    }) else {
        return false;
    };
    let Some(body) = &method.body else {
        return false;
    };
    let mut saw_callback_call = false;
    let mut escaped = false;
    scan_block_for_callback_param(
        body,
        param_name,
        callback_method,
        &mut saw_callback_call,
        &mut escaped,
    );
    saw_callback_call && !escaped
}

/// The single abstract method a functional-interface parameter is called through.
fn functional_interface_method_name(java_ty: &JavaType) -> Option<&'static str> {
    let JavaType::Named { name, .. } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(name.as_str());
    crate::stdlib::functional_interface_shape(simple).map(|shape| shape.method())
}

fn scan_block_for_callback_param(
    block: &crate::java_ir::Block,
    param_name: &str,
    callback_method: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
) {
    for stmt in &block.statements {
        scan_stmt_for_callback_param(
            stmt,
            param_name,
            callback_method,
            saw_callback_call,
            escaped,
        );
    }
}

fn scan_stmt_for_callback_param(
    stmt: &crate::java_ir::Stmt,
    param_name: &str,
    callback_method: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
) {
    match stmt {
        crate::java_ir::Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        crate::java_ir::Stmt::Assign { target, value } => {
            scan_expr_for_callback_param(
                target,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            scan_expr_for_callback_param(
                value,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        crate::java_ir::Stmt::Expr(expr)
        | crate::java_ir::Stmt::Return(Some(expr))
        | crate::java_ir::Stmt::Throw(expr) => {
            scan_expr_for_callback_param(
                expr,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        crate::java_ir::Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            scan_expr_for_callback_param(
                condition,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            scan_block_for_callback_param(
                then_block,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            if let Some(block) = else_block {
                scan_block_for_callback_param(
                    block,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        crate::java_ir::Stmt::While {
            condition, body, ..
        }
        | crate::java_ir::Stmt::DoWhile {
            condition, body, ..
        } => {
            scan_expr_for_callback_param(
                condition,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        crate::java_ir::Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                scan_stmt_for_callback_param(
                    stmt,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            if let Some(expr) = condition {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            for expr in update {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        crate::java_ir::Stmt::EnhancedFor { iterable, body, .. } => {
            scan_expr_for_callback_param(
                iterable,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        crate::java_ir::Stmt::Switch { selector, cases } => {
            scan_expr_for_callback_param(
                selector,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        scan_expr_for_callback_param(
                            expr,
                            param_name,
                            callback_method,
                            saw_callback_call,
                            escaped,
                        );
                    }
                }
                for stmt in &case.statements {
                    scan_stmt_for_callback_param(
                        stmt,
                        param_name,
                        callback_method,
                        saw_callback_call,
                        escaped,
                    );
                }
            }
        }
        crate::java_ir::Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            for stmt in resources {
                scan_stmt_for_callback_param(
                    stmt,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            scan_block_for_callback_param(
                body,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            for catch in catches {
                scan_block_for_callback_param(
                    &catch.body,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            if let Some(block) = finally_block {
                scan_block_for_callback_param(
                    block,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        crate::java_ir::Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                scan_expr_for_callback_param(
                    arg,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        crate::java_ir::Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            scan_block_for_callback_param(
                block,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        crate::java_ir::Stmt::Return(None)
        | crate::java_ir::Stmt::LocalType(_)
        | crate::java_ir::Stmt::Break(_)
        | crate::java_ir::Stmt::Continue(_)
        | crate::java_ir::Stmt::Unsupported(_) => {}
    }
}

fn scan_expr_for_callback_param(
    expr: &Expr,
    param_name: &str,
    callback_method: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
) {
    match expr {
        Expr::Var(name) if name == param_name => *escaped = true,
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if matches!(receiver.as_ref(), Expr::Var(receiver_name) if receiver_name == param_name)
            // The interface's own single abstract method, from
            // `functional_interface_shape` -- not a list of names kept beside
            // it. The list held six and stopped at `Comparator::compare`, so a
            // comparator parameter was never recognised as a callback and stayed
            // an opaque struct that `.compare(..)` could not be called on.
            && name.as_str() == callback_method =>
        {
            *saw_callback_call = true;
            for arg in args {
                scan_expr_for_callback_param(
                    arg,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        Expr::Call {
            receiver,
            name,
            args,
            ..
        } if native_collections_sort_consumes_callback_param(
            receiver.as_deref(),
            name,
            args,
            param_name,
        ) =>
        {
            *saw_callback_call = true;
            if let Some(receiver) = receiver {
                scan_expr_for_callback_param(
                    receiver,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            if let Some(target) = args.first() {
                scan_expr_for_callback_param(
                    target,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            scan_expr_for_callback_param(
                receiver,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        Expr::Binary { left, right, .. } => {
            scan_expr_for_callback_param(
                left,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            scan_expr_for_callback_param(
                right,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            scan_expr_for_callback_param(
                condition,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            scan_expr_for_callback_param(
                then_expr,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            scan_expr_for_callback_param(
                else_expr,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
        }
        Expr::Switch { selector, cases } => {
            scan_expr_for_callback_param(
                selector,
                param_name,
                callback_method,
                saw_callback_call,
                escaped,
            );
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        scan_expr_for_callback_param(
                            expr,
                            param_name,
                            callback_method,
                            saw_callback_call,
                            escaped,
                        );
                    }
                }
                scan_expr_for_callback_param(
                    &case.value,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                scan_expr_for_callback_param(
                    receiver,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            for arg in args {
                scan_expr_for_callback_param(
                    arg,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        Expr::New { ty, args, body, .. }
            if priority_queue_constructor_consumes_callback_param(ty, args, param_name) =>
        {
            *saw_callback_call = true;
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            if let Some(qualifier) = qualifier {
                scan_expr_for_callback_param(
                    qualifier,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            for arg in args {
                scan_expr_for_callback_param(
                    arg,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            if body
                .as_ref()
                .is_some_and(|members| members_capture_callback_param(members, param_name))
            {
                *escaped = true;
            }
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    scan_expr_for_callback_param(
                        expr,
                        param_name,
                        callback_method,
                        saw_callback_call,
                        escaped,
                    );
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for expr in values {
                scan_expr_for_callback_param(
                    expr,
                    param_name,
                    callback_method,
                    saw_callback_call,
                    escaped,
                );
            }
        }
        Expr::Lambda { body, .. } => {
            if lambda_body_captures_callback_param(body, param_name) {
                *escaped = true;
            }
        }
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => {}
    }
}

fn members_capture_callback_param(members: &[Member], param_name: &str) -> bool {
    members.iter().any(|member| match member {
        Member::Field(field) => field
            .initializer
            .as_ref()
            .is_some_and(|expr| expr_references_callback_param(expr, param_name)),
        Member::Method(method) => method
            .body
            .as_ref()
            .is_some_and(|body| block_references_callback_param(body, param_name)),
        Member::Constructor(ctor) => ctor
            .body
            .as_ref()
            .is_some_and(|body| block_references_callback_param(body, param_name)),
        Member::Type(_) | Member::Unsupported { .. } => false,
    })
}

fn lambda_body_captures_callback_param(
    body: &crate::java_ir::LambdaBody,
    param_name: &str,
) -> bool {
    match body {
        crate::java_ir::LambdaBody::Expr(expr) => expr_references_callback_param(expr, param_name),
        crate::java_ir::LambdaBody::Block(block) => {
            block_references_callback_param(block, param_name)
        }
    }
}

fn block_references_callback_param(block: &crate::java_ir::Block, param_name: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_references_callback_param(stmt, param_name))
}

fn stmt_references_callback_param(stmt: &crate::java_ir::Stmt, param_name: &str) -> bool {
    match stmt {
        crate::java_ir::Stmt::VarDecl { initializer, .. } => initializer
            .as_ref()
            .is_some_and(|expr| expr_references_callback_param(expr, param_name)),
        crate::java_ir::Stmt::Assign { target, value } => {
            expr_references_callback_param(target, param_name)
                || expr_references_callback_param(value, param_name)
        }
        crate::java_ir::Stmt::Expr(expr)
        | crate::java_ir::Stmt::Return(Some(expr))
        | crate::java_ir::Stmt::Throw(expr) => expr_references_callback_param(expr, param_name),
        crate::java_ir::Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_references_callback_param(condition, param_name)
                || block_references_callback_param(then_block, param_name)
                || else_block
                    .as_ref()
                    .is_some_and(|block| block_references_callback_param(block, param_name))
        }
        crate::java_ir::Stmt::While {
            condition, body, ..
        }
        | crate::java_ir::Stmt::DoWhile {
            condition, body, ..
        } => {
            expr_references_callback_param(condition, param_name)
                || block_references_callback_param(body, param_name)
        }
        crate::java_ir::Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_references_callback_param(stmt, param_name))
                || condition
                    .as_ref()
                    .is_some_and(|expr| expr_references_callback_param(expr, param_name))
                || update
                    .iter()
                    .any(|expr| expr_references_callback_param(expr, param_name))
                || block_references_callback_param(body, param_name)
        }
        crate::java_ir::Stmt::EnhancedFor { iterable, body, .. } => {
            expr_references_callback_param(iterable, param_name)
                || block_references_callback_param(body, param_name)
        }
        crate::java_ir::Stmt::Switch { selector, cases } => {
            expr_references_callback_param(selector, param_name)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_references_callback_param(expr, param_name)
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || case
                        .statements
                        .iter()
                        .any(|stmt| stmt_references_callback_param(stmt, param_name))
                })
        }
        crate::java_ir::Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            resources
                .iter()
                .any(|resource| stmt_references_callback_param(resource, param_name))
                || block_references_callback_param(body, param_name)
                || catches
                    .iter()
                    .any(|catch| block_references_callback_param(&catch.body, param_name))
                || finally_block
                    .as_ref()
                    .is_some_and(|block| block_references_callback_param(block, param_name))
        }
        crate::java_ir::Stmt::ConstructorInvocation { args, .. } => args
            .iter()
            .any(|expr| expr_references_callback_param(expr, param_name)),
        crate::java_ir::Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_references_callback_param(block, param_name)
        }
        crate::java_ir::Stmt::Return(None)
        | crate::java_ir::Stmt::LocalType(_)
        | crate::java_ir::Stmt::Break(_)
        | crate::java_ir::Stmt::Continue(_)
        | crate::java_ir::Stmt::Unsupported(_) => false,
    }
}

fn expr_references_callback_param(expr: &Expr, param_name: &str) -> bool {
    match expr {
        Expr::Var(name) => name == param_name,
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            expr_references_callback_param(receiver, param_name)
        }
        Expr::Binary { left, right, .. } => {
            expr_references_callback_param(left, param_name)
                || expr_references_callback_param(right, param_name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_references_callback_param(condition, param_name)
                || expr_references_callback_param(then_expr, param_name)
                || expr_references_callback_param(else_expr, param_name)
        }
        Expr::Switch { selector, cases } => {
            expr_references_callback_param(selector, param_name)
                || cases.iter().any(|case| {
                    case.labels.iter().any(|label| match label {
                        crate::java_ir::SwitchLabel::Expr(expr) => {
                            expr_references_callback_param(expr, param_name)
                        }
                        crate::java_ir::SwitchLabel::Default => false,
                    }) || expr_references_callback_param(&case.value, param_name)
                })
        }
        Expr::Call { receiver, args, .. } => {
            receiver
                .as_ref()
                .is_some_and(|receiver| expr_references_callback_param(receiver, param_name))
                || args
                    .iter()
                    .any(|arg| expr_references_callback_param(arg, param_name))
        }
        Expr::New {
            qualifier,
            args,
            body,
            ..
        } => {
            qualifier
                .as_ref()
                .is_some_and(|qualifier| expr_references_callback_param(qualifier, param_name))
                || args
                    .iter()
                    .any(|arg| expr_references_callback_param(arg, param_name))
                || body
                    .as_ref()
                    .is_some_and(|members| members_capture_callback_param(members, param_name))
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_references_callback_param(expr, param_name))
                || initializer.as_ref().is_some_and(|values| {
                    values
                        .iter()
                        .any(|expr| expr_references_callback_param(expr, param_name))
                })
        }
        Expr::ArrayInitializer(values) => values
            .iter()
            .any(|expr| expr_references_callback_param(expr, param_name)),
        Expr::Lambda { body, .. } => lambda_body_captures_callback_param(body, param_name),
        Expr::Literal(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_)
        | Expr::Unsupported { .. } => false,
    }
}

fn native_collections_sort_consumes_callback_param(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    param_name: &str,
) -> bool {
    if name != "sort" || args.len() != 2 {
        return false;
    }
    let Some(Expr::Var(receiver_name)) = receiver else {
        return false;
    };
    crate::stdlib::canonical_external_receiver_type_key(receiver_name) == "Collections"
        && matches!(&args[1], Expr::Var(arg_name) if arg_name == param_name)
}

fn priority_queue_constructor_consumes_callback_param(
    ty: &JavaType,
    args: &[Expr],
    param_name: &str,
) -> bool {
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = ty else {
        return false;
    };
    crate::stdlib::canonical_external_receiver_type_key(name)
        .rsplit('.')
        .next()
        .is_some_and(|simple| simple == "PriorityQueue")
        && matches!(args, [Expr::Var(arg_name)] if arg_name == param_name)
}

fn method_stores_priority_queue_comparator_callback(method: &MethodDecl, param_name: &str) -> bool {
    method
        .body
        .as_ref()
        .is_some_and(|body| block_stores_priority_queue_comparator_callback(body, param_name))
}

fn block_stores_priority_queue_comparator_callback(block: &Block, param_name: &str) -> bool {
    block
        .statements
        .iter()
        .any(|stmt| stmt_stores_priority_queue_comparator_callback(stmt, param_name))
}

fn stmt_stores_priority_queue_comparator_callback(stmt: &Stmt, param_name: &str) -> bool {
    match stmt {
        Stmt::Return(Some(expr)) | Stmt::Expr(expr) | Stmt::Throw(expr) => {
            expr_stores_priority_queue_comparator_callback(expr, param_name)
        }
        Stmt::VarDecl {
            initializer: Some(expr),
            ..
        } => expr_stores_priority_queue_comparator_callback(expr, param_name),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            expr_stores_priority_queue_comparator_callback(condition, param_name)
                || block_stores_priority_queue_comparator_callback(then_block, param_name)
                || else_block.as_ref().is_some_and(|block| {
                    block_stores_priority_queue_comparator_callback(block, param_name)
                })
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            body, condition, ..
        } => {
            expr_stores_priority_queue_comparator_callback(condition, param_name)
                || block_stores_priority_queue_comparator_callback(body, param_name)
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            init.iter()
                .any(|stmt| stmt_stores_priority_queue_comparator_callback(stmt, param_name))
                || condition.as_ref().is_some_and(|expr| {
                    expr_stores_priority_queue_comparator_callback(expr, param_name)
                })
                || update
                    .iter()
                    .any(|expr| expr_stores_priority_queue_comparator_callback(expr, param_name))
                || block_stores_priority_queue_comparator_callback(body, param_name)
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            block_stores_priority_queue_comparator_callback(block, param_name)
        }
        _ => false,
    }
}

fn expr_stores_priority_queue_comparator_callback(expr: &Expr, param_name: &str) -> bool {
    match expr {
        Expr::New {
            ty,
            qualifier,
            args,
            ..
        } => {
            priority_queue_constructor_consumes_callback_param(ty, args, param_name)
                || qualifier.as_ref().is_some_and(|qualifier| {
                    expr_stores_priority_queue_comparator_callback(qualifier, param_name)
                })
                || args
                    .iter()
                    .any(|arg| expr_stores_priority_queue_comparator_callback(arg, param_name))
        }
        Expr::Call { receiver, args, .. } => {
            receiver.as_ref().is_some_and(|receiver| {
                expr_stores_priority_queue_comparator_callback(receiver, param_name)
            }) || args
                .iter()
                .any(|arg| expr_stores_priority_queue_comparator_callback(arg, param_name))
        }
        Expr::Binary { left, right, .. } => {
            expr_stores_priority_queue_comparator_callback(left, param_name)
                || expr_stores_priority_queue_comparator_callback(right, param_name)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_stores_priority_queue_comparator_callback(condition, param_name)
                || expr_stores_priority_queue_comparator_callback(then_expr, param_name)
                || expr_stores_priority_queue_comparator_callback(else_expr, param_name)
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            expr_stores_priority_queue_comparator_callback(receiver, param_name)
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            dimensions
                .iter()
                .any(|expr| expr_stores_priority_queue_comparator_callback(expr, param_name))
                || initializer.as_ref().is_some_and(|values| {
                    values.iter().any(|expr| {
                        expr_stores_priority_queue_comparator_callback(expr, param_name)
                    })
                })
        }
        Expr::ArrayInitializer(values) => values
            .iter()
            .any(|expr| expr_stores_priority_queue_comparator_callback(expr, param_name)),
        _ => false,
    }
}

fn maybe_add_lifetime_to_refs(ty: RustType, lifetime: bool) -> RustType {
    if lifetime {
        add_lifetime_to_refs(ty)
    } else {
        ty
    }
}

fn add_lifetime_to_refs(ty: RustType) -> RustType {
    match ty {
        RustType::Ref { mutable, inner, .. } => {
            let inner = add_lifetime_to_refs(*inner);
            RustType::Ref {
                mutable,
                lifetime: Some("'a".to_string()),
                inner: Box::new(inner),
            }
        }
        RustType::Option(inner) => RustType::Option(Box::new(add_lifetime_to_refs(*inner))),
        RustType::Vec(inner) => RustType::Vec(Box::new(add_lifetime_to_refs(*inner))),
        RustType::Array2(inner) => RustType::Array2(Box::new(add_lifetime_to_refs(*inner))),
        RustType::Slice(inner) => RustType::Slice(Box::new(add_lifetime_to_refs(*inner))),
        RustType::ImplFnTrait {
            trait_name,
            params,
            return_type,
        } => RustType::ImplFnTrait {
            trait_name,
            params: params.into_iter().map(add_lifetime_to_refs).collect(),
            return_type: return_type.map(|ty| Box::new(add_lifetime_to_refs(*ty))),
        },
        other => other,
    }
}

fn rust_type_contains_ref(ty: &RustType) -> bool {
    match ty {
        RustType::Ref { .. } => true,
        RustType::Option(inner)
        | RustType::Vec(inner)
        | RustType::Array2(inner)
        | RustType::Slice(inner) => rust_type_contains_ref(inner),
        RustType::Generic { args, .. } => args.iter().any(rust_type_contains_ref),
        RustType::Tuple(items) => items.iter().any(rust_type_contains_ref),
        RustType::FnPointer {
            params,
            return_type,
        } => params.iter().any(rust_type_contains_ref) || rust_type_contains_ref(return_type),
        RustType::ImplFnTrait {
            params,
            return_type,
            ..
        } => {
            params.iter().any(rust_type_contains_ref)
                || return_type.as_deref().is_some_and(rust_type_contains_ref)
        }
        RustType::Unit
        | RustType::Bool
        | RustType::I8
        | RustType::I16
        | RustType::I32
        | RustType::I64
        | RustType::F32
        | RustType::F64
        | RustType::Char
        | RustType::String
        | RustType::Str
        | RustType::DynTrait(_)
        | RustType::ImplTrait(_)
        | RustType::Named(_)
        | RustType::Raw(_) => false,
    }
}

fn method_return_type_with_optional_lifetime(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
    lifetime: bool,
    representations: Option<&RepresentationPlan>,
) -> Option<RustType> {
    let base = method_return_type_with_optional_representations(
        ty,
        method,
        signatures,
        facts,
        representations,
    )
    .map(|ty| maybe_add_lifetime_to_refs(ty, lifetime));
    if exceptions.wrap_returns_in_result() {
        Some(exception_result_type(base.unwrap_or(RustType::Unit)))
    } else {
        base
    }
}

fn method_return_type_with_optional_representations(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> Option<RustType> {
    if let Some(ty) = rust_crate_mapped_external_rust_type(&method.return_type, facts) {
        return Some(ty);
    }
    let return_shape = method_return_shape(ty, method, signatures);
    if let Some(shape) = return_shape.as_ref() {
        return Some(rust_type_for_slot_with_optional_representations(
            shape,
            facts,
            representations,
        ));
    }
    if rust_crate_mapped_external_java_type(&method.return_type, facts) {
        return Some(rust_type_for_java_type_with_facts(
            &method.return_type,
            facts,
        ));
    }
    None
}

fn rust_type_for_slot_with_optional_representations(
    shape: &RustSlotShape,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> RustType {
    representations
        .map(|representations| {
            crate::slot_render::rust_type_for_slot_with_facts_and_representations(
                shape,
                facts,
                representations,
            )
        })
        .unwrap_or_else(|| rust_type_for_slot_with_facts(shape, facts))
}

fn method_receiver(ty: &TypeDecl, method: &MethodDecl, signatures: &SignaturePlan) -> ReceiverPlan {
    if method.modifiers.contains(&Modifier::Static) {
        ReceiverPlan::None
    } else {
        signatures.required_method_signature(ty, method).receiver()
    }
}

fn lower_receiver(receiver: ReceiverPlan) -> Option<RustReceiver> {
    match receiver {
        ReceiverPlan::None => None,
        ReceiverPlan::Shared => Some(RustReceiver::Shared),
        ReceiverPlan::Mutable => Some(RustReceiver::Mutable),
    }
}
