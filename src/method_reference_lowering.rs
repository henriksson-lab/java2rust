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
        MethodReferencePlan::CurrentReceiverMethod { method } => {
            RustExpr::Raw(format!("|value| self.{method}(value)"))
        }
    }
}
