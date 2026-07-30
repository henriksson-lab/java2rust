use crate::conversion_plan::ConversionDiagnostic;
use crate::diagnostics::Severity;
use crate::facts::FactSet;
use crate::java_ir::{
    BinaryOp, Block, CatchClause, Expr, JavaType, LambdaBody, Member, Program, Stmt, SwitchCase,
    TypeDecl,
};
use crate::representation::RepresentationPlan;
use crate::rewrite::{MethodControlFlowPlan, control_flow_rewrite_diagnostics};
use crate::signature::SignaturePlan;
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringDiagnosticPlan {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

pub fn lowering_diagnostic_from_conversion_diagnostic(
    diagnostic: ConversionDiagnostic,
) -> LoweringDiagnosticPlan {
    LoweringDiagnosticPlan {
        severity: Severity::Unsupported,
        code: diagnostic.code,
        message: diagnostic.message,
    }
}

pub fn collect_lowering_diagnostic_plans(
    program: &Program,
    facts: &FactSet,
    representations: &RepresentationPlan,
    signatures: &SignaturePlan,
    control_flow_plans: &[MethodControlFlowPlan],
) -> Vec<LoweringDiagnosticPlan> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(
        facts
            .diagnostics()
            .into_iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Warning,
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    diagnostics.extend(
        representations
            .diagnostics_for_program(program)
            .into_iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Unsupported,
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    diagnostics.extend(
        signatures
            .diagnostics()
            .iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Warning,
                code: diagnostic.code,
                message: diagnostic.message.clone(),
            }),
    );
    diagnostics.extend(
        control_flow_rewrite_diagnostics(control_flow_plans)
            .into_iter()
            .map(|diagnostic| LoweringDiagnosticPlan {
                severity: Severity::Unsupported,
                code: diagnostic.code,
                message: diagnostic.message,
            }),
    );
    diagnostics.extend(collect_java_reference_identity_equality_diagnostics(
        program,
    ));
    diagnostics
}

fn collect_java_reference_identity_equality_diagnostics(
    program: &Program,
) -> Vec<LoweringDiagnosticPlan> {
    let mut diagnostics = Vec::new();
    for ty in &program.types {
        collect_type_reference_identity_equality_diagnostics(ty, &mut diagnostics);
    }
    diagnostics
}

fn collect_type_reference_identity_equality_diagnostics(
    ty: &TypeDecl,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    let mut field_env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            field_env.insert_field(field.name.clone(), field.ty.clone());
            field_env.insert_type_field(ty.name.clone(), field.name.clone(), field.ty.clone());
        }
    }
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &method.body {
                    let mut env = field_env.clone();
                    for param in &method.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    collect_block_reference_identity_equality_diagnostics(
                        body,
                        &mut env,
                        diagnostics,
                    );
                }
            }
            Member::Constructor(constructor) => {
                if let Some(body) = &constructor.body {
                    let mut env = field_env.clone();
                    for param in &constructor.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    collect_block_reference_identity_equality_diagnostics(
                        body,
                        &mut env,
                        diagnostics,
                    );
                }
            }
            Member::Field(field) => {
                if let Some(initializer) = &field.initializer {
                    collect_expr_reference_identity_equality_diagnostics(
                        initializer,
                        &field_env,
                        diagnostics,
                    );
                }
            }
            Member::Type(nested) => {
                collect_type_reference_identity_equality_diagnostics(nested, diagnostics)
            }
            Member::Unsupported { .. } => {}
        }
    }
}

fn collect_block_reference_identity_equality_diagnostics(
    block: &Block,
    env: &mut TypeEnv,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    for stmt in &block.statements {
        collect_stmt_reference_identity_equality_diagnostics(stmt, env, diagnostics);
    }
}

fn collect_stmt_reference_identity_equality_diagnostics(
    stmt: &Stmt,
    env: &mut TypeEnv,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
        } => {
            if let Some(initializer) = initializer {
                collect_expr_reference_identity_equality_diagnostics(initializer, env, diagnostics);
            }
            env.insert_local(name.clone(), ty.clone());
        }
        Stmt::Assign { target, value } => {
            collect_expr_reference_identity_equality_diagnostics(target, env, diagnostics);
            collect_expr_reference_identity_equality_diagnostics(value, env, diagnostics);
        }
        Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
            collect_expr_reference_identity_equality_diagnostics(expr, env, diagnostics);
        }
        Stmt::Return(None) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            collect_expr_reference_identity_equality_diagnostics(condition, env, diagnostics);
            collect_block_reference_identity_equality_diagnostics(
                then_block,
                &mut env.clone(),
                diagnostics,
            );
            if let Some(else_block) = else_block {
                collect_block_reference_identity_equality_diagnostics(
                    else_block,
                    &mut env.clone(),
                    diagnostics,
                );
            }
        }
        Stmt::While {
            condition, body, ..
        }
        | Stmt::DoWhile {
            condition, body, ..
        } => {
            collect_expr_reference_identity_equality_diagnostics(condition, env, diagnostics);
            collect_block_reference_identity_equality_diagnostics(
                body,
                &mut env.clone(),
                diagnostics,
            );
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            let mut loop_env = env.clone();
            for init_stmt in init {
                collect_stmt_reference_identity_equality_diagnostics(
                    init_stmt,
                    &mut loop_env,
                    diagnostics,
                );
            }
            if let Some(condition) = condition {
                collect_expr_reference_identity_equality_diagnostics(
                    condition,
                    &loop_env,
                    diagnostics,
                );
            }
            for update_expr in update {
                collect_expr_reference_identity_equality_diagnostics(
                    update_expr,
                    &loop_env,
                    diagnostics,
                );
            }
            collect_block_reference_identity_equality_diagnostics(body, &mut loop_env, diagnostics);
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
            ..
        } => {
            collect_expr_reference_identity_equality_diagnostics(iterable, env, diagnostics);
            let mut body_env = env.clone();
            body_env.insert_local(var_name.clone(), var_type.clone());
            collect_block_reference_identity_equality_diagnostics(body, &mut body_env, diagnostics);
        }
        Stmt::Switch { selector, cases } => {
            collect_expr_reference_identity_equality_diagnostics(selector, env, diagnostics);
            for case in cases {
                collect_switch_case_reference_identity_equality_diagnostics(case, env, diagnostics);
            }
        }
        Stmt::Try {
            resources,
            body,
            catches,
            finally_block,
        } => {
            let mut try_env = env.clone();
            for resource in resources {
                collect_stmt_reference_identity_equality_diagnostics(
                    resource,
                    &mut try_env,
                    diagnostics,
                );
            }
            collect_block_reference_identity_equality_diagnostics(body, &mut try_env, diagnostics);
            for catch in catches {
                collect_catch_reference_identity_equality_diagnostics(catch, env, diagnostics);
            }
            if let Some(finally_block) = finally_block {
                collect_block_reference_identity_equality_diagnostics(
                    finally_block,
                    &mut env.clone(),
                    diagnostics,
                );
            }
        }
        Stmt::LocalType(ty) => {
            collect_type_reference_identity_equality_diagnostics(ty, diagnostics);
        }
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                collect_expr_reference_identity_equality_diagnostics(arg, env, diagnostics);
            }
        }
        Stmt::Block(block) | Stmt::LabeledBlock { body: block, .. } => {
            collect_block_reference_identity_equality_diagnostics(
                block,
                &mut env.clone(),
                diagnostics,
            );
        }
    }
}

fn collect_switch_case_reference_identity_equality_diagnostics(
    case: &SwitchCase,
    env: &TypeEnv,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    let mut case_env = env.clone();
    for label in &case.labels {
        if let crate::java_ir::SwitchLabel::Expr(expr) = label {
            collect_expr_reference_identity_equality_diagnostics(expr, &case_env, diagnostics);
        }
    }
    for stmt in &case.statements {
        collect_stmt_reference_identity_equality_diagnostics(stmt, &mut case_env, diagnostics);
    }
}

fn collect_catch_reference_identity_equality_diagnostics(
    catch: &CatchClause,
    env: &TypeEnv,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    let mut catch_env = env.clone();
    let catch_ty = catch
        .types
        .first()
        .cloned()
        .unwrap_or_else(|| JavaType::Unknown("Throwable".to_string()));
    catch_env.insert_local(catch.name.clone(), catch_ty);
    collect_block_reference_identity_equality_diagnostics(&catch.body, &mut catch_env, diagnostics);
}

fn collect_expr_reference_identity_equality_diagnostics(
    expr: &Expr,
    env: &TypeEnv,
    diagnostics: &mut Vec<LoweringDiagnosticPlan>,
) {
    if let Expr::Binary { op, left, right } = expr
        && matches!(op, BinaryOp::Eq | BinaryOp::Ne)
        && !expr_is_null_literal(left)
        && !expr_is_null_literal(right)
    {
        let left_ty = infer_expr_type(left, env);
        let right_ty = infer_expr_type(right, env);
        if let Some(code) = reference_identity_equality_diagnostic_code(&left_ty, &right_ty) {
            diagnostics.push(LoweringDiagnosticPlan {
                severity: Severity::Warning,
                code,
                message: "Java ==/!= on reference type is currently lowered as Rust equality; object identity is not represented yet".to_string(),
            });
        }
    }
    match expr {
        Expr::FieldAccess(receiver, _) => {
            collect_expr_reference_identity_equality_diagnostics(receiver, env, diagnostics)
        }
        Expr::ArrayAccess(array, index) => {
            collect_expr_reference_identity_equality_diagnostics(array, env, diagnostics);
            collect_expr_reference_identity_equality_diagnostics(index, env, diagnostics);
        }
        Expr::ArrayCreation {
            dimensions,
            initializer,
            ..
        } => {
            for dimension in dimensions {
                collect_expr_reference_identity_equality_diagnostics(dimension, env, diagnostics);
            }
            if let Some(initializer) = initializer {
                for item in initializer {
                    collect_expr_reference_identity_equality_diagnostics(item, env, diagnostics);
                }
            }
        }
        Expr::ArrayInitializer(items) => {
            for item in items {
                collect_expr_reference_identity_equality_diagnostics(item, env, diagnostics);
            }
        }
        Expr::Binary { left, right, .. } => {
            collect_expr_reference_identity_equality_diagnostics(left, env, diagnostics);
            collect_expr_reference_identity_equality_diagnostics(right, env, diagnostics);
        }
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_expr_reference_identity_equality_diagnostics(expr, env, diagnostics)
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_reference_identity_equality_diagnostics(condition, env, diagnostics);
            collect_expr_reference_identity_equality_diagnostics(then_expr, env, diagnostics);
            collect_expr_reference_identity_equality_diagnostics(else_expr, env, diagnostics);
        }
        Expr::Switch { selector, cases } => {
            collect_expr_reference_identity_equality_diagnostics(selector, env, diagnostics);
            for case in cases {
                collect_expr_reference_identity_equality_diagnostics(&case.value, env, diagnostics);
            }
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_expr_reference_identity_equality_diagnostics(receiver, env, diagnostics);
            }
            for arg in args {
                collect_expr_reference_identity_equality_diagnostics(arg, env, diagnostics);
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                collect_expr_reference_identity_equality_diagnostics(arg, env, diagnostics);
            }
            if let Some(body) = body {
                for member in body {
                    if let Member::Type(ty) = member {
                        collect_type_reference_identity_equality_diagnostics(ty, diagnostics);
                    }
                }
            }
        }
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => {
                collect_expr_reference_identity_equality_diagnostics(expr, env, diagnostics)
            }
            LambdaBody::Block(block) => collect_block_reference_identity_equality_diagnostics(
                block,
                &mut env.clone(),
                diagnostics,
            ),
        },
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Unsupported { .. }
        | Expr::Raw(_) => {}
    }
}

fn reference_identity_equality_diagnostic_code(
    left: &JavaType,
    right: &JavaType,
) -> Option<&'static str> {
    if left == &JavaType::String || right == &JavaType::String {
        return Some("java_string_identity");
    }
    if java_type_has_reference_identity(left) || java_type_has_reference_identity(right) {
        return Some("java_reference_identity_equality");
    }
    None
}

fn java_type_has_reference_identity(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Named { .. } | JavaType::Array(_))
}

fn expr_is_null_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(crate::java_ir::Literal::Null))
}
