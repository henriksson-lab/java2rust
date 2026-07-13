use crate::rust_ir::{RustExpr, RustItem, RustStmt, RustType};

pub fn java_exception_support_item() -> RustItem {
    RustItem::Raw(
        r#"#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaException {
    pub class_name: &'static str,
    pub message: String,
}

impl JavaException {
    pub fn new(class_name: &'static str, message: String) -> JavaException {
        JavaException { class_name, message }
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

pub fn return_stmt(value: Option<RustExpr>, wrap_in_ok: bool) -> RustStmt {
    if wrap_in_ok {
        RustStmt::Return(Some(ok_expr(value.unwrap_or_else(unit_expr))))
    } else {
        RustStmt::Return(value)
    }
}

pub fn throw_stmt(exception: RustExpr) -> RustStmt {
    RustStmt::Return(Some(RustExpr::Call {
        function: Box::new(RustExpr::Raw("Err".to_string())),
        args: vec![exception],
    }))
}

pub fn new_exception_expr(class_name: &str, message: RustExpr) -> RustExpr {
    RustExpr::Call {
        function: Box::new(RustExpr::Raw("JavaException::new".to_string())),
        args: vec![RustExpr::Literal(format!("{class_name:?}")), message],
    }
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

pub fn exception_expr_from_new(class_name: &str, args: Vec<RustExpr>) -> RustExpr {
    new_exception_expr(
        class_name,
        args.into_iter()
            .next()
            .unwrap_or_else(empty_exception_message),
    )
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
