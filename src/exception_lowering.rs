use std::collections::BTreeSet;

use crate::coercion::{
    function_call_projection, macro_call_projection, parenthesize_raw_postfix_receiver,
    receiver_method_call_projection_exact,
};
use crate::facts::{ExternalLibraryMode, FactSet};
use crate::rust_ir::{
    RustClosureBody, RustClosureParam, RustExpr, RustItem, RustMatchArm, RustStmt, RustType,
};
use crate::slot_shape::RustSlotShape;

#[derive(Debug, Clone, PartialEq)]
pub struct CatchRenderArm {
    pub variable_name: String,
    pub class_names: Vec<String>,
    pub body: Vec<RustStmt>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExternalExceptionConversion {
    pub java_type: String,
    pub rust_type: String,
}

pub fn java_exception_support_item_with_facts(facts: &FactSet) -> RustItem {
    let conversions = external_exception_conversions(facts);
    let source_parent_arms = source_exception_parent_match_arms(facts);
    let mut item = r#"#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct InputStream;

impl std::fmt::Display for InputStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "java.io.InputStream@0")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException {
    pub class_name: &'static str,
    pub message: String,
    pub cause: Option<std::boxed::Box<JavaException>>,
    pub suppressed: Vec<JavaException>,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message, cause: None, suppressed: Vec::new() }
    }

    pub fn with_cause(
        class_name: &'static str,
        message: String,
        cause: JavaException,
    ) -> JavaException {
        JavaException {
            class_name,
            message,
            cause: Some(std::boxed::Box::new(cause)),
            suppressed: Vec::new(),
        }
    }

    pub fn from_display(class_name: &'static str, err: impl std::fmt::Display) -> JavaException {
        JavaException { class_name, message: err.to_string(), cause: None, suppressed: Vec::new() }
    }

    pub fn is_instance(&self, class_names: &[&str]) -> bool {
        class_names
            .iter()
            .any(|name| self.class_name == *name || java_exception_is_subtype(self.class_name, name))
    }

    pub fn cause_or_self_if_instance(mut self, class_names: &[&str]) -> JavaException {
        if self.cause.as_ref().is_some_and(|cause| cause.is_instance(class_names)) {
            *self.cause.take().expect("checked Java exception cause")
        } else {
            self
        }
    }

    pub fn get_message(&self) -> Option<String> {
        Some(self.message.clone())
    }

    pub fn get_localized_message(&self) -> Option<String> {
        self.get_message()
    }

    pub fn add_suppressed(&mut self, exception: JavaException) {
        self.suppressed.push(exception);
    }

    pub fn get_suppressed(&self) -> Vec<Option<JavaException>> {
        self.suppressed.iter().cloned().map(Some).collect()
    }
}

fn java_exception_parent(class_name: &str) -> Option<&'static str> {
    match class_name.rsplit('.').next().unwrap_or(class_name) {
        "ArrayIndexOutOfBoundsException" => Some("IndexOutOfBoundsException"),
        "ArrayStoreException" => Some("RuntimeException"),
        "ArithmeticException" => Some("RuntimeException"),
        "ClassCastException" => Some("RuntimeException"),
        "ConcurrentModificationException" => Some("RuntimeException"),
        "IllegalArgumentException" => Some("RuntimeException"),
        "IllegalMonitorStateException" => Some("RuntimeException"),
        "IllegalStateException" => Some("RuntimeException"),
        "IndexOutOfBoundsException" => Some("RuntimeException"),
        "InputMismatchException" => Some("NoSuchElementException"),
        "NegativeArraySizeException" => Some("RuntimeException"),
        "NoSuchElementException" => Some("RuntimeException"),
        "NullPointerException" => Some("RuntimeException"),
        "NumberFormatException" => Some("IllegalArgumentException"),
        "StringIndexOutOfBoundsException" => Some("IndexOutOfBoundsException"),
        "UncheckedIOException" => Some("RuntimeException"),
        "UnsupportedOperationException" => Some("RuntimeException"),
        "RuntimeException" => Some("Exception"),
        _ => None,
    }
}

fn java_exception_is_subtype(actual: &str, target: &str) -> bool {
    let target = target.rsplit('.').next().unwrap_or(target);
    let mut current = actual.rsplit('.').next().unwrap_or(actual);
    for _ in 0..32 {
        let Some(parent) = java_exception_parent(current) else {
            return false;
        };
        if parent == target {
            return true;
        }
        current = parent;
    }
    false
}

impl InputStream {
    pub fn read_0(&self) -> Result<i32, JavaException> {
        Ok(0)
    }

    pub fn read<T0: ?Sized>(&self, _arg0: &T0) -> Result<i32, JavaException> {
        Ok(0)
    }

    pub fn read_3<T0: ?Sized>(
        &self,
        _arg0: &T0,
        _arg1: i32,
        _arg2: i32,
    ) -> Result<i32, JavaException> {
        Ok(0)
    }

    pub fn close(&self) -> Result<(), JavaException> {
        Ok(())
    }
}

impl std::fmt::Display for JavaException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.class_name, self.message)
    }
}"#
    .to_string();
    if !source_parent_arms.is_empty() {
        item = item.replace(
            "        \"RuntimeException\" => Some(\"Exception\"),\n        _ => None,",
            &format!(
                "        \"RuntimeException\" => Some(\"Exception\"),\n{source_parent_arms}        _ => None,"
            ),
        );
    }
    for conversion in conversions {
        item.push_str("\n\n");
        item.push_str(&format!(
            "impl From<{}> for JavaException {{\n    fn from(err: {}) -> JavaException {{\n        JavaException::from_display({:?}, err)\n    }}\n}}",
            conversion.rust_type, conversion.rust_type, conversion.java_type
        ));
    }
    RustItem::Raw(item)
}

fn source_exception_parent_match_arms(facts: &FactSet) -> String {
    let built_in_children = BTreeSet::from([
        "ArrayIndexOutOfBoundsException",
        "ArrayStoreException",
        "ArithmeticException",
        "ClassCastException",
        "ConcurrentModificationException",
        "IllegalArgumentException",
        "IllegalMonitorStateException",
        "IllegalStateException",
        "IndexOutOfBoundsException",
        "InputMismatchException",
        "NegativeArraySizeException",
        "NoSuchElementException",
        "NullPointerException",
        "NumberFormatException",
        "StringIndexOutOfBoundsException",
        "UncheckedIOException",
        "UnsupportedOperationException",
        "RuntimeException",
    ]);
    let mut arms = BTreeSet::new();
    for exception in &facts.exception_facts {
        let sub_name = exception
            .type_name
            .rsplit('.')
            .next()
            .unwrap_or(&exception.type_name);
        if built_in_children.contains(sub_name) {
            continue;
        }
        let Some(parent_name) = facts.extends_type_name_for(&exception.type_name) else {
            continue;
        };
        let parent_simple = parent_name.rsplit('.').next().unwrap_or(&parent_name);
        if parent_simple == sub_name || parent_simple.is_empty() {
            continue;
        }
        arms.insert(format!(
            "        {sub_name:?} => Some({parent_simple:?}),\n"
        ));
    }
    arms.into_iter().collect()
}

pub fn external_exception_conversions(facts: &FactSet) -> Vec<ExternalExceptionConversion> {
    let mut conversions = BTreeSet::new();
    for exception in &facts.exception_facts {
        let Some(pack) = facts.external_library_pack_for_type(&exception.type_name) else {
            continue;
        };
        if pack.mode != ExternalLibraryMode::RustCrateMapping {
            continue;
        }
        let Some(crate_name) = pack.rust_target.as_deref() else {
            continue;
        };
        let Some(simple) = exception
            .type_name
            .split('.')
            .filter(|segment| !segment.is_empty())
            .next_back()
        else {
            continue;
        };
        conversions.insert(ExternalExceptionConversion {
            java_type: exception.type_name.clone(),
            rust_type: format!("{crate_name}::{simple}"),
        });
    }
    conversions.into_iter().collect()
}

pub fn exception_result_type(ok_type: RustType) -> RustType {
    crate::slot_render::rust_generic_type(
        "Result",
        vec![ok_type, RustType::Named("JavaException".to_string())],
    )
}

pub fn exception_try_ok_type(
    return_shape: Option<&RustSlotShape>,
    facts: &crate::facts::FactSet,
) -> RustType {
    return_shape
        .map(|shape| crate::slot_render::rust_type_for_slot_with_facts(shape, facts))
        .unwrap_or(RustType::Unit)
}

pub fn return_stmt(value: Option<RustExpr>, wrap_in_ok: bool) -> RustStmt {
    if wrap_in_ok {
        RustStmt::Return(Some(ok_expr(value.unwrap_or_else(unit_expr))))
    } else {
        RustStmt::Return(value)
    }
}

pub fn throw_stmt(exception: RustExpr) -> RustStmt {
    throw_stmt_with_result_policy(exception, true)
}

pub(crate) fn throw_stmt_with_result_policy(exception: RustExpr, wrap_in_result: bool) -> RustStmt {
    if !wrap_in_result {
        return uncaught_java_exception_panic_stmt(exception);
    }
    RustStmt::Return(Some(RustExpr::Call {
        function: Box::new(RustExpr::path(["Err"])),
        args: vec![exception],
    }))
}

fn uncaught_java_exception_panic_stmt(exception: RustExpr) -> RustStmt {
    RustStmt::Expr(uncaught_java_exception_panic_expr(exception))
}

fn uncaught_java_exception_panic_expr(exception: RustExpr) -> RustExpr {
    RustExpr::MacroCall {
        macro_name: "panic".to_string(),
        args: vec![
            RustExpr::Literal("\"uncaught Java exception: {:?}\"".to_string()),
            exception,
        ],
    }
}

pub fn try_catch_stmt(
    ok_type: RustType,
    try_body: Vec<RustStmt>,
    catches: Vec<CatchRenderArm>,
    outer_returns_result: bool,
    ok_falls_through: bool,
) -> RustStmt {
    let result_type = exception_result_type(ok_type);
    let mut arms = Vec::new();
    arms.push(RustMatchArm {
        pattern: if ok_falls_through {
            "Ok(())".to_string()
        } else {
            "Ok(value)".to_string()
        },
        body: if ok_falls_through {
            RustExpr::Block {
                stmts: Vec::new(),
                tail: None,
            }
        } else {
            RustExpr::Block {
                stmts: vec![RustStmt::Return(Some(if outer_returns_result {
                    ok_expr(RustExpr::Var("value".to_string()))
                } else {
                    RustExpr::Var("value".to_string())
                }))],
                tail: None,
            }
        },
    });
    for catch in catches {
        let pattern = catch_guard_pattern(&catch);
        let body = if outer_returns_result {
            catch.body
        } else {
            plain_return_catch_body(catch.body)
        };
        arms.push(RustMatchArm {
            pattern,
            body: RustExpr::Block {
                stmts: body,
                tail: None,
            },
        });
    }
    arms.push(RustMatchArm {
        pattern: "Err(ex)".to_string(),
        body: RustExpr::Block {
            stmts: vec![if outer_returns_result {
                RustStmt::Return(Some(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Err"])),
                    args: vec![RustExpr::Var("ex".to_string())],
                }))
            } else {
                uncaught_java_exception_panic_stmt(RustExpr::Var("ex".to_string()))
            }],
            tail: None,
        },
    });
    RustStmt::Expr(RustExpr::Match {
        expr: Box::new(result_scope_call_expr(result_type, try_body)),
        arms,
    })
}

pub fn try_catch_value_fallthrough_stmt(
    ok_type: RustType,
    try_body: Vec<RustStmt>,
    catches: Vec<CatchRenderArm>,
    outer_returns_result: bool,
    outer_result_returns_option: bool,
) -> RustStmt {
    let option_ok_type = RustType::Option(Box::new(ok_type));
    let result_type = exception_result_type(option_ok_type);
    let mut body_with_fallthrough = wrap_ok_returns_in_some(try_body);
    body_with_fallthrough.push(RustStmt::Return(Some(ok_none_expr())));

    let mut arms = Vec::new();
    arms.push(RustMatchArm {
        pattern: "Ok(Some(value))".to_string(),
        body: RustExpr::Block {
            stmts: vec![RustStmt::Return(Some(
                if outer_returns_result && outer_result_returns_option {
                    ok_expr(some_expr(RustExpr::Var("value".to_string())))
                } else if outer_returns_result {
                    ok_expr(RustExpr::Var("value".to_string()))
                } else {
                    RustExpr::Var("value".to_string())
                },
            ))],
            tail: None,
        },
    });
    arms.push(RustMatchArm {
        pattern: "Ok(None)".to_string(),
        body: RustExpr::Block {
            stmts: if outer_returns_result && outer_result_returns_option {
                vec![RustStmt::Return(Some(ok_none_expr()))]
            } else {
                Vec::new()
            },
            tail: None,
        },
    });
    for catch in catches {
        let pattern = catch_guard_pattern(&catch);
        arms.push(RustMatchArm {
            pattern,
            body: RustExpr::Block {
                stmts: catch.body,
                tail: None,
            },
        });
    }
    arms.push(RustMatchArm {
        pattern: "Err(ex)".to_string(),
        body: RustExpr::Block {
            stmts: vec![if outer_returns_result {
                RustStmt::Return(Some(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Err"])),
                    args: vec![RustExpr::Var("ex".to_string())],
                }))
            } else {
                uncaught_java_exception_panic_stmt(RustExpr::Var("ex".to_string()))
            }],
            tail: None,
        },
    });
    RustStmt::Expr(RustExpr::Match {
        expr: Box::new(result_scope_call_expr(result_type, body_with_fallthrough)),
        arms,
    })
}

fn wrap_ok_returns_in_some(stmts: Vec<RustStmt>) -> Vec<RustStmt> {
    stmts.into_iter().map(wrap_ok_return_in_some).collect()
}

fn wrap_ok_return_in_some(stmt: RustStmt) -> RustStmt {
    match stmt {
        RustStmt::Return(Some(RustExpr::Call { function, mut args }))
            if rust_expr_is_path_name(&function, &["Ok"]) && args.len() == 1 =>
        {
            let value = args.remove(0);
            RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::path(["Ok"])),
                args: vec![RustExpr::Call {
                    function: Box::new(RustExpr::path(["Some"])),
                    args: vec![value],
                }],
            }))
        }
        RustStmt::If {
            condition,
            then_body,
            else_body,
        } => RustStmt::If {
            condition,
            then_body: wrap_ok_returns_in_some(then_body),
            else_body: wrap_ok_returns_in_some(else_body),
        },
        RustStmt::IfLet {
            pattern,
            expr,
            then_body,
            else_body,
        } => RustStmt::IfLet {
            pattern,
            expr,
            then_body: wrap_ok_returns_in_some(then_body),
            else_body: wrap_ok_returns_in_some(else_body),
        },
        RustStmt::While { condition, body } => RustStmt::While {
            condition,
            body: wrap_ok_returns_in_some(body),
        },
        RustStmt::Loop { label, body } => RustStmt::Loop {
            label,
            body: wrap_ok_returns_in_some(body),
        },
        other => other,
    }
}

fn plain_return_catch_body(body: Vec<RustStmt>) -> Vec<RustStmt> {
    body.into_iter()
        .map(|stmt| match stmt {
            RustStmt::Return(Some(RustExpr::Call { function, mut args }))
                if rust_expr_is_path_name(&function, &["Err"]) && args.len() == 1 =>
            {
                let exception = args.remove(0);
                uncaught_java_exception_panic_stmt(exception)
            }
            other => other,
        })
        .collect()
}

fn rust_expr_is_path_name(expr: &RustExpr, segments: &[&str]) -> bool {
    match expr.unshaped() {
        RustExpr::Path(actual) => {
            actual.len() == segments.len()
                && actual
                    .iter()
                    .zip(segments)
                    .all(|(actual, expected)| actual == expected)
        }
        _ => false,
    }
}

fn catch_guard_pattern(catch: &CatchRenderArm) -> String {
    let classes = catch
        .class_names
        .iter()
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Err({}) if {}.is_instance(&[{}])",
        catch.variable_name, catch.variable_name, classes
    )
}

fn return_result_var_stmt(result_name: &str) -> RustStmt {
    RustStmt::Return(Some(RustExpr::Var(result_name.to_string())))
}

fn return_result_or_panic_stmt(result_name: &str) -> RustStmt {
    RustStmt::Return(Some(RustExpr::MethodCall {
        receiver: Box::new(RustExpr::Var(result_name.to_string())),
        method: "unwrap_or_else".to_string(),
        args: vec![RustExpr::Closure {
            is_move: false,
            return_type: None,
            params: vec![RustClosureParam {
                pattern: "ex".to_string(),
                ty: None,
            }],
            body: RustClosureBody::Expr(Box::new(uncaught_java_exception_panic_expr(
                RustExpr::Var("ex".to_string()),
            ))),
        }],
    }))
}

fn panic_if_result_err_stmt(result_name: &str) -> RustStmt {
    RustStmt::IfLet {
        pattern: "Err(ex)".to_string(),
        expr: RustExpr::Var(result_name.to_string()),
        then_body: vec![uncaught_java_exception_panic_stmt(RustExpr::Var(
            "ex".to_string(),
        ))],
        else_body: Vec::new(),
    }
}

fn propagate_result_stmt(result_name: &str) -> RustStmt {
    RustStmt::Expr(RustExpr::Try(Box::new(RustExpr::Var(
        result_name.to_string(),
    ))))
}

fn primary_result_then_cleanup_result_expr(
    body_result_name: &str,
    cleanup_result_name: &str,
    body_terminates_with_value: bool,
) -> RustExpr {
    let ok_pattern = if body_terminates_with_value {
        "value"
    } else {
        "()"
    };
    let ok_expr = if body_terminates_with_value {
        RustExpr::Var("value".to_string())
    } else {
        unit_expr()
    };
    RustExpr::Match {
        expr: Box::new(RustExpr::Tuple(vec![
            RustExpr::Var(body_result_name.to_string()),
            RustExpr::Var(cleanup_result_name.to_string()),
        ])),
        arms: vec![
            RustMatchArm {
                pattern: "(Err(mut ex), Err(cleanup_ex))".to_string(),
                body: RustExpr::Block {
                    stmts: vec![RustStmt::Expr(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("ex".to_string())),
                        method: "add_suppressed".to_string(),
                        args: vec![RustExpr::Var("cleanup_ex".to_string())],
                    })],
                    tail: Some(Box::new(RustExpr::Call {
                        function: Box::new(RustExpr::path(["Err"])),
                        args: vec![RustExpr::Var("ex".to_string())],
                    })),
                },
            },
            RustMatchArm {
                pattern: "(Err(ex), _)".to_string(),
                body: RustExpr::Call {
                    function: Box::new(RustExpr::path(["Err"])),
                    args: vec![RustExpr::Var("ex".to_string())],
                },
            },
            RustMatchArm {
                pattern: format!("(Ok({ok_pattern}), Err(ex))"),
                body: RustExpr::Call {
                    function: Box::new(RustExpr::path(["Err"])),
                    args: vec![RustExpr::Var("ex".to_string())],
                },
            },
            RustMatchArm {
                pattern: format!("(Ok({ok_pattern}), Ok(()))"),
                body: RustExpr::Call {
                    function: Box::new(RustExpr::path(["Ok"])),
                    args: vec![ok_expr],
                },
            },
        ],
    }
}

fn return_primary_result_then_cleanup_result_stmt(
    body_result_name: &str,
    cleanup_result_name: &str,
    body_terminates_with_value: bool,
) -> RustStmt {
    RustStmt::Return(Some(primary_result_then_cleanup_result_expr(
        body_result_name,
        cleanup_result_name,
        body_terminates_with_value,
    )))
}

fn propagate_primary_result_then_cleanup_result_stmt(
    body_result_name: &str,
    cleanup_result_name: &str,
    body_terminates_with_value: bool,
) -> RustStmt {
    RustStmt::Expr(RustExpr::Try(Box::new(
        primary_result_then_cleanup_result_expr(
            body_result_name,
            cleanup_result_name,
            body_terminates_with_value,
        ),
    )))
}

fn panic_if_primary_result_then_cleanup_result_err_stmt(
    body_result_name: &str,
    cleanup_result_name: &str,
    body_terminates_with_value: bool,
) -> RustStmt {
    RustStmt::Expr(RustExpr::MethodCall {
        receiver: Box::new(primary_result_then_cleanup_result_expr(
            body_result_name,
            cleanup_result_name,
            body_terminates_with_value,
        )),
        method: "unwrap_or_else".to_string(),
        args: vec![RustExpr::Closure {
            is_move: false,
            return_type: None,
            params: vec![RustClosureParam {
                pattern: "ex".to_string(),
                ty: None,
            }],
            body: RustClosureBody::Expr(Box::new(uncaught_java_exception_panic_expr(
                RustExpr::Var("ex".to_string()),
            ))),
        }],
    })
}

fn result_scope_call_let_stmt(
    result_name: &str,
    ok_type: RustType,
    body: Vec<RustStmt>,
) -> RustStmt {
    RustStmt::Let {
        mutable: false,
        name: result_name.to_string(),
        ty: None,
        value: Some(result_scope_call_expr(exception_result_type(ok_type), body)),
    }
}

fn result_scope_call_expr(result_type: RustType, body: Vec<RustStmt>) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Grouped(Box::new(RustExpr::Closure {
            is_move: false,
            return_type: Some(result_type),
            params: Vec::new(),
            body: RustClosureBody::Expr(Box::new(RustExpr::Block {
                stmts: body,
                tail: None,
            })),
        }))),
        args: Vec::new(),
    }
}

pub fn try_with_resources_scope_stmt(
    ok_type: RustType,
    resource_initializers: Vec<RustStmt>,
    body: Vec<RustStmt>,
    resource_cleanup_stmts: Vec<RustStmt>,
    body_terminates_with_value: bool,
    outer_returns_result: bool,
) -> RustStmt {
    let mut scope_body = resource_initializers;
    let mut body_with_ok = body;
    if !body_terminates_with_value {
        body_with_ok.push(return_stmt(None, true));
    }
    scope_body.push(result_scope_call_let_stmt(
        "__java_twr_result",
        ok_type,
        body_with_ok,
    ));
    let cleanup_body = try_resource_accumulating_cleanup_body(resource_cleanup_stmts);
    scope_body.push(result_scope_call_let_stmt(
        "__java_twr_cleanup_result",
        RustType::Unit,
        cleanup_body,
    ));
    if body_terminates_with_value {
        if outer_returns_result {
            scope_body.push(return_primary_result_then_cleanup_result_stmt(
                "__java_twr_result",
                "__java_twr_cleanup_result",
                true,
            ));
        } else {
            scope_body.push(RustStmt::Return(Some(RustExpr::MethodCall {
                receiver: Box::new(primary_result_then_cleanup_result_expr(
                    "__java_twr_result",
                    "__java_twr_cleanup_result",
                    true,
                )),
                method: "unwrap_or_else".to_string(),
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: vec![RustClosureParam {
                        pattern: "ex".to_string(),
                        ty: None,
                    }],
                    body: RustClosureBody::Expr(Box::new(uncaught_java_exception_panic_expr(
                        RustExpr::Var("ex".to_string()),
                    ))),
                }],
            })));
        }
    } else if outer_returns_result {
        scope_body.push(propagate_primary_result_then_cleanup_result_stmt(
            "__java_twr_result",
            "__java_twr_cleanup_result",
            false,
        ));
    } else {
        scope_body.push(panic_if_primary_result_then_cleanup_result_err_stmt(
            "__java_twr_result",
            "__java_twr_cleanup_result",
            false,
        ));
    }
    RustStmt::Expr(RustExpr::Block {
        stmts: scope_body,
        tail: None,
    })
}

pub fn try_with_resources_finally_scope_stmt(
    ok_type: RustType,
    resource_initializers: Vec<RustStmt>,
    body: Vec<RustStmt>,
    resource_cleanup_stmts: Vec<RustStmt>,
    finally_body: Vec<RustStmt>,
    body_terminates_with_value: bool,
    outer_returns_result: bool,
) -> RustStmt {
    let mut scope_body = resource_initializers;
    let mut body_with_ok = body;
    if !body_terminates_with_value {
        body_with_ok.push(return_stmt(None, true));
    }
    scope_body.push(result_scope_call_let_stmt(
        "__java_twr_result",
        ok_type,
        body_with_ok,
    ));
    let cleanup_body = try_resource_accumulating_cleanup_body(resource_cleanup_stmts);
    scope_body.push(result_scope_call_let_stmt(
        "__java_twr_cleanup_result",
        RustType::Unit,
        cleanup_body,
    ));
    scope_body.extend(finally_body);
    if rust_stmts_definitely_terminate(&scope_body) {
        return RustStmt::Expr(RustExpr::Block {
            stmts: scope_body,
            tail: None,
        });
    }
    if body_terminates_with_value {
        if outer_returns_result {
            scope_body.push(return_primary_result_then_cleanup_result_stmt(
                "__java_twr_result",
                "__java_twr_cleanup_result",
                true,
            ));
        } else {
            scope_body.push(RustStmt::Return(Some(RustExpr::MethodCall {
                receiver: Box::new(primary_result_then_cleanup_result_expr(
                    "__java_twr_result",
                    "__java_twr_cleanup_result",
                    true,
                )),
                method: "unwrap_or_else".to_string(),
                args: vec![RustExpr::Closure {
                    is_move: false,
                    return_type: None,
                    params: vec![RustClosureParam {
                        pattern: "ex".to_string(),
                        ty: None,
                    }],
                    body: RustClosureBody::Expr(Box::new(uncaught_java_exception_panic_expr(
                        RustExpr::Var("ex".to_string()),
                    ))),
                }],
            })));
        }
    } else if outer_returns_result {
        scope_body.push(propagate_primary_result_then_cleanup_result_stmt(
            "__java_twr_result",
            "__java_twr_cleanup_result",
            false,
        ));
    } else {
        scope_body.push(panic_if_primary_result_then_cleanup_result_err_stmt(
            "__java_twr_result",
            "__java_twr_cleanup_result",
            false,
        ));
    }
    RustStmt::Expr(RustExpr::Block {
        stmts: scope_body,
        tail: None,
    })
}

fn try_resource_accumulating_cleanup_body(resource_cleanup_stmts: Vec<RustStmt>) -> Vec<RustStmt> {
    let mut body = vec![RustStmt::Let {
        mutable: true,
        name: "__java_twr_cleanup_error".to_string(),
        ty: Some(RustType::Option(Box::new(RustType::Named(
            "JavaException".to_string(),
        )))),
        value: Some(RustExpr::Var("None".to_string())),
    }];
    for stmt in resource_cleanup_stmts.into_iter().rev() {
        if let Some(result_expr) = try_resource_cleanup_result_expr(&stmt) {
            body.push(RustStmt::Expr(RustExpr::Match {
                expr: Box::new(result_expr),
                arms: vec![
                    RustMatchArm {
                        pattern: "Ok(())".to_string(),
                        body: RustExpr::Block {
                            stmts: Vec::new(),
                            tail: None,
                        },
                    },
                    RustMatchArm {
                        pattern: "Err(ex)".to_string(),
                        body: RustExpr::Block {
                            stmts: vec![RustStmt::IfLet {
                                pattern: "Some(primary)".to_string(),
                                expr: RustExpr::MethodCall {
                                    receiver: Box::new(RustExpr::Var(
                                        "__java_twr_cleanup_error".to_string(),
                                    )),
                                    method: "as_mut".to_string(),
                                    args: Vec::new(),
                                },
                                then_body: vec![RustStmt::Expr(RustExpr::MethodCall {
                                    receiver: Box::new(RustExpr::Var("primary".to_string())),
                                    method: "add_suppressed".to_string(),
                                    args: vec![RustExpr::Var("ex".to_string())],
                                })],
                                else_body: vec![RustStmt::Assign {
                                    target: RustExpr::Var("__java_twr_cleanup_error".to_string()),
                                    value: some_expr(RustExpr::Var("ex".to_string())),
                                }],
                            }],
                            tail: None,
                        },
                    },
                ],
            }));
        } else {
            body.push(stmt);
        }
    }
    body.push(RustStmt::Return(Some(RustExpr::Match {
        expr: Box::new(RustExpr::Var("__java_twr_cleanup_error".to_string())),
        arms: vec![
            RustMatchArm {
                pattern: "Some(ex)".to_string(),
                body: RustExpr::Call {
                    function: Box::new(RustExpr::path(["Err"])),
                    args: vec![RustExpr::Var("ex".to_string())],
                },
            },
            RustMatchArm {
                pattern: "None".to_string(),
                body: ok_expr(unit_expr()),
            },
        ],
    })));
    body
}

fn try_resource_cleanup_result_expr(stmt: &RustStmt) -> Option<RustExpr> {
    match stmt {
        RustStmt::Expr(expr) => match expr.unshaped() {
            RustExpr::Try(inner) => Some((**inner).clone()),
            _ => None,
        },
        _ => None,
    }
}

pub fn try_finally_scope_stmt(
    ok_type: RustType,
    body: Vec<RustStmt>,
    finally_body: Vec<RustStmt>,
    body_terminates_with_value: bool,
    outer_returns_result: bool,
) -> RustStmt {
    let mut body_with_ok = body;
    if !body_terminates_with_value {
        body_with_ok.push(return_stmt(None, true));
    }
    let mut scope_body = Vec::new();
    scope_body.push(result_scope_call_let_stmt(
        "__java_try_result",
        ok_type,
        body_with_ok,
    ));
    scope_body.extend(finally_body);
    if rust_stmts_definitely_terminate(&scope_body) {
        return RustStmt::Expr(RustExpr::Block {
            stmts: scope_body,
            tail: None,
        });
    }
    if body_terminates_with_value {
        if outer_returns_result {
            scope_body.push(return_result_var_stmt("__java_try_result"));
        } else {
            scope_body.push(return_result_or_panic_stmt("__java_try_result"));
        }
    } else if outer_returns_result {
        scope_body.push(propagate_result_stmt("__java_try_result"));
    } else {
        scope_body.push(panic_if_result_err_stmt("__java_try_result"));
    }
    RustStmt::Expr(RustExpr::Block {
        stmts: scope_body,
        tail: None,
    })
}

pub fn try_catch_finally_scope_stmt(
    ok_type: RustType,
    try_body: Vec<RustStmt>,
    catches: Vec<CatchRenderArm>,
    finally_body: Vec<RustStmt>,
    body_terminates_with_value: bool,
    outer_returns_result: bool,
    ok_falls_through: bool,
) -> RustStmt {
    let result_type = exception_result_type(ok_type);
    let mut body_with_ok = try_body;
    if !body_terminates_with_value {
        body_with_ok.push(return_stmt(None, true));
    }

    let mut match_arms = Vec::new();
    match_arms.push(RustMatchArm {
        pattern: if ok_falls_through {
            "Ok(())".to_string()
        } else {
            "Ok(value)".to_string()
        },
        body: ok_expr(if ok_falls_through {
            unit_expr()
        } else {
            RustExpr::Var("value".to_string())
        }),
    });
    for catch in catches {
        let pattern = catch_guard_pattern(&catch);
        let mut catch_body = catch.body;
        if ok_falls_through {
            catch_body.push(return_stmt(None, true));
        }
        match_arms.push(RustMatchArm {
            pattern,
            body: result_scope_call_expr(result_type.clone(), catch_body),
        });
    }
    match_arms.push(RustMatchArm {
        pattern: "Err(ex)".to_string(),
        body: RustExpr::Call {
            function: Box::new(RustExpr::path(["Err"])),
            args: vec![RustExpr::Var("ex".to_string())],
        },
    });
    let mut scope_body = Vec::new();
    scope_body.push(RustStmt::Let {
        mutable: false,
        name: "__java_try_result".to_string(),
        ty: None,
        value: Some(RustExpr::Match {
            expr: Box::new(result_scope_call_expr(result_type, body_with_ok)),
            arms: match_arms,
        }),
    });
    scope_body.extend(finally_body);
    if rust_stmts_definitely_terminate(&scope_body) {
        return RustStmt::Expr(RustExpr::Block {
            stmts: scope_body,
            tail: None,
        });
    }
    let post_finally_stmt = if body_terminates_with_value {
        if outer_returns_result {
            return_result_var_stmt("__java_try_result")
        } else {
            return_result_or_panic_stmt("__java_try_result")
        }
    } else if outer_returns_result {
        propagate_result_stmt("__java_try_result")
    } else {
        panic_if_result_err_stmt("__java_try_result")
    };
    scope_body.push(post_finally_stmt);
    RustStmt::Expr(RustExpr::Block {
        stmts: scope_body,
        tail: None,
    })
}

fn rust_stmts_definitely_terminate(stmts: &[RustStmt]) -> bool {
    matches!(
        stmts.last(),
        Some(RustStmt::Return(_) | RustStmt::Break(_) | RustStmt::Continue(_))
    )
}

pub fn new_exception_expr(class_name: &str, message: RustExpr) -> RustExpr {
    java_exception_new_projection(class_name, exception_message_expr(message))
}

pub fn new_exception_expr_with_cause(
    class_name: &str,
    message: RustExpr,
    cause: RustExpr,
) -> RustExpr {
    java_exception_with_cause_projection(class_name, exception_message_expr(message), cause)
}

fn java_exception_new_projection(class_name: &str, message: RustExpr) -> RustExpr {
    function_call_projection(
        RustExpr::path(["JavaException", "new"]),
        vec![RustExpr::Literal(format!("{class_name:?}")), message],
    )
}

fn java_exception_with_cause_projection(
    class_name: &str,
    message: RustExpr,
    cause: RustExpr,
) -> RustExpr {
    function_call_projection(
        RustExpr::path(["JavaException", "with_cause"]),
        vec![RustExpr::Literal(format!("{class_name:?}")), message, cause],
    )
}

fn exception_message_expr(message: RustExpr) -> RustExpr {
    receiver_method_call_projection_exact(
        parenthesize_raw_postfix_receiver(message),
        "to_string",
        Vec::new(),
    )
}

pub(crate) fn fallback_exception_expr(value: RustExpr) -> RustExpr {
    java_exception_new_projection("JavaException", fallback_exception_message_expr(value))
}

fn fallback_exception_message_expr(value: RustExpr) -> RustExpr {
    macro_call_projection(
        "format",
        vec![RustExpr::Literal("\"{:?}\"".to_string()), value],
    )
}

pub fn exception_cause_or_self_if_instance_expr(
    wrapper: RustExpr,
    class_names: &[String],
) -> RustExpr {
    let class_names = class_names
        .iter()
        .map(|name| RustExpr::Literal(format!("{name:?}")))
        .collect();
    RustExpr::MethodCall {
        receiver: Box::new(parenthesize_raw_postfix_receiver(wrapper)),
        method: "cause_or_self_if_instance".to_string(),
        args: vec![RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(RustExpr::ArrayLiteral(class_names)),
        }],
    }
}

pub fn exception_expr_from_new(class_name: &str, args: Vec<RustExpr>) -> RustExpr {
    exception_expr_from_new_with_cause_hint(class_name, args, false)
}

pub fn exception_expr_from_new_with_cause_hint(
    class_name: &str,
    args: Vec<RustExpr>,
    second_arg_is_cause: bool,
) -> RustExpr {
    let mut args = args.into_iter();
    let message = args.next().unwrap_or_else(|| RustExpr::Call {
        function: Box::new(RustExpr::path(["String", "new"])),
        args: Vec::new(),
    });
    if second_arg_is_cause && let Some(cause) = args.next() {
        new_exception_expr_with_cause(class_name, message, cause)
    } else if args.len() > 0 {
        new_exception_expr(
            class_name,
            combined_exception_message(message, args.collect()),
        )
    } else {
        new_exception_expr(class_name, message)
    }
}

fn combined_exception_message(first: RustExpr, rest: Vec<RustExpr>) -> RustExpr {
    let mut args = Vec::with_capacity(rest.len() + 2);
    let placeholders = std::iter::repeat_n("{}", rest.len() + 1)
        .collect::<Vec<_>>()
        .join(": ");
    args.push(RustExpr::Literal(format!("{placeholders:?}")));
    args.push(first);
    args.extend(rest);
    macro_call_projection("format", args)
}

fn ok_expr(value: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["Ok"])),
        args: vec![value],
    }
}

fn some_expr(value: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["Some"])),
        args: vec![value],
    }
}

fn ok_none_expr() -> RustExpr {
    ok_expr(RustExpr::path(["None"]))
}

fn unit_expr() -> RustExpr {
    RustExpr::Tuple(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::facts::{ExceptionFact, ExceptionKind, FactProvenance, TypeCapabilityOverride};
    use crate::java_ir::JavaType;

    #[test]
    fn plain_return_catch_rewrites_returned_err_to_panic() {
        let rewritten = plain_return_catch_body(vec![RustStmt::Return(Some(RustExpr::Call {
            function: Box::new(RustExpr::path(["Err"])),
            args: vec![RustExpr::Call {
                function: Box::new(RustExpr::path(["JavaException", "new"])),
                args: vec![
                    RustExpr::Literal("\"WrappedProblem\"".to_string()),
                    RustExpr::Raw("e.to_string()".to_string()),
                ],
            }],
        }))]);
        assert!(matches!(
            rewritten.as_slice(),
            [RustStmt::Expr(RustExpr::MacroCall { macro_name, args })]
                if macro_name == "panic"
                    && matches!(args.as_slice(), [RustExpr::Literal(message), RustExpr::Call { .. }]
                        if message == "\"uncaught Java exception: {:?}\"")
        ));

        let rendered = try_catch_stmt(
            RustType::Raw("i32".to_string()),
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::path(["Ok"])),
                args: vec![RustExpr::Raw("1".to_string())],
            }))],
            vec![CatchRenderArm {
                variable_name: "e".to_string(),
                class_names: vec!["IOException".to_string()],
                body: vec![RustStmt::Return(Some(RustExpr::Call {
                    function: Box::new(RustExpr::path(["Err"])),
                    args: vec![RustExpr::Raw(
                        "JavaException::new(\"WrappedProblem\", e.to_string())".to_string(),
                    )],
                }))],
            }],
            false,
            false,
        );
        assert!(matches!(rendered, RustStmt::Expr(RustExpr::Match { .. })));
        let rendered = crate::emit::emit_statements(&[rendered], 0);
        assert!(
            rendered.contains(
                "panic!(\"uncaught Java exception: {:?}\", JavaException::new(\"WrappedProblem\", e.to_string()));"
            ) && !rendered.contains("return Err(JavaException::new(\"WrappedProblem\""),
            "{rendered}"
        );
    }

    #[test]
    fn java_exception_support_includes_source_exception_parent_edges() {
        let mut facts = FactSet::default();
        facts.exception_facts.push(ExceptionFact {
            type_name: "BaseProblem".to_string(),
            kind: ExceptionKind::Checked,
            provenance: FactProvenance::ParsedSource,
        });
        facts.exception_facts.push(ExceptionFact {
            type_name: "SpecificProblem".to_string(),
            kind: ExceptionKind::Checked,
            provenance: FactProvenance::ParsedSource,
        });
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: "BaseProblem".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: Some(JavaType::Named {
                    name: "Exception".to_string(),
                    args: Vec::new(),
                }),
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::ParsedSource,
            });
        facts
            .type_capability_overrides
            .push(TypeCapabilityOverride {
                type_name: "SpecificProblem".to_string(),
                copy: None,
                clone: None,
                string_like: None,
                sequence_like: None,
                implements: Vec::new(),
                extends: Some(JavaType::Named {
                    name: "BaseProblem".to_string(),
                    args: Vec::new(),
                }),
                final_type: None,
                object_safe_trait: None,
                provenance: FactProvenance::ParsedSource,
            });

        let RustItem::Raw(rendered) = java_exception_support_item_with_facts(&facts) else {
            panic!("Java exception support should render as a raw support item");
        };

        assert!(
            rendered.contains("\"BaseProblem\" => Some(\"Exception\")")
                && rendered.contains("\"SpecificProblem\" => Some(\"BaseProblem\")"),
            "{rendered}"
        );
    }

    #[test]
    fn try_with_resources_cleanup_accumulates_multiple_close_failures() {
        let stmt = try_with_resources_scope_stmt(
            RustType::Unit,
            Vec::new(),
            Vec::new(),
            vec![
                RustStmt::Expr(RustExpr::shaped(
                    crate::rust_ir::RustExprShape::ResultPropagated,
                    RustExpr::Try(Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("a".to_string())),
                        method: "close".to_string(),
                        args: Vec::new(),
                    })),
                )),
                RustStmt::Expr(RustExpr::shaped(
                    crate::rust_ir::RustExprShape::ResultPropagated,
                    RustExpr::Try(Box::new(RustExpr::MethodCall {
                        receiver: Box::new(RustExpr::Var("b".to_string())),
                        method: "close".to_string(),
                        args: Vec::new(),
                    })),
                )),
            ],
            false,
            true,
        );
        let rendered = crate::emit::emit_statements(&[stmt], 0);

        assert!(
            rendered.contains("let mut __java_twr_cleanup_error: Option<JavaException> = None;"),
            "{rendered}"
        );
        assert!(
            rendered.contains("match b.close()")
                && rendered.contains("match a.close()")
                && !rendered.contains("b.close()?")
                && !rendered.contains("a.close()?"),
            "{rendered}"
        );
        assert!(
            rendered.contains("primary.add_suppressed(ex);")
                && rendered.contains("__java_twr_cleanup_error = Some(ex);"),
            "{rendered}"
        );
    }

    #[test]
    fn terminating_finally_body_suppresses_post_finally_try_result_use() {
        let stmt = try_finally_scope_stmt(
            RustType::String,
            vec![RustStmt::Return(Some(ok_expr(RustExpr::Literal(
                "\"try\".to_string()".to_string(),
            ))))],
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::path(["Err"])),
                args: vec![new_exception_expr(
                    "RuntimeException",
                    RustExpr::Literal("\"finally\".to_string()".to_string()),
                )],
            }))],
            true,
            true,
        );
        let rendered = crate::emit::emit_statements(&[stmt], 0);

        assert!(rendered.contains("let __java_try_result ="), "{rendered}");
        assert!(
            rendered.contains("return Err(JavaException::new(\"RuntimeException\""),
            "{rendered}"
        );
        assert!(
            !rendered.contains("return __java_try_result;")
                && !rendered.contains("__java_try_result?;"),
            "{rendered}"
        );
    }

    #[test]
    fn terminating_catch_finally_body_suppresses_post_finally_try_result_use() {
        let stmt = try_catch_finally_scope_stmt(
            RustType::String,
            vec![RustStmt::Return(Some(ok_expr(RustExpr::Literal(
                "\"try\".to_string()".to_string(),
            ))))],
            vec![CatchRenderArm {
                variable_name: "ex".to_string(),
                class_names: vec!["RuntimeException".to_string()],
                body: vec![RustStmt::Return(Some(ok_expr(RustExpr::Literal(
                    "\"catch\".to_string()".to_string(),
                ))))],
            }],
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::path(["Err"])),
                args: vec![new_exception_expr(
                    "RuntimeException",
                    RustExpr::Literal("\"finally\".to_string()".to_string()),
                )],
            }))],
            true,
            true,
            false,
        );
        let rendered = crate::emit::emit_statements(&[stmt], 0);

        assert!(rendered.contains("let __java_try_result ="), "{rendered}");
        assert!(
            rendered.contains("return Err(JavaException::new(\"RuntimeException\""),
            "{rendered}"
        );
        assert!(
            !rendered.contains("return __java_try_result;"),
            "{rendered}"
        );
    }

    #[test]
    fn terminating_try_with_resources_finally_body_suppresses_post_finally_twr_result_use() {
        let stmt = try_with_resources_finally_scope_stmt(
            RustType::String,
            Vec::new(),
            vec![RustStmt::Return(Some(ok_expr(RustExpr::Literal(
                "\"try\".to_string()".to_string(),
            ))))],
            vec![RustStmt::Expr(RustExpr::shaped(
                crate::rust_ir::RustExprShape::ResultPropagated,
                RustExpr::Try(Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("resource".to_string())),
                    method: "close".to_string(),
                    args: Vec::new(),
                })),
            ))],
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::path(["Err"])),
                args: vec![new_exception_expr(
                    "RuntimeException",
                    RustExpr::Literal("\"finally\".to_string()".to_string()),
                )],
            }))],
            true,
            true,
        );
        let rendered = crate::emit::emit_statements(&[stmt], 0);

        assert!(rendered.contains("let __java_twr_result ="), "{rendered}");
        assert!(
            rendered.contains("let __java_twr_cleanup_result ="),
            "{rendered}"
        );
        assert!(
            rendered.contains("return Err(JavaException::new(\"RuntimeException\""),
            "{rendered}"
        );
        assert!(
            !rendered.contains("return match (__java_twr_result, __java_twr_cleanup_result)")
                && !rendered.contains("__java_twr_result, __java_twr_cleanup_result"),
            "{rendered}"
        );
    }

    #[test]
    fn fallback_exception_message_keeps_structured_value() {
        fn grouped_plus_binary(left_name: &str, right_literal: &str) -> RustExpr {
            RustExpr::Grouped(Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var(left_name.to_string())),
                right: Box::new(RustExpr::Literal(right_literal.to_string())),
            }))
        }
        fn grouped_plus_vars(left_name: &str, right_name: &str) -> RustExpr {
            RustExpr::Grouped(Box::new(RustExpr::Binary {
                op: "+".to_string(),
                left: Box::new(RustExpr::Var(left_name.to_string())),
                right: Box::new(RustExpr::Var(right_name.to_string())),
            }))
        }

        let message = exception_message_expr(RustExpr::Raw("value + 1".to_string()));
        assert_eq!(crate::emit::emit_expr(&message), "(value + 1).to_string()");
        assert_eq!(
            message,
            RustExpr::MethodCall {
                receiver: Box::new(grouped_plus_binary("value", "1")),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );
        let method_receiver = exception_message_expr(RustExpr::Raw("source.value()".to_string()));
        assert_eq!(
            crate::emit::emit_expr(&method_receiver),
            "source.value().to_string()"
        );
        assert_eq!(
            method_receiver,
            RustExpr::MethodCall {
                receiver: Box::new(RustExpr::MethodCall {
                    receiver: Box::new(RustExpr::Var("source".to_string())),
                    method: "value".to_string(),
                    args: Vec::new(),
                }),
                method: "to_string".to_string(),
                args: Vec::new(),
            }
        );

        let structured = fallback_exception_message_expr(RustExpr::Var("value".to_string()));
        assert_eq!(
            crate::emit::emit_expr(&structured),
            "format!(\"{:?}\", value)"
        );
        assert_eq!(
            structured,
            RustExpr::MacroCall {
                macro_name: "format".to_string(),
                args: vec![
                    RustExpr::Literal("\"{:?}\"".to_string()),
                    RustExpr::Var("value".to_string())
                ],
            }
        );

        let raw = fallback_exception_message_expr(RustExpr::Raw("value + 1".to_string()));
        assert_eq!(crate::emit::emit_expr(&raw), "format!(\"{:?}\", value + 1)");

        assert_eq!(
            fallback_exception_expr(RustExpr::Var("err".to_string())),
            RustExpr::Call {
                function: Box::new(RustExpr::path(["JavaException", "new"])),
                args: vec![
                    RustExpr::Literal("\"JavaException\"".to_string()),
                    RustExpr::MacroCall {
                        macro_name: "format".to_string(),
                        args: vec![
                            RustExpr::Literal("\"{:?}\"".to_string()),
                            RustExpr::Var("err".to_string()),
                        ],
                    },
                ],
            }
        );

        let uncaught = throw_stmt_with_result_policy(
            new_exception_expr(
                "IllegalArgumentException",
                RustExpr::Literal("\"bad\".to_string()".to_string()),
            ),
            false,
        );
        assert_eq!(
            uncaught,
            RustStmt::Expr(RustExpr::MacroCall {
                macro_name: "panic".to_string(),
                args: vec![
                    RustExpr::Literal("\"uncaught Java exception: {:?}\"".to_string()),
                    RustExpr::Call {
                        function: Box::new(RustExpr::path(["JavaException", "new"])),
                        args: vec![
                            RustExpr::Literal("\"IllegalArgumentException\"".to_string()),
                            RustExpr::MethodCall {
                                receiver: Box::new(RustExpr::Literal(
                                    "\"bad\".to_string()".to_string()
                                )),
                                method: "to_string".to_string(),
                                args: Vec::new(),
                            },
                        ],
                    },
                ],
            })
        );
        assert_eq!(
            crate::emit::emit_statements(&[uncaught], 0),
            "panic!(\"uncaught Java exception: {:?}\", JavaException::new(\"IllegalArgumentException\", \"bad\".to_string().to_string()));\n"
        );

        let cause_or_self = exception_cause_or_self_if_instance_expr(
            RustExpr::Raw("left + right".to_string()),
            &["PrimaryProblem".to_string(), "SecondaryProblem".to_string()],
        );
        assert_eq!(
            crate::emit::emit_expr(&cause_or_self),
            "(left + right).cause_or_self_if_instance(&[\"PrimaryProblem\", \"SecondaryProblem\"])"
        );
        assert_eq!(
            cause_or_self,
            RustExpr::MethodCall {
                receiver: Box::new(grouped_plus_vars("left", "right")),
                method: "cause_or_self_if_instance".to_string(),
                args: vec![RustExpr::Unary {
                    op: "&".to_string(),
                    expr: Box::new(RustExpr::ArrayLiteral(vec![
                        RustExpr::Literal("\"PrimaryProblem\"".to_string()),
                        RustExpr::Literal("\"SecondaryProblem\"".to_string()),
                    ])),
                }],
            }
        );
    }
}
