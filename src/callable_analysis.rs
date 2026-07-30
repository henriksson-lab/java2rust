use crate::exception_plan::{ProgramExceptionPlan, ProgramFallibilityPlan};
use crate::java_signature::JavaCallableKey;
use crate::signature::{MethodSignaturePlan, SignaturePlan};
use crate::usage::{CallGraph, MethodUsageFacts, UsageConstructorKey, UsageFacts, UsageMethodKey};

#[derive(Debug, Clone, Copy)]
pub struct CallableAnalysis<'a> {
    pub usage: &'a UsageFacts,
    pub call_graph: &'a CallGraph,
    pub signatures: &'a SignaturePlan,
    pub exceptions: &'a ProgramExceptionPlan,
    pub fallibility: &'a ProgramFallibilityPlan,
}

impl<'a> CallableAnalysis<'a> {
    pub fn new(
        usage: &'a UsageFacts,
        call_graph: &'a CallGraph,
        signatures: &'a SignaturePlan,
        exceptions: &'a ProgramExceptionPlan,
        fallibility: &'a ProgramFallibilityPlan,
    ) -> Self {
        Self {
            usage,
            call_graph,
            signatures,
            exceptions,
            fallibility,
        }
    }

    pub fn method_usage(&self, key: &UsageMethodKey) -> Option<&'a MethodUsageFacts> {
        self.usage.methods.get(key)
    }

    pub fn method_usage_by_rust_name(&self, key: &UsageMethodKey) -> Option<&'a MethodUsageFacts> {
        self.usage.methods_by_rust_name.get(key)
    }

    pub fn method_usage_by_java_key(&self, key: &JavaCallableKey) -> Option<&'a MethodUsageFacts> {
        self.usage.methods_by_java_key.get(key)
    }

    pub fn method_signature_by_java_key(
        &self,
        key: &JavaCallableKey,
    ) -> Option<&'a MethodSignaturePlan> {
        self.signatures.method_signature_by_java_key(key)
    }

    pub fn constructor_usage(&self, key: &UsageConstructorKey) -> Option<&'a MethodUsageFacts> {
        self.usage.constructors.get(key)
    }

    pub fn call_graph_edge_count(&self) -> usize {
        self.call_graph.edges.len()
    }

    pub fn callable_returns_result(&self, key: &crate::usage::CallableKey) -> bool {
        self.fallibility.returns_result(key)
    }

    pub fn external_method_returns_result(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: usize,
    ) -> bool {
        let key = crate::usage::CallableKey::ExternalMethod {
            receiver_type: receiver_type.map(str::to_string),
            method_name: method_name.to_string(),
            arg_count,
        };
        self.callable_returns_result(&key)
    }
}
