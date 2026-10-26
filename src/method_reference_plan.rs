use crate::java_signature::JavaOwnerKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MethodReferencePlan {
    Constructor {
        type_name: JavaOwnerKey,
    },
    AssociatedFunction {
        owner: JavaOwnerKey,
        method: String,
    },
    BoundReceiver {
        receiver: BoundMethodReferenceReceiver,
        method: String,
    },
    UnboundAccessor {
        method: String,
    },
    CurrentReceiverMethod {
        method: String,
        arity: Option<usize>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BoundMethodReferenceReceiver {
    Local(String),
    InstanceField(String),
}

impl BoundMethodReferenceReceiver {
    pub(crate) fn source_expr(&self) -> crate::java_ir::Expr {
        match self {
            Self::Local(name) => crate::java_ir::Expr::Var(name.clone()),
            Self::InstanceField(name) => crate::java_ir::Expr::FieldAccess(
                Box::new(crate::java_ir::Expr::This),
                name.clone(),
            ),
        }
    }
}

/// Lexical bindings, not capitalization, distinguish a bound receiver from a
/// type owner. Shared by ordinary callbacks and stream callbacks.
pub(crate) fn bound_method_reference_receiver(
    target: &str,
    env: &crate::expr_shape::ShapeEnv,
) -> Option<BoundMethodReferenceReceiver> {
    use crate::types::MethodReferenceValueBinding;
    match env
        .type_env()
        .method_reference_value_binding(target, false)?
    {
        MethodReferenceValueBinding::Local(name, _) => Some(BoundMethodReferenceReceiver::Local(
            env.binding_access_name(&name)
                .map(str::to_string)
                .unwrap_or_else(|| crate::parser::to_snake_case(&name)),
        )),
        MethodReferenceValueBinding::Field(name, _) => {
            Some(BoundMethodReferenceReceiver::InstanceField(name))
        }
    }
}

/// Keep frontend receiver calls/casts intact. Ambiguous expression names still
/// use lexical/type evidence rather than capitalization to distinguish owners.
pub(crate) fn bound_method_reference_source(
    target: &str,
    receiver: Option<&crate::java_ir::Expr>,
    env: &crate::expr_shape::ShapeEnv,
) -> Option<crate::java_ir::Expr> {
    use crate::java_ir::{Expr, JavaType};
    if let Some(receiver) = receiver {
        match receiver {
            Expr::This | Expr::Super | Expr::Var(_) => {}
            Expr::FieldAccess(_, _) => {
                let ty = crate::types::infer_expr_type(receiver, env.type_env());
                if !matches!(ty, JavaType::Unknown(_)) {
                    return Some(receiver.clone());
                }
            }
            Expr::Unsupported { .. } => return None,
            _ => return Some(receiver.clone()),
        }
    }
    bound_method_reference_receiver(target, env).map(|receiver| match receiver {
        BoundMethodReferenceReceiver::InstanceField(ref name) if env.is_static_field(name) => {
            Expr::Var(name.clone())
        }
        receiver => receiver.source_expr(),
    })
}

pub(crate) fn plan_method_reference(target: &str, method: &str) -> MethodReferencePlan {
    let method = crate::parser::to_snake_case(method);
    if method == "new" {
        return MethodReferencePlan::Constructor {
            type_name: source_owner_key(target),
        };
    }
    if matches!(target, "this" | "self") {
        return MethodReferencePlan::CurrentReceiverMethod {
            method,
            arity: None,
        };
    }
    if looks_like_type_name(target) && looks_like_accessor_method(&method) {
        return MethodReferencePlan::UnboundAccessor { method };
    }
    MethodReferencePlan::AssociatedFunction {
        owner: source_owner_key(target),
        method,
    }
}

pub(crate) fn plan_current_receiver_method_reference(
    method: &str,
    arity: Option<usize>,
) -> MethodReferencePlan {
    MethodReferencePlan::CurrentReceiverMethod {
        method: crate::parser::to_snake_case(method),
        arity,
    }
}

fn looks_like_type_name(target: &str) -> bool {
    target
        .rsplit(['.', '$'])
        .next()
        .and_then(|name| name.chars().next())
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn looks_like_accessor_method(method: &str) -> bool {
    method.starts_with('_')
        || method.starts_with("get_")
        || method.starts_with("is_")
        || matches!(method, "name" | "ordinal")
}

fn source_owner_path(target: &str) -> String {
    target.replace('$', ".")
}

fn source_owner_key(target: &str) -> JavaOwnerKey {
    JavaOwnerKey::resolved_source(source_owner_path(target))
}

#[cfg(test)]
mod tests {
    #[test]
    fn static_bound_reference_uses_the_ordinary_static_value_producer() {
        let mut env = crate::expr_shape::ShapeEnv::new();
        env.insert_static_field_shape(
            "target",
            crate::slot_shape::RustSlotShape::for_storage(crate::java_ir::JavaType::Named {
                name: "Worker".into(),
                args: vec![],
            }),
        );
        assert_eq!(
            super::bound_method_reference_source("target", None, &env),
            Some(crate::java_ir::Expr::Var("target".into()))
        );
    }

    #[test]
    fn type_owner_is_not_a_normalized_field_but_exact_field_can_shadow_it() {
        let mut env = crate::expr_shape::ShapeEnv::new();
        let ty = crate::java_ir::JavaType::Named {
            name: "Callback".into(),
            args: vec![],
        };
        env.insert_field_shape(
            "sumOfLogs",
            crate::slot_shape::RustSlotShape::for_storage(ty.clone()),
        );
        env.insert_static_type_alias("SumOfLogs", "SumOfLogs");
        assert_eq!(
            super::bound_method_reference_receiver("SumOfLogs", &env),
            None
        );
        assert_eq!(
            super::bound_method_reference_receiver("sumOfLogs", &env),
            Some(super::BoundMethodReferenceReceiver::InstanceField(
                "sumOfLogs".into()
            ))
        );
        env.insert_field("SumOfLogs", ty);
        assert_eq!(
            super::bound_method_reference_receiver("SumOfLogs", &env),
            Some(super::BoundMethodReferenceReceiver::InstanceField(
                "SumOfLogs".into()
            ))
        );
    }

    #[test]
    fn type_owner_is_not_a_normalized_lexical_binding() {
        let mut env = crate::expr_shape::ShapeEnv::new();
        let worker = crate::java_ir::JavaType::Named {
            name: "Worker".to_string(),
            args: Vec::new(),
        };
        env.insert_local("worker", worker.clone());
        env.insert_static_type_alias("Worker", "Worker");
        assert_eq!(super::bound_method_reference_receiver("Worker", &env), None);
        assert_eq!(
            super::bound_method_reference_receiver("worker", &env),
            Some(super::BoundMethodReferenceReceiver::Local(
                "worker".to_string()
            ))
        );
        // Java permits capitalized variable names. Exact lexical binding has
        // precedence over the type owner, unlike a normalized near-match.
        env.insert_local("Worker", worker);
        assert_eq!(
            super::bound_method_reference_receiver("Worker", &env),
            Some(super::BoundMethodReferenceReceiver::Local(
                "worker".to_string()
            ))
        );
    }

    use super::{JavaOwnerKey, MethodReferencePlan, plan_method_reference};

    #[test]
    fn method_reference_owner_preserves_package_and_nested_identity() {
        assert_eq!(
            plan_method_reference("a.Mapper", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: JavaOwnerKey::resolved_source("a.Mapper"),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("b.Mapper", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: JavaOwnerKey::resolved_source("b.Mapper"),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("K.AB", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: JavaOwnerKey::resolved_source("K.AB"),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("K.A.B", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: JavaOwnerKey::resolved_source("K.A.B"),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("a.Mapper", "new"),
            MethodReferencePlan::Constructor {
                type_name: JavaOwnerKey::resolved_source("a.Mapper"),
            }
        );
        assert_eq!(
            plan_method_reference("K.Kind", "name"),
            MethodReferencePlan::UnboundAccessor {
                method: "name".to_string(),
            }
        );
    }
}
