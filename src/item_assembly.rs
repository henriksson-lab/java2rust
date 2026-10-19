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
#[cfg(test)]
use crate::source_callbacks::{
    expr_references_callback_param, expr_stores_priority_queue_comparator_callback,
    method_param_is_nonescaping_functional_callback,
    method_param_is_nonescaping_functional_callback_with_forwarding,
};
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
        associated_consts: associated_consts_for_type(
            ty,
            &std::collections::BTreeMap::new(),
            support_program,
        ),
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
    associated_const_value_overrides: &std::collections::BTreeMap<String, RustExpr>,
) -> Vec<RustItem> {
    let associated_consts =
        associated_consts_for_type(ty, associated_const_value_overrides, support_program);
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

/// Project dependent Java bounds using their lexical declarations. A bound
/// naming another type parameter is a widening capability, not a Rust trait.
/// Retain the upper parameter's declared capabilities transitively as well.
pub(crate) fn rust_bounds_for_java_type_param_in_scope(
    bound: &JavaType,
    support_programs: &[&Program],
    owner: Option<&TypeDecl>,
    callable_params: &[String],
    callable_bounds: &[Option<JavaType>],
) -> Vec<String> {
    fn project(
        bound: &JavaType,
        programs: &[&Program],
        owner: Option<&TypeDecl>,
        names: &[String],
        bounds: &[Option<JavaType>],
        visiting: &mut HashSet<String>,
    ) -> Vec<String> {
        if let JavaType::Intersection(parts) = bound {
            let mut result = Vec::new();
            for part in parts {
                for capability in project(part, programs, owner, names, bounds, visiting) {
                    if !result.contains(&capability) {
                        result.push(capability);
                    }
                }
            }
            return result;
        }
        if let JavaType::Named { name, args } = bound
            && args.is_empty()
        {
            let declared = names
                .iter()
                .position(|candidate| candidate == name)
                .map(|index| bounds.get(index).and_then(Option::as_ref))
                .or_else(|| {
                    owner.and_then(|owner| {
                        owner
                            .type_params
                            .iter()
                            .position(|candidate| candidate == name)
                            .map(|index| {
                                owner.type_param_bounds.get(index).and_then(Option::as_ref)
                            })
                    })
                });
            if let Some(upper) = declared {
                // This name was proved to be a binder, not a Java nominal
                // type. In particular Integer/Object must not be unboxed or
                // erased by the ordinary type-argument renderer.
                let mut result = vec![format!("Into<{name}>")];
                if visiting.insert(name.clone()) {
                    if let Some(upper) = upper {
                        for capability in project(upper, programs, owner, names, bounds, visiting) {
                            if !result.contains(&capability) {
                                result.push(capability);
                            }
                        }
                    }
                    visiting.remove(name);
                }
                return result;
            }
        }
        if let (Some(owner), JavaType::Named { name, args }) = (owner, bound) {
            // Resolve the nominal bound in its lexical owner before the
            // shared capability projection. This also serves method bodies
            // and trait signatures; neither should invent a fallback policy
            // for ambiguous same-simple-name declarations.
            let qualified = programs.iter().find_map(|program| {
                crate::inherited_members::find_type_decl_for_owner_context(
                    program,
                    &owner.name,
                    name,
                )
            });
            if let Some(declaration) = qualified {
                return rust_bounds_for_java_type_param(
                    &JavaType::Named {
                        name: declaration.name.clone(),
                        args: args.clone(),
                    },
                    programs,
                );
            }
        }
        rust_bounds_for_java_type_param(bound, programs)
    }
    project(
        bound,
        support_programs,
        owner,
        callable_params,
        callable_bounds,
        &mut HashSet::new(),
    )
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
                    bounds.extend(rust_bounds_for_java_type_param_in_scope(
                        bound,
                        &[support_program],
                        Some(ty),
                        &[],
                        &[],
                    ));
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

pub(crate) fn java_type_uses_type_param_as_hash_key(ty: &JavaType, type_param: &str) -> bool {
    let JavaType::Named { args, .. } = ty else {
        return false;
    };
    java_type_directly_uses_type_param_as_hash_key(ty, type_param)
        || args
            .iter()
            .any(|arg| java_type_uses_type_param_as_hash_key(arg, type_param))
}

fn java_type_directly_uses_type_param_as_hash_key(ty: &JavaType, type_param: &str) -> bool {
    let JavaType::Named { name, args } = ty else {
        return false;
    };
    let simple = name.rsplit('.').next().unwrap_or(name);
    match simple {
        "Map" | "HashMap" | "LinkedHashMap" | "Hashtable" | "ConcurrentHashMap" => args
            .first()
            .is_some_and(|key| java_type_contains_type_param(key, type_param)),
        "Set" | "HashSet" | "LinkedHashSet" => args
            .first()
            .is_some_and(|element| java_type_contains_type_param(element, type_param)),
        _ => false,
    }
}

fn java_type_requires_hash_capability_for_type_param(
    ty: &JavaType,
    type_param: &str,
    support_programs: &[&Program],
    visiting: &mut HashSet<(String, String)>,
) -> bool {
    let JavaType::Named { name, args } = ty else {
        return false;
    };
    let Some(owner) = source_type_bound_decl(name, support_programs) else {
        // Mentioning a key-bearing carrier as a list element/map value does
        // not hash that carrier's payload. Only this instantiation's own key
        // position supplies the native requirement. Parsed source definitions
        // above take precedence over same-named platform collections.
        return java_type_directly_uses_type_param_as_hash_key(ty, type_param);
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
fn callable_type_param_needs_clone_bound_excluding(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    clone_shielded_names: &HashSet<String>,
) -> bool {
    let Some(body) = body else {
        return false;
    };
    let mut typed_params = params
        .iter()
        .filter(|param| java_type_contains_type_param(&param.ty, type_param))
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    // Reusing an owned local T has the same copying obligation as reusing a
    // parameter T. Do not extend this rule to arbitrary carriers mentioning T:
    // copying an interface/reference handle need not copy its payload.
    collect_matching_local_names(
        body,
        &|ty| {
            matches!(ty, JavaType::Named { name, args } if name == type_param && args.is_empty())
                || matches!(ty, JavaType::Unknown(name) if name == type_param)
        },
        &mut typed_params,
    );
    typed_params.retain(|name| !clone_shielded_names.contains(name));
    // Several lowering obligations arise from locals and enhanced-for
    // bindings, not only from formal parameters.  This is especially common
    // for class type parameters: a method can copy `T` values between local
    // collections without accepting a `T`-typed argument.  Evaluate these
    // whole-body capability analyses before the parameter-only fast path.
    if callable_type_param_repeating_region_needs_clone_bound(
        type_param,
        params,
        body,
        clone_shielded_names,
    ) || callable_type_param_collection_get_needs_clone_bound(type_param, params, body)
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
        .any(|name| counts.get(name).copied().unwrap_or(0) > 1)
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
    clone_shielded_names: &HashSet<String>,
) -> bool {
    let mut typed_names = params
        .iter()
        .filter(|param| java_type_contains_type_param(&param.ty, type_param))
        .map(|param| param.name.clone())
        .collect::<HashSet<_>>();
    collect_type_param_local_names(body, type_param, &mut typed_names);
    typed_names.retain(|name| !clone_shielded_names.contains(name));
    block_repeating_region_reads_any(body, &typed_names)
}

fn collect_type_param_local_names(block: &Block, type_param: &str, names: &mut HashSet<String>) {
    collect_matching_local_names(
        block,
        &|ty| java_type_contains_type_param(ty, type_param),
        names,
    );
}

fn collect_matching_local_names(
    block: &Block,
    matches_type: &impl Fn(&JavaType) -> bool,
    names: &mut HashSet<String>,
) {
    for stmt in &block.statements {
        match stmt {
            Stmt::VarDecl { ty, name, .. } => {
                if matches_type(ty) {
                    names.insert(name.clone());
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_matching_local_names(then_block, matches_type, names);
                if let Some(else_block) = else_block {
                    collect_matching_local_names(else_block, matches_type, names);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Block(body)
            | Stmt::LabeledBlock { body, .. } => {
                collect_matching_local_names(body, matches_type, names);
            }
            Stmt::For { init, body, .. } => {
                collect_matching_local_names(
                    &Block {
                        statements: init.clone(),
                    },
                    matches_type,
                    names,
                );
                collect_matching_local_names(body, matches_type, names);
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                body,
                ..
            } => {
                if matches_type(var_type) {
                    names.insert(var_name.clone());
                }
                collect_matching_local_names(body, matches_type, names);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_matching_local_names(
                        &Block {
                            statements: case.statements.clone(),
                        },
                        matches_type,
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
                collect_matching_local_names(
                    &Block {
                        statements: resources.clone(),
                    },
                    matches_type,
                    names,
                );
                collect_matching_local_names(body, matches_type, names);
                for catch in catches {
                    collect_matching_local_names(&catch.body, matches_type, names);
                }
                if let Some(finally_block) = finally_block {
                    collect_matching_local_names(finally_block, matches_type, names);
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
        None,
        None,
        None,
    )
}

type CloneRequirementKey = (String, String, String);

#[derive(Debug, Default)]
struct CloneRequirementSearch<'a> {
    storage_context: Option<CloneStorageContext<'a>>,
    memo: CloneRequirementMemoAccess<'a>,
}

#[derive(Debug, Default)]
struct CloneRequirementMemo {
    visiting: HashSet<CloneRequirementKey>,
    trait_impl_visiting: HashSet<(String, String, String, TraitImplCapability)>,
    method_visiting: Vec<(String, MethodDecl, String)>,
    method_memo: Vec<(String, MethodDecl, String, bool)>,
    inherited_fields: HashMap<String, HashMap<String, JavaType>>,
    inherited_dispatch: HashMap<(String, String, usize), Option<(String, String, usize, usize)>>,
    stored_fields: HashMap<String, std::sync::Arc<HashMap<String, RustType>>>,
}

/// Translation-scoped memoization for inferred `Clone` capabilities.
///
/// A single source method can be queried while rendering its signature and
/// many more times while lowering moves in its body. In inheritance-heavy
/// generic code those queries recursively inspect the same call graph. Keep
/// the traversal state local to one translation, while sharing completed
/// answers across every emitted method in that translation.
#[derive(Debug, Default)]
pub(crate) struct CloneRequirementCache<'a> {
    storage_context: Option<CloneStorageContext<'a>>,
    search: RefCell<CloneRequirementMemo>,
}

#[derive(Debug)]
enum CloneRequirementMemoAccess<'a> {
    Owned(CloneRequirementMemo),
    Borrowed(&'a mut CloneRequirementMemo),
}

impl Default for CloneRequirementMemoAccess<'_> {
    fn default() -> Self {
        Self::Owned(CloneRequirementMemo::default())
    }
}

impl std::ops::Deref for CloneRequirementSearch<'_> {
    type Target = CloneRequirementMemo;
    fn deref(&self) -> &Self::Target {
        match &self.memo {
            CloneRequirementMemoAccess::Owned(memo) => memo,
            CloneRequirementMemoAccess::Borrowed(memo) => memo,
        }
    }
}

impl std::ops::DerefMut for CloneRequirementSearch<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.memo {
            CloneRequirementMemoAccess::Owned(memo) => memo,
            CloneRequirementMemoAccess::Borrowed(memo) => memo,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CloneStorageContext<'a> {
    facts: &'a FactSet,
    representations: &'a RepresentationPlan,
    signatures: &'a SignaturePlan,
    program: &'a Program,
}

impl<'a> CloneRequirementCache<'a> {
    pub(crate) fn with_storage_context(
        facts: &'a FactSet,
        representations: &'a RepresentationPlan,
        signatures: &'a SignaturePlan,
        program: &'a Program,
    ) -> Self {
        Self {
            storage_context: Some(CloneStorageContext {
                facts,
                representations,
                signatures,
                program,
            }),
            ..Default::default()
        }
    }

    fn search_with_memo<'b>(
        &'b self,
        memo: &'b mut CloneRequirementMemo,
    ) -> CloneRequirementSearch<'b> {
        CloneRequirementSearch {
            storage_context: self.storage_context,
            memo: CloneRequirementMemoAccess::Borrowed(memo),
        }
    }
}

impl CloneRequirementSearch<'_> {
    fn stored_field_types(
        &mut self,
        ty: &TypeDecl,
    ) -> Option<std::sync::Arc<HashMap<String, RustType>>> {
        let context = self.storage_context?;
        if let Some(fields) = self.stored_fields.get(&ty.name) {
            return Some(fields.clone());
        }
        let fields = std::sync::Arc::new(
            struct_fields_for_type(
                ty,
                context.signatures,
                context.facts,
                Some(context.representations),
                context.program,
            )
            .into_iter()
            .map(|field| (field.name, field.ty))
            .collect::<HashMap<_, _>>(),
        );
        self.stored_fields.insert(ty.name.clone(), fields.clone());
        Some(fields)
    }

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
    let mut memo = cache.search.borrow_mut();
    let mut search = cache.search_with_memo(&mut memo);
    source_method_emits_clone_bound_for_type_param_inner(
        ty,
        method,
        type_param,
        representations,
        support_programs,
        &mut search,
    )
}

fn source_method_emits_clone_bound_for_type_param_inner(
    ty: &TypeDecl,
    method: &MethodDecl,
    type_param: &str,
    representations: Option<&RepresentationPlan>,
    support_programs: &[&Program],
    search: &mut CloneRequirementSearch,
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
                search,
                Some(ty),
                Some(&method.return_type),
                Some(&method.name),
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
    let fields = search.inherited_fields(ty, support_programs);
    source_method_class_type_param_needs_clone_bound(
        ty,
        method,
        type_param,
        uses_shared_storage,
        &fields,
        support_programs,
        search,
    )
}

/// Constructor counterpart of
/// [`source_method_emits_clone_bound_for_type_param_with_cache`].
pub(crate) fn source_constructor_emits_clone_bound_for_type_param_with_cache(
    ty: &TypeDecl,
    constructor: &ConstructorDecl,
    type_param: &str,
    support_programs: &[&Program],
    cache: &CloneRequirementCache,
) -> bool {
    let mut memo = cache.search.borrow_mut();
    let mut search = cache.search_with_memo(&mut memo);
    source_constructor_emits_clone_bound_for_type_param_inner(
        ty,
        constructor,
        type_param,
        support_programs,
        &mut search,
    )
}

fn source_constructor_emits_clone_bound_for_type_param_inner(
    ty: &TypeDecl,
    constructor: &ConstructorDecl,
    type_param: &str,
    support_programs: &[&Program],
    search: &mut CloneRequirementSearch,
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
            || callable_type_param_needs_clone_bound_with_support_contexts_inner(
                type_param,
                &constructor.params,
                constructor.body.as_ref(),
                support_programs,
                search,
                Some(ty),
                None,
                None,
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
    declared
        || source_constructor_class_type_param_needs_clone_bound(
            ty,
            constructor,
            type_param,
            support_programs,
            search,
        )
}

fn callable_type_param_needs_clone_bound_with_support_contexts_inner(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&crate::java_ir::Block>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
    owner: Option<&TypeDecl>,
    return_type: Option<&JavaType>,
    callable_name: Option<&str>,
) -> bool {
    if retained_iteration_requires_parameter(
        type_param,
        params,
        body,
        owner,
        return_type,
        callable_name,
        visiting.storage_context,
        false,
    ) {
        return true;
    }
    let shielded = retained_clone_shielded_names(
        type_param,
        params,
        body,
        owner,
        return_type,
        callable_name,
        visiting.storage_context,
    );
    if callable_type_param_needs_clone_bound_excluding(type_param, params, body, &shielded) {
        return true;
    }
    if callable_stream_materialization_capabilities(type_param, params, body, support_programs).0 {
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
    let mut callable_types = owner
        .map(|owner| source_callable_type_env(owner, params))
        .unwrap_or_default();
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
            ) || expr_call_requires_clone_capability(
                expr,
                type_param,
                &callable_types,
                support_programs,
                visiting,
            );
        }
    });
    constructed_owner_requires_clone
        || callable_map_view_materialization_requires_clone(
            type_param,
            body,
            &callable_types,
            support_programs,
            visiting,
        )
        || callable_source_interface_conversion_requires_capability(
            type_param,
            body,
            return_type,
            &callable_types,
            support_programs,
            TraitImplCapability::Clone,
            visiting,
        )
        || (!typed_params.is_empty()
            && block_delegates_type_param_to_clone_requiring_constructor(
                body,
                &typed_params,
                support_programs,
                visiting,
            ))
}

/// Both receivers and selected method instantiations can carry caller variables
/// without a value argument: e.g. `new Owner<U>().read()` or a generic factory.
/// Propagate requirements by binder substitution, not by argument spelling.
fn callable_map_view_materialization_requires_clone(
    type_param: &str,
    body: &Block,
    types: &TypeEnv,
    support_programs: &[&Program],
    search: &CloneRequirementSearch,
) -> bool {
    let copied_element = |expr: &Expr| {
        let Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            resolved,
            ..
        } = expr
        else {
            return None;
        };
        let receiver_ty = infer_expr_type(receiver, types);
        let (key, value) = crate::stdlib::java_map_type_args(&receiver_ty)?;
        if resolved
            .as_ref()
            .is_some_and(|call| source_method_for_resolved_call(call, support_programs).is_some())
            || (resolved.is_none()
                && matches!(&receiver_ty, JavaType::Named { name, .. } if source_type_bound_decl(name, support_programs).is_some()))
        {
            return None;
        }
        let crate::call_plan::CallTargetPlan::NativeCollectionMethod { method, .. } =
            crate::call_plan::plan_native_collection_receiver_call_for_type(
                receiver,
                name,
                Some(args),
                &receiver_ty,
                types,
            )?
        else {
            return None;
        };
        match method {
            crate::call_plan::NativeCollectionMethod::MapKeys => Some(key.clone()),
            crate::call_plan::NativeCollectionMethod::MapValues => Some(value.clone()),
            _ => None,
        }
    };
    let requires = |expr: &Expr| {
        copied_element(expr).is_some_and(|element| {
            stored_java_type_clone_requires_parameter(&element, type_param, search.storage_context)
        })
    };
    let mut required = false;
    // Returning a native view currently materializes its owned Rust carrier.
    // A mere size/contains call on the view does not copy any map elements.
    crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| {
        if let Stmt::Return(Some(expr)) = stmt {
            required |= requires(expr);
        }
    });
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if let Expr::New { ty, args, .. } = expr
            && crate::stdlib::native_collection_constructor_kind_for_type(ty).is_some()
            && !matches!(ty, JavaType::Named { name, .. } if source_type_bound_decl(name, support_programs).is_some())
            && let [view] = args.as_slice()
        {
            required |= requires(view);
        }
    });
    required
}

fn expr_call_requires_clone_capability(
    expr: &Expr,
    type_param: &str,
    caller_types: &TypeEnv,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return false;
    };
    let receiver_ty = receiver
        .as_ref()
        .map(|receiver| infer_expr_type(receiver, caller_types));
    let carries_parameter = receiver_ty
        .as_ref()
        .is_some_and(|ty| java_type_contains_type_param(ty, type_param))
        || resolved.as_ref().is_some_and(|call| {
            java_type_contains_type_param(&call.return_type, type_param)
                || call
                    .arg_target_types
                    .iter()
                    .any(|ty| java_type_contains_type_param(ty, type_param))
        });
    if !carries_parameter {
        return false;
    }
    let Some((owner, method, rust_name)) = source_call_for_capability_requirement(
        resolved.as_ref(),
        receiver.as_deref(),
        name,
        args.len(),
        caller_types,
        support_programs,
    ) else {
        return false;
    };
    if let Some(call) = resolved {
        let substitutions =
            crate::resolve::resolved_method_type_substitutions(&owner.type_params, method, call);
        if method.type_params.iter().any(|parameter| {
            if !substitutions.get(parameter).is_some_and(|actual| {
                stored_java_type_clone_requires_parameter(
                    actual,
                    type_param,
                    visiting.storage_context,
                )
            }) {
                return false;
            }
            let key = (owner.name.clone(), rust_name.clone(), parameter.clone());
            if !visiting.insert(key.clone()) {
                return false;
            }
            let required = callable_type_param_needs_clone_bound_with_support_contexts_inner(
                parameter,
                &method.params,
                method.body.as_ref(),
                support_programs,
                visiting,
                Some(owner),
                Some(&method.return_type),
                Some(&method.name),
            );
            visiting.remove(&key);
            required
        }) {
            return true;
        }
    }
    if method.modifiers.contains(&Modifier::Static) {
        return false;
    }
    let Some(receiver_ty) = receiver_ty else {
        return false;
    };
    if !java_type_contains_type_param(&receiver_ty, type_param) {
        return false;
    }
    let substitutions =
        source_receiver_owner_type_substitutions(owner, method, &receiver_ty, support_programs);
    let fields = visiting.inherited_fields(owner, support_programs);
    owner.type_params.iter().any(|parameter| {
        if !substitutions.get(parameter).is_some_and(|actual| {
            stored_java_type_clone_requires_parameter(actual, type_param, visiting.storage_context)
        }) {
            return false;
        }
        let key = (owner.name.clone(), rust_name.clone(), parameter.clone());
        if !visiting.insert(key.clone()) {
            return false;
        }
        let required = source_method_class_type_param_needs_clone_bound(
            owner,
            method,
            parameter,
            false,
            &fields,
            support_programs,
            visiting,
        );
        visiting.remove(&key);
        required
    })
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
    args: &[Expr],
    typed_params: &HashSet<&str>,
    support_programs: &[&Program],
    visiting: &mut CloneRequirementSearch,
) -> bool {
    let Some((owner, method)) = source_method_for_resolved_call(resolved, support_programs) else {
        return false;
    };
    // Receiver requirements are handled by
    // expr_call_requires_clone_capability, which projects the
    // instantiated receiver arguments. A name-only receiver test here used
    // to impose every owner capability on every mentioned caller parameter.
    args.iter().enumerate().any(|(arg_index, arg)| {
        if !expr_reads_named_type_param_param(arg, typed_params) {
            return false;
        }
        let Some(target_param) = method.params.get(arg_index) else {
            return false;
        };
        // Callable-owned binders are already projected through the resolver's
        // selected instantiation by expr_call_requires_clone_capability.
        // Inferring those again from an argument name would incorrectly copy
        // K's requirement onto V when one argument is a Map<K, V>.
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
                    Some(owner),
                    None,
                    None,
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
    stored_fields: Option<&HashMap<String, RustType>>,
    storage_context: Option<CloneStorageContext<'_>>,
) -> bool {
    let Some(body) = &method.body else {
        return false;
    };
    let mut lexical_types = TypeEnv::new();
    for (name, ty) in fields {
        lexical_types.insert_field(name.clone(), ty.clone());
    }
    for param in &method.params {
        lexical_types.insert_param(param.name.clone(), param.ty.clone());
        lexical_types.insert_param(param.source_name.clone(), param.ty.clone());
    }
    for statement in &body.statements {
        lexical_types.learn_stmt(statement);
    }
    let needs_clone = |ty: &JavaType| {
        if !java_type_contains_type_param(ty, type_param) {
            return false;
        }
        let Some(stored) = stored_fields else {
            return true;
        };
        let mut matched = false;
        for (name, field_ty) in fields {
            if field_ty != ty {
                continue;
            }
            let Some(actual) = stored
                .get(name)
                .or_else(|| stored.get(&crate::parser::to_snake_case(name)))
            else {
                return true;
            };
            matched = true;
            if rust_clone_requires_parameter(actual, type_param) {
                return true;
            }
        }
        !matched
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
                    && (expr_java_type(initializer, fields, &lexical_types).is_some_and(|field_ty| {
                            needs_clone(&field_ty)
                        })
                        || expr_recovers_owned_class_type_param_from_field(
                            initializer,
                            type_param,
                            fields,
                            &needs_clone,
                            &lexical_types,
                        ))
        )
    }) {
        // Reading a generic field into an owned local cannot move the field;
        // the emitted `.cloned()` requires this capability on the class impl.
        return true;
    }
    let mut copies_argument_field = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if !copies_argument_field {
            let args = match expr {
                Expr::Call { args, .. } | Expr::New { args, .. } => args,
                _ => return,
            };
            let candidates = args
                .iter()
                .enumerate()
                .filter_map(|(index, argument)| {
                    expr_recovers_owned_class_type_param_from_field(
                        argument,
                        type_param,
                        fields,
                        &needs_clone,
                        &lexical_types,
                    )
                    .then_some(index)
                })
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                return;
            }
            // The shared argument plan owns the borrowing decision. Merely
            // mentioning a field in e.g. contains(field) does not copy it.
            let plan = storage_context.map(|context| match expr {
                Expr::Call {
                    receiver,
                    name,
                    resolved,
                    args,
                    ..
                } => crate::call_plan::plan_call_arguments_with_facts_and_args(
                    receiver.as_deref(),
                    name,
                    resolved.as_ref(),
                    args,
                    context.signatures,
                    context.facts,
                    &lexical_types,
                ),
                Expr::New { resolved, args, .. } => crate::call_plan::plan_constructor_arguments(
                    resolved.as_ref(),
                    args.len(),
                    context.signatures,
                ),
                _ => unreachable!(),
            });
            let owns = |shape: &RustSlotShape| {
                matches!(
                    shape.ownership,
                    OwnershipShape::Owned | OwnershipShape::CopyValue
                )
            };
            copies_argument_field = candidates.into_iter().any(|index| match &plan {
                Some(crate::call_plan::ArgumentLoweringPlan::Shaped(shapes)) => {
                    shapes.get(index).is_none_or(owns)
                }
                Some(crate::call_plan::ArgumentLoweringPlan::Slotted(slots)) => {
                    slots.get(index).is_none_or(|slot| match slot {
                        crate::conversion_plan::Slot::Argument(shape)
                        | crate::conversion_plan::Slot::Assignment(shape)
                        | crate::conversion_plan::Slot::Return(shape) => owns(shape),
                        crate::conversion_plan::Slot::Value => true,
                        _ => false,
                    })
                }
                _ => true, // No solved metadata: retain conservative copying requirements.
            });
        }
    });
    if copies_argument_field {
        return true;
    }
    if !java_type_contains_type_param(&method.return_type, type_param) {
        return false;
    }
    body.statements.iter().any(|stmt| {
        method_return_expr(stmt).is_some_and(|expr| {
            expr_recovers_owned_class_type_param_from_field(
                expr,
                type_param,
                fields,
                &needs_clone,
                &lexical_types,
            )
        })
    })
}

fn method_return_expr(stmt: &Stmt) -> Option<&Expr> {
    match stmt {
        Stmt::Return(Some(expr)) => Some(expr),
        _ => None,
    }
}

/// Cloning an owned reference carrier can be independent of its type
/// arguments: Rc<T>, for example, copies the handle rather than cloning T.
/// Reading a checked cursor copies the stored Java reference. Inspect the
/// solved element carrier, not its nominal type arguments: an Rc element is
/// unconditionally Clone even when its payload contains an unbounded T.
struct RetainedCallableShapeContext<'a> {
    env: crate::expr_shape::ShapeEnv,
    params: &'a [crate::signature::ParamPlan],
    locals: &'a [crate::signature::LocalPlan],
}

fn retained_callable_shape_context<'a>(
    params: &[crate::java_ir::Param],
    body: &Block,
    owner: &TypeDecl,
    return_type: Option<&JavaType>,
    callable_name: Option<&str>,
    context: CloneStorageContext<'a>,
) -> Option<RetainedCallableShapeContext<'a>> {
    let matches_method = |method: &MethodDecl| {
        callable_name.is_none_or(|name| {
            name == method.name
                || method.rust_name.as_deref() == Some(name)
                || crate::java_signature::source_method_rust_name(method) == name
        }) && method.params == params
            && return_type.is_some_and(|target| target == &method.return_type)
            && method
                .body
                .as_ref()
                .is_some_and(|candidate| std::ptr::eq(candidate, body) || candidate == body)
    };
    let method_context = |method: &MethodDecl| {
        let signature = context.signatures.method_signature(owner, method)?;
        Some(RetainedCallableShapeContext {
            env: crate::expr_shape::shape_env_for_method(
                owner,
                method,
                signature,
                context.signatures,
            ),
            params: signature.params(),
            locals: signature.locals(),
        })
    };
    for member in &owner.members {
        match member {
            Member::Method(method) if matches_method(method) => return method_context(method),
            Member::Constructor(ctor)
                if return_type.is_none()
                    && ctor.params == params
                    && ctor.body.as_ref().is_some_and(|candidate| {
                        std::ptr::eq(candidate, body) || candidate == body
                    }) =>
            {
                let signature = context.signatures.constructor_signature(owner, ctor)?;
                return Some(RetainedCallableShapeContext {
                    env: crate::expr_shape::shape_env_for_constructor(
                        owner,
                        ctor,
                        signature,
                        context.signatures,
                    ),
                    params: signature.params(),
                    locals: signature.locals(),
                });
            }
            _ => {}
        }
    }
    // The emitted body is specialized in the child's binder scope. Match the
    // same inherited method construction used by lowering/signature planning,
    // then consume its already-applied signature instead of a declaration's
    // unrelated generic names or an empty shape environment.
    for inherited in
        crate::inherited_members::inherited_instance_method_refs_for_type(owner, context.program)
    {
        let method = crate::inherited_members::specialized_inherited_method_for_type(
            context.program,
            owner,
            &inherited,
        );
        let mut method = crate::inherited_members::method_with_synthesized_source_wildcard_params(
            &method,
            owner,
            &[context.program],
        );
        method.rust_name = Some(inherited.rust_name().to_string());
        if matches_method(&method) {
            return method_context(&method);
        }
    }
    None
}

fn retained_clone_shielded_names(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    owner: Option<&TypeDecl>,
    return_type: Option<&JavaType>,
    callable_name: Option<&str>,
    context: Option<CloneStorageContext<'_>>,
) -> HashSet<String> {
    let (Some(body), Some(owner), Some(context)) = (body, owner, context) else {
        return HashSet::new();
    };
    let Some(callable) =
        retained_callable_shape_context(params, body, owner, return_type, callable_name, context)
    else {
        return HashSet::new();
    };
    let mut agreed = HashMap::new();
    for (name, shape) in callable
        .params
        .iter()
        .map(|param| (param.name(), param.shape()))
        .chain(
            callable
                .locals
                .iter()
                .map(|local| (local.name(), local.shape())),
        )
    {
        if !java_type_contains_type_param(&shape.java_ty, type_param) {
            continue;
        }
        let mut owned = shape.clone();
        owned.ownership = crate::slot_shape::OwnershipShape::Owned;
        let shields = owned.native_allocation.shared()
            && !rust_clone_requires_parameter(
                &rust_type_for_slot_with_facts_and_representations(
                    &owned,
                    context.facts,
                    context.representations,
                ),
                type_param,
            );
        // Old syntax-only analyses use names. Relax them only if every
        // same-named declaration has the same unconditional handle proof.
        *agreed.entry(name.to_string()).or_insert(true) &= shields;
    }
    agreed
        .into_iter()
        .filter_map(|(name, shields)| shields.then_some(name))
        .collect()
}

fn record_retained_iterable_conversion_site(
    source: &Expr,
    target: &JavaType,
    facts: &crate::effective_facts::EffectiveFacts<'_>,
    sites: &mut HashMap<*const Expr, JavaType>,
) {
    // The solved ABI can accept a nullable callback, but a Java lambda
    // creates its non-null payload. Classify that payload, retaining the
    // original nullable target for conversion rather than erasing its ABI.
    let payload = crate::types::optional_element_type(target).unwrap_or_else(|| target.clone());
    if facts.native_iterable_element_type(&payload).is_some()
        || facts.native_functional_carrier(&payload).is_some()
    {
        sites.insert(source as *const Expr, target.clone());
    }
}

fn record_retained_iterable_argument_sites(
    args: &[Expr],
    plan: crate::call_plan::ArgumentLoweringPlan,
    facts: &crate::effective_facts::EffectiveFacts<'_>,
    sites: &mut HashMap<*const Expr, JavaType>,
) {
    use crate::call_plan::ArgumentLoweringPlan;
    use crate::conversion_plan::Slot;
    match plan {
        ArgumentLoweringPlan::Shaped(shapes) => {
            for (source, shape) in args.iter().zip(shapes) {
                record_retained_iterable_conversion_site(source, &shape.java_ty, facts, sites);
            }
        }
        ArgumentLoweringPlan::Slotted(slots) => {
            for (source, slot) in args.iter().zip(slots) {
                if let Slot::Argument(shape) | Slot::Assignment(shape) | Slot::Return(shape) = slot
                {
                    record_retained_iterable_conversion_site(source, &shape.java_ty, facts, sites);
                }
            }
        }
        ArgumentLoweringPlan::Unshaped { .. } => {}
    }
}

fn retained_iterable_boundary_requires_parameter(
    source: &Expr,
    target: &JavaType,
    env: &crate::expr_shape::ShapeEnv,
    ctx: crate::expr_shape::ExprShapeContext<'_>,
    locals: &[crate::signature::LocalPlan],
    requires: &impl Fn(&RustSlotShape) -> bool,
) -> bool {
    let Some(facts) = ctx.facts() else {
        return false;
    };
    let effective = crate::effective_facts::EffectiveFacts::new(facts);
    // The conversion is applied to the selected branch, whose physical
    // allocation can be more precise than the joined expression.
    if let Expr::Ternary {
        then_expr,
        else_expr,
        ..
    } = source
    {
        return retained_iterable_boundary_requires_parameter(
            then_expr, target, env, ctx, locals, requires,
        ) || retained_iterable_boundary_requires_parameter(
            else_expr, target, env, ctx, locals, requires,
        );
    }
    if effective.native_iterable_element_type(target).is_some() {
        return requires(&crate::expr_shape::infer_expr_shape_with_context(
            source, env, ctx,
        ));
    }
    let Expr::Lambda { params, body } = source else {
        return false;
    };
    let target = crate::types::optional_element_type(target).unwrap_or_else(|| target.clone());
    let Some(functional) = effective.native_functional_carrier(&target) else {
        return false;
    };
    let JavaType::Named { args, .. } = target else {
        return false;
    };
    let (Some(inputs), Some(result)) =
        (functional.input_types(&args), functional.return_type(&args))
    else {
        return false;
    };
    if inputs.len() != params.len() {
        return false;
    }
    let mut lambda_env = env.clone();
    for (param, input) in params.iter().zip(inputs) {
        let shape = ctx.representations().map_or_else(
            || RustSlotShape::for_storage(input.clone()),
            |representations| {
                crate::shape_projection::storage_shape_for_java_type(representations, input.clone())
            },
        );
        lambda_env.insert_local_shape(param.name.clone(), shape.clone());
        lambda_env.insert_local_shape(param.source_name.clone(), shape);
    }
    match body {
        crate::java_ir::LambdaBody::Expr(expr) => retained_iterable_boundary_requires_parameter(
            expr,
            &result,
            &lambda_env,
            ctx,
            locals,
            requires,
        ),
        crate::java_ir::LambdaBody::Block(block) => {
            let mut returns = HashSet::new();
            crate::semantic_analysis::walk_block_stmts(block, &mut |stmt| {
                if let Stmt::Return(Some(expr)) = stmt {
                    returns.insert(expr as *const Expr);
                }
            });
            let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
                returns.contains(&(expr as *const Expr))
                    && retained_iterable_boundary_requires_parameter(
                        expr, &result, env, ctx, locals, requires,
                    )
            };
            let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
                if let Some(signatures) = ctx.signatures() {
                    crate::expr_shape::learn_stmt_shapes_with_context(
                        stmt,
                        env,
                        locals,
                        signatures,
                        Some(ctx),
                    );
                }
            };
            crate::exception_plan::block_has_unhandled_expression_effect(
                block,
                &lambda_env,
                &crate::exception_plan::ExpressionEffect {
                    expression: &expression,
                    handles: &|_| false,
                    learn: Some(&learn),
                },
            )
        }
    }
}

fn retained_iteration_requires_parameter(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    owner: Option<&TypeDecl>,
    return_type: Option<&JavaType>,
    callable_name: Option<&str>,
    context: Option<CloneStorageContext<'_>>,
    lifetime: bool,
) -> bool {
    let (Some(body), Some(owner), Some(context)) = (body, owner, context) else {
        return false;
    };
    let sites = crate::exception_plan::enhanced_for_iterable_sites(body);
    let mut conversions = HashMap::new();
    let mut assignment_targets = HashMap::new();
    // Retaining a collection as Iterable creates the same checked cursor
    // factory as traversal does. Its payload Clone/'static obligations belong
    // to the creating callable too, even if it never traverses the value.
    let effective = crate::effective_facts::EffectiveFacts::new(context.facts);
    crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| match stmt {
        Stmt::Assign { target, value } => {
            assignment_targets.insert(value as *const Expr, target.clone());
        }
        Stmt::VarDecl {
            ty,
            initializer: Some(source),
            ..
        } => {
            record_retained_iterable_conversion_site(source, ty, &effective, &mut conversions);
        }
        Stmt::Return(Some(source)) => {
            if let Some(target) = return_type {
                record_retained_iterable_conversion_site(
                    source,
                    target,
                    &effective,
                    &mut conversions,
                );
            }
        }
        Stmt::ConstructorInvocation {
            args,
            resolved: Some(constructor),
            ..
        } => {
            record_retained_iterable_argument_sites(
                args,
                crate::call_plan::plan_constructor_arguments(
                    Some(constructor),
                    args.len(),
                    context.signatures,
                ),
                &effective,
                &mut conversions,
            );
        }
        _ => {}
    });
    let mut has_stream = false;
    let mut has_remove_all = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        has_stream |= matches!(expr, Expr::Call { name, .. } if matches!(name.as_str(), "stream" | "parallelStream" | "parallel_stream"));
        has_remove_all |= matches!(expr, Expr::Call { name, args, .. } if args.len() == 1 && matches!(name.as_str(), "removeAll" | "remove_all"));
        match expr {
            Expr::Call {
                args,
                resolved: Some(call),
                ..
            } => {
                record_retained_iterable_argument_sites(
                    args,
                    crate::call_plan::plan_call_arguments(
                        Some(call),
                        args.len(),
                        context.signatures,
                    ),
                    &effective,
                    &mut conversions,
                );
            }
            Expr::New {
                args,
                resolved: Some(constructor),
                ..
            } => {
                record_retained_iterable_argument_sites(
                    args,
                    crate::call_plan::plan_constructor_arguments(
                        Some(constructor),
                        args.len(),
                        context.signatures,
                    ),
                    &effective,
                    &mut conversions,
                );
            }
            Expr::Cast { ty, expr } => {
                record_retained_iterable_conversion_site(expr, ty, &effective, &mut conversions);
            }
            _ => {}
        }
    });
    if sites.is_empty()
        && conversions.is_empty()
        && assignment_targets.is_empty()
        && !has_stream
        && !has_remove_all
    {
        return false;
    }
    let Some(callable) =
        retained_callable_shape_context(params, body, owner, return_type, callable_name, context)
    else {
        return false;
    };
    let (env, locals) = (callable.env, callable.locals);
    let ctx = crate::expr_shape::ExprShapeContext::new(
        Some(context.signatures),
        Some(context.representations),
        Some(context.facts),
    );
    let expression = |expr: &Expr, env: &crate::expr_shape::ShapeEnv| {
        let requires = |parent: &RustSlotShape| {
            let Some(shape) = crate::shape_projection::retained_iteration_element_shape(
                parent,
                context.facts,
                Some(context.representations),
            ) else {
                return false;
            };
            java_type_contains_type_param(&shape.java_ty, type_param)
                && (lifetime
                    || rust_clone_requires_parameter(
                        &rust_type_for_slot_with_facts_and_representations(
                            &shape,
                            context.facts,
                            context.representations,
                        ),
                        type_param,
                    ))
        };
        let mut found = false;
        crate::semantic_analysis::walk_expr_outside_lambdas(expr, &mut |node| {
            if let Expr::Call {
                receiver: Some(receiver),
                name,
                args,
                ..
            } = node
                && args.len() == 1
                && matches!(name.as_str(), "removeAll" | "remove_all")
            {
                let receiver_shape =
                    crate::expr_shape::infer_expr_shape_with_context(receiver, env, ctx);
                if effective
                    .retained_native_collection_kind(&receiver_shape.java_ty)
                    .is_some()
                {
                    let source =
                        crate::expr_shape::infer_expr_shape_with_context(&args[0], env, ctx);
                    if source.native_allocation.shared() {
                        found |= requires(&source);
                        found |= requires(&receiver_shape);
                    }
                }
            }
            if let Some(target) = assignment_targets.get(&(node as *const Expr)) {
                let target = crate::expr_shape::infer_expr_shape_with_context(target, env, ctx);
                found |= retained_iterable_boundary_requires_parameter(
                    node,
                    &target.java_ty,
                    env,
                    ctx,
                    locals,
                    &requires,
                );
            }
            if sites.contains(&(node as *const Expr)) {
                found |= requires(&crate::expr_shape::infer_expr_shape_with_context(
                    node, env, ctx,
                ));
            }
            if let Some(target) = conversions.get(&(node as *const Expr)) {
                found |= retained_iterable_boundary_requires_parameter(
                    node, target, env, ctx, locals, &requires,
                );
            }
        });
        if has_stream {
            crate::stream_shape::visit_retained_stream_sources(expr, env, ctx, |_, parent, _| {
                found |= requires(&parent);
            });
        }
        found
    };
    let learn = |stmt: &Stmt, env: &mut crate::expr_shape::ShapeEnv| {
        crate::expr_shape::learn_stmt_shapes_with_context(
            stmt,
            env,
            locals,
            context.signatures,
            Some(ctx),
        );
    };
    crate::exception_plan::block_has_unhandled_expression_effect(
        body,
        &env,
        &crate::exception_plan::ExpressionEffect {
            expression: &expression,
            handles: &|_| false,
            learn: Some(&learn),
        },
    )
}

fn stored_java_type_clone_requires_parameter(
    ty: &JavaType,
    type_param: &str,
    context: Option<CloneStorageContext<'_>>,
) -> bool {
    if !java_type_contains_type_param(ty, type_param) {
        return false;
    }
    let Some(context) = context else {
        return true;
    };
    let mut shape = crate::shape_projection::project_java_value_shape(
        Some(context.representations),
        ty.clone(),
    );
    shape.ownership = crate::slot_shape::OwnershipShape::Owned;
    let stored = rust_type_for_slot_with_facts_and_representations(
        &shape,
        context.facts,
        context.representations,
    );
    // This projection is used for substituted ancestor arguments, which need
    // not have a stored field signature. If lexical information was erased by
    // the fallback, relax a bound only across a proven clone-shielding carrier.
    rust_clone_requires_parameter(&stored, type_param) || !rust_clone_shields_arguments(&stored)
}

fn rust_clone_shields_arguments(ty: &RustType) -> bool {
    match ty {
        RustType::Named(raw) | RustType::Raw(raw) => {
            if raw_rust_shared_handle_type_inner(raw).is_some()
                || raw_rust_generic_args(raw, "std::marker::PhantomData")
                    .or_else(|| raw_rust_generic_args(raw, "PhantomData"))
                    .is_some_and(|args| args.len() == 1)
                || raw_rust_box_dyn_trait(raw).is_some_and(raw_dyn_trait_supports_box_clone_adapter)
            {
                return true;
            }
            raw_rust_option_inner(raw)
                .or_else(|| raw_rust_vec_inner(raw))
                .is_some_and(|inner| {
                    rust_clone_shields_arguments(&rust_type_arg_projection_or_raw(inner))
                })
        }
        RustType::Generic { base, args } if unconditional_clone_carrier(base) => args.len() == 1,
        RustType::Generic { base, args } if matches!(base.as_str(), "Box" | "std::boxed::Box") => {
            matches!(args.as_slice(), [RustType::DynTrait(name)] if raw_dyn_trait_supports_box_clone_adapter(name))
        }
        RustType::Vec(inner) | RustType::Array2(inner) | RustType::Option(inner) => {
            rust_clone_shields_arguments(inner)
        }
        RustType::Ref { mutable: false, .. } | RustType::FnPointer { .. } => true,
        _ => false,
    }
}

fn unconditional_clone_carrier(base: &str) -> bool {
    matches!(
        base,
        "std::rc::Rc"
            | "Rc"
            | "std::sync::Arc"
            | "Arc"
            | "std::marker::PhantomData"
            | "PhantomData"
            | "JavaListView"
    )
}

fn rust_clone_requires_parameter(ty: &RustType, type_param: &str) -> bool {
    match ty {
        RustType::Named(name) if name == type_param => true,
        RustType::Named(name) | RustType::Raw(name) => {
            if rust_clone_shields_arguments(ty) {
                return false;
            }
            let parsed = rust_type_arg_projection_or_raw(name);
            if &parsed == ty || matches!(parsed, RustType::Raw(_)) {
                matches!(ty, RustType::Raw(_))
                    || name
                        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                        .any(|identifier| identifier == type_param)
            } else {
                rust_clone_requires_parameter(&parsed, type_param)
            }
        }
        RustType::Generic { base, args }
            if unconditional_clone_carrier(base) && args.len() == 1 =>
        {
            false
        }
        RustType::Generic { base, args }
            if matches!(base.as_str(), "Box" | "std::boxed::Box")
                && matches!(args.as_slice(), [RustType::DynTrait(name)] if raw_dyn_trait_supports_box_clone_adapter(name)) =>
        {
            false
        }
        RustType::Generic { args, .. } | RustType::Tuple(args) => args
            .iter()
            .any(|arg| rust_clone_requires_parameter(arg, type_param)),
        RustType::Vec(inner) | RustType::Array2(inner) | RustType::Option(inner) => {
            rust_clone_requires_parameter(inner, type_param)
        }
        RustType::Ref { .. } | RustType::FnPointer { .. } => false,
        RustType::DynTrait(_)
        | RustType::ImplTrait(_)
        | RustType::ImplFnTrait { .. }
        | RustType::DynFnTrait { .. } => true,
        _ => false,
    }
}

fn expr_recovers_owned_class_type_param_from_field(
    expr: &Expr,
    type_param: &str,
    fields: &HashMap<String, JavaType>,
    needs_clone: &impl Fn(&JavaType) -> bool,
    lexical_types: &TypeEnv,
) -> bool {
    match expr {
        Expr::Var(name) if lexical_types.local_type(name).is_none() => {
            fields.get(name).is_some_and(needs_clone)
        }
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            fields.get(field).is_some_and(needs_clone)
        }
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == "get" && args.len() == 1 => expr_java_type(receiver, fields, lexical_types)
            .is_some_and(|receiver_ty| {
                collection_lookup_value_type(&receiver_ty)
                    .is_some_and(|value_ty| needs_clone(&value_ty))
            }),
        Expr::Call {
            receiver: Some(receiver),
            name,
            args,
            ..
        } if name == "iterator" && args.is_empty() => {
            expr_java_type(receiver, fields, lexical_types).is_some_and(|receiver_ty| {
                sequence_element_type(&receiver_ty)
                    .is_some_and(|element_ty| needs_clone(&element_ty))
            })
        }
        Expr::Cast { expr, .. } => expr_recovers_owned_class_type_param_from_field(
            expr,
            type_param,
            fields,
            needs_clone,
            lexical_types,
        ),
        _ => false,
    }
}

fn expr_java_type(
    expr: &Expr,
    fields: &HashMap<String, JavaType>,
    lexical_types: &TypeEnv,
) -> Option<JavaType> {
    match expr {
        Expr::Var(name) if lexical_types.local_type(name).is_none() => fields.get(name).cloned(),
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
        None,
    )
}

pub(crate) fn constructor_function_shell_with_facts_and_support_contexts(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
    support_programs: &[&Program],
    clone_cache: Option<&CloneRequirementCache>,
) -> RustFunction {
    RustFunction {
        name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        type_params: function_type_params_with_clone_cache(
            None,
            &ctor.type_params,
            &ctor.type_param_bounds,
            false,
            support_programs,
            &ctor.params,
            ctor.body.as_ref(),
            &[],
            Some(ty),
            clone_cache,
            None,
        ),
        params: constructor_params(ty, ctor, signatures, facts),
        return_type: Some(constructor_return_type(ty, ctor, signatures)),
        where_bounds: source_constructor_class_type_param_where_bounds(
            ty,
            ctor,
            support_programs,
            clone_cache,
        ),
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
    owner: Option<&TypeDecl>,
    return_type: Option<&JavaType>,
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
        owner,
        return_type,
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
    owner: Option<&TypeDecl>,
    return_type: Option<&JavaType>,
) -> Vec<String> {
    function_type_params_with_clone_cache(
        callable_name,
        type_params,
        type_param_bounds,
        needs_lifetime,
        support_programs,
        callable_params,
        callable_body,
        required_clone_bounds,
        owner,
        None,
        return_type,
    )
}

fn function_type_params_with_clone_cache(
    callable_name: Option<&str>,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
    needs_lifetime: bool,
    support_programs: &[&Program],
    callable_params: &[crate::java_ir::Param],
    callable_body: Option<&Block>,
    required_clone_bounds: &[bool],
    owner: Option<&TypeDecl>,
    clone_cache: Option<&CloneRequirementCache>,
    return_type: Option<&JavaType>,
) -> Vec<String> {
    let mut params = Vec::new();
    if needs_lifetime {
        params.push("'a".to_string());
    }
    params.extend(type_params.iter().enumerate().map(|(index, name)| {
        let mut bounds = type_param_bounds
            .get(index)
            .and_then(|bound| bound.as_ref())
            .map(|bound| {
                rust_bounds_for_java_type_param_in_scope(
                    bound,
                    support_programs,
                    owner,
                    type_params,
                    type_param_bounds,
                )
            })
            .unwrap_or_default();
        let fallback_cache = CloneRequirementCache::default();
        let cache = clone_cache.unwrap_or(&fallback_cache);
        let needs_clone = {
            let mut memo = cache.search.borrow_mut();
            let mut search = cache.search_with_memo(&mut memo);
            callable_type_param_needs_clone_bound_with_support_contexts_inner(
                name,
                callable_params,
                callable_body,
                support_programs,
                &mut search,
                owner,
                return_type,
                callable_name,
            )
        };
        if needs_clone || required_clone_bounds.get(index).copied().unwrap_or(false) {
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
        let needs_any = {
            let mut memo = cache.search.borrow_mut();
            let mut search = cache.search_with_memo(&mut memo);
            callable_type_param_reaches_source_capability(
                name,
                callable_params,
                callable_body,
                owner,
                support_programs,
                SourceCallableCapability::JavaAny,
                return_type,
                callable_name,
                Some((type_params, type_param_bounds)),
                &mut search,
            )
        };
        if needs_any && !bounds.iter().any(|bound| bound == "std::any::Any") {
            // Runtime-object/static-lifetime obligations must cross the same
            // selected source calls as key capabilities, including reified
            // wildcard witnesses inferred from the actual caller arguments.
            bounds.push("std::any::Any".to_string());
        }
        if callable_type_param_reaches_hash_backed_source_type_in_owner(
            name,
            callable_params,
            callable_body,
            owner,
            support_programs,
            return_type,
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
        {
            let mut memo = cache.search.borrow_mut();
            let mut search = cache.search_with_memo(&mut memo);
            if callable_type_param_reaches_source_capability(
                name,
                callable_params,
                callable_body,
                owner,
                support_programs,
                SourceCallableCapability::CollectionDisplay,
                return_type,
                callable_name,
                Some((type_params, type_param_bounds)),
                &mut search,
            ) && !bounds
                .iter()
                .any(|bound| bound == "JavaArraysDeepToStringElement")
            {
                bounds.push("JavaArraysDeepToStringElement".to_string());
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

#[cfg(test)]
fn callable_type_param_reaches_hash_backed_source_type(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&crate::java_ir::Block>,
    support_programs: &[&Program],
) -> bool {
    callable_type_param_reaches_hash_backed_source_type_in_owner(
        type_param,
        params,
        body,
        None,
        support_programs,
        None,
    )
}

fn callable_type_param_reaches_hash_backed_source_type_in_owner(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    declaring_owner: Option<&TypeDecl>,
    support_programs: &[&Program],
    return_type: Option<&JavaType>,
) -> bool {
    callable_type_param_reaches_source_capability(
        type_param,
        params,
        body,
        declaring_owner,
        support_programs,
        SourceCallableCapability::JavaMapKey,
        return_type,
        None,
        None,
        &mut CloneRequirementSearch::default(),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceCallableCapability {
    JavaMapKey,
    JavaAny,
    CollectionDisplay,
}

fn callable_type_param_reaches_source_capability(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    declaring_owner: Option<&TypeDecl>,
    support_programs: &[&Program],
    capability: SourceCallableCapability,
    return_type: Option<&JavaType>,
    callable_name: Option<&str>,
    callable_generics: Option<(&[String], &[Option<JavaType>])>,
    search: &mut CloneRequirementSearch,
) -> bool {
    // Capability propagation is reachability in the selected source call graph.
    // Visit a (method, binder) once, rather than repeatedly widening signatures
    // or caching a provisional false answer when a recursive edge is reached.
    let mut pending = vec![(
        type_param,
        params,
        body,
        declaring_owner,
        return_type,
        callable_name,
        callable_generics,
    )];
    let mut visited = HashSet::new();
    while let Some((
        type_param,
        params,
        body,
        caller_owner,
        return_type,
        callable_name,
        callable_generics,
    )) = pending.pop()
    {
        if capability == SourceCallableCapability::JavaAny
            && retained_iteration_requires_parameter(
                type_param,
                params,
                body,
                caller_owner,
                return_type,
                callable_name,
                search.storage_context,
                true,
            )
        {
            return true;
        }
        if capability == SourceCallableCapability::JavaAny
            && let (Some(context), Some(owner), Some(name)) =
                (search.storage_context, caller_owner, callable_name)
            && {
                let aliases = crate::java_naming::java_method_name_aliases(name);
                context.facts.captures.owners.iter().any(|capture_owner| {
                    matches!(&capture_owner.origin,
                    crate::facts::CaptureOrigin::Method { type_name, method_name }
                    if type_name == &owner.name
                        && aliases.contains(method_name))
                        && capture_owner
                            .captures
                            .iter()
                            .any(|capture| java_type_contains_type_param(&capture.ty, type_param))
                })
            }
        {
            // The capture/signature plan stores this Java reference in an
            // owned callback. Reuse those facts for its reified lifetime
            // obligation, including callers reached through source forwarding.
            return true;
        }
        if capability == SourceCallableCapability::JavaAny
            && (crate::inherited_members::is_synthesized_source_wildcard_type_param(type_param)
                || callable_type_param_flows_through_source_class_argument(
                    type_param,
                    params,
                    body,
                    support_programs,
                )
                || body.is_some_and(|body| {
                    callable_type_param_trait_object_cast_needs_clone_bound(type_param, body)
                }))
        {
            return true;
        }
        if capability == SourceCallableCapability::JavaMapKey
            && callable_stream_materialization_capabilities(
                type_param,
                params,
                body,
                support_programs,
            )
            .1
        {
            return true;
        }
        let Some(body) = body else {
            continue;
        };
        let mut types = caller_owner
            .map(|owner| source_callable_type_env(owner, params))
            .unwrap_or_default();
        for param in params {
            types.insert_param(param.source_name.clone(), param.ty.clone());
            if param.name != param.source_name {
                types.insert_param(param.name.clone(), param.ty.clone());
            }
        }
        if let Some((names, bounds)) = callable_generics {
            for (index, name) in names.iter().enumerate() {
                types.insert_type_param_declaration(
                    name.clone(),
                    bounds.get(index).cloned().flatten(),
                );
            }
        }
        if capability == SourceCallableCapability::CollectionDisplay
            && let Some(owner) = caller_owner
        {
            for (name, ty) in search.inherited_fields(owner, support_programs) {
                types.insert_field(name, ty);
            }
        }
        for stmt in &body.statements {
            types.learn_stmt(stmt);
        }
        if capability == SourceCallableCapability::JavaMapKey
            && callable_source_interface_conversion_requires_capability(
                type_param,
                body,
                return_type,
                &types,
                support_programs,
                TraitImplCapability::JavaMapKey,
                search,
            )
        {
            return true;
        }
        let mut requires_capability = false;
        crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
            if requires_capability {
                return;
            }
            if capability == SourceCallableCapability::JavaAny
                && let Expr::Call {
                    receiver: Some(receiver),
                    name,
                    args,
                    ..
                } = expr
                && matches!(name.as_str(), "toString" | "to_string")
                && args.is_empty()
                && types.is_unbounded_object_type_parameter(&infer_expr_type(receiver, &types))
                && matches!(crate::types::optional_element_type(&infer_expr_type(receiver, &types))
                    .unwrap_or_else(|| infer_expr_type(receiver, &types)),
                    JavaType::Named { name, args } if args.is_empty() && name == type_param)
            {
                // The virtual object-string protocol observes the reified
                // Java payload. Carry its Any obligation through the same
                // selected source-call reachability as other capabilities.
                requires_capability = true;
                return;
            }
            if capability == SourceCallableCapability::CollectionDisplay
                && expression_requires_collection_display_capability(
                    expr,
                    type_param,
                    &types,
                    support_programs,
                )
            {
                requires_capability = true;
                return;
            }
            let instantiated = if capability == SourceCallableCapability::JavaMapKey {
                match expr {
                    // The resolved constructor includes inferred diamond arguments.
                    Expr::New { .. } => Some(infer_expr_type(expr, &types)),
                    Expr::Call {
                        receiver: Some(receiver),
                        ..
                    } => Some(infer_expr_type(receiver, &types)),
                    _ => None,
                }
            } else {
                None
            };
            requires_capability = instantiated.as_ref().is_some_and(|ty| {
                java_type_requires_hash_capability_for_type_param(
                    ty,
                    type_param,
                    support_programs,
                    &mut HashSet::new(),
                )
            });
            if requires_capability {
                return;
            }
            if capability == SourceCallableCapability::JavaMapKey
                && generic_equals_uses_value_capability(expr, type_param, &types, support_programs)
            {
                requires_capability = true;
                return;
            }
            if let Expr::Call {
                receiver: Some(receiver),
                name,
                args,
                ..
            } = expr
            {
                let receiver_ty = infer_expr_type(receiver, &types);
                let source_receiver = match &receiver_ty {
                    JavaType::Named { name, .. } => {
                        source_type_bound_decl(name, support_programs).is_some()
                    }
                    _ => false,
                };
                if capability == SourceCallableCapability::JavaMapKey
                    && !source_receiver
                    && matches!(
                        crate::call_plan::plan_native_collection_receiver_call_for_type(
                            receiver,
                            name,
                            Some(args),
                            &receiver_ty,
                            &types,
                        ),
                        Some(crate::call_plan::CallTargetPlan::NativeCollectionMethod {
                            method: crate::call_plan::NativeCollectionMethod::ListRemoveObject,
                            ..
                        })
                    )
                {
                    requires_capability |= crate::types::sequence_element_type(&receiver_ty)
                        .is_some_and(|element| java_type_contains_type_param(&element, type_param));
                }
            }
            if let Expr::Call {
                receiver,
                args,
                resolved: Some(call),
                ..
            } = expr
            {
                if let Some((owner, method)) =
                    source_method_for_resolved_call(call, support_programs)
                {
                    if capability == SourceCallableCapability::JavaAny
                        && (method
                            .params
                            .iter()
                            .any(|param| crate::types::contains_wildcard_type(&param.ty))
                            || owner
                                .type_param_bounds
                                .iter()
                                .chain(&method.type_param_bounds)
                                .flatten()
                                .any(crate::types::contains_wildcard_type))
                    {
                        let effective = crate::inherited_members::method_with_synthesized_source_wildcard_params(method, owner, support_programs);
                        if effective.type_params.len() != method.type_params.len() {
                            let mut inferred = call.clone();
                            inferred.arg_target_types = args
                                .iter()
                                .map(|arg| infer_expr_type(arg, &types))
                                .collect();
                            let captures = crate::resolve::resolved_method_type_substitutions(
                                &owner.type_params,
                                &effective,
                                &inferred,
                            );
                            requires_capability |= effective.type_params.iter().filter(|name| crate::inherited_members::is_synthesized_source_wildcard_type_param(name)).any(|capture| {
                                captures.get(capture).is_some_and(|actual| java_type_contains_type_param(actual, type_param))
                            });
                            if requires_capability {
                                return;
                            }
                        }
                    }
                    let mut substitutions = crate::resolve::resolved_method_type_substitutions(
                        &owner.type_params,
                        method,
                        call,
                    );
                    if !method.modifiers.contains(&Modifier::Static) {
                        let receiver_ty = if receiver
                            .as_deref()
                            .is_none_or(|receiver| matches!(receiver, Expr::This))
                        {
                            caller_owner.map(|owner| JavaType::Named {
                                name: owner.name.clone(),
                                args: owner
                                    .type_params
                                    .iter()
                                    .map(|name| JavaType::Named {
                                        name: name.clone(),
                                        args: vec![],
                                    })
                                    .collect(),
                            })
                        } else {
                            receiver
                                .as_ref()
                                .map(|receiver| infer_expr_type(receiver, &types))
                        };
                        if let Some(receiver_ty) = receiver_ty {
                            substitutions.extend(source_receiver_owner_type_substitutions(
                                owner,
                                method,
                                &receiver_ty,
                                support_programs,
                            ));
                        }
                    }
                    for callee_param in method.type_params.iter().chain(
                        owner
                            .type_params
                            .iter()
                            .filter(|param| !method.type_params.contains(param)),
                    ) {
                        if substitutions
                            .get(callee_param)
                            .is_some_and(|actual| java_type_contains_type_param(actual, type_param))
                            && visited.insert((
                                owner.name.as_str(),
                                call.rust_name.clone(),
                                callee_param.as_str(),
                            ))
                        {
                            pending.push((
                                callee_param.as_str(),
                                method.params.as_slice(),
                                method.body.as_ref(),
                                Some(owner),
                                Some(&method.return_type),
                                Some(&method.name),
                                Some((&method.type_params, &method.type_param_bounds)),
                            ));
                        }
                    }
                }
            }
        });
        if requires_capability {
            return true;
        }
    }
    false
}

/// The element-display obligation belongs to a native display boundary, not
/// every generic collection parameter. Preserve source lookalikes and lexical
/// static-owner shadowing, and reuse native call metadata for named APIs.
fn expression_requires_collection_display_capability(
    expr: &Expr,
    type_param: &str,
    types: &TypeEnv,
    support_programs: &[&Program],
) -> bool {
    let displayed_collection = |expr: &Expr| {
        let ty = infer_expr_type(expr, types);
        let ty = optional_element_type(&ty).unwrap_or(ty);
        let source = match &ty {
            JavaType::Named { name, .. } | JavaType::Unknown(name) => {
                source_type_bound_decl(name, support_programs).is_some()
            }
            _ => false,
        };
        !source
            && crate::stdlib::java_collection_kind_for_type(&ty).is_some()
            && java_type_contains_type_param(&ty, type_param)
    };
    if let Expr::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = expr
        && infer_expr_type(expr, types) == JavaType::String
    {
        return displayed_collection(left) || displayed_collection(right);
    }
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return false;
    };
    let receiver_ty = infer_expr_type(receiver, types);
    let native_collection = displayed_collection(receiver);
    if native_collection
        && matches!(
            crate::call_plan::plan_native_collection_receiver_call_for_type(
                receiver,
                name,
                Some(args),
                &receiver_ty,
                types
            ),
            Some(crate::call_plan::CallTargetPlan::NativeCollectionMethod {
                method: crate::call_plan::NativeCollectionMethod::ToString,
                ..
            })
        )
    {
        return true;
    }
    let owner = match receiver.as_ref() {
        Expr::Var(name) => Some(name.clone()),
        receiver => crate::stdlib::qualified_static_owner_java_name(receiver),
    };
    let Some(owner) = owner else {
        return false;
    };
    let root = owner.split('.').next().unwrap_or(&owner);
    if types.local_type(root).is_some() || types.field_type(root).is_some() {
        return false;
    }
    let owner = types.static_type_alias(&owner).unwrap_or(&owner);
    if source_type_bound_decl(owner, support_programs).is_some() {
        return false;
    }
    if !["String", "Arrays"]
        .iter()
        .any(|native| crate::call_plan::native_static_owner_matches(owner, native))
    {
        return false;
    }
    let owner_expr = Expr::Var(owner.to_string());
    let target = crate::call_plan::plan_native_collection_static_call(
        Some(&owner_expr),
        name,
        resolved.as_ref(),
        types,
        // This capability-only pass already excludes source owners through
        // support_programs above, and does not carry a program FactSet.
        &crate::facts::FactSet::default(),
    )
    .or_else(|| {
        crate::call_plan::plan_native_java_static_owner_call(owner, name, Some(args), types)
    });
    match target {
        Some(crate::call_plan::CallTargetPlan::NativeJavaStatic {
            method:
                crate::call_plan::NativeJavaStaticMethod::StringFormat
                | crate::call_plan::NativeJavaStaticMethod::StringValueOf,
            ..
        }) => args.iter().any(displayed_collection),
        Some(crate::call_plan::CallTargetPlan::NativeCollectionStatic {
            method: crate::call_plan::NativeCollectionStaticMethod::ArraysDeepToString,
            ..
        }) => args
            .iter()
            .any(|arg| java_type_contains_type_param(&infer_expr_type(arg, types), type_param)),
        _ => false,
    }
}

/// Bind an explicitly instantiated source receiver to the declaring owner's
/// parameters. Inherited calls compose the existing shared ancestor projection;
/// a callable's own binder shadows an identically named owner binder.
fn source_receiver_owner_type_substitutions(
    owner: &TypeDecl,
    method: &MethodDecl,
    receiver_ty: &JavaType,
    support_programs: &[&Program],
) -> HashMap<String, JavaType> {
    let receiver_ty = optional_element_type(receiver_ty).unwrap_or_else(|| receiver_ty.clone());
    let JavaType::Named { name, args } = &receiver_ty else {
        return HashMap::new();
    };
    let Some(receiver_owner) = source_type_bound_decl(name, support_programs) else {
        return HashMap::new();
    };
    let receiver_bindings = crate::resolve::owner_type_argument_substitutions(
        &receiver_owner.type_params,
        &[],
        args,
        &receiver_owner.type_param_bounds,
    );
    let bindings = if receiver_owner.name == owner.name {
        receiver_bindings
    } else {
        let Some(ancestor_bindings) = support_programs.iter().find_map(|program| {
            crate::inherited_members::source_ancestor_type_arguments(program, receiver_owner, owner)
        }) else {
            return HashMap::new();
        };
        ancestor_bindings
            .into_iter()
            .map(|(param, actual)| {
                (
                    param,
                    crate::resolve::substitute_method_type_variables(&actual, &receiver_bindings),
                )
            })
            .collect()
    };
    bindings
        .into_iter()
        .filter(|(name, _)| !method.type_params.contains(name))
        .collect()
}

/// Use the resolver-selected identity, shared by Rust capability planners.
fn generic_equals_uses_value_capability(
    expr: &Expr,
    type_param: &str,
    types: &TypeEnv,
    support_programs: &[&Program],
) -> bool {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        resolved,
        ..
    } = expr
    else {
        return false;
    };
    if name != "equals"
        || args.len() != 1
        || resolved
            .as_ref()
            .is_some_and(|call| source_method_for_resolved_call(call, support_programs).is_some())
    {
        return false;
    }
    let is_parameter = |ty: JavaType| {
        let ty = optional_element_type(&ty).unwrap_or(ty);
        matches!(&ty, JavaType::Named { name, args } if name == type_param && args.is_empty())
            || matches!(&ty, JavaType::Unknown(name) if name == type_param)
    };
    is_parameter(infer_expr_type(receiver, types)) && is_parameter(infer_expr_type(&args[0], types))
}

/// Use the resolver-selected identity, shared by Rust capability planners.
fn source_method_for_resolved_call<'a>(
    call: &crate::java_ir::ResolvedCall,
    support_programs: &[&'a Program],
) -> Option<(&'a TypeDecl, &'a MethodDecl)> {
    let owner = source_type_bound_decl(&call.declaring_type, support_programs)?;
    let method = owner.members.iter().find_map(|member| match member {
        Member::Method(method)
            if crate::java_signature::source_method_rust_name(method) == call.rust_name
                && method.params.len() == call.arg_target_types.len() =>
        {
            Some(method)
        }
        _ => None,
    })?;
    Some((owner, method))
}

/// Stream planning already knows which elements escape and which collection
/// owns them. Reuse that plan for Rust-only generic capabilities rather than
/// matching collector names independently in signature rendering.
fn callable_stream_materialization_capabilities(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    support_programs: &[&Program],
) -> (bool, bool) {
    let Some(body) = body else {
        return (false, false);
    };
    // Avoid rebuilding the source inventory for ordinary generic methods.
    // This is only a cheap necessary syntax condition; source identity,
    // element ownership and collection capabilities still come from StreamPlan.
    let mut may_materialize = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        may_materialize |= matches!(expr, Expr::Call { name, .. }
            if matches!(name.as_str(), "collect" | "toArray" | "to_array"));
    });
    if !may_materialize {
        return (false, false);
    }
    let mut source_emission = crate::source_emission::SourceEmissionPlan::default();
    for program in support_programs {
        source_emission.add_program(program);
    }
    let mut types = TypeEnv::new();
    types.set_source_emission(std::sync::Arc::new(source_emission));
    for param in params {
        types.insert_param(param.name.clone(), param.ty.clone());
        types.insert_param(param.source_name.clone(), param.ty.clone());
    }
    for stmt in &body.statements {
        types.learn_stmt(stmt);
    }
    let facts = FactSet::default();
    let mut clone = false;
    let mut key = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        let Some(plan) = crate::stream::build_stream_plan(expr, &types, &facts) else {
            return;
        };
        let element = match plan.actual() {
            crate::conversion_plan::LoweredType::VecElementBorrow { element_shape, .. }
            | crate::conversion_plan::LoweredType::SetElementBorrow { element_shape, .. } => {
                Some(element_shape)
            }
            _ => None,
        };
        clone |=
            element.is_some_and(|shape| java_type_contains_type_param(&shape.java_ty, type_param));
        key |= plan
            .actual()
            .java_ty()
            .is_some_and(|ty| java_type_uses_type_param_as_hash_key(ty, type_param));
    });
    (clone, key)
}

fn callable_type_param_flows_through_source_class_argument(
    type_param: &str,
    params: &[crate::java_ir::Param],
    body: Option<&Block>,
    support_programs: &[&Program],
) -> bool {
    let uses_class_parameter = |ty: &JavaType| {
        let JavaType::Named { name, args } = ty else {
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
    };
    if params.iter().any(|param| uses_class_parameter(&param.ty)) {
        return true;
    }
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
    for statement in &body.statements {
        types.learn_stmt(statement);
    }
    let mut required = false;
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
        if !required {
            required = match expr {
                Expr::New { .. } => uses_class_parameter(&infer_expr_type(expr, &types)),
                Expr::Call {
                    receiver: Some(receiver),
                    ..
                } => uses_class_parameter(&infer_expr_type(receiver, &types)),
                _ => false,
            };
        }
    });
    required
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

    #[test]
    fn clone_requirements_follow_storage_carriers_not_mentioned_parameters() {
        for source in [
            "std::rc::Rc<std::cell::RefCell<T>>",
            "std::sync::Arc<T>",
            "std::marker::PhantomData<T>",
            "Option<Box<dyn Source<T>>>",
            "Vec<Box<dyn Source<T>>>",
            "Option<std::rc::Rc<T>>",
        ] {
            let ty = rust_type_arg_projection_or_raw(source);
            assert!(!rust_clone_requires_parameter(&ty, "T"), "{source}: {ty:?}");
        }
        for source in ["T", "Option<T>", "Vec<T>", "Box<T>", "T::Item"] {
            let ty = rust_type_arg_projection_or_raw(source);
            assert!(rust_clone_requires_parameter(&ty, "T"), "{source}: {ty:?}");
        }
        let unrelated = RustType::Named("OtherT".into());
        assert!(!rust_clone_requires_parameter(&unrelated, "T"));
        // Any does not have the source-interface clone_box adapter.
        assert!(rust_clone_requires_parameter(
            &RustType::Generic {
                base: "Box".into(),
                args: vec![RustType::DynTrait("std::any::Any".into())],
            },
            "T",
        ));
    }

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
            infer_type_arguments: false,
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
                infer_type_arguments: false,
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
                    type_args: Vec::new(),
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
            functional_callback_param_type(&method.params[0].ty, &FactSet::default(), None),
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
    fn functional_callback_slots_use_solved_source_handle_representation() {
        let parsed = crate::parser::parse_java_source(
            r#"
            class Cell {
                int value;
                void set(int value) { this.value = value; }
            }
            class Use {
                static void alias() {
                    Cell value = new Cell();
                    Cell other = value;
                    other.set(4);
                }
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let mut semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        semantic.facts.storage_strategies =
            crate::storage_strategy::plan_storage_strategies_from_facts(
                &resolved.program,
                &semantic.usage,
                &resolved.facts,
            );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let cell = JavaType::Named {
            name: "Cell".to_string(),
            args: Vec::new(),
        };
        let canonical = crate::slot_render::rust_type_for_slot_with_facts_and_representations(
            &crate::shape_projection::project_java_value_shape(
                Some(&representations),
                cell.clone(),
            ),
            &semantic.facts,
            &representations,
        );
        assert!(
            crate::emit::emit_type(&canonical).contains("Rc<"),
            "{canonical:?}"
        );
        let callback = JavaType::Named {
            name: "Function".to_string(),
            args: vec![cell.clone(), cell],
        };
        assert_eq!(
            functional_callback_param_type(&callback, &semantic.facts, Some(&representations)),
            Some(RustType::ImplFnTrait {
                trait_name: "FnMut".to_string(),
                params: vec![canonical.clone()],
                return_type: Some(Box::new(canonical)),
            })
        );
    }

    #[test]
    fn callback_interface_inputs_share_the_owned_storage_contract() {
        let parsed = crate::parser::parse_java_source("interface Port<T> { int size(); }");
        let facts = FactSet::default();
        let representations = crate::representation::plan_representations(&parsed.program, &facts);
        let callback =
            crate::parser::parse_hierarchy_type("Function<? super Port<String>, Integer>");
        let rendered =
            functional_callback_param_type(&callback, &facts, Some(&representations)).unwrap();
        assert_eq!(
            crate::emit::emit_type(&rendered),
            "impl FnMut(Box<dyn Port<String>>) -> i32"
        );
    }

    #[test]
    fn callback_escape_scan_accepts_only_proven_forwarding_arguments() {
        let parsed = crate::parser::parse_java_source(
            r#"
            import java.util.function.IntPredicate;
            class Use {
                static boolean invoke(IntPredicate predicate, int value) { return predicate.test(value); }
                static boolean forward(IntPredicate predicate, int value) { return invoke(predicate, value); }
                static IntPredicate returned(IntPredicate predicate) { return predicate; }
                static Runnable captured(IntPredicate predicate) { return () -> predicate.test(1); }
                static boolean unknown(IntPredicate predicate, int value) { return external(predicate, value); }
                static native boolean external(IntPredicate predicate, int value);
            }
        "#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let accepts = |expr: &Expr, index: usize| {
            index == 0
                && matches!(expr,
                    Expr::Call { resolved: Some(call), .. }
                        if call.declaring_type == "Use" && call.rust_name == "invoke"
                )
        };
        let owner = &resolved.program.types[0];
        for member in &owner.members {
            let Member::Method(method) = member else {
                continue;
            };
            let direct = method_param_is_nonescaping_functional_callback(method, "predicate");
            let forwarded = method_param_is_nonescaping_functional_callback_with_forwarding(
                method,
                "predicate",
                Some(&accepts),
            );
            assert_eq!(direct, method.name == "invoke", "{}", method.name);
            assert_eq!(
                forwarded,
                matches!(method.name.as_str(), "invoke" | "forward"),
                "{}",
                method.name
            );
        }
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
    fn dependent_bound_projection_preserves_lexical_parameter_identifiers() {
        for name in ["Integer", "Object", "Result", "INST", "itemType"] {
            let bound = JavaType::Named {
                name: name.to_string(),
                args: Vec::new(),
            };
            let names = vec![name.to_string()];
            assert_eq!(
                rust_bounds_for_java_type_param_in_scope(&bound, &[], None, &names, &[None]),
                vec![format!("Into<{name}>")],
            );
            let parsed = crate::parser::parse_java_source(&format!("class Owner<{name}> {{}}"));
            assert_eq!(
                rust_bounds_for_java_type_param_in_scope(
                    &bound,
                    &[],
                    Some(&parsed.program.types[0]),
                    &[],
                    &[]
                ),
                vec![format!("Into<{name}>")],
            );
        }
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
    fn generic_key_capabilities_follow_selected_binders_and_recursive_calls() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.*;
interface Pair<A, B> {}
interface Flipped<X, Y> extends Pair<Y, X> {}
class Use<Left> {
    static <Key, Payload> int direct(Pair<Payload, Key> pair) {
        Set<Key> keys = new HashSet<Key>();
        return keys.size();
    }
    static <Left, Right> int nested(Pair<Left, Right> pair) { return direct(pair); }
    static <Left, Right> int flipped(Flipped<Left, Right> pair) { return direct(pair); }
    static <One, Two> int cycleOne(Pair<One, Two> pair, boolean stop) {
        if (stop) return cycleTwo(pair, false);
        return 1;
    }
    static <First, Second> int cycleTwo(Pair<First, Second> pair, boolean stop) {
        if (stop) return cycleOne(pair, false);
        return direct(pair);
    }
    static <Neutral> int neutralOne(Neutral value) { return neutralTwo(value); }
    static <Other> int neutralTwo(Other value) { return neutralOne(value); }
}
"#,
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Use")
            .unwrap();
        for (method_name, expected) in [
            ("direct", vec![true, false]),
            ("nested", vec![false, true]),
            ("flipped", vec![true, false]),
            ("cycleOne", vec![false, true]),
            ("cycleTwo", vec![false, true]),
            ("neutralOne", vec![false]),
            ("neutralTwo", vec![false]),
        ] {
            let method = owner
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == method_name => Some(method),
                    _ => None,
                })
                .unwrap();
            let actual = method
                .type_params
                .iter()
                .map(|param| {
                    callable_type_param_reaches_hash_backed_source_type(
                        param,
                        &method.params,
                        method.body.as_ref(),
                        &[&resolved.program],
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{method_name}");
        }
    }

    #[test]
    fn generic_list_key_capability_uses_native_overload_and_source_identity() {
        for (source, expected) in [
            (
                r#"
import java.util.*;
class Use {
    static <T> boolean removeValue(List<T> values, T value) { return values.remove(value); }
    static <T> T removeIndex(List<T> values) { return values.remove(0); }
    static <T> int count(List<T> values) { return values.size(); }
}
"#,
                vec![
                    ("removeValue", true),
                    ("removeIndex", false),
                    ("count", false),
                ],
            ),
            (
                r#"
class List<T> { boolean remove(T value) { return true; } }
class Use {
    static <T> boolean removeValue(List<T> values, T value) { return values.remove(value); }
}
"#,
                vec![("removeValue", false)],
            ),
        ] {
            let parsed = crate::parser::parse_java_source(source);
            assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
            let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
            assert!(
                !resolved.diagnostics.has_errors(),
                "{:?}",
                resolved.diagnostics
            );
            let owner = resolved
                .program
                .types
                .iter()
                .find(|ty| ty.name == "Use")
                .unwrap();
            for (name, required) in expected {
                let method = owner
                    .members
                    .iter()
                    .find_map(|member| match member {
                        Member::Method(method) if method.name == name => Some(method),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    callable_type_param_reaches_hash_backed_source_type(
                        "T",
                        &method.params,
                        method.body.as_ref(),
                        &[&resolved.program],
                    ),
                    required,
                    "{name}",
                );
            }
        }
    }

    #[test]
    fn receiver_key_capabilities_project_owner_arguments_and_respect_method_shadowing() {
        let parsed = crate::parser::parse_java_source(
            r#"
import java.util.*;
class Table<Value, Key> {
    int remember(Key key, Value payload) {
        Set<Key> seen = new HashSet<Key>();
        seen.add(key);
        return seen.size();
    }
    int remember(int value) { return value; }
    int indirect(Key key, Value payload) { return remember(key, payload); }
    <Key> int shadow(Key value) { return 1; }
}
class Flipped<A, B> extends Table<B, A> {}
class Use {
    static <Left, Right> int direct(Table<Left, Right> table, Right key, Left payload) {
        return table.indirect(key, payload);
    }
    static <Left, Right> int inherited(Flipped<Left, Right> table, Left key, Right payload) {
        return table.remember(key, payload);
    }
    static <Left, Right> int shadow(Table<Left, Right> table, Left value) {
        return table.shadow(value);
    }
    static <Left, Right> int neutral(Table<Left, Right> table) { return table.remember(0); }
}
"#,
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        assert!(
            !resolved.diagnostics.has_errors(),
            "{:?}",
            resolved.diagnostics
        );
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Use")
            .unwrap();
        for (name, expected) in [
            ("direct", vec![false, true]),
            ("inherited", vec![true, false]),
            ("shadow", vec![false, false]),
            ("neutral", vec![false, false]),
        ] {
            let method = owner
                .members
                .iter()
                .find_map(|member| match member {
                    Member::Method(method) if method.name == name => Some(method),
                    _ => None,
                })
                .unwrap();
            let required = method
                .type_params
                .iter()
                .map(|param| {
                    callable_type_param_reaches_hash_backed_source_type(
                        param,
                        &method.params,
                        method.body.as_ref(),
                        &[&resolved.program],
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(required, expected, "{name}");
        }
    }

    #[test]
    fn retained_callable_context_distinguishes_identical_method_bodies() {
        let parsed = crate::parser::parse_java_source(
            r#"
class SameBodies<T> {
    T first(T value) { return value; }
    T second(T value) { return value; }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let semantic = crate::semantic_analysis::analyze_program(
            &resolved.program,
            &resolved.facts,
            &FactSet::default(),
        );
        let representations =
            crate::representation::plan_representations(&resolved.program, &semantic.facts);
        let signatures = crate::signature::solve_signatures_with_facts(
            &resolved.program,
            &representations,
            &semantic.usage,
            &semantic.facts,
        );
        let owner = &resolved.program.types[0];
        let context = CloneStorageContext {
            facts: &semantic.facts,
            representations: &representations,
            signatures: &signatures,
            program: &resolved.program,
        };
        for method in owner.members.iter().filter_map(|member| match member {
            Member::Method(method) => Some(method),
            _ => None,
        }) {
            let actual = retained_callable_shape_context(
                &method.params,
                method.body.as_ref().unwrap(),
                owner,
                Some(&method.return_type),
                Some(&method.name),
                context,
            )
            .unwrap();
            let expected = signatures.method_signature(owner, method).unwrap();
            assert!(
                std::ptr::eq(actual.params, expected.params()),
                "{} selected another method's parameter slots",
                method.name
            );
        }
    }

    #[test]
    fn clone_requirement_cache_preserves_reachable_witnesses_across_cycles() {
        let parsed = crate::parser::parse_java_source(
            r#"
interface View<T> { T read(); }
class Value<T> implements View<T> {
    T item;
    public T read() { return item; }
}
class Cycle<T> {
    Value<T> item;
    View<T> first() { second(); return item; }
    View<T> second() { return first(); }
}
"#,
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let owner = resolved
            .program
            .types
            .iter()
            .find(|ty| ty.name == "Cycle")
            .unwrap();
        for names in [["first", "second"], ["second", "first"]] {
            let cache = CloneRequirementCache::default();
            for name in names {
                let method = owner
                    .members
                    .iter()
                    .find_map(|member| match member {
                        Member::Method(method) if method.name == name => Some(method),
                        _ => None,
                    })
                    .unwrap();
                assert!(
                    source_method_emits_clone_bound_for_type_param_with_cache(
                        owner,
                        method,
                        "T",
                        None,
                        &[&resolved.program],
                        &cache,
                    ),
                    "missing reachable Clone witness for {name} after query order {names:?}"
                );
            }
        }
    }

    #[test]
    fn method_binders_do_not_become_owner_capability_requirements() {
        let parsed = crate::parser::parse_java_source(
            r#"
interface Copies<T> {
    default <T> java.util.List<T> twice(T value) {
        java.util.List<T> result = new java.util.ArrayList<T>();
        result.add(value); result.add(value);
        return result;
    }
}
"#,
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        let owner = &parsed.program.types[0];
        let method = owner
            .members
            .iter()
            .find_map(|member| match member {
                Member::Method(method) => Some(method),
                _ => None,
            })
            .unwrap();
        assert!(
            source_method_class_type_param_where_bounds(owner, method, None, &[&parsed.program],)
                .is_empty()
        );
        assert_eq!(
            function_type_params_with_optional_lifetime_and_support_contexts(
                Some(&method.name),
                &method.type_params,
                &method.type_param_bounds,
                false,
                &[&parsed.program],
                &method.params,
                method.body.as_ref(),
                Some(owner),
                Some(&method.return_type),
            ),
            vec!["T: Clone"]
        );
    }

    #[test]
    fn key_requirements_belong_to_direct_keys_not_nested_value_carriers() {
        let parsed = crate::parser::parse_java_source(
            "class Set<T> {} class Stored<T> { java.util.Map<T, Integer> entries; }",
        );
        assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
        for (name, expected) in [
            ("java.util.List<java.util.Set<T>>", false),
            ("java.util.Map<Integer, java.util.Set<T>>", false),
            ("java.util.Map<java.util.List<T>, Integer>", true),
            ("java.util.Set<T>", true),
            ("Set<T>", false),
            ("Stored<T>", true),
        ] {
            let ty = crate::parser::parse_hierarchy_type(name);
            assert_eq!(
                java_type_requires_hash_capability_for_type_param(
                    &ty,
                    "T",
                    &[&parsed.program],
                    &mut HashSet::new(),
                ),
                expected,
                "{name}"
            );
        }
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
                false,
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
    wrap_in_ok: bool,
) -> Vec<RustStmt> {
    let returned = function_call_projection(self_associated_path_projection(rust_name), args);
    let returned = if wrap_in_ok {
        function_call_projection(RustExpr::path(["Ok"]), vec![returned])
    } else {
        returned
    };
    vec![RustStmt::Return(Some(returned))]
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
    callable_analysis: Option<crate::callable_analysis::CallableAnalysis<'_>>,
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
        type_params: function_type_params_with_clone_cache(
            Some(&method.name),
            &retained_type_params,
            &retained_type_param_bounds,
            needs_lifetime,
            support_programs,
            &method.params,
            method.body.as_ref(),
            &[],
            Some(ty),
            clone_requirement_cache,
            Some(&method.return_type),
        ),
        params: method_params_with_optional_lifetime(
            ty,
            method,
            signatures,
            facts,
            needs_lifetime,
            representations,
            callable_analysis,
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

pub(crate) fn method_type_param_is_checked_exception_only(
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
                    if crate::stdlib::is_known_throwable_carrier_type(name)
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
    // This compatibility entry point has no solved field signatures. Keep its
    // conservative Java-level analysis rather than inventing an empty storage
    // plan: hierarchy field projection requires actual solver-owned shapes.
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
    source_method_class_type_param_capabilities_with_cache(
        ty,
        method,
        representations,
        support_programs,
        cache,
    )
    .into_iter()
    .map(|(parameter, bounds)| format!("{parameter}: {}", bounds.join(" + ")))
    .collect()
}

/// Keep declaration requirements structured until the emitting surface has
/// substituted its owner arguments. Inherited contracts cannot render the
/// declaring owner's binder names directly in the child's scope.
pub(crate) fn source_method_class_type_param_capabilities_with_cache(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: Option<&RepresentationPlan>,
    support_programs: &[&Program],
    cache: &CloneRequirementCache,
) -> Vec<(String, Vec<&'static str>)> {
    let uses_shared_storage = representations.is_some_and(|representations| {
        let placement = representations.storage_placement_for(&ty.name);
        placement.shared() && placement.is_implemented()
    });
    let mut memo = cache.search.borrow_mut();
    let mut visiting = cache.search_with_memo(&mut memo);
    let fields = visiting.inherited_fields(ty, support_programs);
    ty.type_params
        .iter()
        .filter(|type_param| !method.type_params.contains(type_param))
        .filter_map(|type_param| {
            let mut bounds = Vec::new();
            if source_method_class_type_param_needs_clone_bound(
                ty,
                method,
                type_param,
                uses_shared_storage,
                &fields,
                support_programs,
                &mut visiting,
            ) {
                bounds.push("Clone");
            }
            if callable_type_param_reaches_hash_backed_source_type_in_owner(
                type_param,
                &method.params,
                method.body.as_ref(),
                Some(ty),
                support_programs,
                Some(&method.return_type),
            ) {
                bounds.push("JavaMapKey");
            }
            if callable_type_param_reaches_source_capability(
                type_param,
                &method.params,
                method.body.as_ref(),
                Some(ty),
                support_programs,
                SourceCallableCapability::CollectionDisplay,
                Some(&method.return_type),
                Some(&method.name),
                Some((&method.type_params, &method.type_param_bounds)),
                &mut visiting,
            ) {
                bounds.push("JavaArraysDeepToStringElement");
            }
            // Concrete source impls already carry their reified generic Any
            // obligation. Interface declarations do not; a default body that
            // constructs/forwards such a value needs it on that method.
            if ty.kind == TypeKind::Interface
                && callable_type_param_reaches_source_capability(
                    type_param,
                    &method.params,
                    method.body.as_ref(),
                    Some(ty),
                    support_programs,
                    SourceCallableCapability::JavaAny,
                    Some(&method.return_type),
                    Some(&method.name),
                    Some((&method.type_params, &method.type_param_bounds)),
                    &mut visiting,
                )
            {
                bounds.push("std::any::Any");
            }
            (!bounds.is_empty()).then(|| (type_param.clone(), bounds))
        })
        .collect()
}

/// Requirements of an implementation combine copying its stored fields with
/// the bounds of the methods it forwards. Interface handles do not expose the
/// clone requirements of their generic payloads merely by mentioning them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TraitImplCapability {
    Clone,
    JavaMapKey,
}

impl TraitImplCapability {
    pub(crate) fn rust_bound(self) -> &'static str {
        match self {
            Self::Clone => "Clone",
            Self::JavaMapKey => "JavaMapKey",
        }
    }
}

pub(crate) fn source_type_param_needs_trait_impl_capability(
    ty: &TypeDecl,
    interface: &TypeDecl,
    type_param: &str,
    capability: TraitImplCapability,
    signatures: &SignaturePlan,
    facts: &FactSet,
    representations: &RepresentationPlan,
    program: &Program,
    cache: &CloneRequirementCache,
) -> bool {
    let mut memo = cache.search.borrow_mut();
    let mut search = CloneRequirementSearch {
        storage_context: Some(CloneStorageContext {
            facts,
            representations,
            signatures,
            program,
        }),
        memo: CloneRequirementMemoAccess::Borrowed(&mut memo),
    };
    source_type_param_needs_trait_impl_capability_inner(
        ty,
        Some(interface),
        type_param,
        capability,
        &[program],
        &mut search,
    )
}

/// The same implementation requirements are queried while emitting an impl
/// and while satisfying a source generic bound at a call site. Keep recursion
/// inside the current search instead of reborrowing its translation cache.
fn source_type_param_needs_trait_impl_capability_inner(
    ty: &TypeDecl,
    interface: Option<&TypeDecl>,
    type_param: &str,
    capability: TraitImplCapability,
    support_programs: &[&Program],
    search: &mut CloneRequirementSearch,
) -> bool {
    if capability == TraitImplCapability::Clone
        && let Some(interface) = interface
        && let Some(required) = mutable_shared_interface_clone_requirement(
            ty,
            interface,
            type_param,
            support_programs,
            search,
        )
    {
        return required;
    }
    let field_requires = match capability {
        TraitImplCapability::Clone => search.stored_field_types(ty).map_or_else(
            || {
                source_field_types(ty).values().any(|field| {
                    stored_java_type_clone_requires_parameter(
                        field,
                        type_param,
                        search.storage_context,
                    )
                })
            },
            |fields| {
                fields
                    .values()
                    .any(|field| rust_clone_requires_parameter(field, type_param))
            },
        ),
        TraitImplCapability::JavaMapKey => source_field_types(ty).values().any(|field| {
            java_type_requires_hash_capability_for_type_param(
                field,
                type_param,
                support_programs,
                &mut HashSet::new(),
            )
        }),
    };
    if field_requires {
        return true;
    }
    let storage_context = search.storage_context;
    let mut method_requires =
        |owner: &TypeDecl, method: &MethodDecl, parameter: &str| match capability {
            TraitImplCapability::Clone => source_method_emits_clone_bound_for_type_param_inner(
                owner,
                method,
                parameter,
                search
                    .storage_context
                    .map(|context| context.representations),
                support_programs,
                search,
            ),
            TraitImplCapability::JavaMapKey => callable_type_param_reaches_source_capability(
                parameter,
                &method.params,
                method.body.as_ref(),
                Some(owner),
                support_programs,
                SourceCallableCapability::JavaMapKey,
                Some(&method.return_type),
                Some(&method.name),
                Some((&method.type_params, &method.type_param_bounds)),
                search,
            ),
        };
    if ty.members.iter().any(|member| match member {
        Member::Method(method) if !method.type_params.iter().any(|name| name == type_param) => {
            method_requires(ty, method, type_param)
        }
        _ => false,
    }) {
        return true;
    }
    support_programs.iter().any(|program| {
        crate::inherited_members::inherited_instance_method_refs_for_type(ty, program)
            .into_iter()
            .any(|inherited| {
                let owner = inherited.declaring_ty();
                let Some(bindings) =
                    crate::inherited_members::source_ancestor_type_arguments(program, ty, owner)
                else {
                    return true; // Preserve the old conservative obligation if rebasing is unknown.
                };
                owner.type_params.iter().any(|param| {
                    !inherited.method().type_params.contains(param)
                        && bindings.get(param).is_some_and(|actual| match capability {
                            TraitImplCapability::JavaMapKey => {
                                java_type_contains_type_param(actual, type_param)
                            }
                            TraitImplCapability::Clone => {
                                stored_java_type_clone_requires_parameter(
                                    actual,
                                    type_param,
                                    storage_context,
                                )
                            }
                        })
                        && method_requires(owner, inherited.method(), param)
                })
            })
    })
}

/// A shared adapter clones its reference, not its payload. Only the selected
/// interface's implementations matter, and a method-local trait predicate is
/// already available while forwarding that method. Abstract contracts without
/// such a predicate still require an adapter-wide implementation bound.
fn mutable_shared_interface_clone_requirement(
    ty: &TypeDecl,
    interface: &TypeDecl,
    type_param: &str,
    support_programs: &[&Program],
    search: &mut CloneRequirementSearch,
) -> Option<bool> {
    let context = search.storage_context?;
    let placement = crate::storage_strategy::effective_storage_placement(
        context.facts.storage_placement_for(&ty.name),
        context.representations.storage_placement_for(&ty.name),
    );
    if !placement.shared() || !placement.interior_mutable() || interface.kind != TypeKind::Interface
    {
        return None;
    }
    let (program, substitutions) = support_programs.iter().find_map(|program| {
        crate::inherited_members::source_ancestor_type_arguments(program, ty, interface)
            .map(|substitutions| (*program, substitutions))
    })?;
    for contract in crate::representation_lowering::specialized_interface_instance_contracts(
        program,
        interface,
        &substitutions,
    ) {
        // Only an exact binder predicate supplies T: Clone. A bound such as
        // Vec<T>: Clone cannot be inverted to infer T: Clone in Rust.
        let allowed = contract.owner.type_params.iter().any(|parameter| {
            !contract.method.type_params.contains(parameter)
                && contract.owner_substitutions.get(parameter).is_some_and(|actual|
                    matches!(actual, JavaType::Named { name, args } if name == type_param && args.is_empty())
                    || matches!(actual, JavaType::Unknown(name) if name == type_param))
                && source_method_emits_clone_bound_for_type_param_inner(
                    contract.owner, contract.method, parameter,
                    Some(context.representations), support_programs, search,
                )
        });
        if allowed {
            continue;
        }
        let mut effective = contract.method.clone();
        effective.return_type = contract.return_type;
        for (param, ty) in effective.params.iter_mut().zip(contract.param_types) {
            param.ty = ty;
        }
        let Some((owner, method)) =
            crate::representation_lowering::selected_interface_implementation(
                ty, &effective, program,
            )
        else {
            continue; // Missing/incompatible implementations keep their existing emission marker.
        };
        let Some(bindings) =
            crate::inherited_members::source_ancestor_type_arguments(program, ty, owner)
        else {
            return None;
        };
        if owner.type_params.iter().any(|parameter| {
            !method.type_params.contains(parameter)
                && (if owner.name == ty.name {
                    // The ancestry API encodes self-specialization as an
                    // empty substitution, not as explicit T -> T entries.
                    parameter == type_param
                } else {
                    bindings.get(parameter).is_some_and(|actual| {
                        stored_java_type_clone_requires_parameter(actual, type_param, Some(context))
                    })
                })
                && source_method_emits_clone_bound_for_type_param_inner(
                    owner,
                    method,
                    parameter,
                    Some(context.representations),
                    support_programs,
                    search,
                )
        }) {
            return Some(true);
        }
    }
    Some(false)
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
    let stored_fields = visiting.stored_field_types(ty);
    let self_clone_requires_parameter = stored_fields.as_ref().map_or_else(
        || {
            fields
                .values()
                .any(|field| java_type_contains_type_param(field, type_param))
        },
        |stored| {
            stored
                .values()
                .any(|field| rust_clone_requires_parameter(field, type_param))
        },
    );
    let needs_clone = callable_type_param_needs_clone_bound_with_support_contexts_inner(
        type_param,
        &method.params,
        method.body.as_ref(),
        support_programs,
        visiting,
        Some(ty),
        Some(&method.return_type),
        Some(&method.name),
    ) || source_method_copies_borrowed_type_param_call_return(
        ty,
        method,
        type_param,
        visiting.storage_context,
    ) || source_callable_delegates_class_type_param_clone_requirement(
        ty,
        &method.params,
        method.body.as_ref(),
        type_param,
        support_programs,
        visiting,
    ) || (!uses_shared_storage
        && java_type_contains_type_param(&method.return_type, type_param)
        && method.body.as_ref().is_some_and(block_returns_this)
        && self_clone_requires_parameter)
        || source_method_clones_class_type_param_param_to_owned_local(method, type_param)
        || (fields
            .values()
            .any(|field_ty| java_type_contains_type_param(field_ty, type_param))
            && source_method_needs_class_type_param_clone_bound(
                method,
                type_param,
                fields,
                stored_fields.as_deref(),
                visiting.storage_context,
            ));
    visiting.method_visiting.pop();
    visiting.method_memo.push((
        ty.name.clone(),
        method.clone(),
        type_param.to_string(),
        needs_clone,
    ));
    needs_clone
}

/// An owned generic result cannot move out of a borrowed source-call result.
/// Match the already solved producer/consumer slots; this is independent of
/// field names and does not impose payload bounds on boxed interface handles.
fn source_method_copies_borrowed_type_param_call_return(
    ty: &TypeDecl,
    method: &MethodDecl,
    type_param: &str,
    context: Option<CloneStorageContext<'_>>,
) -> bool {
    let Some(context) = context else {
        return false;
    };
    let Some(caller) = context.signatures.method_signature(ty, method) else {
        return false;
    };
    if !caller
        .return_shape()
        .is_some_and(|shape| shape.ownership == OwnershipShape::Owned)
    {
        return false;
    }
    let Some(body) = &method.body else {
        return false;
    };
    body.statements.iter().filter_map(method_return_expr).any(|expr| {
        let Expr::Call { resolved: Some(call), name, .. } = expr else { return false; };
        let actual = optional_element_type(&call.return_type).unwrap_or_else(|| call.return_type.clone());
        let direct_parameter = matches!(&actual, JavaType::Named { name, args } if name == type_param && args.is_empty())
            || matches!(&actual, JavaType::Unknown(name) if name == type_param);
        direct_parameter && context.signatures.resolved_source_method_return_shape(call, name)
            .is_some_and(|shape| matches!(shape.ownership, OwnershipShape::SharedBorrow | OwnershipShape::MutableBorrow))
    })
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
    // A concrete actual for `G extends SourceInterface<...>` must satisfy
    // the generated implementation's obligations, even when the constructor
    // itself only stores G and never clones it.
    let constructed_actual = infer_expr_type(expr, caller_types);
    if constructed_source_bounds_require_clone(
        owner,
        &constructed_actual,
        caller_type_param,
        support_programs,
        visiting,
    ) {
        return true;
    }
    owner
        .type_params
        .iter()
        .enumerate()
        .any(|(index, callee_type_param)| {
            // Class binders can arrive through the constructed type even when
            // the constructor has no value parameters. Do not lose its callable
            // obligations at a generic no-argument factory boundary.
            let receives_through_type = matches!(&constructed_actual,
            JavaType::Named { args, .. } if args.get(index).is_some_and(|argument|
                java_type_contains_type_param(argument, caller_type_param)));
            let receives_caller_type_param = receives_through_type
                || args
                    .iter()
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

/// An implicit Java upcast selects a generated concrete trait implementation.
/// Its obligations belong to that conversion, not to every construction or
/// every occurrence of the interface's payload type.
fn callable_source_interface_conversion_requires_capability(
    type_param: &str,
    body: &Block,
    return_type: Option<&JavaType>,
    types: &TypeEnv,
    support_programs: &[&Program],
    capability: TraitImplCapability,
    search: &mut CloneRequirementSearch,
) -> bool {
    let mut required = false;
    let mut boundary = |expr: &Expr, target: &JavaType| {
        if !required {
            required = source_interface_conversion_requires_capability(
                expr,
                target,
                type_param,
                types,
                support_programs,
                capability,
                search,
            );
        }
    };
    crate::semantic_analysis::walk_block_stmts(body, &mut |stmt| match stmt {
        Stmt::Return(Some(expr)) => {
            if let Some(target) = return_type {
                boundary(expr, target);
            }
        }
        Stmt::VarDecl {
            ty,
            initializer: Some(expr),
            ..
        } => boundary(expr, ty),
        Stmt::Assign { target, value } => boundary(value, &infer_expr_type(target, types)),
        Stmt::ConstructorInvocation {
            args,
            resolved: Some(call),
            ..
        } => {
            for conversion in &call.arg_coercions {
                if let Some(arg) = args.get(conversion.arg_index) {
                    boundary(arg, &conversion.target);
                }
            }
        }
        _ => {}
    });
    crate::semantic_analysis::walk_block_exprs(body, &mut |expr| match expr {
        Expr::Cast { ty, expr } => boundary(expr, ty),
        Expr::Call {
            args,
            resolved: Some(call),
            ..
        } => {
            for (arg, target) in args.iter().zip(&call.arg_target_types) {
                boundary(arg, target);
            }
        }
        Expr::New {
            args,
            resolved: Some(call),
            ..
        } => {
            for conversion in &call.arg_coercions {
                if let Some(arg) = args.get(conversion.arg_index) {
                    boundary(arg, &conversion.target);
                }
            }
        }
        Expr::Binary {
            op: BinaryOp::Assign,
            left,
            right,
        } => boundary(right, &infer_expr_type(left, types)),
        _ => {}
    });
    required
}

fn source_interface_conversion_requires_capability(
    expr: &Expr,
    target: &JavaType,
    type_param: &str,
    types: &TypeEnv,
    support_programs: &[&Program],
    capability: TraitImplCapability,
    search: &mut CloneRequirementSearch,
) -> bool {
    // The branches are independently converted to the contextual target;
    // their inferred common interface type can hide the concrete implementor.
    match expr {
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            return [then_expr, else_expr].iter().any(|expr| {
                source_interface_conversion_requires_capability(
                    expr,
                    target,
                    type_param,
                    types,
                    support_programs,
                    capability,
                    search,
                )
            });
        }
        Expr::Switch { cases, .. } => {
            return cases.iter().any(|case| {
                source_interface_conversion_requires_capability(
                    &case.value,
                    target,
                    type_param,
                    types,
                    support_programs,
                    capability,
                    search,
                )
            });
        }
        _ => {}
    }
    let target = optional_element_type(target).unwrap_or_else(|| target.clone());
    let JavaType::Named {
        name: target_name, ..
    } = &target
    else {
        return false;
    };
    let Some(interface) = source_type_bound_decl(target_name, support_programs)
        .filter(|ty| ty.kind == TypeKind::Interface)
    else {
        return false;
    };
    let actual = infer_expr_type(expr, types);
    let actual = optional_element_type(&actual).unwrap_or(actual);
    let JavaType::Named { name, args } = &actual else {
        return false;
    };
    let Some(concrete) = source_type_bound_decl(name, support_programs)
        .filter(|ty| matches!(ty.kind, TypeKind::Class | TypeKind::Enum | TypeKind::Record))
    else {
        return false;
    };
    if !support_programs.iter().any(|program| {
        crate::inherited_members::source_ancestor_type_arguments(program, concrete, interface)
            .is_some()
    }) {
        return false;
    }
    concrete
        .type_params
        .iter()
        .zip(args)
        .any(|(parameter, argument)| {
            let carries_parameter = match capability {
                TraitImplCapability::Clone => stored_java_type_clone_requires_parameter(
                    argument,
                    type_param,
                    search.storage_context,
                ),
                TraitImplCapability::JavaMapKey => {
                    java_type_contains_type_param(argument, type_param)
                }
            };
            if !carries_parameter {
                return false;
            }
            let key = (
                concrete.name.clone(),
                interface.name.clone(),
                parameter.clone(),
                capability,
            );
            if !search.trait_impl_visiting.insert(key.clone()) {
                return false;
            }
            let required = source_type_param_needs_trait_impl_capability_inner(
                concrete,
                Some(interface),
                parameter,
                capability,
                support_programs,
                search,
            );
            search.trait_impl_visiting.remove(&key);
            required
        })
}

fn constructed_source_bounds_require_clone(
    owner: &TypeDecl,
    constructed_type: &JavaType,
    caller_type_param: &str,
    support_programs: &[&Program],
    search: &mut CloneRequirementSearch,
) -> bool {
    let JavaType::Named { args, .. } = constructed_type else {
        return false;
    };
    owner
        .type_param_bounds
        .iter()
        .zip(args)
        .any(|(bound, actual)| {
            let Some(JavaType::Named {
                name: bound_name, ..
            }) = bound
            else {
                return false;
            };
            if !source_type_bound_is_trait_like(bound_name, support_programs) {
                return false;
            }
            let JavaType::Named { name, args } = crate::types::generic_read_type(actual) else {
                return false;
            };
            let Some(concrete) = source_type_bound_decl(&name, support_programs) else {
                return false;
            };
            if !matches!(concrete.kind, TypeKind::Class | TypeKind::Enum) {
                return false;
            }
            concrete
                .type_params
                .iter()
                .zip(&args)
                .any(|(parameter, argument)| {
                    if !stored_java_type_clone_requires_parameter(
                        argument,
                        caller_type_param,
                        search.storage_context,
                    ) {
                        return false;
                    }
                    let key = (
                        concrete.name.clone(),
                        source_type_bound_decl(bound_name, support_programs)
                            .map(|interface| interface.name.clone())
                            .unwrap_or_else(|| bound_name.clone()),
                        parameter.clone(),
                        TraitImplCapability::Clone,
                    );
                    if !search.trait_impl_visiting.insert(key.clone()) {
                        return false;
                    }
                    let required = source_type_param_needs_trait_impl_capability_inner(
                        concrete,
                        source_type_bound_decl(bound_name, support_programs),
                        parameter,
                        TraitImplCapability::Clone,
                        support_programs,
                        search,
                    );
                    search.trait_impl_visiting.remove(&key);
                    required
                })
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
            let substitutions = source_receiver_owner_type_substitutions(
                owner,
                callee,
                &receiver_ty,
                support_programs,
            );
            owner
                .type_params
                .iter()
                .filter(|callee_type_param| {
                    substitutions
                        .get(*callee_type_param)
                        .is_some_and(|argument| java_type_contains_type_param(argument, type_param))
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
    for (index, name) in ty.type_params.iter().enumerate() {
        env.insert_type_param_declaration(
            name.clone(),
            ty.type_param_bounds.get(index).cloned().flatten(),
        );
    }
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
    clone_cache: Option<&CloneRequirementCache>,
) -> Vec<String> {
    let default_cache = CloneRequirementCache::default();
    let cache = clone_cache.unwrap_or(&default_cache);
    let mut memo = cache.search.borrow_mut();
    let mut visiting = cache.search_with_memo(&mut memo);
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
        Some(ty),
        None,
        None,
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
    java_semantic_hash_collection_for_java_type_in_scope(&field.ty, &owner.type_params)
}

pub(crate) fn java_semantic_hash_collection_for_java_type_in_scope(
    ty: &JavaType,
    type_params: &[String],
) -> Option<&'static str> {
    if let Some(element) = crate::types::optional_element_type(ty) {
        return java_semantic_hash_collection_for_java_type_in_scope(&element, type_params);
    }
    let JavaType::Named { name, args } = ty else {
        return None;
    };
    let simple = name.rsplit('.').next().unwrap_or(name);
    let key = match simple {
        "Map" | "HashMap" | "Hashtable" | "ConcurrentHashMap" => args.first()?,
        "Set" | "HashSet" => args.first()?,
        _ => return None,
    };
    if !type_params
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

pub(crate) fn java_semantic_hash_collection_field_rust_type(
    base: RustType,
    semantic_base: &str,
) -> Option<RustType> {
    match base {
        RustType::Option(inner) => {
            java_semantic_hash_collection_field_rust_type(*inner, semantic_base)
                .map(|inner| RustType::Option(Box::new(inner)))
        }
        // Representation planning deliberately renders sets of hierarchy or
        // erased carriers as `Vec<T>` when Rust's structural `Hash`/`Ord`
        // contracts are unavailable. A source field whose Java key is an
        // owner type parameter still needs Java equality semantics, so lift
        // that element type into the semantic set carrier instead of treating
        // the hash-free intermediate representation as an unsupported shape.
        RustType::Vec(inner) if semantic_base == "JavaSemanticHashSet" => Some(
            crate::slot_render::rust_generic_type(semantic_base, vec![*inner]),
        ),
        RustType::Generic { base, mut args }
            if matches!(
                base.as_str(),
                "std::rc::Rc"
                    | "std::sync::Arc"
                    | "std::cell::RefCell"
                    | "std::sync::Mutex"
                    | "std::boxed::Box"
                    | "Box"
            ) && args.len() == 1 =>
        {
            java_semantic_hash_collection_field_rust_type(args.remove(0), semantic_base)
                .map(|payload| crate::slot_render::rust_generic_type(base, vec![payload]))
        }
        RustType::Generic { base, args }
            if base == "JavaNativeCollection" && semantic_base == "JavaSemanticHashSet" =>
        {
            // Retained sets already carry their Java equality witness inside
            // the allocation. Replacing Rc/RefCell or this runtime payload
            // with a semantic set corrupts both identity and element type.
            Some(crate::slot_render::rust_generic_type(base, args))
        }
        RustType::Generic { args, .. } => {
            Some(crate::slot_render::rust_generic_type(semantic_base, args))
        }
        _ => None,
    }
}

#[cfg(test)]
mod semantic_hash_field_type_tests {
    use super::*;

    #[test]
    fn semantic_collection_projection_preserves_physical_allocation_wrappers() {
        for (handle, cell) in [
            ("std::rc::Rc", "std::cell::RefCell"),
            ("std::sync::Arc", "std::sync::Mutex"),
        ] {
            let element = RustType::Option(Box::new(RustType::Named("T".into())));
            let retained = crate::slot_render::rust_generic_type(
                handle,
                vec![crate::slot_render::rust_generic_type(
                    cell,
                    vec![crate::slot_render::rust_generic_type(
                        "JavaNativeCollection",
                        vec![element],
                    )],
                )],
            );
            for nullable in [false, true] {
                let retained = if nullable {
                    RustType::Option(Box::new(retained.clone()))
                } else {
                    retained.clone()
                };
                assert_eq!(
                    java_semantic_hash_collection_field_rust_type(
                        retained.clone(),
                        "JavaSemanticHashSet"
                    ),
                    Some(retained)
                );
            }
            let args = vec![RustType::Named("K".into()), RustType::Named("V".into())];
            let map = crate::slot_render::rust_generic_type(
                handle,
                vec![crate::slot_render::rust_generic_type(
                    cell,
                    vec![crate::slot_render::rust_generic_type(
                        "std::collections::HashMap",
                        args.clone(),
                    )],
                )],
            );
            let expected = crate::slot_render::rust_generic_type(
                handle,
                vec![crate::slot_render::rust_generic_type(
                    cell,
                    vec![crate::slot_render::rust_generic_type(
                        "JavaSemanticHashMap",
                        args,
                    )],
                )],
            );
            assert_eq!(
                java_semantic_hash_collection_field_rust_type(map, "JavaSemanticHashMap"),
                Some(expected)
            );
        }
    }

    #[test]
    fn hash_free_set_carrier_is_lifted_to_java_semantic_set() {
        let element = RustType::Named("HierarchyValue".to_string());
        assert_eq!(
            java_semantic_hash_collection_field_rust_type(
                RustType::Vec(Box::new(element.clone())),
                "JavaSemanticHashSet",
            ),
            Some(crate::slot_render::rust_generic_type(
                "JavaSemanticHashSet",
                vec![element.clone()],
            ))
        );
        assert_eq!(
            java_semantic_hash_collection_field_rust_type(
                RustType::Option(Box::new(RustType::Vec(Box::new(element.clone())))),
                "JavaSemanticHashSet",
            ),
            Some(RustType::Option(Box::new(
                crate::slot_render::rust_generic_type("JavaSemanticHashSet", vec![element],),
            )))
        );
    }
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
    fn has_subtype(declarations: &[TypeDecl], owner: &TypeDecl, program: &Program) -> bool {
        declarations.iter().any(|candidate| {
            (candidate.name != owner.name
                && crate::inherited_members::source_class_ancestor_type_arguments(
                    program, candidate, owner,
                )
                .is_some())
                || candidate.members.iter().any(|member| match member {
                    Member::Type(nested) => {
                        has_subtype(std::slice::from_ref(nested), owner, program)
                    }
                    _ => false,
                })
        })
    }
    // Share hierarchy parsing, lexical owner lookup and the cycle guard with
    // inherited-member specialization. Raw string equality misses Parent<T>.
    has_subtype(&support_program.types, ty, support_program)
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
        RustType::Generic { base, args } if unconditional_clone_carrier(base) => args.len() == 1,
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
        RustType::ImplFnTrait { .. } | RustType::DynFnTrait { .. } => false,
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

/// Unlike derive(Clone), a field-wise impl need not constrain a parameter
/// that occurs only inside an unconditionally cloneable reference carrier.
pub(crate) fn generic_source_struct_clone_impl(
    ty: &TypeDecl,
    strukt: &mut RustStruct,
) -> Option<RustItem> {
    if ty.type_params.is_empty() || !strukt.derives.iter().any(|derive| derive == "Clone") {
        return None;
    }
    strukt.derives.retain(|derive| derive != "Clone");
    let bounds = strukt
        .fields
        .iter()
        .map(|field| format!("{}: std::clone::Clone", crate::emit::emit_type(&field.ty)))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let fields = strukt
        .fields
        .iter()
        .map(|field| {
            let name = crate::representation_lowering::rust_identifier_token(&field.name);
            format!("{name}: std::clone::Clone::clone(&self.{name})")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let where_clause = if bounds.is_empty() {
        String::new()
    } else {
        format!(" where {bounds}")
    };
    Some(RustItem::Raw(format!(
        "impl<{}> Clone for {}{where_clause} {{\n    fn clone(&self) -> Self {{ Self {{ {fields} }} }}\n}}",
        ty.type_params.join(", "),
        rust_use_name_for_source_type(ty),
    )))
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
        | RustType::DynFnTrait { .. }
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
        | RustType::DynFnTrait { .. }
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
        | RustType::DynFnTrait { .. }
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

fn associated_consts_for_type(
    ty: &TypeDecl,
    value_overrides: &std::collections::BTreeMap<String, RustExpr>,
    support_program: &Program,
) -> Vec<RustAssociatedConst> {
    let const_infos = static_final_const_infos(ty);
    let source_static_field_rust_names = crate::java_naming::source_static_field_rust_names(ty);
    let lexical_access_crosses_module = source_type_has_context_subtypes(ty, support_program);
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
                value_overrides.get(&field.name),
            )
            .map(|mut constant| {
                constant.is_pub_crate = !constant.is_pub && lexical_access_crosses_module;
                constant
            }),
            _ => None,
        })
        .collect()
}

pub(crate) fn static_final_field_has_associated_const(ty: &TypeDecl, field: &FieldDecl) -> bool {
    if !is_static_final_field(field) {
        return false;
    }
    let const_infos = static_final_const_infos(ty);
    associated_const_for_field(field, &field.name, &const_infos, None).is_some()
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
    value_override: Option<&RustExpr>,
) -> Option<RustAssociatedConst> {
    let (ty, mut value) = field
        .initializer
        .as_ref()
        .and_then(|initializer| const_type_and_value(&field.ty, initializer, const_infos))
        .or_else(|| non_literal_static_final_const_type_and_value(field))?;
    if let Some(value_override) = value_override {
        value = value_override.clone();
    }
    Some(RustAssociatedConst {
        name: rust_name.to_string(),
        ty,
        value,
        is_pub: !field.modifiers.contains(&Modifier::Private),
        is_pub_crate: false,
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
    method_params_with_optional_lifetime(ty, method, signatures, facts, false, None, None)
}

fn method_params_with_optional_lifetime(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    lifetime: bool,
    representations: Option<&RepresentationPlan>,
    callable_analysis: Option<crate::callable_analysis::CallableAnalysis<'_>>,
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
            let callback_param = signatures.source_callback_is_inline(ty, method, index);
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
            let mutable_optional_borrow_param = param.shape().optional_element_type().is_some()
                && param.shape().ownership == OwnershipShape::MutableBorrow;
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
                    || mutable_optional_borrow_param
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
                        submit_callable_callback_param_type(
                            method,
                            source_java_ty,
                            facts,
                            representations,
                        )
                    } else {
                        functional_callback_param_type(source_java_ty, facts, representations)
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
                        && signatures.source_callback_plan().retains_for_method(
                            &ty.name,
                            &crate::java_signature::source_method_rust_name(method),
                            index,
                        )
                    {
                        RustType::Raw(format!(
                            "{} + 'static",
                            crate::emit::emit_type(&callback_ty)
                        ))
                    } else {
                        if callback_param && let Some(analysis) = callable_analysis {
                            let returns_result = signatures
                                .source_callback_plan()
                                .callable_key_for_method(
                                    &ty.name,
                                    &crate::java_signature::source_method_rust_name(method),
                                    index,
                                )
                                .is_some_and(|key| analysis.callable_returns_result(&key));
                            borrowed_source_callback_type(callback_ty, returns_result)
                        } else {
                            callback_ty
                        }
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
fn functional_callback_param_type(
    java_ty: &JavaType,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> Option<RustType> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(name.as_str());
    let shape = crate::stdlib::functional_interface_shape(simple)?;

    let inputs = shape
        .input_types(args)?
        .iter()
        .map(|ty| {
            // A SAM input is a Java reference value, not an ordinary source
            // method's borrowed receiver view. Match stored native SAMs and
            // typed invocation/lambda lowering through the storage projection.
            let shape = representations.map_or_else(
                || crate::shape_projection::project_java_value_shape(None, ty.clone()),
                |representations| {
                    crate::shape_projection::storage_shape_for_java_type(
                        representations,
                        ty.clone(),
                    )
                },
            );
            rust_type_for_slot_with_optional_representations(&shape, facts, representations)
        })
        .collect();
    // A missing result *type argument* means the source did not spell the
    // generic out; keep the previous behaviour rather than guess.
    let return_type = match shape.returns() {
        Some(_) => Some(Box::new(
            rust_type_for_java_type_with_optional_representations(
                &shape.return_type(args)?,
                facts,
                representations,
            ),
        )),
        None => None,
    };
    Some(RustType::ImplFnTrait {
        trait_name: "FnMut".to_string(),
        params: inputs,
        return_type,
    })
}

/// A non-escaping source callback needs no generic type identity. Erasing it at
/// the call boundary keeps recursion finite and gives every forwarder the same
/// ABI while retaining the SAM's structured inputs and return carrier.
fn borrowed_source_callback_type(ty: RustType, returns_result: bool) -> RustType {
    let RustType::ImplFnTrait {
        trait_name,
        params,
        return_type,
    } = ty
    else {
        return ty;
    };
    let return_type = if returns_result {
        Some(Box::new(exception_result_type(
            return_type.map(|ty| *ty).unwrap_or(RustType::Unit),
        )))
    } else {
        return_type
    };
    RustType::Ref {
        mutable: true,
        lifetime: None,
        inner: Box::new(RustType::DynFnTrait {
            trait_name,
            params,
            return_type,
        }),
    }
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
    representations: Option<&RepresentationPlan>,
) -> Option<RustType> {
    if !java_type_is_callable(java_ty) {
        return None;
    }
    let return_type = match java_ty {
        JavaType::Named { args, .. } => args
            .first()
            .map(|ty| {
                rust_type_for_java_type_with_optional_representations(ty, facts, representations)
            })
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
        RustType::DynFnTrait {
            trait_name,
            params,
            return_type,
        } => RustType::DynFnTrait {
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
        }
        | RustType::DynFnTrait {
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

fn rust_type_for_java_type_with_optional_representations(
    ty: &JavaType,
    facts: &FactSet,
    representations: Option<&RepresentationPlan>,
) -> RustType {
    let shape = crate::shape_projection::project_java_value_shape(representations, ty.clone());
    rust_type_for_slot_with_optional_representations(&shape, facts, representations)
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
