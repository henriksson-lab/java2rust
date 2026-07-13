use crate::exception_lowering::exception_result_type;
use crate::facts::FactSet;
use crate::java_ir::{ConstructorDecl, Member, MethodDecl, Modifier, TypeDecl};
use crate::rust_ir::{
    RustExpr, RustField, RustFunction, RustImpl, RustItem, RustParam, RustReceiver, RustStmt,
    RustStruct, RustType,
};
use crate::signature::{ReceiverPlan, SignaturePlan};
use crate::slot_render::{
    default_expr_for_slot, default_expr_for_slot_with_facts, rust_type_for_slot_with_facts,
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
    RustItem::Struct(RustStruct {
        name: ty.name.clone(),
        fields: struct_fields_for_type(ty, signatures, facts),
    })
}

pub fn impl_item_for_type(ty: &TypeDecl, functions: Vec<RustFunction>) -> RustItem {
    RustItem::Impl(RustImpl {
        target: ty.name.clone(),
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
        name: ty.name.clone(),
        fields,
    }))]
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
    RustFunction {
        name: method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
        params: method_params(ty, method, signatures, facts),
        return_type: method_return_type_with_exceptions(ty, method, signatures, facts),
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
) -> Vec<RustField> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(RustField {
                name: field.name.clone(),
                ty: rust_type_for_slot_with_facts(
                    &signatures.required_field_shape(&ty.name, &field.name),
                    facts,
                ),
                is_pub: false,
            }),
            _ => None,
        })
        .collect()
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
    RustType::Named(ty.name.clone())
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
) -> Option<RustType> {
    if facts.method_has_thrown_exceptions(&ty.name, &method.name) {
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
