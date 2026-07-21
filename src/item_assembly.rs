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
    rust_type_for_slot_with_facts,
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
    RustItem::Struct(RustStruct {
        name: rust_name_for_java_named_type(&ty.name),
        fields: struct_fields_for_type(ty, signatures, facts, support_program),
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
    RustFunction {
        name: method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
        params: method_params(ty, method, signatures, facts),
        return_type: method_return_type_with_exceptions(ty, method, signatures, facts, exceptions),
        body,
        is_pub: true,
        receiver: lower_receiver(method_receiver(ty, method, signatures)),
    }
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
            Member::Field(field) if !is_static_field(field) => Some(RustField {
                name: field.name.clone(),
                ty: rust_type_for_slot_with_facts(
                    &signatures.required_field_shape(&ty.name, &field.name),
                    facts,
                ),
                is_pub: false,
            }),
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
    fields
}

pub fn is_static_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static)
}

pub fn is_static_final_field(field: &FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static) && field.modifiers.contains(&Modifier::Final)
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
    signatures
        .required_method_signature(ty, method)
        .params
        .iter()
        .map(|param| RustParam {
            name: param.name.clone(),
            ty: rust_type_for_slot_with_facts(&param.shape, facts),
        })
        .collect()
}

fn method_return_type_with_exceptions(
    ty: &TypeDecl,
    method: &MethodDecl,
    signatures: &SignaturePlan,
    facts: &FactSet,
    exceptions: MethodExceptionPlan,
) -> Option<RustType> {
    if exceptions.wrap_returns_in_result {
        Some(exception_result_type(
            method_return_type_with_facts(ty, method, signatures, facts).unwrap_or(RustType::Unit),
        ))
    } else {
        method_return_type_with_facts(ty, method, signatures, facts)
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
