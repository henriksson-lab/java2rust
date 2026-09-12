use crate::java_ir::{Expr, Member, Stmt};
use crate::rust_ir::{RustExpr, RustStmt};

pub(crate) fn compile_error_stmt(message: &str) -> RustStmt {
    RustStmt::Expr(compile_error_expr(message))
}

pub(crate) fn compile_error_expr(message: &str) -> RustExpr {
    RustExpr::MacroCall {
        macro_name: "compile_error".to_string(),
        args: vec![RustExpr::Literal(format!("{message:?}"))],
    }
}

/// A `compile_error!` at item position, for a refusal that has no expression or
/// statement to attach to.
pub(crate) fn compile_error_item_text(message: &str) -> String {
    format!("compile_error!({message:?});")
}

/// A `compile_error!` followed by a value of the expected type.
///
/// `compile_error!` already aborts the build, but the placeholder still has to
/// type-check for the surrounding item to parse, so the refusal is paired with
/// a value. Used for external constants that have a declared type and no
/// modelled value.
pub(crate) fn compile_error_value_text(message: &str, value: &str) -> String {
    format!("{{ compile_error!({message:?}); {value} }}")
}

/// A `compile_error!` paired with a diverging expression.
///
/// `!` coerces to any type, so this works where the placeholder's type is not
/// known well enough to name a value — naming the type as a value works only
/// for a unit struct and fails with `E0423` as soon as the type resolves
/// through an alias.
pub(crate) fn compile_error_diverging_text(message: &str) -> String {
    compile_error_value_text(message, &format!("panic!({message:?})"))
}

pub(crate) fn compile_error_expr_consuming(expr: &RustExpr, message: &str) -> RustExpr {
    RustExpr::Block {
        stmts: vec![RustStmt::Let {
            mutable: false,
            name: "_".to_string(),
            ty: None,
            value: Some(expr.clone()),
        }],
        tail: Some(Box::new(compile_error_expr(message))),
    }
}

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
    ParserUnsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsupportedMemberLowering {
    ParserUnsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedLoweringMessagePolicy {
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedDiagnosticPolicy {
    code: &'static str,
    message: String,
}

impl UnsupportedDiagnosticPolicy {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl UnsupportedLoweringMessagePolicy {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn compile_error_stmt(&self) -> RustStmt {
        compile_error_stmt(&self.message)
    }

    pub fn compile_error_expr(&self) -> RustExpr {
        compile_error_expr(&self.message)
    }
}

pub(crate) fn unsupported_constructor_body(kind: UnsupportedConstructorLowering) -> Vec<RustStmt> {
    let policy = unsupported_constructor_message_policy(kind);
    vec![
        policy.compile_error_stmt(),
        RustStmt::Return(Some(RustExpr::MacroCall {
            macro_name: "panic".to_string(),
            args: vec![RustExpr::Literal(format!("{:?}", policy.message()))],
        })),
    ]
}

pub(crate) fn unsupported_statement_stmt(kind: UnsupportedStatementLowering) -> RustStmt {
    unsupported_statement_message_policy(kind).compile_error_stmt()
}

pub(crate) fn unsupported_try_catch_finally_stmt() -> RustStmt {
    unsupported_statement_stmt(UnsupportedStatementLowering::TryCatchFinally)
}

pub(crate) fn unsupported_expression_expr(kind: UnsupportedExpressionLowering) -> RustExpr {
    unsupported_expression_message_policy(kind).compile_error_expr()
}

pub(crate) fn plan_unsupported_expression(expr: &Expr) -> Option<UnsupportedExpressionLowering> {
    match expr {
        Expr::Lambda { .. } => Some(UnsupportedExpressionLowering::Lambda),
        Expr::MethodReference { .. } => Some(UnsupportedExpressionLowering::MethodReference),
        Expr::Unsupported { reason, .. } => Some(UnsupportedExpressionLowering::ParserUnsupported(
            reason.clone(),
        )),
        _ => None,
    }
}

pub(crate) fn plan_unsupported_statement(stmt: &Stmt) -> Option<UnsupportedStatementLowering> {
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
                    | crate::exception_plan::TryStmtPlan::WithResourcesFinally { .. }
                    | crate::exception_plan::TryStmtPlan::WithResourcesCatchFinally { .. }
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

pub(crate) fn plan_unsupported_member(member: &Member) -> Option<UnsupportedMemberLowering> {
    match member {
        Member::Unsupported { reason, .. } => {
            Some(UnsupportedMemberLowering::ParserUnsupported(reason.clone()))
        }
        _ => None,
    }
}

pub(crate) fn unsupported_constructor_message_policy(
    kind: UnsupportedConstructorLowering,
) -> UnsupportedLoweringMessagePolicy {
    UnsupportedLoweringMessagePolicy::new(constructor_message(kind))
}

pub(crate) fn unsupported_statement_message_policy(
    kind: UnsupportedStatementLowering,
) -> UnsupportedLoweringMessagePolicy {
    UnsupportedLoweringMessagePolicy::new(statement_message(kind))
}

pub(crate) fn unsupported_expression_message_policy(
    kind: UnsupportedExpressionLowering,
) -> UnsupportedLoweringMessagePolicy {
    UnsupportedLoweringMessagePolicy::new(expression_message(kind))
}

pub(crate) fn unsupported_member_diagnostic_policy(
    kind: UnsupportedMemberLowering,
) -> UnsupportedDiagnosticPolicy {
    UnsupportedDiagnosticPolicy::new("unsupported_member", member_message(kind))
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

#[cfg(test)]
#[test]
fn unsupported_lowering_policy_owns_compile_error_placeholders() {
    let statement_policy =
        unsupported_statement_message_policy(UnsupportedStatementLowering::LocalTypeDeclaration);
    assert_eq!(
        statement_policy.message(),
        "unsupported Java statement: local type declaration"
    );
    assert_eq!(
        statement_policy.compile_error_stmt(),
        RustStmt::Expr(RustExpr::MacroCall {
            macro_name: "compile_error".to_string(),
            args: vec![RustExpr::Literal(
                "\"unsupported Java statement: local type declaration\"".to_string()
            )],
        })
    );

    let constructor_policy = unsupported_constructor_message_policy(
        UnsupportedConstructorLowering::SuperRequiresInheritanceLowering,
    );
    assert_eq!(
        constructor_policy.message(),
        "unsupported Java constructor: super(...) requires inheritance lowering"
    );
    assert_eq!(
        unsupported_constructor_body(
            UnsupportedConstructorLowering::SuperRequiresInheritanceLowering
        ),
        vec![
            RustStmt::Expr(RustExpr::MacroCall {
                macro_name: "compile_error".to_string(),
                args: vec![RustExpr::Literal(
                    "\"unsupported Java constructor: super(...) requires inheritance lowering\""
                        .to_string()
                )],
            }),
            RustStmt::Return(Some(RustExpr::MacroCall {
                macro_name: "panic".to_string(),
                args: vec![RustExpr::Literal(
                    "\"unsupported Java constructor: super(...) requires inheritance lowering\""
                        .to_string()
                )],
            })),
        ]
    );

    assert_eq!(
        unsupported_expression_message_policy(UnsupportedExpressionLowering::MethodReference)
            .message(),
        "unsupported Java expression: method reference"
    );
    let unsupported_member_policy = unsupported_member_diagnostic_policy(
        UnsupportedMemberLowering::ParserUnsupported("record component".to_string()),
    );
    assert_eq!(unsupported_member_policy.code(), "unsupported_member");
    assert_eq!(unsupported_member_policy.message(), "record component");
    assert_eq!(
        unsupported_statement_stmt(UnsupportedStatementLowering::LocalTypeDeclaration),
        RustStmt::Expr(RustExpr::MacroCall {
            macro_name: "compile_error".to_string(),
            args: vec![RustExpr::Literal(
                "\"unsupported Java statement: local type declaration\"".to_string()
            )],
        })
    );
    assert_eq!(
        unsupported_expression_expr(UnsupportedExpressionLowering::MethodReference),
        compile_error_expr("unsupported Java expression: method reference")
    );
    assert_eq!(
        unsupported_expression_expr(UnsupportedExpressionLowering::ParserUnsupported(
            "unsupported expression node: weird_\"quoted\"_\\_node".to_string()
        )),
        compile_error_expr(
            "unsupported Java expression: unsupported expression node: weird_\"quoted\"_\\_node"
        )
    );
    let message = "unsupported \"quoted\" Java path \\pkg\\Type with {braces} and \"#";
    assert_eq!(
        crate::emit::emit_expr(&compile_error_expr_consuming(
            &RustExpr::Var("value".to_string()),
            message
        )),
        format!("{{ let _ = value; compile_error!({message:?}) }}")
    );
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
        UnsupportedExpressionLowering::Lambda => {
            crate::output_markers::UNSUPPORTED_JAVA_LAMBDA.to_string()
        }
        UnsupportedExpressionLowering::MethodReference => {
            crate::output_markers::UNSUPPORTED_JAVA_METHOD_REFERENCE.to_string()
        }
        UnsupportedExpressionLowering::ParserUnsupported(reason) => {
            format!("unsupported Java expression: {reason}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_ir::{ConstructorInvocationKind, TypeDecl, TypeKind};
    use crate::parser::parse_expr;

    #[test]
    fn unsupported_planners_classify_only_unsupported_nodes() {
        assert_eq!(
            plan_unsupported_expression(&parse_expr("String::valueOf")),
            Some(UnsupportedExpressionLowering::MethodReference)
        );
        assert_eq!(
            plan_unsupported_expression(&parse_expr("String.class")),
            None
        );
        assert_eq!(plan_unsupported_expression(&parse_expr("value")), None);

        assert_eq!(
            plan_unsupported_statement(&Stmt::Try {
                resources: Vec::new(),
                body: Default::default(),
                catches: Vec::new(),
                finally_block: None,
            }),
            Some(UnsupportedStatementLowering::TryCatchFinally)
        );
        assert_eq!(
            plan_unsupported_statement(&Stmt::ConstructorInvocation {
                kind: ConstructorInvocationKind::This,
                args: Vec::new(),
                resolved: None,
            }),
            Some(UnsupportedStatementLowering::ConstructorDelegation)
        );
        assert_eq!(
            plan_unsupported_statement(&Stmt::Unsupported("raw parser node".to_string())),
            Some(UnsupportedStatementLowering::ParserUnsupported(
                "raw parser node".to_string()
            ))
        );
        assert_eq!(plan_unsupported_statement(&Stmt::Break(None)), None);

        assert_eq!(
            plan_unsupported_member(&Member::Unsupported {
                reason: "record component".to_string(),
                span: Default::default(),
            }),
            Some(UnsupportedMemberLowering::ParserUnsupported(
                "record component".to_string()
            ))
        );
        assert_eq!(
            plan_unsupported_member(&Member::Type(TypeDecl {
                static_init_blocks: Vec::new(),
                kind: TypeKind::Class,
                name: "Nested".to_string(),
                type_params: Vec::new(),
                type_param_bounds: Vec::new(),
                modifiers: Vec::new(),
                extends: None,
                implements: Vec::new(),
                permits: Vec::new(),
                members: Vec::new(),
                span: Default::default(),
            })),
            None
        );
    }
}
