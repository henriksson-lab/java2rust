use crate::rust_ir::{RustExpr, RustStmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedConstructorLowering {
    ThisWithTrailingStatements,
    UnresolvedDelegation,
    SuperRequiresInheritanceLowering,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedStatementLowering {
    TryCatchFinally,
    LocalTypeDeclaration,
    ConstructorDelegation,
    ParserUnsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedExpressionLowering {
    Lambda,
    MethodReference,
    ClassLiteral,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedLoweringMessagePolicy {
    pub message: String,
}

impl UnsupportedLoweringMessagePolicy {
    pub fn compile_error_stmt(&self) -> RustStmt {
        RustStmt::Raw(format!("compile_error!({:?});", self.message))
    }

    pub fn compile_error_expr(&self) -> RustExpr {
        RustExpr::Raw(format!("compile_error!({:?})", self.message))
    }
}

pub fn unsupported_constructor_stmt(kind: UnsupportedConstructorLowering) -> RustStmt {
    unsupported_constructor_message_policy(kind).compile_error_stmt()
}

pub fn unsupported_statement_stmt(kind: UnsupportedStatementLowering) -> RustStmt {
    unsupported_statement_message_policy(kind).compile_error_stmt()
}

pub fn unsupported_expression_expr(kind: UnsupportedExpressionLowering) -> RustExpr {
    unsupported_expression_message_policy(kind).compile_error_expr()
}

pub fn unsupported_constructor_message_policy(
    kind: UnsupportedConstructorLowering,
) -> UnsupportedLoweringMessagePolicy {
    UnsupportedLoweringMessagePolicy {
        message: constructor_message(kind),
    }
}

pub fn unsupported_statement_message_policy(
    kind: UnsupportedStatementLowering,
) -> UnsupportedLoweringMessagePolicy {
    UnsupportedLoweringMessagePolicy {
        message: statement_message(kind),
    }
}

pub fn unsupported_expression_message_policy(
    kind: UnsupportedExpressionLowering,
) -> UnsupportedLoweringMessagePolicy {
    UnsupportedLoweringMessagePolicy {
        message: expression_message(kind),
    }
}

fn constructor_message(kind: UnsupportedConstructorLowering) -> String {
    match kind {
        UnsupportedConstructorLowering::ThisWithTrailingStatements => {
            "unsupported Java constructor: this(...) followed by additional statements".to_string()
        }
        UnsupportedConstructorLowering::UnresolvedDelegation => {
            "unresolved Java constructor delegation".to_string()
        }
        UnsupportedConstructorLowering::SuperRequiresInheritanceLowering => {
            "unsupported Java constructor: super(...) requires inheritance lowering".to_string()
        }
    }
}

fn statement_message(kind: UnsupportedStatementLowering) -> String {
    match kind {
        UnsupportedStatementLowering::TryCatchFinally => {
            "unsupported Java statement: try/catch/finally".to_string()
        }
        UnsupportedStatementLowering::LocalTypeDeclaration => {
            "unsupported Java statement: local type declaration".to_string()
        }
        UnsupportedStatementLowering::ConstructorDelegation => {
            "unsupported Java statement: constructor delegation".to_string()
        }
        UnsupportedStatementLowering::ParserUnsupported(reason) => {
            format!("unsupported Java statement: {reason}")
        }
    }
}

fn expression_message(kind: UnsupportedExpressionLowering) -> String {
    match kind {
        UnsupportedExpressionLowering::Lambda => "unsupported Java expression: lambda".to_string(),
        UnsupportedExpressionLowering::MethodReference => {
            "unsupported Java expression: method reference".to_string()
        }
        UnsupportedExpressionLowering::ClassLiteral => {
            "unsupported Java expression: class literal".to_string()
        }
    }
}
