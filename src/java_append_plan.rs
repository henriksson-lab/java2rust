//! Shared proof for native builder append(Object). Display is a checked Java
//! callback after receiver/argument evaluation, not a Rust formatting borrow.
use crate::{
    call_plan::java_type_owner_is_parsed_source,
    facts::FactSet,
    java_ir::{Expr, JavaType},
    types::{TypeEnv, infer_expr_type, optional_element_type},
};

pub(crate) const NATIVE_BUILDER_TYPES: [&str; 2] = ["StringBuilder", "StringBuffer"];

pub(crate) fn qualified_native_builder_name(name: &str) -> Option<&'static str> {
    let suffix = name.strip_prefix("java.lang.")?;
    NATIVE_BUILDER_TYPES
        .into_iter()
        .find(|native| *native == suffix)
}

pub(crate) fn native_builder_receiver_type(
    receiver: &Expr,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<JavaType> {
    let ty = infer_expr_type(receiver, types);
    let ty = optional_element_type(&ty).unwrap_or(ty);
    let ty = types.type_parameter_receiver_bound(&ty).unwrap_or(ty);
    let (JavaType::Named { name, .. } | JavaType::Unknown(name)) = &ty else {
        return None;
    };
    if java_type_owner_is_parsed_source(name, types, facts)
        || !NATIVE_BUILDER_TYPES
            .iter()
            .any(|native| crate::call_plan::native_static_owner_matches(name, native))
    {
        return None;
    }
    Some(ty)
}

pub(crate) struct ObjectAppend<'a> {
    pub receiver: &'a Expr,
    pub argument: &'a Expr,
    pub return_type: JavaType,
}

pub(crate) fn plan_object_append<'a>(
    expr: &'a Expr,
    types: &TypeEnv,
    facts: &FactSet,
) -> Option<ObjectAppend<'a>> {
    let Expr::Call {
        receiver: Some(receiver),
        name,
        args,
        ..
    } = expr
    else {
        return None;
    };
    let [argument] = args.as_slice() else {
        return None;
    };
    if name != "append" {
        return None;
    }
    let receiver_ty = native_builder_receiver_type(receiver, types, facts)?;

    let argument_ty = infer_expr_type(argument, types);
    let argument_ty = optional_element_type(&argument_ty).unwrap_or(argument_ty);
    let argument_ty = types
        .type_parameter_receiver_bound(&argument_ty)
        .unwrap_or(argument_ty);
    // Primitive, String and array overloads retain their own native lowering.
    // An explicit Object cast has Object's static type and takes this route.
    if !matches!(
        argument_ty,
        JavaType::Named { .. } | JavaType::Unknown(_) | JavaType::Intersection(_)
    ) {
        return None;
    }
    let char_sequence = crate::parser::parse_hierarchy_type("java.lang.CharSequence");
    fn is_sequence(ty: &JavaType, sequence: &JavaType, facts: &FactSet) -> bool {
        if let JavaType::Intersection(bounds) = ty {
            return bounds
                .iter()
                .any(|bound| is_sequence(bound, sequence, facts));
        }
        facts.hierarchy_subtype_policy(ty, sequence).reaches()
    }
    if is_sequence(&argument_ty, &char_sequence, facts) {
        return None;
    }
    Some(ObjectAppend {
        receiver,
        argument,
        return_type: receiver_ty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_append_does_not_conflate_native_overloads_or_source_owners() {
        let facts = FactSet::default();
        let mut types = TypeEnv::default();
        types.insert_param(
            "builder",
            JavaType::Named {
                name: "java.lang.StringBuilder".into(),
                args: vec![],
            },
        );
        for (ty, object) in [
            ("Object", true),
            ("Widget", true),
            ("String", false),
            ("int", false),
            ("char[]", false),
            ("CharSequence", false),
            ("StringBuilder", false),
        ] {
            types.insert_param("value", crate::parser::parse_hierarchy_type(ty));
            assert_eq!(
                plan_object_append(
                    &crate::parser::parse_expr("builder.append(value)"),
                    &types,
                    &facts
                )
                .is_some(),
                object,
                "{ty}"
            );
        }
        types.insert_type_param_bound("T", char_sequence());
        types.insert_param("value", crate::parser::parse_hierarchy_type("T"));
        assert!(
            plan_object_append(
                &crate::parser::parse_expr("builder.append(value)"),
                &types,
                &facts
            )
            .is_none()
        );
        types.insert_param(
            "builder",
            JavaType::Named {
                name: "example.StringBuilder".into(),
                args: vec![],
            },
        );
        types.insert_param("value", crate::parser::parse_hierarchy_type("Object"));
        assert!(
            plan_object_append(
                &crate::parser::parse_expr("builder.append(value)"),
                &types,
                &facts
            )
            .is_none()
        );
    }

    fn char_sequence() -> JavaType {
        crate::parser::parse_hierarchy_type("java.lang.CharSequence")
    }
}
