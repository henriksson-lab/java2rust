use std::collections::HashMap;

use crate::diagnostics::DiagnosticBag;
use crate::java_ir::{
    Block, CallKind, ConstructorDecl, Expr, JavaType, LambdaBody, Member, MethodDecl, Modifier,
    Param, Program, ResolvedCall, Stmt, TypeDecl,
};
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub program: Program,
    pub diagnostics: DiagnosticBag,
}

pub fn resolve_program(program: &Program, mut diagnostics: DiagnosticBag) -> ResolvedProgram {
    let mut program = program.clone();
    for ty in &mut program.types {
        resolve_type(ty, &mut diagnostics);
    }
    ResolvedProgram {
        program,
        diagnostics,
    }
}

fn resolve_type(ty: &mut TypeDecl, diagnostics: &mut DiagnosticBag) {
    let mut method_counts: HashMap<String, usize> = HashMap::new();
    let mut ctor_count = 0usize;
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                *method_counts.entry(method.name.clone()).or_default() += 1;
            }
            Member::Constructor(_) => ctor_count += 1,
            _ => {}
        }
    }

    for member in &mut ty.members {
        match member {
            Member::Method(method) => resolve_method(method, &method_counts),
            Member::Constructor(ctor) => resolve_constructor(ctor, ctor_count),
            Member::Type(nested) => resolve_type(nested, diagnostics),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }

    let methods = collect_method_symbols(ty);
    let fields = collect_field_env(ty);
    for member in &mut ty.members {
        match member {
            Member::Method(method) => {
                if let Some(body) = &mut method.body {
                    let mut env = fields.clone();
                    for param in &method.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(body, &methods, &mut env, diagnostics);
                }
            }
            Member::Constructor(ctor) => {
                if let Some(body) = &mut ctor.body {
                    let mut env = fields.clone();
                    for param in &ctor.params {
                        env.insert_local(param.name.clone(), param.ty.clone());
                    }
                    resolve_block_calls(body, &methods, &mut env, diagnostics);
                }
            }
            _ => {}
        }
    }
}

fn resolve_method(method: &mut MethodDecl, counts: &HashMap<String, usize>) {
    let base = crate::parser::to_snake_case(&method.name);
    if counts.get(&method.name).copied().unwrap_or(0) <= 1 {
        method.rust_name = Some(base);
        return;
    }

    let suffix = method
        .params
        .iter()
        .map(|p| crate::parser::type_suffix(&p.ty, p.is_varargs))
        .collect::<Vec<_>>()
        .join("_");
    method.rust_name = Some(format!("{base}_{suffix}"));
}

fn resolve_constructor(ctor: &mut ConstructorDecl, ctor_count: usize) {
    if ctor.params.is_empty() {
        ctor.rust_name = Some("new".to_string());
    } else if ctor_count == 1 {
        ctor.rust_name = Some("new".to_string());
    } else {
        let suffix = ctor
            .params
            .iter()
            .map(|p| crate::parser::type_suffix(&p.ty, p.is_varargs))
            .collect::<Vec<_>>()
            .join("_");
        ctor.rust_name = Some(format!("new_{suffix}"));
    }
}

#[derive(Debug, Clone)]
struct MethodSymbol {
    java_name: String,
    rust_name: String,
    params: Vec<Param>,
    return_type: JavaType,
    is_static: bool,
}

fn collect_method_symbols(ty: &TypeDecl) -> Vec<MethodSymbol> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) => Some(MethodSymbol {
                java_name: method.name.clone(),
                rust_name: method
                    .rust_name
                    .clone()
                    .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
                params: method.params.clone(),
                return_type: method.return_type.clone(),
                is_static: method.modifiers.contains(&Modifier::Static),
            }),
            _ => None,
        })
        .collect()
}

fn collect_field_env(ty: &TypeDecl) -> TypeEnv {
    let mut env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
        }
    }
    env
}

fn resolve_block_calls(
    block: &mut Block,
    methods: &[MethodSymbol],
    env: &mut TypeEnv,
    diagnostics: &mut DiagnosticBag,
) {
    for stmt in &mut block.statements {
        resolve_stmt_calls(stmt, methods, env, diagnostics);
        env.learn_stmt(stmt);
    }
}

fn resolve_stmt_calls(
    stmt: &mut Stmt,
    methods: &[MethodSymbol],
    env: &mut TypeEnv,
    diagnostics: &mut DiagnosticBag,
) {
    match stmt {
        Stmt::VarDecl { initializer, .. } => {
            if let Some(expr) = initializer {
                resolve_expr_calls(expr, methods, env, diagnostics);
            }
        }
        Stmt::Assign { target, value } => {
            resolve_expr_calls(target, methods, env, diagnostics);
            resolve_expr_calls(value, methods, env, diagnostics);
        }
        Stmt::Expr(expr) => resolve_expr_calls(expr, methods, env, diagnostics),
        Stmt::Return(Some(expr)) => resolve_expr_calls(expr, methods, env, diagnostics),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            resolve_expr_calls(condition, methods, env, diagnostics);
            resolve_block_calls(then_block, methods, &mut env.clone(), diagnostics);
            if let Some(block) = else_block {
                resolve_block_calls(block, methods, &mut env.clone(), diagnostics);
            }
        }
        Stmt::While { condition, body } => {
            resolve_expr_calls(condition, methods, env, diagnostics);
            resolve_block_calls(body, methods, &mut env.clone(), diagnostics);
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                resolve_stmt_calls(stmt, methods, env, diagnostics);
                env.learn_stmt(stmt);
            }
            if let Some(condition) = condition {
                resolve_expr_calls(condition, methods, env, diagnostics);
            }
            for expr in update {
                resolve_expr_calls(expr, methods, env, diagnostics);
            }
            resolve_block_calls(body, methods, &mut env.clone(), diagnostics);
        }
        Stmt::EnhancedFor {
            var_type,
            var_name,
            iterable,
            body,
        } => {
            resolve_expr_calls(iterable, methods, env, diagnostics);
            let mut body_env = env.clone();
            body_env.insert_local(var_name.clone(), var_type.clone());
            resolve_block_calls(body, methods, &mut body_env, diagnostics);
        }
        Stmt::Switch { selector, cases } => {
            resolve_expr_calls(selector, methods, env, diagnostics);
            for case in cases {
                for label in &mut case.labels {
                    if let crate::java_ir::SwitchLabel::Expr(expr) = label {
                        resolve_expr_calls(expr, methods, env, diagnostics);
                    }
                }
                let mut case_env = env.clone();
                for stmt in &mut case.statements {
                    resolve_stmt_calls(stmt, methods, &mut case_env, diagnostics);
                    case_env.learn_stmt(stmt);
                }
            }
        }
        Stmt::Try {
            body,
            catches,
            finally_block,
        } => {
            resolve_block_calls(body, methods, &mut env.clone(), diagnostics);
            for catch in catches {
                let mut catch_env = env.clone();
                for ty in &catch.types {
                    catch_env.insert_local(catch.name.clone(), ty.clone());
                }
                resolve_block_calls(&mut catch.body, methods, &mut catch_env, diagnostics);
            }
            if let Some(block) = finally_block {
                resolve_block_calls(block, methods, &mut env.clone(), diagnostics);
            }
        }
        Stmt::Throw(expr) => resolve_expr_calls(expr, methods, env, diagnostics),
        Stmt::ConstructorInvocation { args, .. } => {
            for arg in args {
                resolve_expr_calls(arg, methods, env, diagnostics);
            }
        }
        Stmt::Block(block) => resolve_block_calls(block, methods, &mut env.clone(), diagnostics),
        _ => {}
    }
}

fn resolve_expr_calls(
    expr: &mut Expr,
    methods: &[MethodSymbol],
    env: &TypeEnv,
    diagnostics: &mut DiagnosticBag,
) {
    match expr {
        Expr::FieldAccess(receiver, _) => resolve_expr_calls(receiver, methods, env, diagnostics),
        Expr::ArrayAccess(array, index) => {
            resolve_expr_calls(array, methods, env, diagnostics);
            resolve_expr_calls(index, methods, env, diagnostics);
        }
        Expr::Binary { left, right, .. } => {
            resolve_expr_calls(left, methods, env, diagnostics);
            resolve_expr_calls(right, methods, env, diagnostics);
        }
        Expr::Unary { expr, .. } => resolve_expr_calls(expr, methods, env, diagnostics),
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            resolve_expr_calls(condition, methods, env, diagnostics);
            resolve_expr_calls(then_expr, methods, env, diagnostics);
            resolve_expr_calls(else_expr, methods, env, diagnostics);
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            if let Some(receiver) = receiver {
                resolve_expr_calls(receiver, methods, env, diagnostics);
            }
            for arg in args.iter_mut() {
                resolve_expr_calls(arg, methods, env, diagnostics);
            }

            if receiver.is_none()
                || receiver
                    .as_deref()
                    .is_some_and(|expr| matches!(expr, Expr::This))
            {
                let arg_types = args
                    .iter()
                    .map(|arg| infer_expr_type(arg, env))
                    .collect::<Vec<_>>();
                match resolve_same_class_call(name, &arg_types, methods) {
                    Ok(Some(call)) => *resolved = Some(call),
                    Ok(None) => {}
                    Err(message) => diagnostics.error("resolve_call_ambiguous", message),
                }
            }
        }
        Expr::New { args, body, .. } => {
            for arg in args {
                resolve_expr_calls(arg, methods, env, diagnostics);
            }
            if let Some(members) = body {
                for member in members {
                    match member {
                        Member::Method(method) => {
                            if let Some(body) = &mut method.body {
                                resolve_block_calls(body, methods, &mut env.clone(), diagnostics);
                            }
                        }
                        Member::Constructor(ctor) => {
                            if let Some(body) = &mut ctor.body {
                                resolve_block_calls(body, methods, &mut env.clone(), diagnostics);
                            }
                        }
                        Member::Type(ty) => resolve_type(ty, diagnostics),
                        _ => {}
                    }
                }
            }
        }
        Expr::Cast { expr, .. } => resolve_expr_calls(expr, methods, env, diagnostics),
        Expr::InstanceOf { expr, .. } => resolve_expr_calls(expr, methods, env, diagnostics),
        Expr::Lambda { body, .. } => match body {
            LambdaBody::Expr(expr) => resolve_expr_calls(expr, methods, env, diagnostics),
            LambdaBody::Block(block) => {
                resolve_block_calls(block, methods, &mut env.clone(), diagnostics)
            }
        },
        _ => {}
    }
}

fn resolve_same_class_call(
    name: &str,
    arg_types: &[JavaType],
    methods: &[MethodSymbol],
) -> Result<Option<ResolvedCall>, String> {
    let candidates = methods
        .iter()
        .filter(|method| method.java_name == name && method.params.len() == arg_types.len())
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }

    let exact = candidates
        .iter()
        .copied()
        .filter(|method| {
            method
                .params
                .iter()
                .zip(arg_types)
                .all(|(param, arg_ty)| arg_ty == &param.ty)
        })
        .collect::<Vec<_>>();
    let assignable = candidates
        .iter()
        .copied()
        .filter(|method| {
            method
                .params
                .iter()
                .zip(arg_types)
                .all(|(param, arg_ty)| java_type_assignable(arg_ty, &param.ty))
        })
        .collect::<Vec<_>>();

    let selected = match exact.as_slice() {
        [one] => *one,
        [] if assignable.len() == 1 => assignable[0],
        [] if assignable.is_empty() && candidates.len() == 1 => candidates[0],
        [] => {
            return Err(format!(
                "no overload of {name} matches argument types {:?}",
                arg_types
            ));
        }
        _ => {
            return Err(format!(
                "ambiguous overload of {name} for argument types {:?}",
                arg_types
            ));
        }
    };

    Ok(Some(ResolvedCall {
        rust_name: selected.rust_name.clone(),
        kind: if selected.is_static {
            CallKind::Static
        } else {
            CallKind::Instance
        },
        return_type: selected.return_type.clone(),
    }))
}

fn java_type_assignable(actual: &JavaType, target: &JavaType) -> bool {
    if actual == target {
        return true;
    }
    match (actual, target) {
        (JavaType::Primitive(actual), JavaType::Primitive(target)) => {
            primitive_rank(*actual) <= primitive_rank(*target)
        }
        (JavaType::String, JavaType::Named { name, .. }) if name == "Object" => true,
        (JavaType::Unknown(name), _) if name == "null" => !matches!(target, JavaType::Primitive(_)),
        _ => false,
    }
}

fn primitive_rank(primitive: crate::java_ir::PrimitiveType) -> u8 {
    use crate::java_ir::PrimitiveType::*;
    match primitive {
        Boolean => 0,
        Byte => 1,
        Short | Char => 2,
        Int => 3,
        Long => 4,
        Float => 5,
        Double => 6,
    }
}
