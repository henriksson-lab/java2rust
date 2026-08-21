use std::collections::BTreeSet;

use crate::coercion::{
    function_call_projection, macro_call_projection, raw_receiver_projection,
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

pub fn java_exception_support_item() -> RustItem {
    java_exception_support_item_with_facts(&FactSet::default())
}

pub fn java_exception_support_item_with_facts(facts: &FactSet) -> RustItem {
    let conversions = external_exception_conversions(facts);
    let mut item = r#"#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct InputStream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException {
    pub class_name: &'static str,
    pub message: String,
    pub cause: Option<Box<JavaException>>,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message, cause: None }
    }

    pub fn with_cause(
        class_name: &'static str,
        message: String,
        cause: JavaException,
    ) -> JavaException {
        JavaException { class_name, message, cause: Some(Box::new(cause)) }
    }

    pub fn from_display(class_name: &'static str, err: impl std::fmt::Display) -> JavaException {
        JavaException { class_name, message: err.to_string(), cause: None }
    }

    pub fn is_instance(&self, class_names: &[&str]) -> bool {
        class_names.iter().any(|name| self.class_name == *name)
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
    for conversion in conversions {
        item.push_str("\n\n");
        item.push_str(&format!(
            "impl From<{}> for JavaException {{\n    fn from(err: {}) -> JavaException {{\n        JavaException::from_display({:?}, err)\n    }}\n}}",
            conversion.rust_type, conversion.rust_type, conversion.java_type
        ));
    }
    RustItem::Raw(item)
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

pub fn legacy_java_exception_support_item() -> RustItem {
    RustItem::Raw(
        r#"#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct InputStream;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException {
    pub class_name: &'static str,
    pub message: String,
    pub cause: Option<Box<JavaException>>,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message, cause: None }
    }

    pub fn with_cause(
        class_name: &'static str,
        message: String,
        cause: JavaException,
    ) -> JavaException {
        JavaException { class_name, message, cause: Some(Box::new(cause)) }
    }

    pub fn is_instance(&self, class_names: &[&str]) -> bool {
        class_names.iter().any(|name| self.class_name == *name)
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
        .to_string(),
    )
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

pub fn throw_stmt_with_result_policy(exception: RustExpr, wrap_in_result: bool) -> RustStmt {
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
            stmts: Vec::new(),
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
    for cleanup_stmt in resource_cleanup_stmts.into_iter().rev() {
        scope_body.push(cleanup_stmt);
    }
    if body_terminates_with_value {
        if outer_returns_result {
            scope_body.push(return_result_var_stmt("__java_twr_result"));
        } else {
            scope_body.push(return_result_or_panic_stmt("__java_twr_result"));
        }
    } else if outer_returns_result {
        scope_body.push(propagate_result_stmt("__java_twr_result"));
    } else {
        scope_body.push(panic_if_result_err_stmt("__java_twr_result"));
    }
    RustStmt::Expr(RustExpr::Block {
        stmts: scope_body,
        tail: None,
    })
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
        parenthesize_raw_method_receiver(message),
        "to_string",
        Vec::new(),
    )
}

pub fn fallback_exception_expr(value: RustExpr) -> RustExpr {
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
        receiver: Box::new(parenthesize_raw_method_receiver(wrapper)),
        method: "cause_or_self_if_instance".to_string(),
        args: vec![RustExpr::Unary {
            op: "&".to_string(),
            expr: Box::new(RustExpr::ArrayLiteral(class_names)),
        }],
    }
}

fn parenthesize_raw_method_receiver(expr: RustExpr) -> RustExpr {
    raw_receiver_projection(expr, raw_needs_method_receiver_parentheses)
}

fn raw_needs_method_receiver_parentheses(raw: &str) -> bool {
    if raw.starts_with('(') && raw.ends_with(')') {
        return false;
    }
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut chars = raw.char_indices().peekable();
    while let Some((idx, ch)) = chars.next() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '"' | '\'' => {
                let quote = ch;
                let mut escaped = false;
                for (_, inner) in chars.by_ref() {
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == quote {
                        break;
                    }
                }
            }
            '+' | '-' | '*' | '/' | '%' | '^' | '&' | '|' | '=' | '<' | '>'
                if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 =>
            {
                return true;
            }
            '!' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && (idx == 0 || raw[idx + 1..].starts_with('=')) =>
            {
                return true;
            }
            'a' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && raw[idx..].starts_with("as")
                && raw[..idx].ends_with(char::is_whitespace)
                && raw[idx + 2..].starts_with(char::is_whitespace) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
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
    let message = args.next().unwrap_or_else(empty_exception_message);
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

pub fn empty_exception_message() -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::path(["String", "new"])),
        args: Vec::new(),
    }
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
