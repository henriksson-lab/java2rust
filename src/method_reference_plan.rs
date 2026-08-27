#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MethodReferencePlan {
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
pub enum BoundMethodReferenceReceiver {
    Local(String),
    InstanceField(String),
}

pub fn plan_method_reference(target: &str, method: &str) -> MethodReferencePlan {
    let method = crate::parser::to_snake_case(method);
    if method == "new" {
        return MethodReferencePlan::Constructor {
            type_name: rust_owner_path(target),
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
        owner: rust_owner_path(target),
        method,
    }
}

pub fn plan_current_receiver_method_reference(
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
    method.starts_with('_') || method.starts_with("get_") || method.starts_with("is_")
}

fn rust_owner_path(target: &str) -> String {
    crate::java_naming::rust_name_for_java_named_type(&target.replace('$', "."))
}

#[cfg(test)]
mod tests {
    use super::{MethodReferencePlan, plan_method_reference};

    #[test]
    fn method_reference_owner_preserves_package_and_nested_identity() {
        assert_eq!(
            plan_method_reference("a.Mapper", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "AMapper".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("b.Mapper", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "BMapper".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("K.AB", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "K_AB".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("K.A.B", "tag"),
            MethodReferencePlan::AssociatedFunction {
                owner: "K_A_B".to_string(),
                method: "tag".to_string(),
            }
        );
        assert_eq!(
            plan_method_reference("a.Mapper", "new"),
            MethodReferencePlan::Constructor {
                type_name: "AMapper".to_string(),
            }
        );
    }
}
