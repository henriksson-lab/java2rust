use crate::method_reference_plan::MethodReferencePlan;
use crate::rust_ir::RustExpr;

pub fn render_method_reference(plan: &MethodReferencePlan) -> RustExpr {
    match plan {
        MethodReferencePlan::Constructor { type_name } => {
            RustExpr::Raw(format!("{type_name}::new"))
        }
        MethodReferencePlan::AssociatedFunction { owner, method } => {
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
