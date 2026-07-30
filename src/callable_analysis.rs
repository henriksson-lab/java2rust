use crate::exception_plan::ProgramExceptionPlan;
use crate::signature::SignaturePlan;
use crate::usage::{CallGraph, MethodUsageFacts, UsageConstructorKey, UsageFacts, UsageMethodKey};

#[derive(Debug, Clone, Copy)]
pub struct CallableAnalysis<'a> {
    pub usage: &'a UsageFacts,
    pub call_graph: &'a CallGraph,
    pub signatures: &'a SignaturePlan,
    pub exceptions: &'a ProgramExceptionPlan,
}

impl<'a> CallableAnalysis<'a> {
    pub fn new(
        usage: &'a UsageFacts,
        call_graph: &'a CallGraph,
        signatures: &'a SignaturePlan,
        exceptions: &'a ProgramExceptionPlan,
    ) -> Self {
        Self {
            usage,
            call_graph,
            signatures,
            exceptions,
        }
    }

    pub fn method_usage(&self, key: &UsageMethodKey) -> Option<&'a MethodUsageFacts> {
        self.usage.methods.get(key)
    }

    pub fn method_usage_by_rust_name(&self, key: &UsageMethodKey) -> Option<&'a MethodUsageFacts> {
        self.usage.methods_by_rust_name.get(key)
    }

    pub fn constructor_usage(&self, key: &UsageConstructorKey) -> Option<&'a MethodUsageFacts> {
        self.usage.constructors.get(key)
    }

    pub fn call_graph_edge_count(&self) -> usize {
        self.call_graph.edges.len()
    }
}
