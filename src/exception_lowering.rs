use std::collections::BTreeSet;

use crate::facts::{ExternalLibraryMode, FactSet};
use crate::rust_ir::{RustExpr, RustItem, RustStmt, RustType};
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
    let mut item = r#"#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn get_message(&self) -> String {
        self.message.clone()
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
        r#"#[derive(Debug, Clone, PartialEq, Eq)]
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

    pub fn get_message(&self) -> String {
        self.message.clone()
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
    RustType::Raw(format!(
        "Result<{}, JavaException>",
        crate::emit::emit_type(&ok_type)
    ))
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
        return RustStmt::Raw(format!(
            "panic!(\"uncaught Java exception: {{:?}}\", {});",
            crate::emit::emit_expr(&exception)
        ));
    }
    RustStmt::Return(Some(RustExpr::Call {
        function: Box::new(RustExpr::Raw("Err".to_string())),
        args: vec![exception],
    }))
}

pub fn try_catch_stmt(
    ok_type: RustType,
    try_body: Vec<RustStmt>,
    catches: Vec<CatchRenderArm>,
    outer_returns_result: bool,
    ok_falls_through: bool,
) -> RustStmt {
    let mut out = String::new();
    out.push_str("match (|| -> ");
    out.push_str(&crate::emit::emit_type(&exception_result_type(ok_type)));
    out.push_str(" {\n");
    out.push_str(&crate::emit::emit_statements(&try_body, 1));
    out.push_str("})() {\n");
    if ok_falls_through {
        out.push_str("    Ok(()) => {}\n");
    } else if outer_returns_result {
        out.push_str("    Ok(value) => return Ok(value),\n");
    } else {
        out.push_str("    Ok(value) => return value,\n");
    }
    for catch in catches {
        let classes = catch
            .class_names
            .iter()
            .map(|name| format!("{name:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "    Err({}) if {}.is_instance(&[{}]) => {{\n",
            catch.variable_name, catch.variable_name, classes
        ));
        if outer_returns_result {
            out.push_str(&crate::emit::emit_statements(&catch.body, 2));
        } else {
            out.push_str(&crate::emit::emit_statements(
                &plain_return_catch_body(catch.body),
                2,
            ));
        }
        out.push_str("    }\n");
    }
    if outer_returns_result {
        out.push_str("    Err(ex) => return Err(ex),\n");
    } else {
        out.push_str("    Err(ex) => panic!(\"uncaught Java exception: {:?}\", ex),\n");
    }
    out.push('}');
    RustStmt::Raw(out)
}

fn plain_return_catch_body(body: Vec<RustStmt>) -> Vec<RustStmt> {
    body.into_iter()
        .map(|stmt| match stmt {
            RustStmt::Return(Some(RustExpr::Call { function, mut args }))
                if matches!(&*function, RustExpr::Raw(name) if name == "Err")
                    && args.len() == 1 =>
            {
                let exception = args.remove(0);
                RustStmt::Raw(format!(
                    "panic!(\"uncaught Java exception: {{:?}}\", {});",
                    crate::emit::emit_expr(&exception)
                ))
            }
            other => other,
        })
        .collect()
}

pub fn try_with_resources_scope_stmt(
    ok_type: RustType,
    resource_initializers: Vec<RustStmt>,
    body: Vec<RustStmt>,
    resource_cleanup_stmts: Vec<RustStmt>,
    body_terminates_with_value: bool,
) -> RustStmt {
    let mut scope_body = resource_initializers;
    let mut body_with_ok = body;
    if !body_terminates_with_value {
        body_with_ok.push(return_stmt(None, true));
    }
    scope_body.push(RustStmt::Raw(format!(
        "let __java_twr_result = (|| -> {} {{\n{}}})();",
        crate::emit::emit_type(&exception_result_type(ok_type)),
        crate::emit::emit_statements(&body_with_ok, 1)
    )));
    for cleanup_stmt in resource_cleanup_stmts.into_iter().rev() {
        scope_body.push(cleanup_stmt);
    }
    if body_terminates_with_value {
        scope_body.push(RustStmt::Raw("return __java_twr_result;".to_string()));
    } else {
        scope_body.push(RustStmt::Raw("__java_twr_result?;".to_string()));
    }
    RustStmt::Raw(format!(
        "{{\n{}\n}}",
        crate::emit::emit_statements(&scope_body, 1)
    ))
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
    scope_body.push(RustStmt::Raw(format!(
        "let __java_try_result = (|| -> {} {{\n{}}})();",
        crate::emit::emit_type(&exception_result_type(ok_type)),
        crate::emit::emit_statements(&body_with_ok, 1)
    )));
    scope_body.extend(finally_body);
    if body_terminates_with_value {
        if outer_returns_result {
            scope_body.push(RustStmt::Raw("return __java_try_result;".to_string()));
        } else {
            scope_body.push(RustStmt::Raw("return __java_try_result.unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex));".to_string()));
        }
    } else if outer_returns_result {
        scope_body.push(RustStmt::Raw("__java_try_result?;".to_string()));
    } else {
        scope_body.push(RustStmt::Raw(
            "if let Err(ex) = __java_try_result { panic!(\"uncaught Java exception: {:?}\", ex); }"
                .to_string(),
        ));
    }
    RustStmt::Raw(format!(
        "{{\n{}\n}}",
        crate::emit::emit_statements(&scope_body, 1)
    ))
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
    let rendered_result_type = crate::emit::emit_type(&result_type);
    let mut body_with_ok = try_body;
    if !body_terminates_with_value {
        body_with_ok.push(return_stmt(None, true));
    }

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("    let __java_try_result = match (|| -> ");
    out.push_str(&rendered_result_type);
    out.push_str(" {\n");
    out.push_str(&crate::emit::emit_statements(&body_with_ok, 2));
    out.push_str("    })() {\n");
    if ok_falls_through {
        out.push_str("        Ok(()) => Ok(()),\n");
    } else {
        out.push_str("        Ok(value) => Ok(value),\n");
    }
    for catch in catches {
        let classes = catch
            .class_names
            .iter()
            .map(|name| format!("{name:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut catch_body = catch.body;
        if ok_falls_through {
            catch_body.push(return_stmt(None, true));
        }
        out.push_str(&format!(
            "        Err({}) if {}.is_instance(&[{}]) => (|| -> {} {{\n",
            catch.variable_name, catch.variable_name, classes, rendered_result_type
        ));
        out.push_str(&crate::emit::emit_statements(&catch_body, 3));
        out.push_str("        })(),\n");
    }
    out.push_str("        Err(ex) => Err(ex),\n");
    out.push_str("    };\n");
    out.push_str(&crate::emit::emit_statements(&finally_body, 1));
    if body_terminates_with_value {
        if outer_returns_result {
            out.push_str("    return __java_try_result;\n");
        } else {
            out.push_str("    return __java_try_result.unwrap_or_else(|ex| panic!(\"uncaught Java exception: {:?}\", ex));\n");
        }
    } else if outer_returns_result {
        out.push_str("    __java_try_result?;\n");
    } else {
        out.push_str(
            "    if let Err(ex) = __java_try_result { panic!(\"uncaught Java exception: {:?}\", ex); }\n",
        );
    }
    out.push('}');
    RustStmt::Raw(out)
}

pub fn new_exception_expr(class_name: &str, message: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
        args: vec![
            RustExpr::Literal(format!("{class_name:?}")),
            exception_message_expr(message),
        ],
    }
}

pub fn new_exception_expr_with_cause(
    class_name: &str,
    message: RustExpr,
    cause: RustExpr,
) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("JavaException::with_cause".to_string())),
        args: vec![
            RustExpr::Literal(format!("{class_name:?}")),
            exception_message_expr(message),
            cause,
        ],
    }
}

fn exception_message_expr(message: RustExpr) -> RustExpr {
    RustExpr::Raw(format!("{}.to_string()", crate::emit::emit_expr(&message)))
}

pub fn fallback_exception_expr(value: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
        args: vec![
            RustExpr::Literal("\"JavaException\"".to_string()),
            RustExpr::Raw(format!(
                "format!(\"{{:?}}\", {})",
                crate::emit::emit_expr(&value)
            )),
        ],
    }
}

pub fn exception_cause_or_self_if_instance_expr(
    wrapper: RustExpr,
    class_names: &[String],
) -> RustExpr {
    let class_names = class_names
        .iter()
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    RustExpr::Raw(format!(
        "{}.cause_or_self_if_instance(&[{}])",
        crate::emit::emit_expr(&wrapper),
        class_names
    ))
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
    let mut parts = Vec::with_capacity(rest.len() + 1);
    parts.push(first);
    parts.extend(rest);
    let placeholders = std::iter::repeat_n("{}", parts.len())
        .collect::<Vec<_>>()
        .join(": ");
    let args = parts
        .iter()
        .map(crate::emit::emit_expr)
        .collect::<Vec<_>>()
        .join(", ");
    RustExpr::Raw(format!("format!(\"{placeholders}\", {args})"))
}

pub fn empty_exception_message() -> RustExpr {
    RustExpr::Raw("String::new()".to_string())
}

fn ok_expr(value: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("Ok".to_string())),
        args: vec![value],
    }
}

fn unit_expr() -> RustExpr {
    RustExpr::Raw("()".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_return_catch_rewrites_returned_err_to_panic() {
        let rendered = try_catch_stmt(
            RustType::Raw("i32".to_string()),
            vec![RustStmt::Return(Some(RustExpr::Call {
                function: Box::new(RustExpr::Raw("Ok".to_string())),
                args: vec![RustExpr::Raw("1".to_string())],
            }))],
            vec![CatchRenderArm {
                variable_name: "e".to_string(),
                class_names: vec!["IOException".to_string()],
                body: vec![RustStmt::Return(Some(RustExpr::Call {
                    function: Box::new(RustExpr::Raw("Err".to_string())),
                    args: vec![RustExpr::Raw(
                        "JavaException::new(\"RuntimeException\", e.to_string())".to_string(),
                    )],
                }))],
            }],
            false,
            false,
        );
        let RustStmt::Raw(rendered) = rendered else {
            panic!("expected raw try/catch rendering");
        };
        assert!(
            rendered.contains(
                "panic!(\"uncaught Java exception: {:?}\", JavaException::new(\"RuntimeException\", e.to_string()));"
            ) && !rendered.contains("return Err(JavaException::new(\"RuntimeException\""),
            "{rendered}"
        );
    }
}
