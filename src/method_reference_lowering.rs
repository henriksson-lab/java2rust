use crate::coercion::{
    current_receiver_method_call_projection, current_receiver_method_reference_projection,
    facade_receiver_method_call_projection, function_call_projection,
    owner_associated_path_projection, receiver_method_call_projection,
    typed_single_input_callback_expr, unbound_method_call_projection,
    unbound_method_reference_projection,
};
use crate::facts::{CallableKey, FacadeSlot, FactSet, ReceiverAbi, ResultAbi, Staticness};
use crate::method_reference_plan::{BoundMethodReferenceReceiver, MethodReferencePlan};
use crate::method_reference_shape::method_reference_receiver_type_for_call_plan_with_facts;
use crate::rust_ir::{RustClosureBody, RustClosureParam, RustExpr};
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

pub fn method_reference_expr(plan: &MethodReferencePlan) -> RustExpr {
    match plan {
        MethodReferencePlan::Constructor { type_name } => {
            owner_associated_path_projection(type_name, "new")
        }
        MethodReferencePlan::AssociatedFunction { owner, method } => {
            if owner == "Objects" && method == "non_null" {
                return always_true_method_reference_closure_leaf();
            }
            owner_associated_path_projection(owner, method)
        }
        MethodReferencePlan::BoundReceiver { receiver, method } => {
            bound_receiver_method_reference_closure_leaf(receiver, method)
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            unbound_method_reference_projection(method)
        }
        MethodReferencePlan::CurrentReceiverMethod { method, arity } => {
            current_receiver_method_reference_projection(method, arity.unwrap_or(1))
        }
    }
}

fn always_true_method_reference_closure_leaf() -> RustExpr {
    method_reference_closure_expr("_value", RustExpr::Literal("true".to_string()))
}

fn bound_receiver_method_reference_closure_leaf(
    receiver: &BoundMethodReferenceReceiver,
    method: &str,
) -> RustExpr {
    method_reference_closure_expr(
        "value",
        receiver_method_call_projection(
            bound_method_reference_receiver_expr(receiver),
            method,
            vec![RustExpr::Var("value".to_string())],
        ),
    )
}

fn bound_method_reference_receiver_expr(receiver: &BoundMethodReferenceReceiver) -> RustExpr {
    match receiver {
        BoundMethodReferenceReceiver::Local(name) => RustExpr::Var(name.clone()),
        BoundMethodReferenceReceiver::InstanceField(name) => {
            crate::coercion::self_field_projection(name)
        }
    }
}

fn method_reference_closure_expr(param_name: &str, body: RustExpr) -> RustExpr {
    RustExpr::Closure {
        is_move: false,
        return_type: None,
        params: vec![RustClosureParam {
            pattern: param_name.to_string(),
            ty: None,
        }],
        body: RustClosureBody::Expr(Box::new(body)),
    }
}

pub fn typed_single_input_method_reference_callback_expr(
    plan: &MethodReferencePlan,
    param_type: &str,
) -> RustExpr {
    typed_single_input_callback_expr(
        param_type,
        single_input_method_reference_body_expr(
            plan,
            RustExpr::Var("value".to_string()),
            RustExpr::Var("value".to_string()),
        ),
    )
}

pub fn typed_single_input_method_reference_callback_expr_with_facts(
    plan: &MethodReferencePlan,
    param_type: &str,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    let body = single_input_method_reference_body_expr_with_facts(
        plan,
        RustExpr::Var("value".to_string()),
        RustExpr::Var("value".to_string()),
        input_shape,
        facts,
    );
    typed_single_input_callback_expr(param_type, body)
}

pub fn single_input_method_reference_body_expr(
    plan: &MethodReferencePlan,
    receiver: RustExpr,
    value_expr: RustExpr,
) -> RustExpr {
    match plan {
        MethodReferencePlan::Constructor { type_name } => function_call_projection(
            owner_associated_path_projection(type_name, "new"),
            vec![receiver],
        ),
        MethodReferencePlan::AssociatedFunction { owner, method } => {
            associated_function_method_reference_body_expr_from_operands(
                owner, method, receiver, value_expr,
            )
        }
        MethodReferencePlan::BoundReceiver {
            receiver: bound_receiver,
            method,
        } => receiver_method_call_projection(
            bound_method_reference_receiver_expr(bound_receiver),
            method,
            vec![value_expr],
        ),
        MethodReferencePlan::UnboundAccessor { method } => {
            unbound_method_call_projection(receiver, method)
        }
        MethodReferencePlan::CurrentReceiverMethod { method, .. } => {
            current_receiver_method_call_projection(method, vec![receiver])
        }
    }
}

pub fn single_input_method_reference_body_expr_with_facts(
    plan: &MethodReferencePlan,
    receiver: RustExpr,
    value_expr: RustExpr,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> RustExpr {
    if let MethodReferencePlan::AssociatedFunction { owner, method } = plan
        && let Some(body) = facade_abi_method_reference_body_expr(
            owner,
            method,
            receiver.clone(),
            input_shape,
            facts,
        )
    {
        return body;
    }
    single_input_method_reference_body_expr(plan, receiver, value_expr)
}

fn associated_function_method_reference_body_expr_from_operands(
    owner: &str,
    method: &str,
    receiver: RustExpr,
    value_expr: RustExpr,
) -> RustExpr {
    match associated_function_method_reference_body_plan(owner, method) {
        AssociatedFunctionMethodReferenceBodyPlan::PrimitiveIdentityValue => value_expr,
        AssociatedFunctionMethodReferenceBodyPlan::ReceiverToString => {
            receiver_method_call_projection(receiver, "to_string", Vec::new())
        }
        AssociatedFunctionMethodReferenceBodyPlan::AlwaysTrue => {
            RustExpr::Literal("true".to_string())
        }
        AssociatedFunctionMethodReferenceBodyPlan::StaticFunction => function_call_projection(
            owner_associated_path_projection(owner, method),
            vec![receiver],
        ),
    }
}

fn facade_abi_method_reference_body_expr(
    owner: &str,
    method: &str,
    receiver: RustExpr,
    input_shape: &RustSlotShape,
    facts: &FactSet,
) -> Option<RustExpr> {
    let abi = method_reference_facade_abi(owner, method, input_shape, facts)?;
    match abi.receiver {
        ReceiverAbi::AssociatedFunction => match abi.rust_path.as_str() {
            "Option::is_some" => Some(receiver_method_call_projection(
                receiver,
                "is_some",
                Vec::new(),
            )),
            "Option::is_none" => Some(receiver_method_call_projection(
                receiver,
                "is_none",
                Vec::new(),
            )),
            _ => None,
        },
        ReceiverAbi::BorrowedSelf | ReceiverAbi::MutBorrowedSelf | ReceiverAbi::OwnedSelf
            if abi.args.is_empty()
                && abi.return_slot == FacadeSlot::Owned
                && abi.result == ResultAbi::Plain =>
        {
            Some(facade_receiver_method_call_projection(
                receiver,
                &abi.rust_path,
                Vec::new(),
            ))
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
    let receiver_ty = method_reference_receiver_type_for_call_plan_with_facts(
        owner,
        method,
        Some(input_shape),
        facts,
    )?;
    crate::stdlib::external_api_method_receiver_owner_key(
        &receiver_ty,
        &crate::types::TypeEnv::new(),
        method,
    )
}
