use crate::diagnostics::Severity;
use crate::facts::FactSet;
use crate::java_ir::Program;
use crate::representation::RepresentationPlan;
use crate::rewrite::{MethodControlFlowPlan, control_flow_rewrite_diagnostics};
use crate::signature::SignaturePlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringDiagnosticPlan {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

pub fn collect_lowering_diagnostic_plans(
    program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
    control_flow_plans: &[MethodControlFlowPlan],
) -> Vec<LoweringDiagnosticPlan> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(
        facts
            .diagnostics()
            .into_iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Warning,
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    diagnostics.extend(
        representations
            .diagnostics_for_program(program)
            .into_iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Unsupported,
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    diagnostics.extend(
        signatures
            .diagnostics()
            .iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Warning,
                code: diagnostic.code,
                message: diagnostic.message.clone(),
            }),
    );
    diagnostics.extend(
        control_flow_rewrite_diagnostics(control_flow_plans)
            .into_iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Unsupported,
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    diagnostics
}
