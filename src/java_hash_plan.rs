//! Shared proof for checked Java hash invocation. Analyses and lowering must
//! agree whether a reference denotes a List value operation or Object dispatch.

use crate::{
    call_plan::{
        CallTargetPlan, NativeCollectionMethod, plan_native_collection_receiver_call_for_type,
    },
    effective_facts::EffectiveFacts,
    facts::FactSet,
    java_ir::{Expr, JavaType},
    types::{TypeEnv, infer_expr_type, optional_element_type},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JavaHashKind {
    Object,
    List,
    Set,
    Map,
}

pub(crate) struct JavaHashCall<'a> {
    pub(crate) receiver: &'a Expr,
    pub(crate) kind: JavaHashKind,
}

pub(crate) fn plan_java_hash_call<'a>(
    expr: &'a Expr,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<JavaHashCall<'a>> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    if !matches!(name.as_str(), "hashCode" | "hash_code") || !args.is_empty() {
        return None;
    }
    if matches!(receiver.as_ref(), Expr::Super) {
        return None;
    }
    let ty = infer_expr_type(receiver, types);
    let ty = optional_element_type(&ty).unwrap_or(ty);
    let type_name = match &ty {
        JavaType::Named { name, args } if args.is_empty() => Some(name.as_str()),
        JavaType::Unknown(name) => Some(name.as_str()),
        _ => None,
    };
    if type_name.is_some_and(|name| types.is_type_parameter_name(name)) {
        return Some(JavaHashCall {
            receiver,
            kind: JavaHashKind::Object,
        });
    }
    if EffectiveFacts::new(facts).parsed_source_controls_native_carrier(&ty) {
        // A source class named Set/List/Map still owns its Object override.
        return Some(JavaHashCall {
            receiver,
            kind: JavaHashKind::Object,
        });
    }
    if type_name.is_some_and(|name| matches!(name, "Object" | "java.lang.Object")) {
        return Some(JavaHashCall {
            receiver,
            kind: JavaHashKind::Object,
        });
    }
    let CallTargetPlan::NativeCollectionMethod { method, .. } =
        plan_native_collection_receiver_call_for_type(receiver, name, Some(args), &ty, types)?
    else {
        return None;
    };
    let kind = match method {
        NativeCollectionMethod::ListHashCode => JavaHashKind::List,
        NativeCollectionMethod::SetHashCode => JavaHashKind::Set,
        NativeCollectionMethod::MapHashCode => JavaHashKind::Map,
        _ => return None,
    };
    Some(JavaHashCall { receiver, kind })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_planning_distinguishes_value_carriers_and_preserves_source_precedence() {
        for (ty, expected) in [
            ("java.util.List<String>", JavaHashKind::List),
            ("java.util.Set<String>", JavaHashKind::Set),
            ("java.util.Map<String,Integer>", JavaHashKind::Map),
            ("java.lang.Object", JavaHashKind::Object),
            ("T", JavaHashKind::Object),
        ] {
            let mut types = TypeEnv::default();
            types.insert_type_param_declaration("T", None);
            types.insert_param("value", crate::parser::parse_hierarchy_type(ty));
            let expr = crate::parser::parse_expr("value.hashCode()");
            let planned = plan_java_hash_call(&expr, &types, &FactSet::default()).unwrap();
            assert_eq!(planned.kind, expected, "{ty}");
            assert!(matches!(planned.receiver, Expr::Var(name) if name == "value"));
            assert!(
                plan_java_hash_call(
                    &crate::parser::parse_expr("value.hashCode(1)"),
                    &types,
                    &FactSet::default()
                )
                .is_none()
            );
        }
        let parsed = crate::parser::parse_java_source(
            "class Set<T> { public int hashCode() { return 7; } }",
        );
        let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
        let mut types = TypeEnv::default();
        types.insert_param("value", crate::parser::parse_hierarchy_type("Set<String>"));
        assert_eq!(
            plan_java_hash_call(
                &crate::parser::parse_expr("value.hashCode()"),
                &types,
                &resolved.facts
            )
            .unwrap()
            .kind,
            JavaHashKind::Object
        );
    }
}
