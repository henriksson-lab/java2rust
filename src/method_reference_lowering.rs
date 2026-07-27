use crate::coercion::{
    current_receiver_method_call_projection, current_receiver_method_reference_projection,
    owner_associated_path_projection, receiver_method_call_projection,
    typed_single_input_callback_projection, unbound_method_call_projection,
    unbound_method_reference_projection,
};
use crate::emit;
use crate::facts::{CallableKey, FacadeSlot, FactSet, ReceiverAbi, ResultAbi, Staticness};
use crate::method_reference_plan::MethodReferencePlan;
use crate::rust_ir::RustExpr;
use crate::slot_shape::RustSlotShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssociatedFunctionMethodReferenceBodyPlan {
    PrimitiveIdentityValue,
    ReceiverToString,
    AlwaysTrue,
    StaticFunction,
}

fn associated_function_method_reference_body_plan(
    owner: &str,
    method: &str,
) -> AssociatedFunctionMethodReferenceBodyPlan {
    if owner == "Integer" && method == "int_value" {
        return AssociatedFunctionMethodReferenceBodyPlan::PrimitiveIdentityValue;
    }
    if (owner == "Object" && method == "to_string") || (owner == "String" && method == "value_of") {
        return AssociatedFunctionMethodReferenceBodyPlan::ReceiverToString;
    }
    if owner == "Objects" && method == "non_null" {
        return AssociatedFunctionMethodReferenceBodyPlan::AlwaysTrue;
    }
    AssociatedFunctionMethodReferenceBodyPlan::StaticFunction
}

pub fn render_method_reference(plan: &MethodReferencePlan) -> RustExpr {
    match plan {
        MethodReferencePlan::Constructor { type_name } => {
            owner_associated_path_projection(type_name, "new")
        }
        MethodReferencePlan::AssociatedFunction { owner, method } => {
            if owner == "Objects" && method == "non_null" {
                return RustExpr::Raw("|_value| true".to_string());
            }
            owner_associated_path_projection(owner, method)
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            unbound_method_reference_projection(method)
        }
        MethodReferencePlan::CurrentReceiverMethod { method, arity } => {
            current_receiver_method_reference_projection(method, arity.unwrap_or(1))
        }
    }
}

pub fn render_typed_single_input_method_reference_callback(
    plan: &MethodReferencePlan,
    param_type: &str,
) -> RustExpr {
    match plan {
        MethodReferencePlan::AssociatedFunction { .. } => {
            let body = render_single_input_method_reference_body(plan, "value", "value");
            typed_single_input_callback_projection(param_type, &body)
        }
        MethodReferencePlan::UnboundAccessor { .. }
        | MethodReferencePlan::CurrentReceiverMethod { .. }
        | MethodReferencePlan::Constructor { .. } => {
            let body = render_single_input_method_reference_body(plan, "value", "value");
            typed_single_input_callback_projection(param_type, &body)
        }
    }
}

pub fn render_typed_single_input_method_reference_callback_with_facts(
    plan: &MethodReferencePlan,
    param_type: &str,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    let body = render_single_input_method_reference_body_with_facts(
        plan,
        "value",
        "value",
        input_shape,
        facts,
    );
    typed_single_input_callback_projection(param_type, &body)
}

pub fn render_single_input_method_reference_body(
    plan: &MethodReferencePlan,
    receiver: &str,
    value_expr: &str,
) -> String {
    match plan {
        MethodReferencePlan::Constructor { type_name } => emit::emit_expr(&RustExpr::Call {
            function: Box::new(owner_associated_path_projection(type_name, "new")),
            args: vec![RustExpr::Raw(receiver.to_string())],
        }),
        MethodReferencePlan::AssociatedFunction { owner, method } => {
            render_associated_function_method_reference_body(owner, method, receiver, value_expr)
        }
        MethodReferencePlan::UnboundAccessor { method } => emit::emit_expr(
            &unbound_method_call_projection(RustExpr::Raw(receiver.to_string()), method),
        ),
        MethodReferencePlan::CurrentReceiverMethod { method, .. } => {
            emit::emit_expr(&current_receiver_method_call_projection(
                method,
                vec![RustExpr::Raw(receiver.to_string())],
            ))
        }
    }
}

pub fn render_single_input_method_reference_body_with_facts(
    plan: &MethodReferencePlan,
    receiver: &str,
    value_expr: &str,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> String {
    if let MethodReferencePlan::AssociatedFunction { owner, method } = plan
        && let Some(body) = render_facade_abi_associated_function_method_reference_body(
            owner,
            method,
            receiver,
            input_shape,
            facts,
        )
    {
        return body;
    }
    render_single_input_method_reference_body(plan, receiver, value_expr)
}

fn render_associated_function_method_reference_body(
    owner: &str,
    method: &str,
    receiver: &str,
    value_expr: &str,
) -> String {
    match associated_function_method_reference_body_plan(owner, method) {
        AssociatedFunctionMethodReferenceBodyPlan::PrimitiveIdentityValue => value_expr.to_string(),
        AssociatedFunctionMethodReferenceBodyPlan::ReceiverToString => {
            emit::emit_expr(&receiver_method_call_projection(
                RustExpr::Raw(receiver.to_string()),
                "to_string",
                Vec::new(),
            ))
        }
        AssociatedFunctionMethodReferenceBodyPlan::AlwaysTrue => "true".to_string(),
        AssociatedFunctionMethodReferenceBodyPlan::StaticFunction => {
            emit::emit_expr(&RustExpr::Call {
                function: Box::new(owner_associated_path_projection(owner, method)),
                args: vec![RustExpr::Raw(receiver.to_string())],
            })
        }
    }
}

fn render_facade_abi_associated_function_method_reference_body(
    owner: &str,
    method: &str,
    receiver: &str,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<String> {
    let abi = method_reference_facade_abi(owner, method, input_shape, facts)?;
    match abi.rust_path.as_str() {
        "Option::is_some" => Some(emit::emit_expr(&receiver_method_call_projection(
            RustExpr::Raw(receiver.to_string()),
            "is_some",
            Vec::new(),
        ))),
        "Option::is_none" => Some(emit::emit_expr(&receiver_method_call_projection(
            RustExpr::Raw(receiver.to_string()),
            "is_none",
            Vec::new(),
        ))),
        _ => None,
    }
}

fn method_reference_facade_abi<'a>(
    owner: &str,
    method: &str,
    input_shape: &RustSlotShape,
    facts: &'a FactSet,
) -> Option<&'a crate::facts::FacadeAbiFact> {
    if input_shape.optional_element_type().is_none() {
        return None;
    }
    method_reference_callable_key_candidates(owner, method)
        .into_iter()
        .find_map(|key| facts.facade_abi_for_key(&key))
        .filter(|abi| {
            abi.receiver == ReceiverAbi::AssociatedFunction
                && abi.args == vec![FacadeSlot::Optional(Box::new(FacadeSlot::Borrowed))]
                && abi.return_slot == FacadeSlot::Owned
                && abi.result == ResultAbi::Plain
        })
}

fn method_reference_callable_key_candidates(owner: &str, method: &str) -> Vec<CallableKey> {
    let mut methods = vec![method.to_string()];
    match method {
        "non_null" => methods.push("nonNull".to_string()),
        "is_null" => methods.push("isNull".to_string()),
        "to_string" => methods.push("toString".to_string()),
        "value_of" => methods.push("valueOf".to_string()),
        "int_value" => methods.push("intValue".to_string()),
        _ => {}
    }
    methods
        .into_iter()
        .map(|method| CallableKey::method(owner.to_string(), method, Staticness::Unknown, 1))
        .collect()
}
