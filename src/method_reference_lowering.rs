use crate::coercion::{
    AssociatedFunctionMethodReferenceProjection,
    associated_function_method_reference_body_projection, current_receiver_method_call_projection,
    current_receiver_method_reference_projection, facade_receiver_method_call_projection,
    owner_associated_path_projection, receiver_method_call_projection,
    typed_single_input_callback_projection, unbound_method_call_projection,
    unbound_method_reference_projection,
};
use crate::emit;
use crate::facts::{CallableKey, FacadeSlot, FactSet, ReceiverAbi, ResultAbi, Staticness};
use crate::java_ir::JavaType;
use crate::method_reference_plan::MethodReferencePlan;
use crate::method_reference_shape::method_reference_receiver_type_for_call_plan_with_facts;
use crate::rust_ir::RustExpr;
use crate::slot_shape::RustSlotShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssociatedFunctionMethodReferenceBodyPlan {
    PrimitiveIdentityValue,
    ReceiverToString,
    AlwaysTrue,
    StaticFunction,
}

impl AssociatedFunctionMethodReferenceBodyPlan {
    fn projection(self) -> AssociatedFunctionMethodReferenceProjection {
        match self {
            AssociatedFunctionMethodReferenceBodyPlan::PrimitiveIdentityValue => {
                AssociatedFunctionMethodReferenceProjection::PrimitiveIdentityValue
            }
            AssociatedFunctionMethodReferenceBodyPlan::ReceiverToString => {
                AssociatedFunctionMethodReferenceProjection::ReceiverToString
            }
            AssociatedFunctionMethodReferenceBodyPlan::AlwaysTrue => {
                AssociatedFunctionMethodReferenceProjection::AlwaysTrue
            }
            AssociatedFunctionMethodReferenceBodyPlan::StaticFunction => {
                AssociatedFunctionMethodReferenceProjection::StaticFunction
            }
        }
    }
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
        && let Some(body) =
            render_facade_abi_method_reference_body(owner, method, receiver, input_shape, facts)
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
    associated_function_method_reference_body_projection(
        associated_function_method_reference_body_plan(owner, method).projection(),
        owner,
        method,
        receiver,
        value_expr,
    )
}

fn render_facade_abi_method_reference_body(
    owner: &str,
    method: &str,
    receiver: &str,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<String> {
    let abi = method_reference_facade_abi(owner, method, input_shape, facts)?;
    match abi.receiver {
        ReceiverAbi::AssociatedFunction => match abi.rust_path.as_str() {
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
        },
        ReceiverAbi::BorrowedSelf | ReceiverAbi::MutBorrowedSelf | ReceiverAbi::OwnedSelf
            if abi.args.is_empty()
                && abi.return_slot == FacadeSlot::Owned
                && abi.result == ResultAbi::Plain =>
        {
            Some(emit::emit_expr(&facade_receiver_method_call_projection(
                RustExpr::Raw(receiver.to_string()),
                &abi.rust_path,
                Vec::new(),
            )))
        }
        ReceiverAbi::FreeFunction => None,
        _ => None,
    }
}

fn method_reference_facade_abi<'a>(
    owner: &str,
    method: &str,
    input_shape: &RustSlotShape,
    facts: &'a FactSet,
) -> Option<&'a crate::facts::FacadeAbiFact> {
    method_reference_callable_key_candidates(owner, method, input_shape, facts)
        .into_iter()
        .find_map(|key| facts.facade_abi_for_key(&key))
        .filter(|abi| method_reference_facade_abi_is_supported(abi, input_shape))
}

fn method_reference_facade_abi_is_supported(
    abi: &crate::facts::FacadeAbiFact,
    input_shape: &RustSlotShape,
) -> bool {
    match abi.receiver {
        ReceiverAbi::AssociatedFunction => {
            input_shape.optional_element_type().is_some()
                && abi.args == vec![FacadeSlot::Optional(Box::new(FacadeSlot::Borrowed))]
                && abi.return_slot == FacadeSlot::Owned
                && abi.result == ResultAbi::Plain
        }
        ReceiverAbi::BorrowedSelf | ReceiverAbi::MutBorrowedSelf | ReceiverAbi::OwnedSelf => {
            abi.args.is_empty()
                && abi.return_slot == FacadeSlot::Owned
                && abi.result == ResultAbi::Plain
        }
        ReceiverAbi::FreeFunction => false,
    }
}

fn method_reference_callable_key_candidates(
    owner: &str,
    method: &str,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> Vec<CallableKey> {
    let mut keys = Vec::new();
    for method in crate::java_naming::java_method_name_aliases(method) {
        keys.push(CallableKey::method(
            owner.to_string(),
            method.clone(),
            Staticness::Unknown,
            1,
        ));
        if let Some(receiver_owner) =
            method_reference_receiver_owner_key(owner, &method, input_shape, facts)
        {
            keys.push(CallableKey::method(
                receiver_owner,
                method,
                Staticness::Unknown,
                0,
            ));
        }
    }
    keys
}

fn method_reference_receiver_owner_key(
    owner: &str,
    method: &str,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<String> {
    java_type_callable_owner_key(&method_reference_receiver_type_for_call_plan_with_facts(
        owner,
        method,
        Some(input_shape),
        facts,
    )?)
}

fn java_type_callable_owner_key(ty: &JavaType) -> Option<String> {
    match ty {
        JavaType::String => Some("String".to_string()),
        JavaType::Named { name, .. } | JavaType::Unknown(name) => {
            Some(crate::stdlib::canonical_external_receiver_type_key(name))
        }
        JavaType::Primitive(_) | JavaType::Void | JavaType::Array(_) | JavaType::Wildcard(_) => {
            None
        }
    }
}
