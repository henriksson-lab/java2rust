use crate::coercion::{
    current_receiver_method_call_projection, current_receiver_method_reference_projection,
    owner_associated_path_projection, typed_single_input_callback_projection,
    unbound_method_call_projection, unbound_method_reference_projection,
};
use crate::emit;
use crate::method_reference_plan::MethodReferencePlan;
use crate::rust_ir::RustExpr;

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
            if owner == "Integer" && method == "int_value" {
                return value_expr.to_string();
            }
            if (owner == "Object" && method == "to_string")
                || (owner == "String" && method == "value_of")
            {
                return format!("{receiver}.to_string()");
            }
            if owner == "Objects" && method == "non_null" {
                return "true".to_string();
            }
            emit::emit_expr(&RustExpr::Call {
                function: Box::new(owner_associated_path_projection(owner, method)),
                args: vec![RustExpr::Raw(receiver.to_string())],
            })
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
