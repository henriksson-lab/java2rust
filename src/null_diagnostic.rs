//! Source-proven diagnostics for a bound reference dereference. Diagnostic wording
//! is independent of the internal null-projection/idempotence markers.
use crate::rust_ir::{RustClosureBody, RustExpr, RustExprShape, RustStmt};

/// Default javac local-register provenance for ordinary lexical method bodies.
/// This is diagnostic metadata, independent of Rust storage and liveness.
/// Complex compiler lowerings decline the model rather than guessing hidden
/// registers. Constructors also need synthetic JVM parameters and are excluded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SourceLocalSlots {
    slots: std::collections::HashMap<crate::binding_id::BindingId, SourceLocalSlot>,
    array_loop_receivers: std::collections::HashMap<crate::binding_id::BindingId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceLocalSlot {
    pub index: usize,
    pub java_ty: crate::java_ir::JavaType,
}

impl SourceLocalSlots {
    pub fn get(&self, binding: &crate::binding_id::BindingId) -> Option<&SourceLocalSlot> {
        self.slots.get(binding)
    }

    pub fn array_loop_receiver(&self, binding: &crate::binding_id::BindingId) -> Option<usize> {
        self.array_loop_receivers.get(binding).copied()
    }

    pub fn for_method(method: &crate::java_ir::MethodDecl) -> Option<Self> {
        use crate::java_ir::{Expr, JavaType, Literal, Modifier, PrimitiveType, Stmt};
        use crate::source_constant_values::{ConstantScope, ConstantValue, fold_constant_expr};
        #[derive(Clone, Default)]
        struct Scope {
            constants: std::collections::BTreeMap<String, ConstantValue>,
            runtime: std::collections::HashSet<String>,
            types: crate::types::TypeEnv,
        }
        fn width(ty: &JavaType) -> usize {
            if matches!(
                ty,
                JavaType::Primitive(PrimitiveType::Long | PrimitiveType::Double)
            ) {
                2
            } else {
                1
            }
        }
        fn runtime(expr: &Expr, scope: &Scope) -> bool {
            match expr {
                Expr::Call { .. }
                | Expr::New { .. }
                | Expr::ArrayAccess(..)
                | Expr::InstanceOf { .. }
                | Expr::Literal(Literal::Null) => true,
                Expr::Var(name) => scope.runtime.contains(name),
                Expr::FieldAccess(receiver, field) if field == "length" => matches!(
                    crate::types::infer_expr_type(receiver, &scope.types),
                    JavaType::Array(_)
                ),
                Expr::Unary { expr, .. } | Expr::Cast { expr, .. } => runtime(expr, scope),
                Expr::Binary { left, right, .. } => runtime(left, scope) || runtime(right, scope),
                Expr::Ternary {
                    condition,
                    then_expr,
                    else_expr,
                } => {
                    runtime(condition, scope)
                        || runtime(then_expr, scope)
                        || runtime(else_expr, scope)
                }
                _ => false,
            }
        }
        fn block(
            body: &crate::java_ir::Block,
            next: &mut usize,
            scope: &mut Scope,
            slots: &mut SourceLocalSlots,
        ) -> Option<()> {
            for stmt in &body.statements {
                statement(stmt, next, scope, slots)?;
            }
            Some(())
        }
        fn scoped(
            body: &crate::java_ir::Block,
            mut next: usize,
            scope: &Scope,
            slots: &mut SourceLocalSlots,
        ) -> Option<()> {
            block(body, &mut next, &mut scope.clone(), slots)
        }
        fn statement(
            stmt: &Stmt,
            next: &mut usize,
            scope: &mut Scope,
            slots: &mut SourceLocalSlots,
        ) -> Option<()> {
            match stmt {
                Stmt::VarDecl {
                    is_final,
                    ty,
                    name,
                    source_name,
                    initializer,
                    binding,
                } => {
                    if matches!(ty, JavaType::Unknown(_)) {
                        return None;
                    }
                    let constant =
                        if *is_final && matches!(ty, JavaType::Primitive(_) | JavaType::String) {
                            let initializer = initializer.as_ref()?;
                            let mut constants = ConstantScope::default();
                            constants.focus_on_owner(None, scope.constants.clone());
                            let value = fold_constant_expr(
                                &Expr::Cast {
                                    ty: ty.clone(),
                                    expr: Box::new(initializer.clone()),
                                },
                                &constants,
                            );
                            if value.is_none() && !runtime(initializer, scope) {
                                return None;
                            }
                            value
                        } else {
                            None
                        };
                    let index = *next;
                    if constant.is_none() {
                        *next += width(ty);
                        if !matches!(ty, JavaType::Primitive(_)) {
                            slots.slots.insert(
                                binding.clone()?,
                                SourceLocalSlot {
                                    index,
                                    java_ty: ty.clone(),
                                },
                            );
                        }
                    }
                    for alias in [name, source_name] {
                        scope.types.insert_local(alias.clone(), ty.clone());
                        if let Some(value) = &constant {
                            scope.constants.insert(alias.clone(), value.clone());
                            scope.runtime.remove(alias);
                        } else {
                            scope.constants.remove(alias);
                            scope.runtime.insert(alias.clone());
                        }
                    }
                }
                Stmt::Block(body)
                | Stmt::LabeledBlock { body, .. }
                | Stmt::While { body, .. }
                | Stmt::DoWhile { body, .. } => scoped(body, *next, scope, slots)?,
                Stmt::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    scoped(then_block, *next, scope, slots)?;
                    if let Some(body) = else_block {
                        scoped(body, *next, scope, slots)?;
                    }
                }
                Stmt::For { init, body, .. } => {
                    let mut inner = *next;
                    let mut loop_scope = scope.clone();
                    for stmt in init {
                        statement(stmt, &mut inner, &mut loop_scope, slots)?;
                    }
                    scoped(body, inner, &loop_scope, slots)?;
                }
                Stmt::EnhancedFor {
                    var_type,
                    var_name,
                    source_var_name,
                    iterable,
                    body,
                    binding,
                    ..
                } => {
                    if !matches!(
                        crate::types::infer_expr_type(iterable, &scope.types),
                        JavaType::Array(_)
                    ) || matches!(var_type, JavaType::Unknown(_))
                    {
                        return None;
                    }
                    let binding = binding.as_ref()?;
                    // javac captures array, length and index before the loop variable.
                    slots.array_loop_receivers.insert(binding.clone(), *next);
                    let index = *next + 3;
                    if !matches!(var_type, JavaType::Primitive(_)) {
                        slots.slots.insert(
                            binding.clone(),
                            SourceLocalSlot {
                                index,
                                java_ty: var_type.clone(),
                            },
                        );
                    }
                    let mut inner = scope.clone();
                    for alias in [var_name, source_var_name] {
                        inner.types.insert_local(alias.clone(), var_type.clone());
                        inner.constants.remove(alias);
                        inner.runtime.insert(alias.clone());
                    }
                    scoped(body, index + width(var_type), &inner, slots)?;
                }
                Stmt::Try { .. } | Stmt::Switch { .. } | Stmt::Unsupported(_) => return None,
                _ => {}
            }
            Some(())
        }
        let body = method.body.as_ref()?;
        let mut supported = true;
        crate::semantic_analysis::walk_block_exprs(body, &mut |expr| {
            if matches!(
                expr,
                Expr::Switch { .. }
                    | Expr::InstanceOf {
                        binding: Some(_),
                        ..
                    }
                    | Expr::Lambda { .. }
                    | Expr::Raw(_)
                    | Expr::Unsupported { .. }
            ) {
                supported = false;
            }
        });
        if !supported {
            return None;
        }
        let mut next = usize::from(!method.modifiers.contains(&Modifier::Static));
        let mut scope = Scope::default();
        for param in &method.params {
            next += width(&param.ty);
            for alias in [&param.name, &param.source_name] {
                scope.types.insert_param(alias.clone(), param.ty.clone());
                scope.runtime.insert(alias.clone());
            }
        }
        let mut slots = Self::default();
        block(body, &mut next, &mut scope, &mut slots)?;
        (!slots.slots.is_empty() || !slots.array_loop_receivers.is_empty()).then_some(slots)
    }
}

fn replace_marked_failure(args: &mut [RustExpr], exception: RustExpr) {
    if let [
        RustExpr::Closure {
            body: RustClosureBody::Expr(body),
            ..
        },
    ] = args
    {
        if body.recorded_shape() == Some(RustExprShape::SourceNullDiagnostic) {
            *body = Box::new(exception);
        }
    }
}

/// Rewrite only the required-value projection introduced at an ABI argument
/// boundary. Calls and block statements compute the argument itself; failures
/// inside those operations retain their own diagnostic and evaluation order.
pub(crate) fn apply_argument_null_exception(expr: RustExpr, exception: &RustExpr) -> RustExpr {
    match expr {
        RustExpr::MethodCall {
            receiver,
            method,
            mut args,
        } if method == "ok_or_else" => {
            replace_marked_failure(
                &mut args,
                RustExpr::shaped(RustExprShape::ExplicitJavaNullFailure, exception.clone()),
            );
            RustExpr::MethodCall {
                receiver,
                method,
                args,
            }
        }
        RustExpr::MethodCall {
            receiver,
            method,
            args,
        } if method == "expect" && crate::coercion::is_java_null_receiver_expect_args(&args) => {
            crate::coercion::java_null_check_with_exception_projection(*receiver, exception.clone())
        }
        RustExpr::Shaped {
            shape,
            actual,
            expr,
        } => RustExpr::Shaped {
            shape,
            actual,
            expr: Box::new(apply_argument_null_exception(*expr, exception)),
        },
        RustExpr::Try(expr) => {
            RustExpr::Try(Box::new(apply_argument_null_exception(*expr, exception)))
        }
        RustExpr::Grouped(expr) => {
            RustExpr::Grouped(Box::new(apply_argument_null_exception(*expr, exception)))
        }
        RustExpr::Unary { op, expr } => RustExpr::Unary {
            op,
            expr: Box::new(apply_argument_null_exception(*expr, exception)),
        },
        RustExpr::Cast { expr, ty } => RustExpr::Cast {
            ty,
            expr: Box::new(apply_argument_null_exception(*expr, exception)),
        },
        RustExpr::Block { stmts, tail } => RustExpr::Block {
            stmts,
            tail: tail.map(|tail| Box::new(apply_argument_null_exception(*tail, exception))),
        },
        other => other,
    }
}

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
    /// checked operand must still be the original bound reference, not a
    /// computed value, a generated staging alias or a different argument.
    pub fn apply(&self, expr: RustExpr) -> RustExpr {
        match expr {
            RustExpr::MethodCall {
                receiver,
                method,
                mut args,
            } => {
                if method == "ok_or_else" && self.checks_parameter(&receiver) {
                    replace_marked_failure(
                        &mut args,
                        RustExpr::shaped(
                            RustExprShape::SourceNullDiagnostic,
                            crate::exception_lowering::new_exception_expr(
                                "NullPointerException",
                                crate::coercion::rust_string_literal_projection(&self.message),
                            ),
                        ),
                    );
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
    fn local_slots_decline_unmodelled_compiler_temporaries_and_unresolved_constants() {
        for body in [
            "for (Object row : unknown()) { return ((int[])row).length; } return 0;",
            "try { int[] row=rows[0]; return row.length; } finally { rows[0]=null; }",
            "switch (rows.length) { case 1: int[] row=rows[0]; return row.length; default: return 0; }",
            "final int padding=Missing.CONSTANT; int[] row=rows[0]; return row.length;",
        ] {
            let source = format!("class Probe {{ static int probe(int[][] rows) {{ {body} }} }}");
            let parsed = crate::parser::parse_java_source(&source);
            assert!(!parsed.diagnostics.has_errors(), "{:?}", parsed.diagnostics);
            let resolved = crate::resolve::resolve_program(&parsed.program, parsed.diagnostics);
            let method = resolved.program.types[0]
                .members
                .iter()
                .find_map(|m| match m {
                    crate::java_ir::Member::Method(m) if m.name == "probe" => Some(m),
                    _ => None,
                })
                .unwrap();
            assert!(SourceLocalSlots::for_method(method).is_none(), "{body}");
        }
    }

    #[test]
    fn argument_diagnostic_rewrites_only_the_outer_required_value_projection() {
        let original = fallible_optional_receiver_projection(
            RustExpr::Var("argument".into()),
            OptionalReceiverProjection::Borrowed,
        );
        let failure = crate::exception_lowering::new_exception_expr(
            "NullPointerException",
            crate::coercion::rust_string_literal_projection("foreign contract"),
        );
        let rewritten = apply_argument_null_exception(original.clone(), &failure);
        let rendered = crate::emit::emit_expr(&rewritten);
        assert!(rendered.contains("foreign contract"), "{rendered}");
        assert!(!rendered.contains("Java null receiver"), "{rendered}");
        let caller = ParameterNullDiagnostic {
            rust_name: "argument".into(),
            message: "caller receiver".into(),
        };
        assert_eq!(
            caller.apply(rewritten.clone()),
            rewritten,
            "an explicit ABI failure must retain its library diagnostic"
        );
        let nested = crate::coercion::function_call_projection(
            RustExpr::path(["produce_argument"]),
            vec![original],
        );
        assert_eq!(
            apply_argument_null_exception(nested.clone(), &failure),
            nested,
            "evaluating an argument is a different operation from requiring its result"
        );
    }

    #[test]
    fn argument_diagnostic_preserves_staged_operand_failures() {
        let operand = fallible_optional_receiver_projection(
            RustExpr::Var("source".into()),
            OptionalReceiverProjection::Borrowed,
        );
        let required = fallible_optional_receiver_projection(
            RustExpr::Var("argument".into()),
            OptionalReceiverProjection::Borrowed,
        );
        let statement = RustStmt::Let {
            mutable: false,
            name: "argument".into(),
            ty: None,
            value: Some(operand),
        };
        let expression = RustExpr::Block {
            stmts: vec![statement.clone()],
            tail: Some(Box::new(required)),
        };
        let failure = crate::exception_lowering::new_exception_expr(
            "NullPointerException",
            crate::coercion::rust_string_literal_projection("foreign contract"),
        );
        let RustExpr::Block { stmts, tail } = apply_argument_null_exception(expression, &failure)
        else {
            panic!("staging block must remain structured");
        };
        assert_eq!(stmts, vec![statement]);
        assert!(crate::emit::emit_expr(&tail.unwrap()).contains("foreign contract"));
    }

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
