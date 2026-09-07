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
    let owner = method_reference_owner_simple_name(owner);
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

pub(crate) fn method_reference_expr(plan: &MethodReferencePlan) -> RustExpr {
    match plan {
        MethodReferencePlan::Constructor { type_name } => {
            owner_associated_path_projection(&method_reference_owner_rust_path(type_name), "new")
        }
        MethodReferencePlan::AssociatedFunction { owner, method } => {
            if method_reference_owner_simple_name(owner) == "Objects" && method == "non_null" {
                return always_true_method_reference_closure_leaf();
            }
            owner_associated_path_projection(&method_reference_owner_rust_path(owner), method)
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

fn method_reference_owner_rust_path(owner: &str) -> String {
    crate::java_naming::rust_name_for_java_named_type(&owner.replace('$', "."))
}

fn method_reference_owner_simple_name(owner: &str) -> String {
    let canonical = crate::stdlib::canonical_external_receiver_type_key(owner);
    canonical
        .rsplit(['.', '$'])
        .next()
        .unwrap_or(owner)
        .to_string()
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

pub(crate) fn typed_single_input_method_reference_callback_expr(
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

pub(crate) fn typed_single_input_method_reference_callback_expr_with_facts(
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

pub(crate) fn single_input_method_reference_body_expr(
    plan: &MethodReferencePlan,
    receiver: RustExpr,
    value_expr: RustExpr,
) -> RustExpr {
    match plan {
        MethodReferencePlan::Constructor { type_name } => {
            let rust_owner = method_reference_owner_rust_path(type_name);
            function_call_projection(
                owner_associated_path_projection(&rust_owner, "new"),
                vec![receiver],
            )
        }
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

pub(crate) fn single_input_method_reference_body_expr_with_facts(
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
            owner_associated_path_projection(&method_reference_owner_rust_path(owner), method),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::emit_expr;
    use crate::facts::{
        ExternalApiEffectFact, ExternalApiRole, FacadeAbiFact, FactProvenance, NullabilityState,
        ReceiverMutabilityFact,
    };
    use crate::java_ir::JavaType;
    use crate::method_reference_plan::plan_method_reference;
    use crate::types::optional_java_type;

    #[test]
    fn typed_single_input_callback_helper_stays_structured() {
        assert_eq!(
            typed_single_input_method_reference_callback_expr(
                &plan_method_reference("NameHolder", "getName"),
                "&String",
            ),
            RustExpr::Closure {
                is_move: false,
                return_type: None,
                params: vec![crate::rust_ir::RustClosureParam {
                    pattern: "value".to_string(),
                    ty: Some("&String".to_string()),
                }],
                body: RustClosureBody::Expr(Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("value".to_string())),
                    method: "get_name".to_string(),
                    args: Vec::new(),
                })),
            }
        );
    }

    #[test]
    fn method_reference_renderer_and_body_helpers_stay_structured() {
        assert_eq!(
            method_reference_expr(&plan_method_reference("IllegalArgumentException", "new")),
            RustExpr::Path(vec![
                "IllegalArgumentException".to_string(),
                "new".to_string(),
            ])
        );
        assert_eq!(
            method_reference_expr(&plan_method_reference("Score", "add")),
            RustExpr::Path(vec!["Score".to_string(), "add".to_string()])
        );
        assert_eq!(
            method_reference_expr(&plan_method_reference("a.Mapper", "tag")),
            RustExpr::Path(vec!["AMapper".to_string(), "tag".to_string()])
        );

        let grouped_value = RustExpr::Grouped(Box::new(RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(RustExpr::Var("value".to_string())),
        }));
        let deref_value = RustExpr::Unary {
            op: "*".to_string(),
            expr: Box::new(RustExpr::Var("value".to_string())),
        };

        assert_eq!(
            emit_expr(&single_input_method_reference_body_expr(
                &plan_method_reference("Integer", "intValue"),
                grouped_value.clone(),
                deref_value.clone(),
            )),
            "*value"
        );
        assert_eq!(
            emit_expr(&single_input_method_reference_body_expr(
                &plan_method_reference("Object", "toString"),
                grouped_value.clone(),
                deref_value.clone(),
            )),
            "(*value).to_string()"
        );
        assert_eq!(
            emit_expr(&single_input_method_reference_body_expr(
                &plan_method_reference("NameHolder", "getName"),
                grouped_value,
                deref_value,
            )),
            "(*value).get_name()"
        );
    }

    #[test]
    fn fact_aware_method_reference_callback_uses_facade_abi() {
        let stdlib_facts = crate::stdlib::rust_stdlib_signature_facts();
        assert_eq!(
            typed_single_input_method_reference_callback_expr_with_facts(
                &plan_method_reference("Objects", "nonNull"),
                "Option<&String>",
                &RustSlotShape::conservative(optional_java_type(JavaType::String)),
                &stdlib_facts,
            ),
            typed_single_input_callback_expr(
                "Option<&String>",
                receiver_method_call_projection(
                    RustExpr::Var("value".to_string()),
                    "is_some",
                    Vec::new(),
                ),
            )
        );

        let mut facts = FactSet::default();
        facts.facade_abi_facts.push(FacadeAbiFact {
            key: CallableKey::method(
                "String".to_string(),
                "toString".to_string(),
                Staticness::Unknown,
                0,
            ),
            rust_path: "StringFacade::display_name".to_string(),
            receiver: ReceiverAbi::BorrowedSelf,
            args: Vec::new(),
            return_slot: FacadeSlot::Owned,
            result: ResultAbi::Plain,
            borrow_source: None,
            borrowed_return_recovery: None,
            provenance: FactProvenance::UserOverride,
        });
        assert_eq!(
            typed_single_input_method_reference_callback_expr_with_facts(
                &plan_method_reference("Object", "toString"),
                "&String",
                &RustSlotShape::for_storage(JavaType::String),
                &facts,
            ),
            typed_single_input_callback_expr(
                "&String",
                receiver_method_call_projection(
                    RustExpr::Var("value".to_string()),
                    "display_name",
                    Vec::new(),
                ),
            )
        );

        let mut external_facts = FactSet::default();
        external_facts
            .external_api_effect_facts
            .push(ExternalApiEffectFact {
                method_name: "label".to_string(),
                receiver_type: Some("FacadeValue".to_string()),
                arg_count: Some(0),
                parameter_patterns: None,
                role: ExternalApiRole::Getter,
                receiver_mutability: ReceiverMutabilityFact::ReadOnly,
                return_type: Some(JavaType::String),
                return_nullability: NullabilityState::DefinitelyNonNull,
                thrown: Vec::new(),
                provenance: FactProvenance::UserOverride,
            });
        external_facts.facade_abi_facts.push(FacadeAbiFact {
            key: CallableKey::method(
                "FacadeValue".to_string(),
                "label".to_string(),
                Staticness::Unknown,
                0,
            ),
            rust_path: "FacadeValueFacade::label_text".to_string(),
            receiver: ReceiverAbi::BorrowedSelf,
            args: Vec::new(),
            return_slot: FacadeSlot::Owned,
            result: ResultAbi::Plain,
            borrow_source: None,
            borrowed_return_recovery: None,
            provenance: FactProvenance::UserOverride,
        });
        let facade_value_shape = RustSlotShape::for_storage(JavaType::Named {
            name: "FacadeValue".to_string(),
            args: Vec::new(),
        });
        assert_eq!(
            typed_single_input_method_reference_callback_expr_with_facts(
                &plan_method_reference("FacadeValue", "label"),
                "&FacadeValue",
                &facade_value_shape,
                &external_facts,
            ),
            typed_single_input_callback_expr(
                "&FacadeValue",
                receiver_method_call_projection(
                    RustExpr::Var("value".to_string()),
                    "label_text",
                    Vec::new(),
                ),
            )
        );
    }
}
