#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MethodReferencePlan {
    Constructor {
        type_name: String,
    },
    AssociatedFunction {
        owner: String,
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

pub(crate) fn plan_method_reference(target: &str, method: &str) -> MethodReferencePlan {
    let method = crate::parser::to_snake_case(method);
    if method == "new" {
        return MethodReferencePlan::Constructor {
            type_name: source_owner_path(target),
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
        owner: source_owner_path(target),
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

#[cfg(test)]
mod tests {
    use super::{MethodReferencePlan, plan_method_reference};

    #[test]
    fn method_reference_owner_preserves_package_and_nested_identity() {
        assert_eq!(
            plan_method_reference("a.Mapper", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "a.Mapper".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("b.Mapper", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "b.Mapper".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("K.AB", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "K.AB".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("K.A.B", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "K.A.B".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("a.Mapper", "new"),
            MethodReferencePlan::Constructor {
                type_name: "a.Mapper".to_string(),
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
