#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteNeed {
    LoopContinueUpdatePath,
    BorrowConflict,
    EscapingInnerCapture,
    ComplexFinally,
}

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
