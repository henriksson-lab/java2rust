use crate::java_ir::{Expr, Member, Stmt};
use crate::rust_ir::{RustExpr, RustStmt};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedConstructorLowering {
    ThisWithTrailingStatements,
    UnresolvedDelegation,
    SuperRequiresInheritanceLowering,
    SuperWithUnsupportedTrailingStatements,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedMemberLowering {
    ParserUnsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedLoweringMessagePolicy {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedDiagnosticPolicy {
    pub code: &'static str,
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

pub fn unsupported_constructor_body(kind: UnsupportedConstructorLowering) -> Vec<RustStmt> {
    let policy = unsupported_constructor_message_policy(kind);
    vec![
        policy.compile_error_stmt(),
        RustStmt::Return(Some(RustExpr::Raw(format!("panic!({:?})", policy.message)))),
    ]
}

pub fn unsupported_statement_stmt(kind: UnsupportedStatementLowering) -> RustStmt {
    unsupported_statement_message_policy(kind).compile_error_stmt()
}

pub fn unsupported_try_catch_finally_stmt() -> RustStmt {
    unsupported_statement_stmt(UnsupportedStatementLowering::TryCatchFinally)
}

pub fn unsupported_expression_expr(kind: UnsupportedExpressionLowering) -> RustExpr {
    unsupported_expression_message_policy(kind).compile_error_expr()
}

pub fn plan_unsupported_expression(expr: &Expr) -> Option<UnsupportedExpressionLowering> {
    match expr {
        Expr::Lambda { .. } => Some(UnsupportedExpressionLowering::Lambda),
        Expr::MethodReference { .. } => Some(UnsupportedExpressionLowering::MethodReference),
        _ => None,
    }
}

pub fn plan_unsupported_statement(stmt: &Stmt) -> Option<UnsupportedStatementLowering> {
    match stmt {
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            if matches!(
                crate::exception_plan::plan_try_stmt(
                    resources,
                    body,
                    catches,
                    finally_block.as_ref()
                ),
                crate::exception_plan::TryStmtPlan::CatchOnly { .. }
                    | crate::exception_plan::TryStmtPlan::WithResources { .. }
                    | crate::exception_plan::TryStmtPlan::FinallyOnly { .. }
                    | crate::exception_plan::TryStmtPlan::CatchFinally { .. }
            ) {
                None
            } else {
                Some(UnsupportedStatementLowering::TryCatchFinally)
            }
        }
        Stmt::LocalType(_) => Some(UnsupportedStatementLowering::LocalTypeDeclaration),
        Stmt::ConstructorInvocation { .. } => {
            Some(UnsupportedStatementLowering::ConstructorDelegation)
        }
        Stmt::Unsupported(reason) => Some(UnsupportedStatementLowering::ParserUnsupported(
            reason.clone(),
        )),
        _ => None,
    }
}

pub fn plan_unsupported_member(member: &Member) -> Option<UnsupportedMemberLowering> {
    match member {
        Member::Unsupported { reason, .. } => {
            Some(UnsupportedMemberLowering::ParserUnsupported(reason.clone()))
        }
        _ => None,
    }
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

pub fn unsupported_member_diagnostic_policy(
    kind: UnsupportedMemberLowering,
) -> UnsupportedDiagnosticPolicy {
    UnsupportedDiagnosticPolicy {
        code: "unsupported_member",
        message: member_message(kind),
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
        UnsupportedConstructorLowering::SuperWithUnsupportedTrailingStatements => {
            "unsupported Java constructor: super(...) followed by unsupported object graph mutation"
                .to_string()
        }
    }
}

fn member_message(kind: UnsupportedMemberLowering) -> String {
    match kind {
        UnsupportedMemberLowering::ParserUnsupported(reason) => reason,
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
    }
}
