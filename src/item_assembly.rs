use std::collections::HashSet;

use crate::exception_lowering::exception_result_type;
use crate::exception_plan::MethodExceptionPlan;
use crate::facts::FactSet;
use crate::java_ir::{
    ConstructorDecl, Expr, FieldDecl, JavaType, Literal, Member, MethodDecl, Modifier,
    PrimitiveType, Program, TypeDecl,
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
use crate::slot_shape::RustSlotShape;

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
    vec![RustStmt::Return(Some(RustExpr::Call {
        function: Box::new(RustExpr::Raw(format!("Self::{rust_name}"))),
        args,
    }))]
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
    method_return_shape(ty, method, signatures)
        .map(|shape| rust_type_for_slot_with_facts(&shape, facts))
}

fn struct_fields_for_type(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    support_program: &Program,
) -> Vec<RustField> {
    let mut fields: Vec<_> = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field)
                if !is_static_field(field) || is_static_final_non_const_field(field) =>
            {
                Some(RustField {
                    name: field.name.clone(),
                    ty: rust_type_for_slot_with_facts(
                        &signatures.required_field_shape(&ty.name, &field.name),
                        facts,
                    ),
                    is_pub: false,
                })
            }
            _ => None,
        })
        .collect();
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
            }),
    );
    let mut seen = HashSet::new();
    fields.retain(|field| seen.insert(field.name.clone()));
    fields
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
        RustType::Slice(_) | RustType::Ref { .. } | RustType::Raw(_) => false,
    }
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
        .is_none_or(|initializer| const_type_and_value(&field.ty, initializer).is_none())
}

fn associated_consts_for_type(ty: &TypeDecl) -> Vec<RustAssociatedConst> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) if is_static_final_field(field) => {
                associated_const_for_field(field)
            }
            _ => None,
        })
        .collect()
}

fn associated_const_for_field(field: &FieldDecl) -> Option<RustAssociatedConst> {
    let initializer = field.initializer.as_ref()?;
    let (ty, value) = const_type_and_value(&field.ty, initializer)?;
    Some(RustAssociatedConst {
        name: field.name.clone(),
        ty,
        value,
        is_pub: !field.modifiers.contains(&Modifier::Private),
    })
}

fn const_type_and_value(ty: &JavaType, initializer: &Expr) -> Option<(RustType, RustExpr)> {
    let Expr::Literal(literal) = initializer else {
        return None;
    };
    match (ty, literal) {
        (JavaType::String, Literal::String(value)) => Some((
            RustType::Raw("&'static str".to_string()),
            RustExpr::Literal(format!("{value:?}")),
        )),
        (JavaType::Primitive(PrimitiveType::Boolean), Literal::Bool(value)) => {
            Some((RustType::Bool, RustExpr::Literal(value.to_string())))
        }
        (JavaType::Primitive(PrimitiveType::Byte), Literal::Int(value))
        | (JavaType::Primitive(PrimitiveType::Short), Literal::Int(value))
        | (JavaType::Primitive(PrimitiveType::Int), Literal::Int(value)) => {
            Some((RustType::I32, RustExpr::Literal(value.to_string())))
        }
        (JavaType::Primitive(PrimitiveType::Long), Literal::Long(value)) => {
            Some((RustType::I64, RustExpr::Literal(format!("{value}i64"))))
        }
        (JavaType::Primitive(PrimitiveType::Float), Literal::Float(value)) => {
            Some((RustType::F32, RustExpr::Literal(format!("{value}f32"))))
        }
        (JavaType::Primitive(PrimitiveType::Double), Literal::Double(value)) => {
            Some((RustType::F64, RustExpr::Literal(format!("{value}f64"))))
        }
        (JavaType::Primitive(PrimitiveType::Char), Literal::Char(value)) => {
            Some((RustType::Char, RustExpr::Literal(format!("{value:?}"))))
        }
        _ => None,
    }
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
    signatures
        .required_method_signature(ty, method)
        .params
        .iter()
        .map(|param| {
            let callback_param =
                method_param_is_nonescaping_functional_callback(method, &param.name);
            RustParam {
                name: if callback_param {
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
