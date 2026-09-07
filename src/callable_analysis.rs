use crate::binding_id::BindingId;
use crate::exception_plan::{MethodExceptionPlan, ProgramExceptionPlan, ProgramFallibilityPlan};
use crate::java_ir::{MethodDecl, TypeDecl};
use crate::java_signature::JavaCallableKey;
use crate::signature::{MethodSignaturePlan, SignaturePlan};
use crate::usage::BindingUseFacts;
use crate::usage::{MethodUsageFacts, UsageConstructorKey, UsageFacts, UsageMethodKey};

#[derive(Debug, Clone, Copy)]
pub struct CallableAnalysis<'a> {
    usage: &'a UsageFacts,
    signatures: &'a SignaturePlan,
    exceptions: &'a ProgramExceptionPlan,
    fallibility: &'a ProgramFallibilityPlan,
}

impl<'a> CallableAnalysis<'a> {
    pub fn new(
        usage: &'a UsageFacts,
        signatures: &'a SignaturePlan,
        exceptions: &'a ProgramExceptionPlan,
        fallibility: &'a ProgramFallibilityPlan,
    ) -> Self {
        Self {
            usage,
            signatures,
            exceptions,
            fallibility,
        }
    }

    pub fn method_usage(&self, key: &UsageMethodKey) -> Option<&'a MethodUsageFacts> {
        self.usage.methods.get(key)
    }

    pub fn method_usage_by_java_key(&self, key: &JavaCallableKey) -> Option<&'a MethodUsageFacts> {
        self.usage.methods_by_java_key.get(key)
    }

    pub(crate) fn binding_use_count_for_rust_method_family(
        &self,
        type_name: &str,
        method_name: &str,
        binding_id: &BindingId,
    ) -> Option<usize> {
        self.usage
            .methods_by_rust_name
            .iter()
            .filter_map(|(key, method)| {
                rust_method_family_key_matches(key, type_name, method_name)
                    .then(|| method.binding_facts_for_id(binding_id))
                    .flatten()
            })
            .map(|binding| binding.uses.len())
            .max()
    }

    pub(crate) fn binding_facts_for_rust_method_family(
        &self,
        type_name: &str,
        method_name: &str,
        binding_id: &BindingId,
    ) -> Option<&'a BindingUseFacts> {
        self.usage
            .methods_by_rust_name
            .iter()
            .find_map(|(key, method)| {
                rust_method_family_key_matches(key, type_name, method_name)
                    .then(|| method.binding_facts_for_id(binding_id))
                    .flatten()
            })
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

    pub fn callable_returns_result(&self, key: &crate::usage::CallableKey) -> bool {
        self.fallibility.returns_result(key)
    }

    pub fn method_exception_plan(&self, ty: &TypeDecl, method: &MethodDecl) -> MethodExceptionPlan {
        self.exceptions.method_plan(ty, method)
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

fn rust_method_family_key_matches(
    key: &UsageMethodKey,
    type_name: &str,
    method_name: &str,
) -> bool {
    if key.type_name != type_name {
        return false;
    }
    let rust_base = crate::parser::to_snake_case(method_name);
    let overload_prefix = format!("{rust_base}_");
    key.method_name == rust_base || key.method_name.starts_with(&overload_prefix)
}
