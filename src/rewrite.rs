use crate::facts::MethodKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodControlFlowPlan {
    method: MethodKey,
    rewrites: Vec<ControlFlowRewritePlan>,
}

impl MethodControlFlowPlan {
    pub fn new(method: MethodKey, rewrites: Vec<ControlFlowRewritePlan>) -> Self {
        Self { method, rewrites }
    }

    pub fn method(&self) -> &MethodKey {
        &self.method
    }

    pub fn rewrites(&self) -> &[ControlFlowRewritePlan] {
        &self.rewrites
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlFlowRewritePlan {
    AssignmentInCondition(AssignmentInConditionPlan),
    TailRecursiveLoop(TailRecursiveLoopPlan),
}

impl ControlFlowRewritePlan {
    pub fn assignment_in_condition(&self) -> Option<&AssignmentInConditionPlan> {
        match self {
            ControlFlowRewritePlan::AssignmentInCondition(plan) => Some(plan),
            _ => None,
        }
    }

    pub fn tail_recursive_loop(&self) -> Option<&TailRecursiveLoopPlan> {
        match self {
            ControlFlowRewritePlan::TailRecursiveLoop(plan) => Some(plan),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFlowRewriteDiagnostic {
    code: &'static str,
    message: String,
}

impl ControlFlowRewriteDiagnostic {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentInConditionPlan {
    local_name: String,
    source_call: String,
    null_test: NullTestKind,
    normalized_shape: AssignmentConditionNormalizedShape,
}

impl AssignmentInConditionPlan {
    pub fn new(
        local_name: String,
        source_call: String,
        null_test: NullTestKind,
        normalized_shape: AssignmentConditionNormalizedShape,
    ) -> Self {
        Self {
            local_name,
            source_call,
            null_test,
            normalized_shape,
        }
    }

    pub fn local_name(&self) -> &str {
        &self.local_name
    }

    pub fn source_call(&self) -> &str {
        &self.source_call
    }

    pub fn null_test(&self) -> NullTestKind {
        self.null_test
    }

    pub fn normalized_shape(&self) -> AssignmentConditionNormalizedShape {
        self.normalized_shape
    }
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
    method_name: String,
    arg_count: usize,
    normalized_shape: TailRecursiveNormalizedShape,
}

impl TailRecursiveLoopPlan {
    pub fn new(
        method_name: String,
        arg_count: usize,
        normalized_shape: TailRecursiveNormalizedShape,
    ) -> Self {
        Self {
            method_name,
            arg_count,
            normalized_shape,
        }
    }

    pub fn method_name(&self) -> &str {
        &self.method_name
    }

    pub fn arg_count(&self) -> usize {
        self.arg_count
    }

    pub fn normalized_shape(&self) -> TailRecursiveNormalizedShape {
        self.normalized_shape
    }
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
            plan.rewrites()
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
        plan.method().type_name() == type_name
            && plan.method().method_name() == method_name
            && plan.rewrites().iter().any(|rewrite| {
                rewrite.tail_recursive_loop().is_some_and(|tail| {
                    tail.normalized_shape()
                        == TailRecursiveNormalizedShape::LoopWithParameterReassignment
                })
            })
    })
}

fn control_flow_rewrite_diagnostic(
    plan: &MethodControlFlowPlan,
    rewrite: &ControlFlowRewritePlan,
) -> ControlFlowRewriteDiagnostic {
    ControlFlowRewriteDiagnostic::new(
        "control_flow_rewrite_pending",
        match rewrite {
            ControlFlowRewritePlan::AssignmentInCondition(_) => format!(
                "{}::{} requires assignment-in-condition normalization before idiomatic lowering",
                plan.method().type_name(),
                plan.method().method_name()
            ),
            ControlFlowRewritePlan::TailRecursiveLoop(_) => format!(
                "{}::{} requires tail-recursive loop normalization before idiomatic lowering",
                plan.method().type_name(),
                plan.method().method_name()
            ),
        },
    )
}
