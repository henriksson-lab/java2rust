use crate::binding_id::BindingId;
use crate::exception_plan::{MethodExceptionPlan, ProgramExceptionPlan, ProgramFallibilityPlan};
use crate::java_ir::{ConstructorDecl, MethodDecl, ResolvedCall, ResolvedConstructor, TypeDecl};
use crate::usage::BindingUseFacts;
use crate::usage::{MethodUsageFacts, UsageFacts};

#[derive(Debug, Clone, Copy)]
pub(crate) struct CallableAnalysis<'a> {
    usage: &'a UsageFacts,
    exceptions: &'a ProgramExceptionPlan,
    fallibility: &'a ProgramFallibilityPlan,
}

impl<'a> CallableAnalysis<'a> {
    pub(crate) fn new(
        usage: &'a UsageFacts,
        exceptions: &'a ProgramExceptionPlan,
        fallibility: &'a ProgramFallibilityPlan,
    ) -> Self {
        Self {
            usage,
            exceptions,
            fallibility,
        }
    }

    pub(crate) fn current_rust_method_usage(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<&'a MethodUsageFacts> {
        self.usage.current_rust_method_usage(type_name, method_name)
    }

    pub(crate) fn source_method_usage_for_resolved_owner(
        &self,
        owner: &str,
        method: &MethodDecl,
    ) -> Option<&'a MethodUsageFacts> {
        self.usage
            .resolved_source_method_usage_for_owner(owner, method)
    }

    pub(crate) fn source_method_has_nonself_call(&self, owner: &str, method: &MethodDecl) -> bool {
        self.usage.method_entries().any(|(_, usage)| {
            usage.call_sites().iter().any(|call| {
                call.resolved().is_some_and(|resolved| {
                    resolved.kind == crate::java_ir::CallKind::Instance
                        && crate::facts::external_type_name_matches(&resolved.declaring_type, owner)
                        && resolved.rust_name
                            == method
                                .rust_name
                                .clone()
                                .unwrap_or_else(|| crate::parser::to_snake_case(&method.name))
                        && !call.receiver_is_self()
                })
            })
        })
    }

    pub(crate) fn source_method_name_anywhere_mutates(&self, method_name: &str) -> bool {
        self.usage.source_method_name_anywhere_mutates(method_name)
    }

    pub(crate) fn resolved_source_method_mutates(
        &self,
        resolved: &crate::java_ir::ResolvedCall,
        source_name: &str,
    ) -> bool {
        self.usage
            .resolved_source_method_usage(resolved, source_name)
            .is_some_and(crate::usage::MethodUsageFacts::mutates_self)
    }

    pub(crate) fn binding_use_count_for_rust_method_family(
        &self,
        type_name: &str,
        method_name: &str,
        binding_id: &BindingId,
    ) -> Option<usize> {
        self.usage
            .binding_facts_for_rust_method_family(type_name, method_name, binding_id)
            .map(|binding| binding.uses().len())
    }

    pub(crate) fn binding_facts_for_rust_method_family(
        &self,
        type_name: &str,
        method_name: &str,
        binding_id: &BindingId,
    ) -> Option<&'a BindingUseFacts> {
        self.usage
            .binding_facts_for_rust_method_family(type_name, method_name, binding_id)
    }

    pub(crate) fn current_rust_constructor_usage(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<&'a MethodUsageFacts> {
        self.usage
            .current_rust_constructor_usage(type_name, rust_name)
    }

    pub(crate) fn callable_returns_result(&self, key: &crate::usage::CallableKey) -> bool {
        self.fallibility.returns_result(key)
    }

    pub(crate) fn source_method_identity_returns_result(
        &self,
        type_name: impl Into<String>,
        method_name: impl Into<String>,
    ) -> bool {
        let key = crate::usage::CallableKey::method_identity(type_name, method_name);
        self.callable_returns_result(&key)
    }

    pub(crate) fn resolved_source_method_returns_result(&self, resolved: &ResolvedCall) -> bool {
        let key = crate::usage::CallableKey::resolved_method(resolved);
        self.callable_returns_result(&key)
    }

    pub(crate) fn source_constructor_returns_result(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> bool {
        let key = crate::usage::CallableKey::source_constructor_for_type(ty, ctor);
        self.callable_returns_result(&key)
    }

    pub(crate) fn constructor_expr_returns_result(
        &self,
        resolved: Option<&ResolvedConstructor>,
        external_type_name: impl Into<String>,
        arg_count: usize,
    ) -> bool {
        let key = resolved
            .map(crate::usage::CallableKey::resolved_constructor)
            .unwrap_or_else(|| {
                crate::usage::CallableKey::external_constructor(external_type_name, arg_count)
            });
        self.callable_returns_result(&key)
    }

    pub(crate) fn method_exception_plan(
        &self,
        ty: &TypeDecl,
        method: &MethodDecl,
    ) -> MethodExceptionPlan {
        self.exceptions.method_plan(ty, method)
    }

    pub(crate) fn external_method_returns_result(
        &self,
        receiver_type: Option<&str>,
        method_name: &str,
        arg_count: usize,
    ) -> bool {
        let key = crate::usage::CallableKey::external_method(receiver_type, method_name, arg_count);
        self.callable_returns_result(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding_id::BindingKind;
    use crate::exception_plan::{ProgramExceptionPlan, ProgramFallibilityPlan};
    use crate::java_ir::{JavaType, PrimitiveType};
    use crate::usage::{BindingUse, BindingUseFacts, MethodUsageFacts, UsageFacts, UsageMethodKey};

    fn binding_facts(
        binding: BindingId,
        java_ty: JavaType,
        uses: Vec<BindingUse>,
    ) -> BindingUseFacts {
        let name = binding.name().to_string();
        BindingUseFacts::new(name, Some(binding), java_ty, uses)
    }

    #[test]
    fn rust_method_family_binding_lookup_refuses_ambiguous_overloads() {
        let binding = BindingId::new(BindingKind::Param, "value", 0);
        let mut usage = UsageFacts::default();
        usage.insert_rust_name_method_usage(
            UsageMethodKey::current_rust_method_name("Owner", "pick_int"),
            MethodUsageFacts::from_exact_binding_facts([binding_facts(
                binding.clone(),
                JavaType::Primitive(PrimitiveType::Int),
                vec![BindingUse::ContentRead],
            )]),
        );
        usage.insert_rust_name_method_usage(
            UsageMethodKey::current_rust_method_name("Owner", "pick_string"),
            MethodUsageFacts::from_exact_binding_facts([binding_facts(
                binding.clone(),
                JavaType::String,
                vec![BindingUse::ContentRead, BindingUse::Returned],
            )]),
        );

        let exceptions = ProgramExceptionPlan::default();
        let fallibility = ProgramFallibilityPlan::default();
        let analysis = CallableAnalysis::new(&usage, &exceptions, &fallibility);

        assert_eq!(
            analysis.binding_facts_for_rust_method_family("Owner", "pick", &binding),
            None
        );
        assert_eq!(
            analysis.binding_use_count_for_rust_method_family("Owner", "pick", &binding),
            None
        );
    }
}
