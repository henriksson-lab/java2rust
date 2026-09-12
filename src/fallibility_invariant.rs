//! The invariant [0010](../claude_audits/0010-fallibility-is-decided-three-times-and-disagrees.md)
//! has been asking for since it was filed.
//!
//! Fallibility is decided in three places — the call site that inserts `?`, the
//! callee's signature, and the caller's signature. When they were computed
//! independently they disagreed, and every disagreement was two `rustc` errors:
//! `?` applied to a non-`Try` value, inside a function that does not return
//! `Result`. That class was 214 errors in FastQC, the second largest.
//!
//! Those 214 downstream errors carry no attribution: each names a line of
//! generated Rust, not the decision that produced it. This module converts them
//! into a handful of named analysis failures by checking the property directly:
//!
//! > for every call edge that is not handled locally, if the callee returns
//! > `Result` then the caller must too.
//!
//! At a correct fixpoint this reports nothing — that is the point. It fires if a
//! second store for fallibility is ever reintroduced, or if the call graph and
//! the solver disagree about a callable's identity, which is exactly how the
//! override-set equalization silently did nothing for years: it keyed its groups
//! on the Java method name while the plan was keyed on the Rust name.

use crate::diagnostics::DiagnosticBag;
use crate::exception_plan::ProgramFallibilityPlan;
use crate::usage::{CallGraph, CallGraphEdgeKind, CallableKey};

/// A call edge whose callee is fallible but whose caller is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FallibilityViolation {
    caller: CallableKey,
    callee: CallableKey,
}

impl FallibilityViolation {
    pub(crate) fn caller(&self) -> &CallableKey {
        &self.caller
    }

    pub(crate) fn callee(&self) -> &CallableKey {
        &self.callee
    }
}

/// Every edge that breaks the invariant.
pub(crate) fn fallibility_violations(
    call_graph: &CallGraph,
    fallibility: &ProgramFallibilityPlan,
) -> Vec<FallibilityViolation> {
    let mut violations = Vec::new();
    for edge in call_graph.edges() {
        // A call whose exception the caller handles does not propagate, so the
        // caller is legitimately not `Result`. This is the same edge kind the
        // solver skips, so the check and the solve agree about what an edge
        // means.
        if edge.kind() == CallGraphEdgeKind::HandledExceptionBoundary {
            continue;
        }
        if !fallibility.returns_result(edge.callee()) {
            continue;
        }
        if fallibility.returns_result(edge.caller()) {
            continue;
        }
        let violation = FallibilityViolation {
            caller: edge.caller().clone(),
            callee: edge.callee().clone(),
        };
        if !violations.contains(&violation) {
            violations.push(violation);
        }
    }
    violations
}

/// Reports each violation as `fallibility_invariant`.
///
/// A warning rather than a hard error: a violation means the *translator's*
/// analysis is inconsistent, and refusing to emit anything would hide the rest
/// of a file's output behind one solver bug. Naming it is what makes it
/// attributable.
pub(crate) fn report_fallibility_violations(
    call_graph: &CallGraph,
    fallibility: &ProgramFallibilityPlan,
    into: &mut DiagnosticBag,
) {
    for violation in fallibility_violations(call_graph, fallibility) {
        into.warning(
            "fallibility_invariant",
            format!(
                "{} calls {}, which returns Result, but {} does not; \
                 a `?` at this call site cannot compile. This is an analysis \
                 disagreement, not a lowering bug.",
                violation.caller().diagnostic_label(),
                violation.callee().diagnostic_label(),
                violation.caller().diagnostic_label(),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::CallGraphEdge;

    fn method(type_name: &str, method_name: &str) -> CallableKey {
        CallableKey::method_identity(type_name, method_name)
    }

    fn plan(fallible: &[CallableKey]) -> ProgramFallibilityPlan {
        let mut plan = ProgramFallibilityPlan::default();
        for key in fallible {
            plan.mark_returns_result_for_test(key.clone());
        }
        plan
    }

    fn graph(edges: Vec<CallGraphEdge>) -> CallGraph {
        CallGraph::from_edges(edges)
    }

    /// The check has to be able to fire, or it is another detector that reports
    /// success because it matches nothing.
    #[test]
    fn a_fallible_callee_in_an_infallible_caller_is_reported() {
        let caller = method("Caller", "run");
        let callee = method("Callee", "may_throw");
        let graph = graph(vec![CallGraphEdge::new(
            caller.clone(),
            callee.clone(),
            CallGraphEdgeKind::DirectMethodCall,
        )]);

        let violations = fallibility_violations(&graph, &plan(&[callee.clone()]));
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].caller(), &caller);
        assert_eq!(violations[0].callee(), &callee);
    }

    #[test]
    fn a_fallible_caller_is_not_reported() {
        let caller = method("Caller", "run");
        let callee = method("Callee", "may_throw");
        let graph = graph(vec![CallGraphEdge::new(
            caller.clone(),
            callee.clone(),
            CallGraphEdgeKind::DirectMethodCall,
        )]);

        assert!(fallibility_violations(&graph, &plan(&[caller, callee])).is_empty());
    }

    /// A locally handled exception does not propagate, so the caller is
    /// legitimately not `Result`. The solver skips these edges and so must the
    /// check, or every `try`/`catch` would be reported.
    #[test]
    fn a_handled_boundary_is_not_reported() {
        let caller = method("Caller", "guarded");
        let callee = method("Callee", "may_throw");
        let graph = graph(vec![CallGraphEdge::new(
            caller,
            callee.clone(),
            CallGraphEdgeKind::HandledExceptionBoundary,
        )]);

        assert!(fallibility_violations(&graph, &plan(&[callee])).is_empty());
    }
}
