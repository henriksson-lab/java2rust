use crate::facts::MethodKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodControlFlowPlan {
    pub method: MethodKey,
    pub rewrites: Vec<ControlFlowRewritePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlFlowRewritePlan {
    AssignmentInCondition(AssignmentInConditionPlan),
    TailRecursiveLoop(TailRecursiveLoopPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFlowRewriteDiagnostic {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentInConditionPlan {
    pub local_name: String,
    pub source_call: String,
    pub null_test: NullTestKind,
    pub normalized_shape: AssignmentConditionNormalizedShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullTestKind {
    NotNull,
    IsNull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentConditionNormalizedShape {
    LoopAssignThenBreak,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailRecursiveLoopPlan {
    pub method_name: String,
    pub arg_count: usize,
    pub normalized_shape: TailRecursiveNormalizedShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailRecursiveNormalizedShape {
    LoopWithParameterReassignment,
}

pub fn control_flow_rewrite_diagnostics(
    control_flow_plans: &[MethodControlFlowPlan],
) -> Vec<ControlFlowRewriteDiagnostic> {
    control_flow_plans
        .iter()
        .flat_map(|plan| {
            plan.rewrites
                .iter()
                .filter(|rewrite| !control_flow_rewrite_is_lowered(rewrite))
                .map(|rewrite| control_flow_rewrite_diagnostic(plan, rewrite))
        })
        .collect()
}

fn control_flow_rewrite_is_lowered(rewrite: &ControlFlowRewritePlan) -> bool {
    matches!(
        rewrite,
        ControlFlowRewritePlan::AssignmentInCondition(AssignmentInConditionPlan {
            normalized_shape: AssignmentConditionNormalizedShape::LoopAssignThenBreak,
            ..
        }) | ControlFlowRewritePlan::TailRecursiveLoop(TailRecursiveLoopPlan {
            normalized_shape: TailRecursiveNormalizedShape::LoopWithParameterReassignment,
            ..
        })
    )
}

pub fn method_has_tail_recursive_loop_rewrite(
    control_flow_plans: &[MethodControlFlowPlan],
    type_name: &str,
    method_name: &str,
) -> bool {
    control_flow_plans.iter().any(|plan| {
        plan.method.type_name() == type_name
            && plan.method.method_name() == method_name
            && plan.rewrites.iter().any(|rewrite| {
                matches!(
                    rewrite,
                    ControlFlowRewritePlan::TailRecursiveLoop(TailRecursiveLoopPlan {
                        normalized_shape:
                            TailRecursiveNormalizedShape::LoopWithParameterReassignment,
                        ..
                    })
                )
            })
    })
}

fn control_flow_rewrite_diagnostic(
    plan: &MethodControlFlowPlan,
    rewrite: &ControlFlowRewritePlan,
) -> ControlFlowRewriteDiagnostic {
    ControlFlowRewriteDiagnostic {
        code: "control_flow_rewrite_pending",
        message: match rewrite {
            ControlFlowRewritePlan::AssignmentInCondition(_) => format!(
                "{}::{} requires assignment-in-condition normalization before idiomatic lowering",
                plan.method.type_name(),
                plan.method.method_name()
            ),
            ControlFlowRewritePlan::TailRecursiveLoop(_) => format!(
                "{}::{} requires tail-recursive loop normalization before idiomatic lowering",
                plan.method.type_name(),
                plan.method.method_name()
            ),
        },
    }
}
