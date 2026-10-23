//! Source-proven diagnostics for a parameter dereference. Diagnostic wording
//! is independent of the internal null-projection/idempotence markers.
use crate::rust_ir::{RustClosureBody, RustExpr, RustExprShape, RustStmt};

#[derive(Debug, Clone)]
pub(crate) struct ParameterNullDiagnostic {
    pub rust_name: String,
    pub message: String,
}

impl ParameterNullDiagnostic {
    fn checks_parameter(&self, expr: &RustExpr) -> bool {
        match expr.unshaped() {
            RustExpr::Var(name) => name == &self.rust_name,
            RustExpr::Grouped(expr) | RustExpr::Unary { expr, .. } => self.checks_parameter(expr),
            RustExpr::MethodCall {
                receiver,
                method,
                args,
            } if args.is_empty()
                && matches!(
                    method.as_str(),
                    "as_ref" | "as_mut" | "as_deref" | "as_deref_mut"
                ) =>
            {
                self.checks_parameter(receiver)
            }
            _ => false,
        }
    }

    /// Visit only projection structure, never arbitrary callback bodies. The
    /// checked operand must still be the original parameter, not a field,
    /// computed value, staged alias or a different argument.
    pub fn apply(&self, expr: RustExpr) -> RustExpr {
        match expr {
            RustExpr::MethodCall {
                receiver,
                method,
                mut args,
            } => {
                if method == "ok_or_else" && self.checks_parameter(&receiver) {
                    if let [
                        RustExpr::Closure {
                            body: RustClosureBody::Expr(body),
                            ..
                        },
                    ] = args.as_mut_slice()
                    {
                        if body.recorded_shape() == Some(RustExprShape::SourceNullDiagnostic) {
                            *body = Box::new(RustExpr::shaped(
                                RustExprShape::SourceNullDiagnostic,
                                crate::exception_lowering::new_exception_expr(
                                    "NullPointerException",
                                    crate::coercion::rust_string_literal_projection(&self.message),
                                ),
                            ));
                        }
                    }
                }
                RustExpr::MethodCall {
                    receiver: Box::new(self.apply(*receiver)),
                    method,
                    args,
                }
            }
            RustExpr::Shaped {
                shape,
                actual,
                expr,
            } => RustExpr::Shaped {
                shape,
                actual,
                expr: Box::new(self.apply(*expr)),
            },
            RustExpr::Try(expr) => RustExpr::Try(Box::new(self.apply(*expr))),
            RustExpr::Grouped(expr) => RustExpr::Grouped(Box::new(self.apply(*expr))),
            RustExpr::Unary { op, expr } => RustExpr::Unary {
                op,
                expr: Box::new(self.apply(*expr)),
            },
            RustExpr::Cast { expr, ty } => RustExpr::Cast {
                expr: Box::new(self.apply(*expr)),
                ty,
            },
            RustExpr::Field(expr, field) => RustExpr::Field(Box::new(self.apply(*expr)), field),
            RustExpr::PostfixComment { expr, comment } => RustExpr::PostfixComment {
                expr: Box::new(self.apply(*expr)),
                comment,
            },
            RustExpr::MarkedBlock { marker, expr } => RustExpr::MarkedBlock {
                marker,
                expr: Box::new(self.apply(*expr)),
            },
            RustExpr::Block { stmts, tail } => RustExpr::Block {
                stmts: stmts
                    .into_iter()
                    .map(|stmt| match stmt {
                        RustStmt::Let {
                            mutable,
                            name,
                            ty,
                            value,
                        } => RustStmt::Let {
                            mutable,
                            name,
                            ty,
                            value: value.map(|value| self.apply(value)),
                        },
                        RustStmt::Expr(expr) => RustStmt::Expr(self.apply(expr)),
                        other => other,
                    })
                    .collect(),
                tail: tail.map(|expr| Box::new(self.apply(*expr))),
            },
            // Clone/borrow adapters can place a checked projection in a free
            // call argument. A bound source parameter has no operand effects;
            // checks still require the exact parameter at the check itself.
            RustExpr::Call { function, args } => RustExpr::Call {
                function,
                args: args.into_iter().map(|arg| self.apply(arg)).collect(),
            },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coercion::{OptionalReceiverProjection, fallible_optional_receiver_projection};

    #[test]
    fn string_search_descriptors_use_declared_overloads_and_widening() {
        use crate::java_ir::{JavaType, PrimitiveType};
        let int = JavaType::Primitive(PrimitiveType::Int);
        let char_ty = JavaType::Primitive(PrimitiveType::Char);
        assert_eq!(
            crate::stdlib::string_search_declared_parameter_types("indexOf", &[char_ty.clone()]),
            Some(vec![int.clone()])
        );
        assert_eq!(
            crate::stdlib::string_search_declared_parameter_types(
                "lastIndexOf",
                &[char_ty, JavaType::Primitive(PrimitiveType::Short)]
            ),
            Some(vec![int.clone(), int.clone()])
        );
        assert_eq!(
            crate::stdlib::string_search_declared_parameter_types(
                "indexOf",
                &[JavaType::String, int.clone()]
            ),
            Some(vec![JavaType::String, int])
        );
        assert!(
            crate::stdlib::string_search_declared_parameter_types(
                "indexOf",
                &[JavaType::Named {
                    name: "app.String".into(),
                    args: vec![]
                }]
            )
            .is_none()
        );
        assert!(
            crate::stdlib::string_search_declared_parameter_types(
                "indexOf",
                &[JavaType::Primitive(PrimitiveType::Float)]
            )
            .is_none()
        );
    }

    #[test]
    fn synthetic_callback_parameters_do_not_inherit_enclosing_parameter_ordinals() {
        let translated = crate::translate_source(
            r#"
            import java.util.function.IntSupplier;
            class Data { int read() { return 7; } }
            class Factory {
                static IntSupplier capture(Data first, Data second) { return () -> second.read(); }
            }
        "#,
        );
        assert!(
            !translated.diagnostics.has_errors(),
            "{:?}",
            translated.diagnostics
        );
        assert!(
            !translated.rust.contains("<parameter2>"),
            "lambda has a synthetic parameter frame"
        );
    }

    #[test]
    fn inlined_multi_parameter_for_each_keeps_its_synthetic_frame_unknown() {
        let translated = crate::translate_source(
            r#"
            import java.util.Map;
            import java.util.HashMap;
            class Item { int read() { return 7; } }
            class Factory {
                static void visit(Item first, Item second, Map<Integer,Integer> values) {
                    values.forEach((key, value) -> second.read());
                }
                static void run() { visit(null, null, new HashMap<Integer,Integer>()); }
            }
        "#,
        );
        assert!(
            !translated.diagnostics.has_errors(),
            "{:?}",
            translated.diagnostics
        );
        assert!(
            !translated.rust.contains("<parameter2>"),
            "inlined callback is not the enclosing callable's parameter frame"
        );
    }

    #[test]
    fn parameter_diagnostic_changes_only_matching_generated_exception_checks() {
        let plan = ParameterNullDiagnostic {
            rust_name: "value".into(),
            message: "context".into(),
        };
        let check = |name: &str| {
            fallible_optional_receiver_projection(
                RustExpr::Var(name.into()),
                OptionalReceiverProjection::Shared,
            )
        };
        let matching = plan.apply(check("value"));
        assert!(crate::emit::emit_expr(&matching).contains("\"context\""));
        let other = check("other");
        assert_eq!(plan.apply(other.clone()), other);
        let computed = fallible_optional_receiver_projection(
            RustExpr::Field(Box::new(RustExpr::Var("value".into())), "child".into()),
            OptionalReceiverProjection::Shared,
        );
        assert_eq!(plan.apply(computed.clone()), computed);
        let explicit = crate::exception_lowering::new_exception_expr(
            "NullPointerException",
            crate::coercion::rust_string_literal_projection("user message"),
        );
        assert_eq!(plan.apply(explicit.clone()), explicit);
        assert!(crate::coercion::java_optionality_already_discharged(
            &matching
        ));
        let invocation = ParameterNullDiagnostic {
            rust_name: "value".into(),
            message: "original invocation".into(),
        };
        assert!(
            crate::emit::emit_expr(&invocation.apply(matching)).contains("original invocation")
        );
        let mut source_written = check("value").unshaped().clone();
        let RustExpr::Try(inner) = &mut source_written else {
            panic!("expected generated Try");
        };
        let RustExpr::MethodCall { args, .. } = inner.as_mut() else {
            panic!("expected null projection");
        };
        let RustExpr::Closure {
            body: RustClosureBody::Expr(body),
            ..
        } = &mut args[0]
        else {
            panic!("expected error closure");
        };
        *body = Box::new(crate::exception_lowering::new_exception_expr(
            "NullPointerException",
            crate::coercion::rust_string_literal_projection("Java null receiver"),
        ));
        assert_eq!(
            plan.apply(source_written.clone()),
            source_written,
            "identical source-written prose is not generated provenance"
        );
    }
}
