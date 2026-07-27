use std::collections::HashSet;

use crate::coercion::{
    function_call_projection, owner_associated_path_projection, self_associated_path_projection,
};
use crate::exception_lowering::exception_result_type;
use crate::exception_plan::MethodExceptionPlan;
use crate::facts::FactSet;
use crate::java_ir::{
    BinaryOp, ConstructorDecl, Expr, FieldDecl, JavaType, Literal, Member, MethodDecl, Modifier,
    PrimitiveType, Program, Stmt, TypeDecl,
};
use crate::rust_ir::{
    RustAssociatedConst, RustExpr, RustField, RustFunction, RustImpl, RustItem, RustParam,
    RustReceiver, RustStmt, RustStruct, RustType,
};
use crate::signature::{ReceiverPlan, SignaturePlan};
use crate::slot_render::{
    default_expr_for_slot, default_expr_for_slot_with_facts, rust_name_for_java_named_type,
    rust_type_for_java_type_with_facts, rust_type_for_slot_with_facts,
};
use crate::slot_shape::{OwnershipShape, RustSlotShape};
use crate::types::{
    optional_element_type, raw_rust_box_dyn_trait, raw_rust_generic_args, raw_rust_option_inner,
    raw_rust_vec_inner, sequence_element_type, two_dimensional_array_element_type,
};

pub fn struct_item_for_type(ty: &TypeDecl, signatures: &SignaturePlan) -> RustItem {
    struct_item_for_type_with_facts(ty, signatures, &FactSet::default())
}

pub fn struct_item_for_type_with_facts(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> RustItem {
    struct_item_for_type_with_facts_and_support_context(ty, signatures, facts, &Program::default())
}

pub fn struct_item_for_type_with_facts_and_support_context(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
) -> RustItem {
    let fields = struct_fields_for_type(ty, signatures, facts, support_program);
    RustItem::Struct(RustStruct {
        name: rust_name_for_java_named_type(&ty.name),
        derives: struct_derives_for_fields(&fields, signatures, facts, support_program),
        fields,
    })
}

pub fn impl_item_for_type(ty: &TypeDecl, functions: Vec<RustFunction>) -> RustItem {
    RustItem::Impl(RustImpl {
        target: rust_name_for_java_named_type(&ty.name),
        associated_consts: associated_consts_for_type(ty),
        functions,
    })
}

pub fn raw_item(item: String) -> RustItem {
    RustItem::Raw(item)
}

pub fn constructor_function_shell(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
) -> RustFunction {
    constructor_function_shell_with_facts(ty, ctor, signatures, body, &FactSet::default())
}

pub fn constructor_function_shell_with_facts(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
) -> RustFunction {
    RustFunction {
        name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        params: constructor_params(ty, ctor, signatures, facts),
        return_type: Some(constructor_return_type(ty, ctor, signatures)),
        body,
        is_pub: true,
        receiver: None,
    }
}

pub fn constructor_return_body(ty: &TypeDecl, fields: Vec<(String, RustExpr)>) -> Vec<RustStmt> {
    vec![RustStmt::Return(Some(RustExpr::StructLit {
        name: rust_name_for_java_named_type(&ty.name),
        fields: reorder_struct_literal_moves_after_reads(fields),
    }))]
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

fn rust_expr_direct_move_var(expr: &RustExpr) -> Option<String> {
    match expr {
        RustExpr::Var(name) => Some(name.clone()),
        RustExpr::Raw(text) if valid_rust_identifier(text) => Some(text.clone()),
        _ => None,
    }
}

fn rust_expr_reads_var(expr: &RustExpr, var_name: &str) -> bool {
    match expr {
        RustExpr::Var(name) => name == var_name,
        RustExpr::Field(receiver, _) => rust_expr_reads_var(receiver, var_name),
        RustExpr::Index(target, index) => {
            rust_expr_reads_var(target, var_name) || rust_expr_reads_var(index, var_name)
        }
        RustExpr::Unary { expr, .. } => rust_expr_reads_var(expr, var_name),
        RustExpr::Cast { expr, .. } => rust_expr_reads_var(expr, var_name),
        RustExpr::Binary { left, right, .. } => {
            rust_expr_reads_var(left, var_name) || rust_expr_reads_var(right, var_name)
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
        RustExpr::Call { function, args } => {
            rust_expr_reads_var(function, var_name)
                || args.iter().any(|arg| rust_expr_reads_var(arg, var_name))
        }
        RustExpr::MethodCall { receiver, args, .. } => {
            rust_expr_reads_var(receiver, var_name)
                || args.iter().any(|arg| rust_expr_reads_var(arg, var_name))
        }
        RustExpr::StructLit { fields, .. } => fields
            .iter()
            .any(|(_, field_expr)| rust_expr_reads_var(field_expr, var_name)),
        RustExpr::Raw(text) => raw_rust_expr_mentions_identifier(text, var_name),
        RustExpr::Literal(_) => false,
    }
}

fn valid_rust_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn raw_rust_expr_mentions_identifier(text: &str, ident: &str) -> bool {
    text.split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .any(|token| token == ident)
}

pub fn constructor_default_field_value(
    field_name: String,
    shape: RustSlotShape,
) -> (String, RustExpr) {
    (field_name, default_expr_for_slot(&shape))
}

pub fn constructor_default_field_value_with_facts(
    field_name: String,
    shape: RustSlotShape,
    facts: &FactSet,
) -> (String, RustExpr) {
    (field_name, default_expr_for_slot_with_facts(&shape, facts))
}

pub fn delegating_constructor_return_body(rust_name: &str, args: Vec<RustExpr>) -> Vec<RustStmt> {
    vec![RustStmt::Return(Some(function_call_projection(
        self_associated_path_projection(rust_name),
        args,
    )))]
}

pub fn method_function_shell(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
) -> RustFunction {
    method_function_shell_with_facts(ty, method, signatures, body, &FactSet::default())
}

pub fn method_function_shell_with_facts(
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
        MethodExceptionPlan {
            wrap_returns_in_result: false,
        },
    )
}

pub fn method_function_shell_with_facts_and_exception_plan(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    body: Vec<RustStmt>,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
) -> RustFunction {
    let needs_lifetime =
        static_method_needs_borrowed_return_lifetime(ty, method, signatures, facts);
    let mut name = method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name));
    if needs_lifetime {
        name.push_str("<'a>");
    }
    RustFunction {
        name,
        params: method_params_with_optional_lifetime(ty, method, signatures, facts, needs_lifetime),
        return_type: method_return_type_with_optional_lifetime(
            ty,
            method,
            signatures,
            facts,
            exceptions,
            needs_lifetime,
        ),
        body,
        is_pub: true,
        receiver: lower_receiver(method_receiver(ty, method, signatures)),
    }
}

fn static_method_needs_borrowed_return_lifetime(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> bool {
    method.modifiers.contains(&Modifier::Static)
        && method_return_type_with_facts(ty, method, signatures, facts)
            .as_ref()
            .is_some_and(rust_type_contains_ref)
        && method_params(ty, method, signatures, facts)
            .iter()
            .any(|param| rust_type_contains_ref(&param.ty))
}

pub fn method_return_shape(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
) -> Option<RustSlotShape> {
    signatures
        .required_method_signature(ty, method)
        .return_shape
        .clone()
}

pub fn method_return_type(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
) -> Option<RustType> {
    method_return_type_with_facts(ty, method, signatures, &FactSet::default())
}

pub fn method_return_type_with_facts(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Option<RustType> {
    let return_shape = method_return_shape(ty, method, signatures);
    if let Some(shape) = return_shape.as_ref() {
        return Some(rust_type_for_slot_with_facts(shape, facts));
    }
    if let Some(ty) = rust_crate_mapped_external_rust_type(&method.return_type, facts) {
        return Some(ty);
    }
    if rust_crate_mapped_external_java_type(&method.return_type, facts) {
        return Some(rust_type_for_java_type_with_facts(
            &method.return_type,
            facts,
        ));
    }
    None
}

fn rust_crate_mapped_external_rust_type(java_ty: &JavaType, facts: &FactSet) -> Option<RustType> {
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
            .map(|arg| crate::emit::emit_type(&rust_type_for_java_type_with_facts(arg, facts)))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(RustType::Raw(format!(
            "{crate_name}::{rust_type_name}<{args}>"
        )));
    }
    Some(RustType::Raw(format!("{crate_name}::{rust_type_name}")))
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
    support_program: &Program,
) -> Vec<RustField> {
    let mut fields = Vec::new();
    if let Some(outer_type) = facts.captures.outer_this_type_for_type(&ty.name) {
        fields.push(RustField {
            name: "__outer".to_string(),
            ty: RustType::Raw(format!(
                "std::rc::Rc<std::cell::RefCell<{}>>",
                rust_name_for_java_named_type(outer_type)
            )),
            is_pub: false,
            is_pub_crate: false,
        });
    }
    let upcast_visible_fields = source_type_has_context_subtypes(ty, support_program);
    fields.extend(ty.members.iter().filter_map(|member| match member {
        Member::Field(field) if !is_static_field(field) => Some(RustField {
            name: field.name.clone(),
            ty: rust_type_for_instance_field(
                ty,
                &field.name,
                &signatures.required_field_shape(&ty.name, &field.name),
                facts,
            ),
            is_pub: source_field_is_public(field),
            is_pub_crate: !source_field_is_public(field) && upcast_visible_fields,
        }),
        _ => None,
    }));
    fields.extend(
        crate::inherited_members::inherited_instance_fields_for_type(ty, support_program)
            .into_iter()
            .map(|inherited| RustField {
                name: inherited.field.name.clone(),
                ty: rust_type_for_slot_with_facts(
                    &signatures.required_field_shape(&ty.name, &inherited.field.name),
                    facts,
                ),
                is_pub: false,
                is_pub_crate: upcast_visible_fields,
            }),
    );
    fields.extend(
        crate::inherited_members::external_inherited_instance_fields_for_type(ty, facts)
            .into_iter()
            .map(|inherited| RustField {
                name: inherited.rust_name.clone(),
                ty: rust_type_for_slot_with_facts(
                    &signatures.required_field_shape(&ty.name, &inherited.rust_name),
                    facts,
                ),
                is_pub: false,
                is_pub_crate: upcast_visible_fields,
            }),
    );
    let mut seen = HashSet::new();
    fields.retain(|field| seen.insert(field.name.clone()));
    fields
}

fn rust_type_for_instance_field(
    owner: &TypeDecl,
    field_name: &str,
    shape: &RustSlotShape,
    facts: &FactSet,
) -> RustType {
    let rust_ty = rust_type_for_slot_with_facts(shape, facts);
    if instance_field_is_self_referential(owner, field_name, shape) {
        return box_recursive_field_type(rust_ty);
    }
    rust_ty
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
        RustType::Option(inner) => RustType::Option(Box::new(RustType::Raw(format!(
            "Box<{}>",
            crate::emit::emit_type(&inner)
        )))),
        other => RustType::Raw(format!("Box<{}>", crate::emit::emit_type(&other))),
    }
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

fn struct_derives_for_fields(
    fields: &[RustField],
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
) -> Vec<String> {
    let mut derives = Vec::new();
    if fields.iter().all(|field| {
        rust_type_supports_struct_derive_clone(
            &field.ty,
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
    derives
}

fn rust_type_supports_struct_derive_clone(
    ty: &RustType,
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
                signatures,
                facts,
                support_program,
                visiting,
            )
        }
        RustType::Named(name) => rust_named_type_supports_struct_derive_clone(
            name,
            signatures,
            facts,
            support_program,
            visiting,
        ),
        RustType::Raw(raw)
            if raw.starts_with("std::rc::Rc<")
                || raw_rust_type_supports_derive_clone(
                    raw,
                    signatures,
                    facts,
                    support_program,
                    visiting,
                ) =>
        {
            true
        }
        RustType::Slice(_) | RustType::Ref { .. } | RustType::Raw(_) => false,
    }
}

fn raw_rust_type_supports_derive_clone(
    raw: &str,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    let raw = raw.trim();
    if raw_rust_box_dyn_trait(raw).is_some() {
        return true;
    }
    if let Some(args) = raw_rust_generic_args(raw, "std::collections::HashMap")
        && args.len() == 2
    {
        return args.iter().all(|arg| {
            raw_type_arg_supports_derive_clone(arg, signatures, facts, support_program, visiting)
        });
    }
    false
}

fn raw_type_arg_supports_derive_clone(
    arg: &str,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    let raw = arg.trim();
    if matches!(
        raw,
        "String" | "bool" | "char" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64"
    ) {
        return true;
    }
    if let Some(inner) = raw_rust_option_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if let Some(inner) = raw_rust_vec_inner(raw) {
        return raw_type_arg_supports_derive_clone(
            inner,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    if raw_rust_box_dyn_trait(raw).is_some() {
        return true;
    }
    if raw.contains('<') {
        return raw_rust_type_supports_derive_clone(
            raw,
            signatures,
            facts,
            support_program,
            visiting,
        );
    }
    rust_named_type_supports_struct_derive_clone(raw, signatures, facts, support_program, visiting)
}

fn rust_named_type_supports_struct_derive_clone(
    name: &str,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
    visiting: &mut HashSet<String>,
) -> bool {
    let Some(ty) = find_type_decl_by_rust_name(support_program, name) else {
        return true;
    };
    if !visiting.insert(ty.name.clone()) {
        return true;
    }
    let fields = struct_fields_for_type(ty, signatures, facts, support_program);
    let supports_clone = fields.iter().all(|field| {
        rust_type_supports_struct_derive_clone(
            &field.ty,
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
    program
        .types
        .iter()
        .find_map(|ty| find_type_decl_by_rust_name_in_type(ty, rust_name))
}

fn find_type_decl_by_rust_name_in_type<'a>(
    ty: &'a TypeDecl,
    rust_name: &str,
) -> Option<&'a TypeDecl> {
    if rust_name_for_java_named_type(&ty.name) == rust_name {
        return Some(ty);
    }
    ty.members.iter().find_map(|member| match member {
        Member::Type(nested) => find_type_decl_by_rust_name_in_type(nested, rust_name),
        _ => None,
    })
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
        RustType::Named(_) => false,
        RustType::Slice(_) | RustType::Ref { .. } | RustType::Raw(_) => false,
    }
}

pub fn is_static_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static)
}

pub fn is_static_final_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static) && field.modifiers.contains(&Modifier::Final)
}

pub fn is_static_final_non_const_field(field: &FieldDecl) -> bool {
    if !is_static_final_field(field) {
        return false;
    }
    field
        .initializer
        .as_ref()
        .is_none_or(|initializer| const_type_and_value(&field.ty, initializer, &[]).is_none())
}

fn associated_consts_for_type(ty: &TypeDecl) -> Vec<RustAssociatedConst> {
    let const_infos = static_final_const_infos(ty);
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if is_static_final_field(field) => {
                associated_const_for_field(field, &const_infos)
            }
            _ => None,
        })
        .collect()
}

pub fn static_final_field_has_associated_const(ty: &TypeDecl, field: &FieldDecl) -> bool {
    if !is_static_final_field(field) {
        return false;
    }
    let const_infos = static_final_const_infos(ty);
    associated_const_for_field(field, &const_infos).is_some()
}

#[derive(Debug, Clone)]
struct StaticFinalConstInfo {
    normalized: String,
    original: String,
    array_len: Option<usize>,
}

fn static_final_const_infos(ty: &TypeDecl) -> Vec<StaticFinalConstInfo> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if is_static_final_field(field) => Some(StaticFinalConstInfo {
                normalized: crate::parser::to_snake_case(&field.name),
                original: field.name.clone(),
                array_len: static_final_array_initializer_len(field),
            }),
            _ => None,
        })
        .collect()
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
    const_infos: &[StaticFinalConstInfo],
) -> Option<RustAssociatedConst> {
    let (ty, value) = field
        .initializer
        .as_ref()
        .and_then(|initializer| const_type_and_value(&field.ty, initializer, const_infos))
        .or_else(|| non_literal_static_final_const_type_and_value(field))
        .or_else(|| static_final_default_const_type_and_value(field))?;
    Some(RustAssociatedConst {
        name: field.name.clone(),
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
    let JavaType::Named { name, .. } = &field.ty else {
        return None;
    };
    let rust_name = rust_name_for_java_named_type(name);
    Some((RustType::Named(rust_name.clone()), RustExpr::Var(rust_name)))
}

fn static_final_default_const_type_and_value(field: &FieldDecl) -> Option<(RustType, RustExpr)> {
    if !is_static_final_field(field) || field.initializer.is_some() {
        return None;
    }
    if !matches!(field.ty, JavaType::Array(_)) {
        return None;
    }
    let rust_ty = const_rust_type_for_java_type(&field.ty)?;
    Some((rust_ty, RustExpr::Raw("&[]".to_string())))
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
            Some(RustType::Raw(format!(
                "&'static [{}]",
                crate::emit::emit_type(&element_ty)
            )))
        }
        JavaType::String => Some(RustType::Raw("&'static str".to_string())),
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
        Expr::Literal(literal) => const_literal_expr_value(literal, expected_ty),
        Expr::Var(name) => {
            let member = const_reference_name(name, const_infos);
            Some(self_associated_path_projection(&member))
        }
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            let member = const_reference_name(field, const_infos);
            Some(self_associated_path_projection(&member))
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

fn const_array_expr_value(
    elements: &[Expr],
    const_infos: &[StaticFinalConstInfo],
    element_ty: &JavaType,
) -> Option<RustExpr> {
    let values = elements
        .iter()
        .map(|element| const_expr_value(element, const_infos, Some(element_ty)))
        .collect::<Option<Vec<_>>>()?;
    Some(RustExpr::Raw(format!(
        "&[{}]",
        values
            .iter()
            .map(crate::emit::emit_expr)
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

fn expected_array_element_type(expected_ty: Option<&JavaType>) -> Option<&JavaType> {
    match expected_ty {
        Some(JavaType::Array(element)) => Some(element.as_ref()),
        _ => None,
    }
}

fn const_literal_expr_value(literal: &Literal, expected_ty: Option<&JavaType>) -> Option<RustExpr> {
    match (literal, expected_ty) {
        (Literal::String(value), _) => Some(RustExpr::Literal(format!("{value:?}"))),
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
    const_infos
        .iter()
        .find(|info| name == info.normalized || name == info.original)
        .map(|info| info.original.as_str())
        .unwrap_or(name)
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
    signatures
        .required_constructor_signature(ty, ctor)
        .params
        .iter()
        .map(|param| RustParam {
            name: param.name.clone(),
            ty: rust_type_for_slot_with_facts(&param.shape, facts),
        })
        .collect()
}

fn constructor_return_type(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    signatures: &SignaturePlan,
) -> RustType {
    let _ = signatures.required_constructor_signature(ty, ctor);
    RustType::Named(rust_name_for_java_named_type(&ty.name))
}

fn method_params(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
) -> Vec<RustParam> {
    method_params_with_optional_lifetime(ty, method, signatures, facts, false)
}

fn method_params_with_optional_lifetime(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    lifetime: bool,
) -> Vec<RustParam> {
    let signature = signatures.required_method_signature(ty, method);
    let mutable_array_target_params = mutable_array_target_params(method);
    signature
        .params
        .iter()
        .map(|param| {
            let callback_param =
                method_param_is_nonescaping_functional_callback(method, &param.name);
            let mutable_optional_sequence_param = mutable_array_target_params.contains(&param.name)
                && optional_sequence_slot(&param.shape);
            let mutable_owned_binding_param = param.binding_mutable
                && matches!(
                    param.shape.ownership,
                    OwnershipShape::Owned | OwnershipShape::CopyValue
                );
            RustParam {
                name: if callback_param
                    || mutable_optional_sequence_param
                    || mutable_owned_binding_param
                {
                    format!("mut {}", param.name)
                } else {
                    param.name.clone()
                },
                ty: if callback_param {
                    functional_callback_param_type(&param.java_ty, facts).unwrap_or_else(|| {
                        maybe_add_lifetime_to_refs(
                            rust_type_for_slot_with_facts(&param.shape, facts),
                            lifetime,
                        )
                    })
                } else {
                    maybe_add_lifetime_to_refs(
                        rust_type_for_slot_with_facts(&param.shape, facts),
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
            Stmt::While { condition, body } | Stmt::DoWhile { condition, body } => {
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
            Stmt::Block(block) => {
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
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_mutable_array_target_params_in_expr(receiver, names);
            }
            for arg in args {
                collect_mutable_array_target_params_in_expr(arg, names);
            }
        }
        Expr::New { args, .. } | Expr::ArrayInitializer(args) => {
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
        | Expr::Raw(_) => {}
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

fn functional_callback_param_type(java_ty: &JavaType, facts: &FactSet) -> Option<RustType> {
    let JavaType::Named { name, args } = java_ty else {
        return None;
    };
    let canonical = crate::stdlib::canonical_external_receiver_type_key(name);
    let simple = canonical.rsplit('.').next().unwrap_or(name.as_str());
    let input_count = match simple {
        "Supplier" | "Runnable" => 0,
        "Consumer" | "Function" | "Predicate" | "UnaryOperator" => 1,
        "BiConsumer" | "BiFunction" | "BinaryOperator" => 2,
        _ => return None,
    };
    if args.len() < input_count {
        return None;
    }
    let inputs = args
        .iter()
        .take(input_count)
        .map(|arg| crate::emit::emit_type(&rust_type_for_java_type_with_facts(arg, facts)))
        .collect::<Vec<_>>()
        .join(", ");
    let return_ty = match simple {
        "Function" | "UnaryOperator" => args
            .get(1)
            .map(|ty| crate::emit::emit_type(&rust_type_for_java_type_with_facts(ty, facts))),
        "BiFunction" | "BinaryOperator" => args
            .get(2)
            .map(|ty| crate::emit::emit_type(&rust_type_for_java_type_with_facts(ty, facts))),
        "Predicate" => Some("bool".to_string()),
        _ => None,
    };
    let rendered = return_ty
        .map(|ret| format!("impl FnMut({inputs}) -> {ret}"))
        .unwrap_or_else(|| format!("impl FnMut({inputs})"));
    Some(RustType::Raw(rendered))
}

fn method_param_is_nonescaping_functional_callback(method: &MethodDecl, param_name: &str) -> bool {
    if !method.params.iter().any(|param| {
        param.name == param_name
            && functional_callback_param_type(&param.ty, &FactSet::default()).is_some()
    }) {
        return false;
    }
    let Some(body) = &method.body else {
        return false;
    };
    let mut saw_callback_call = false;
    let mut escaped = false;
    scan_block_for_callback_param(body, param_name, &mut saw_callback_call, &mut escaped);
    saw_callback_call && !escaped
}

fn scan_block_for_callback_param(
    block: &crate::java_ir::Block,
    param_name: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
) {
    for stmt in &block.statements {
        scan_stmt_for_callback_param(stmt, param_name, saw_callback_call, escaped);
    }
}

fn scan_stmt_for_callback_param(
    stmt: &crate::java_ir::Stmt,
    param_name: &str,
    saw_callback_call: &mut bool,
    escaped: &mut bool,
) {
    match stmt {
        crate::java_ir::Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
            }
        }
        crate::java_ir::Stmt::Assign { target, value } => {
            scan_expr_for_callback_param(target, param_name, saw_callback_call, escaped);
            scan_expr_for_callback_param(value, param_name, saw_callback_call, escaped);
        }
        crate::java_ir::Stmt::Expr(expr)
        | crate::java_ir::Stmt::Return(Some(expr))
        | crate::java_ir::Stmt::Throw(expr) => {
            scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
        }
        crate::java_ir::Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            scan_expr_for_callback_param(condition, param_name, saw_callback_call, escaped);
            scan_block_for_callback_param(then_block, param_name, saw_callback_call, escaped);
            if let Some(block) = else_block {
                scan_block_for_callback_param(block, param_name, saw_callback_call, escaped);
            }
        }
        crate::java_ir::Stmt::While { condition, body }
        | crate::java_ir::Stmt::DoWhile { condition, body } => {
            scan_expr_for_callback_param(condition, param_name, saw_callback_call, escaped);
            scan_block_for_callback_param(body, param_name, saw_callback_call, escaped);
        }
        crate::java_ir::Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                scan_stmt_for_callback_param(stmt, param_name, saw_callback_call, escaped);
            }
            if let Some(expr) = condition {
                scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
            }
            for expr in update {
                scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
            }
            scan_block_for_callback_param(body, param_name, saw_callback_call, escaped);
        }
        crate::java_ir::Stmt::EnhancedFor { iterable, body, .. } => {
            scan_expr_for_callback_param(iterable, param_name, saw_callback_call, escaped);
            scan_block_for_callback_param(body, param_name, saw_callback_call, escaped);
        }
        crate::java_ir::Stmt::Switch { selector, cases } => {
            scan_expr_for_callback_param(selector, param_name, saw_callback_call, escaped);
            for case in cases {
                for label in &case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
                    }
                }
                for stmt in &case.statements {
                    scan_stmt_for_callback_param(stmt, param_name, saw_callback_call, escaped);
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
                scan_stmt_for_callback_param(stmt, param_name, saw_callback_call, escaped);
            }
            scan_block_for_callback_param(body, param_name, saw_callback_call, escaped);
            for catch in catches {
                scan_block_for_callback_param(&catch.body, param_name, saw_callback_call, escaped);
            }
            if let Some(block) = finally_block {
                scan_block_for_callback_param(block, param_name, saw_callback_call, escaped);
            }
        }
        crate::java_ir::Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                scan_expr_for_callback_param(arg, param_name, saw_callback_call, escaped);
            }
        }
        crate::java_ir::Stmt::Block(block) => {
            scan_block_for_callback_param(block, param_name, saw_callback_call, escaped);
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
            && matches!(
                name.as_str(),
                "accept" | "apply" | "test" | "get" | "run" | "call"
            ) =>
        {
            *saw_callback_call = true;
            for arg in args {
                scan_expr_for_callback_param(arg, param_name, saw_callback_call, escaped);
            }
        }
        Expr::FieldAccess(receiver, _)
        | Expr::ArrayAccess(receiver, _)
        | Expr::Unary { expr: receiver, .. }
        | Expr::Cast { expr: receiver, .. }
        | Expr::InstanceOf { expr: receiver, .. } => {
            scan_expr_for_callback_param(receiver, param_name, saw_callback_call, escaped);
        }
        Expr::Binary { left, right, .. } => {
            scan_expr_for_callback_param(left, param_name, saw_callback_call, escaped);
            scan_expr_for_callback_param(right, param_name, saw_callback_call, escaped);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            scan_expr_for_callback_param(condition, param_name, saw_callback_call, escaped);
            scan_expr_for_callback_param(then_expr, param_name, saw_callback_call, escaped);
            scan_expr_for_callback_param(else_expr, param_name, saw_callback_call, escaped);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                scan_expr_for_callback_param(receiver, param_name, saw_callback_call, escaped);
            }
            for arg in args {
                scan_expr_for_callback_param(arg, param_name, saw_callback_call, escaped);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                scan_expr_for_callback_param(arg, param_name, saw_callback_call, escaped);
            }
            if let Some(members) = body {
                for member in members {
                    if let Member::Method(method) = member
                        && let Some(body) = &method.body
                    {
                        scan_block_for_callback_param(body, param_name, saw_callback_call, escaped);
                    }
                }
            }
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for expr in dimensions {
                scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
            }
            if let Some(initializer) = initializer {
                for expr in initializer {
                    scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
                }
            }
        }
        Expr::ArrayInitializer(values) => {
            for expr in values {
                scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped);
            }
        }
        Expr::Lambda { body, .. } => match body {
            crate::java_ir::LambdaBody::Expr(expr) => {
                scan_expr_for_callback_param(expr, param_name, saw_callback_call, escaped)
            }
            crate::java_ir::LambdaBody::Block(block) => {
                scan_block_for_callback_param(block, param_name, saw_callback_call, escaped)
            }
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
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
        RustType::Ref { mutable, inner } => {
            let inner = add_lifetime_to_refs(*inner);
            RustType::Raw(format!(
                "&'a {}{}",
                if mutable { "mut " } else { "" },
                crate::emit::emit_type(&inner)
            ))
        }
        RustType::Option(inner) => RustType::Option(Box::new(add_lifetime_to_refs(*inner))),
        RustType::Vec(inner) => RustType::Vec(Box::new(add_lifetime_to_refs(*inner))),
        RustType::Array2(inner) => RustType::Array2(Box::new(add_lifetime_to_refs(*inner))),
        RustType::Slice(inner) => RustType::Slice(Box::new(add_lifetime_to_refs(*inner))),
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
) -> Option<RustType> {
    let base = method_return_type_with_facts(ty, method, signatures, facts)
        .map(|ty| maybe_add_lifetime_to_refs(ty, lifetime));
    if exceptions.wrap_returns_in_result {
        Some(exception_result_type(base.unwrap_or(RustType::Unit)))
    } else {
        base
    }
}

fn method_receiver(ty: &TypeDecl, method: &MethodDecl, signatures: &SignaturePlan) -> ReceiverPlan {
    if method.modifiers.contains(&Modifier::Static) {
        ReceiverPlan::None
    } else {
        signatures.required_method_signature(ty, method).receiver
    }
}

fn lower_receiver(receiver: ReceiverPlan) -> Option<RustReceiver> {
    match receiver {
        ReceiverPlan::None => None,
        ReceiverPlan::Shared => Some(RustReceiver::Shared),
        ReceiverPlan::Mutable => Some(RustReceiver::Mutable),
    }
}
