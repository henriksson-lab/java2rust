use std::collections::{HashMap, HashSet};

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
    optional_element_type, raw_rust_box_dyn_trait, raw_rust_box_sized_inner, raw_rust_generic_args,
    raw_rust_option_inner, raw_rust_vec_inner, sequence_element_type,
    two_dimensional_array_element_type,
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

fn rust_impl_target_for_source_type(
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
    let JavaType::Named { name: bound, .. } = bound else {
        return Vec::new();
    };
    if source_type_bound_is_trait_like(bound, support_programs) {
        return vec![rust_name_for_java_named_type(bound)];
    }
    if source_type_bound_is_class_like(bound, support_programs) {
        return vec![crate::representation_lowering::source_class_bound_trait_name(bound)];
    }
    // The bound names a library type rather than a source one. Dropping it left
    // the parameter with no capability at all, so `T extends Comparable<T>`
    // lost `compareTo`.
    jdk_type_param_bound_rust_bounds(bound)
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

/// `Comparable` maps to `Ord`; the generated support library blanket-implements
/// `JavaComparable` (and therefore `compare_to`) for every `T: Ord`.
fn jdk_type_param_bound_rust_bounds(bound: &str) -> Vec<String> {
    match bound.rsplit('.').next().unwrap_or(bound) {
        "Comparable" => vec!["Ord".to_string()],
        "Cloneable" => vec!["Clone".to_string()],
        _ => Vec::new(),
    }
}

fn rust_impl_params_for_source_type(
    ty: &TypeDecl,
    support_program: &Program,
    representations: Option<&RepresentationPlan>,
) -> Vec<String> {
    ty.type_params
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let mut bounds = Vec::new();
            if let Some(Some(bound)) = ty.type_param_bounds.get(index) {
                bounds.extend(rust_bounds_for_java_type_param(bound, &[support_program]));
            }
            if source_type_param_needs_clone_bound(ty, name) {
                bounds.push("Clone".to_string());
            }
            if let Some(Some(JavaType::Named { name: bound, .. })) = ty.type_param_bounds.get(index)
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
        || callable_type_param_collection_get_needs_clone_bound(type_param, params, body)
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

fn source_type_param_needs_clone_bound(ty: &TypeDecl, type_param: &str) -> bool {
    if ty.members.iter().any(|member| {
        matches!(
            member,
            Member::Method(method)
                if source_method_clones_class_type_param_param_to_owned_local(method, type_param)
        )
    }) {
        return true;
    }
    if !type_has_field_containing_type_param(ty, type_param) {
        return false;
    }
    let fields = source_field_types(ty);
    ty.members.iter().any(|member| match member {
        Member::Method(method) => {
            source_method_needs_class_type_param_clone_bound(method, type_param, &fields)
        }
        _ => false,
    })
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
            later_param_read
                && java_type_contains_type_param(ty, type_param)
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

fn type_has_field_containing_type_param(ty: &TypeDecl, type_param: &str) -> bool {
    ty.members.iter().any(|member| match member {
        Member::Field(field) => java_type_contains_type_param(&field.ty, type_param),
        _ => false,
    })
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

fn source_method_needs_class_type_param_clone_bound(
    method: &MethodDecl,
    type_param: &str,
    fields: &HashMap<String, JavaType>,
) -> bool {
    if !java_type_contains_type_param(&method.return_type, type_param) {
        return false;
    }
    let Some(body) = &method.body else {
        return false;
    };
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

fn java_type_contains_type_param(ty: &JavaType, type_param: &str) -> bool {
    match ty {
        JavaType::Named { name, args } => {
            (args.is_empty() && name == type_param)
                || args
                    .iter()
                    .any(|arg| java_type_contains_type_param(arg, type_param))
        }
        JavaType::Unknown(name) => name == type_param,
        JavaType::Array(element) => java_type_contains_type_param(element, type_param),
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
        body,
        is_pub: true,
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

fn function_type_params_with_optional_lifetime_and_support_contexts(
    callable_name: Option<&str>,
    type_params: &[String],
    type_param_bounds: &[Option<JavaType>],
    needs_lifetime: bool,
    support_programs: &[&Program],
    callable_params: &[crate::java_ir::Param],
    callable_body: Option<&crate::java_ir::Block>,
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
        if callable_type_param_needs_clone_bound(name, callable_params, callable_body) {
            bounds.push("Clone".to_string());
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
) -> RustFunction {
    let needs_lifetime = method_needs_borrowed_return_lifetime(ty, method, signatures, facts);
    let name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    RustFunction {
        name,
        type_params: function_type_params_with_optional_lifetime_and_support_contexts(
            Some(&method.name),
            &method.type_params,
            &method.type_param_bounds,
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
        body,
        is_pub: true,
        receiver: lower_receiver(method_receiver(ty, method, signatures)),
    }
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
        fields.push(RustField {
            name: "__outer".to_string(),
            ty: rc_refcell_type(RustType::Named(rust_name_for_java_named_type(outer_type))),
            is_pub: false,
            is_pub_crate: false,
        });
    }
    let upcast_visible_fields = source_type_has_context_subtypes(ty, support_program);
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
                is_pub_crate: !source_field_is_public(field) && upcast_visible_fields,
            }),
            _ => None,
        }
    }));
    fields.extend(
        crate::inherited_members::inherited_instance_fields_for_type(ty, support_program)
            .into_iter()
            .map(|inherited| RustField {
                name: inherited.field().name.clone(),
                ty: rust_type_for_slot_maybe_with_representations(
                    &signatures.required_field_shape(&ty.name, &inherited.field().name),
                    facts,
                    representations,
                ),
                is_pub: false,
                is_pub_crate: upcast_visible_fields,
            }),
    );
    fields.extend(
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
            .into_iter()
            .map(|inherited| RustField {
                name: inherited.rust_name().to_string(),
                ty: rust_type_for_slot_maybe_with_representations(
                    &signatures.required_field_shape(&ty.name, inherited.rust_name()),
                    facts,
                    representations,
                ),
                is_pub: false,
                is_pub_crate: upcast_visible_fields,
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
    ty.members.iter().any(|member| {
        matches!(member, Member::Method(method)
            if method.modifiers.contains(&Modifier::Synchronized)
                && !method.modifiers.contains(&Modifier::Static))
    })
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
    if instance_field_is_self_referential(owner, field_name, shape)
        && !field_type_is_shared_handle(shape, facts)
    {
        return box_recursive_field_type(rust_ty);
    }
    rust_ty
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

fn rc_refcell_type(inner: RustType) -> RustType {
    crate::slot_render::rust_generic_type(
        "std::rc::Rc",
        vec![crate::slot_render::rust_generic_type(
            "std::cell::RefCell",
            vec![inner],
        )],
    )
}

fn source_field_is_public(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Public)
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
    if fields.iter().all(|field| {
        rust_type_supports_struct_derive_clone(
            &field.ty,
            &type_params,
            signatures,
            facts,
            support_program,
            &mut HashSet::new(),
        )
    }) {
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

fn program_needs_constructor_placeholder_default_for_type(
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
                    constructor_stmt_assigns_self_dependent_field_of_type(stmt, &fields, target)
                })
            })
        })
    })
}

fn constructor_stmt_assigns_self_dependent_field_of_type(
    stmt: &Stmt,
    fields: &[&FieldDecl],
    target: &TypeDecl,
) -> bool {
    match stmt {
        Stmt::Assign { target: lhs, value } => {
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
            }) && constructor_expr_reads_self(value)
        }
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            then_block.statements.iter().any(|stmt| {
                constructor_stmt_assigns_self_dependent_field_of_type(stmt, fields, target)
            }) || else_block.as_ref().is_some_and(|block| {
                block.statements.iter().any(|stmt| {
                    constructor_stmt_assigns_self_dependent_field_of_type(stmt, fields, target)
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
        && let Some(value) = static_final_pattern_compile_literal(field.initializer.as_ref()?)
    {
        return Some((
            RustType::Named(rust_name),
            function_call_projection(
                owner_associated_path_projection("Pattern", "compile_static"),
                vec![rust_string_literal_projection(&value)],
            ),
        ));
    }
    Some((RustType::Named(rust_name.clone()), RustExpr::Var(rust_name)))
}

fn static_final_pattern_compile_literal(initializer: &Expr) -> Option<String> {
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
    let [Expr::Literal(Literal::String(value))] = args.as_slice() else {
        return None;
    };
    Some(value.clone())
}

fn const_type_and_value(
    ty: &JavaType,
    initializer: &Expr,
    const_infos: &[StaticFinalConstInfo],
) -> Option<(RustType, RustExpr)> {
    let rust_ty = const_rust_type_for_java_type(ty)?;
    let value = const_expr_value(initializer, const_infos, Some(ty))?;
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
                    maybe_add_lifetime_to_refs(
                        rust_type_for_slot_with_optional_representations(
                            &param.shape(),
                            facts,
                            representations,
                        ),
                        lifetime,
                    )
                },
            }
        })
        .collect()
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
