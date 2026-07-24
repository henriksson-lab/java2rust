use crate::method_reference_plan::MethodReferencePlan;
use crate::rust_ir::RustExpr;

pub fn render_method_reference(plan: &MethodReferencePlan) -> RustExpr {
    match plan {
        MethodReferencePlan::Constructor { type_name } => {
            RustExpr::Raw(format!("{type_name}::new"))
        }
        MethodReferencePlan::AssociatedFunction { owner, method } => {
            if owner == "Objects" && method == "non_null" {
                return RustExpr::Raw("|_value| true".to_string());
            }
            RustExpr::Raw(format!("{owner}::{method}"))
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            RustExpr::Raw(format!("|value| value.{method}()"))
        }
        MethodReferencePlan::CurrentReceiverMethod { method, arity } => {
            let arg_count = arity.unwrap_or(1);
            let args = (0..arg_count)
                .map(|index| {
                    if arg_count == 1 {
                        "value".to_string()
                    } else {
                        format!("arg{index}")
                    }
                })
                .collect::<Vec<_>>();
            RustExpr::Raw(format!(
                "|{}| self.{method}({})",
                args.join(", "),
                args.join(", ")
            ))
        }
    }
}

pub fn render_single_input_method_reference_body(
    plan: &MethodReferencePlan,
    receiver: &str,
    value_expr: &str,
) -> String {
    match plan {
        MethodReferencePlan::Constructor { type_name } => {
            format!("{type_name}::new({receiver})")
        }
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
            format!("{owner}::{method}({receiver})")
        }
        MethodReferencePlan::UnboundAccessor { method } => {
            format!("{receiver}.{method}()")
        }
        MethodReferencePlan::CurrentReceiverMethod { method, .. } => {
            format!("self.{method}({receiver})")
        }
    }
}
