use crate::call_plan::{CallTargetPlan, NewTargetPlan};
use crate::facts::FactSet;
use crate::rust_ir::RustExpr;
use crate::slot_render::rust_type_for_slot_with_facts;

pub fn apply_call_target(
    target: &CallTargetPlan<'_>,
    lowered_receiver: Option<RustExpr>,
    args: Vec<RustExpr>,
) -> RustExpr {
    match target {
        CallTargetPlan::StaticSelf { rust_name } => RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!("Self::{rust_name}"))),
            args,
        },
        CallTargetPlan::InstanceReceiver { rust_name, .. } => RustExpr::MethodCall {
            receiver: Box::new(
                lowered_receiver.expect("receiver target must provide lowered receiver"),
            ),
            method: rust_name.to_string(),
            args,
        },
        CallTargetPlan::FallbackReceiver { rust_name, .. } => RustExpr::MethodCall {
            receiver: Box::new(
                lowered_receiver.expect("receiver target must provide lowered receiver"),
            ),
            method: rust_name.clone(),
            args,
        },
        CallTargetPlan::InstanceSelf { rust_name } => RustExpr::MethodCall {
            receiver: Box::new(RustExpr::Var("self".to_string())),
            method: rust_name.to_string(),
            args,
        },
        CallTargetPlan::FallbackFree { rust_name } => RustExpr::Call {
            function: Box::new(RustExpr::Var(rust_name.clone())),
            args,
        },
    }
}

pub fn apply_new_target(target: &NewTargetPlan<'_>, args: Vec<RustExpr>) -> RustExpr {
    apply_new_target_with_facts(target, args, &FactSet::default())
}

pub fn apply_new_target_with_facts(
    target: &NewTargetPlan<'_>,
    args: Vec<RustExpr>,
    facts: &FactSet,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw(format!(
            "{}::{}",
            crate::emit::emit_type(&rust_type_for_slot_with_facts(&target.owner_shape, facts)),
            target.rust_name
        ))),
        args,
    }
}
