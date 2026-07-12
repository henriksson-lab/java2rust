use crate::diagnostics::DiagnosticBag;
use crate::java_ir::*;
use crate::rust_ir::*;
use crate::types::{
    TypeEnv, default_rust_literal_for_java_type, infer_expr_type, java_type_to_rust,
    rust_return_type,
};

#[derive(Debug, Clone)]
pub struct LoweredProgram {
    pub file: RustFile,
    pub diagnostics: DiagnosticBag,
}

pub fn lower_program(program: &Program, mut diagnostics: DiagnosticBag) -> LoweredProgram {
    let mut file = RustFile::default();
    for ty in &program.types {
        lower_type(ty, &mut file, &mut diagnostics);
    }
    LoweredProgram { file, diagnostics }
}

fn lower_type(ty: &TypeDecl, file: &mut RustFile, diagnostics: &mut DiagnosticBag) {
    if ty.extends.is_some() {
        diagnostics.unsupported(
            "unsupported_inheritance",
            format!(
                "{} uses inheritance, which is outside the first slice",
                ty.name
            ),
        );
    }

    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(RustField {
                name: field.name.clone(),
                ty: crate::coercion::rust_storage_type(&field.ty),
                is_pub: false,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    file.items.push(RustItem::Struct(RustStruct {
        name: ty.name.clone(),
        fields,
    }));

    let mut functions = Vec::new();
    for member in &ty.members {
        match member {
            Member::Constructor(ctor) => functions.push(lower_constructor(ty, ctor)),
            Member::Method(method) => functions.push(lower_method(method)),
            Member::Unsupported { reason, .. } => {
                diagnostics.unsupported("unsupported_member", reason.clone());
            }
            _ => {}
        }
    }
    file.items.push(RustItem::Impl(RustImpl {
        target: ty.name.clone(),
        functions,
    }));
}

fn lower_constructor(ty: &TypeDecl, ctor: &ConstructorDecl) -> RustFunction {
    let mut env = env_for_type(ty);
    for param in &ctor.params {
        env.insert_local(param.name.clone(), param.ty.clone());
    }
    let mut assigned = Vec::new();
    if let Some(body) = &ctor.body {
        for stmt in &body.statements {
            if let Stmt::Assign { target, value } = stmt {
                if let Expr::FieldAccess(receiver, field) = target {
                    if matches!(receiver.as_ref(), Expr::This) {
                        let target_ty = env
                            .field_type(field)
                            .cloned()
                            .unwrap_or(JavaType::Unknown(field.clone()));
                        assigned.push((
                            field.clone(),
                            lower_expr_for_slot(
                                value,
                                &env,
                                crate::coercion::Slot::Assignment(target_ty),
                            ),
                        ));
                    }
                }
            }
            env.learn_stmt(stmt);
        }
    }

    let fields = ty
        .members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => {
                let value = assigned
                    .iter()
                    .find(|(name, _)| name == &field.name)
                    .map(|(_, value)| value.clone())
                    .or_else(|| {
                        field.initializer.as_ref().map(|expr| {
                            lower_expr_for_slot(
                                expr,
                                &env,
                                crate::coercion::Slot::Assignment(field.ty.clone()),
                            )
                        })
                    })
                    .unwrap_or_else(|| default_expr_for_type(&field.ty));
                Some((field.name.clone(), value))
            }
            _ => None,
        })
        .collect();

    RustFunction {
        name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
        params: ctor
            .params
            .iter()
            .map(|p| RustParam {
                name: p.name.clone(),
                ty: crate::coercion::rust_param_slot_type(&p.ty, p.is_varargs),
            })
            .collect(),
        return_type: Some(RustType::Named(ty.name.clone())),
        body: vec![RustStmt::Return(Some(RustExpr::StructLit {
            name: ty.name.clone(),
            fields,
        }))],
        is_pub: true,
        receiver: None,
    }
}

fn lower_method(method: &MethodDecl) -> RustFunction {
    let mutable_receiver = method.body.as_ref().is_some_and(method_mutates_self);
    let mut env = TypeEnv::new();
    for param in &method.params {
        env.insert_local(param.name.clone(), param.ty.clone());
    }
    RustFunction {
        name: method
            .rust_name
            .clone()
            .unwrap_or_else(|| crate::parser::to_snake_case(&method.name)),
        params: method
            .params
            .iter()
            .map(|p| RustParam {
                name: p.name.clone(),
                ty: crate::coercion::rust_param_slot_type(&p.ty, p.is_varargs),
            })
            .collect(),
        return_type: rust_return_type(&method.return_type),
        body: method
            .body
            .as_ref()
            .map(|body| lower_block(body, &mut env, Some(&method.return_type)))
            .unwrap_or_default(),
        is_pub: true,
        receiver: if method.modifiers.contains(&Modifier::Static) {
            None
        } else if mutable_receiver {
            Some(RustReceiver::Mutable)
        } else {
            Some(RustReceiver::Shared)
        },
    }
}

fn lower_block(block: &Block, env: &mut TypeEnv, return_type: Option<&JavaType>) -> Vec<RustStmt> {
    let mut body = Vec::new();
    for stmt in &block.statements {
        lower_stmt(stmt, env, return_type, &mut body);
        env.learn_stmt(stmt);
    }
    body
}

fn lower_stmt(
    stmt: &Stmt,
    env: &mut TypeEnv,
    return_type: Option<&JavaType>,
    out: &mut Vec<RustStmt>,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
        } => out.push(RustStmt::Let {
            mutable: block_reassigns_name(name, stmt),
            name: name.clone(),
            ty: Some(java_type_to_rust(ty)),
            value: initializer.as_ref().map(|expr| {
                lower_expr_for_slot(expr, env, crate::coercion::Slot::Assignment(ty.clone()))
            }),
        }),
        Stmt::Assign { target, value } => out.push(RustStmt::Assign {
            target: lower_expr(target, env),
            value: lower_expr_for_slot(value, env, assignment_slot_for_target(target, env)),
        }),
        Stmt::Expr(expr) => out.push(RustStmt::Expr(lower_expr(expr, env))),
        Stmt::Return(expr) => out.push(RustStmt::Return(expr.as_ref().map(|expr| {
            if let Some(return_type) = return_type {
                lower_expr_for_slot(
                    expr,
                    env,
                    crate::coercion::Slot::Return(return_type.clone()),
                )
            } else {
                lower_expr(expr, env)
            }
        }))),
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => out.push(RustStmt::If {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition),
            then_body: lower_block(then_block, &mut env.clone(), return_type),
            else_body: else_block
                .as_ref()
                .map(|block| lower_block(block, &mut env.clone(), return_type))
                .unwrap_or_default(),
        }),
        Stmt::While { condition, body } => out.push(RustStmt::While {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition),
            body: lower_block(body, &mut env.clone(), return_type),
        }),
        Stmt::For {
            init,
            condition,
            update,
            body,
            label,
        } => {
            for init_stmt in init {
                lower_stmt(init_stmt, env, return_type, out);
                env.learn_stmt(init_stmt);
            }
            let mut loop_body = lower_block_for_body(body, update, &mut env.clone(), return_type);
            for expr in update {
                loop_body.push(RustStmt::Expr(lower_expr(expr, env)));
            }
            out.push(RustStmt::While {
                condition: condition
                    .as_ref()
                    .map(|expr| lower_expr_for_slot(expr, env, crate::coercion::Slot::Condition))
                    .unwrap_or_else(|| RustExpr::Literal("true".to_string())),
                body: if label.is_some() {
                    vec![RustStmt::Loop {
                        label: label.clone(),
                        body: loop_body,
                    }]
                } else {
                    loop_body
                },
            });
        }
        Stmt::EnhancedFor {
            var_name,
            iterable,
            body,
            ..
        } => out.push(RustStmt::Raw(format!(
            "for {var_name} in {}.iter() {{\n{}\n}}",
            crate::emit::emit_expr(&lower_expr(iterable, env)),
            crate::emit::emit_statements(&lower_block(body, &mut env.clone(), return_type), 1)
        ))),
        Stmt::Break(label) => out.push(RustStmt::Break(label.clone())),
        Stmt::Continue(label) => out.push(RustStmt::Continue(label.clone())),
        Stmt::Block(block) => out.push(RustStmt::Raw(format!(
            "{{\n{}\n}}",
            crate::emit::emit_statements(&lower_block(block, &mut env.clone(), return_type), 1)
        ))),
        Stmt::Unsupported(reason) => out.push(RustStmt::Raw(format!(
            "compile_error!({:?});",
            format!("unsupported Java statement: {reason}")
        ))),
    }
}

fn lower_expr(expr: &Expr, env: &TypeEnv) -> RustExpr {
    if let Some(format_expr) = lower_string_concat(expr, env) {
        return format_expr;
    }
    match expr {
        Expr::Literal(lit) => RustExpr::Literal(literal_to_rust(lit)),
        Expr::Var(name) => RustExpr::Var(name.clone()),
        Expr::This => RustExpr::Var("self".to_string()),
        Expr::FieldAccess(receiver, field) => {
            if field == "length" {
                return RustExpr::Raw(format!(
                    "{}.len() as i32",
                    crate::emit::emit_expr(&lower_expr(receiver, env))
                ));
            }
            RustExpr::Field(Box::new(lower_expr(receiver, env)), field.clone())
        }
        Expr::ArrayAccess(array, index) => RustExpr::Index(
            Box::new(lower_expr(array, env)),
            Box::new(lower_expr_for_slot(
                index,
                env,
                crate::coercion::Slot::Index,
            )),
        ),
        Expr::Binary { op, left, right } => RustExpr::Binary {
            op: binary_op_to_rust(*op).to_string(),
            left: Box::new(lower_expr(left, env)),
            right: Box::new(lower_expr(right, env)),
        },
        Expr::Unary { op, expr } => match op {
            UnaryOp::PostInc => RustExpr::Raw(format!(
                "{} += 1",
                crate::emit::emit_expr(&lower_expr(expr, env))
            )),
            UnaryOp::PostDec => RustExpr::Raw(format!(
                "{} -= 1",
                crate::emit::emit_expr(&lower_expr(expr, env))
            )),
            UnaryOp::Not => RustExpr::Unary {
                op: "!".to_string(),
                expr: Box::new(lower_expr(expr, env)),
            },
            UnaryOp::Neg => RustExpr::Unary {
                op: "-".to_string(),
                expr: Box::new(lower_expr(expr, env)),
            },
            UnaryOp::PreInc | UnaryOp::PreDec => {
                RustExpr::Raw(crate::emit::emit_expr(&lower_expr(expr, env)))
            }
        },
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => RustExpr::If {
            condition: Box::new(lower_expr_for_slot(
                condition,
                env,
                crate::coercion::Slot::Condition,
            )),
            then_expr: Box::new(lower_expr(then_expr, env)),
            else_expr: Box::new(lower_expr(else_expr, env)),
        },
        Expr::Call {
            receiver,
            name,
            args,
        } => {
            let rust_args = args.iter().map(|arg| lower_expr(arg, env)).collect();
            if let Some(receiver) = receiver {
                RustExpr::MethodCall {
                    receiver: Box::new(lower_expr(receiver, env)),
                    method: crate::parser::to_snake_case(name),
                    args: rust_args,
                }
            } else {
                RustExpr::Call {
                    function: Box::new(RustExpr::Var(crate::parser::to_snake_case(name))),
                    args: rust_args,
                }
            }
        }
        Expr::New { ty, args } => RustExpr::Call {
            function: Box::new(RustExpr::Raw(format!(
                "{}::new",
                crate::emit::emit_type(&java_type_to_rust(ty))
            ))),
            args: args.iter().map(|arg| lower_expr(arg, env)).collect(),
        },
        Expr::Raw(raw) => RustExpr::Raw(raw.clone()),
    }
}

fn lower_expr_for_slot(expr: &Expr, env: &TypeEnv, slot: crate::coercion::Slot) -> RustExpr {
    let actual = infer_expr_type(expr, env);
    let expr = lower_expr(expr, env);
    let plan = crate::coercion::plan_coercion(&actual, &slot);
    crate::coercion::apply_coercions(expr, &plan)
}

fn lower_string_concat(expr: &Expr, env: &TypeEnv) -> Option<RustExpr> {
    if !crate::types::is_string_concat(expr, env) {
        return None;
    }
    let mut parts = Vec::new();
    crate::types::flatten_add(expr, &mut parts);
    let format_string = parts
        .iter()
        .map(|part| match part {
            Expr::Literal(Literal::String(s)) => s.replace('{', "{{").replace('}', "}}"),
            _ => "{}".to_string(),
        })
        .collect::<String>();
    let args = parts
        .iter()
        .filter(|part| !matches!(part, Expr::Literal(Literal::String(_))))
        .map(|part| crate::emit::emit_expr(&lower_expr(part, env)))
        .collect::<Vec<_>>();
    let mut raw = format!("format!({format_string:?}");
    for arg in args {
        raw.push_str(", ");
        raw.push_str(&arg);
    }
    raw.push(')');
    Some(RustExpr::Raw(raw))
}

fn literal_to_rust(lit: &Literal) -> String {
    match lit {
        Literal::Null => "None".to_string(),
        Literal::Bool(value) => value.to_string(),
        Literal::Int(value) => value.to_string(),
        Literal::Long(value) => format!("{value}i64"),
        Literal::Float(value) => format!("{value}f32"),
        Literal::Double(value) => format!("{value}f64"),
        Literal::String(value) => format!("{value:?}.to_string()"),
        Literal::Char(value) => format!("{value:?}"),
    }
}

fn lower_block_for_body(
    block: &Block,
    update: &[Expr],
    env: &mut TypeEnv,
    return_type: Option<&JavaType>,
) -> Vec<RustStmt> {
    let mut out = Vec::new();
    for stmt in &block.statements {
        lower_stmt_for_body(stmt, update, env, return_type, &mut out);
        env.learn_stmt(stmt);
    }
    out
}

fn lower_stmt_for_body(
    stmt: &Stmt,
    update: &[Expr],
    env: &mut TypeEnv,
    return_type: Option<&JavaType>,
    out: &mut Vec<RustStmt>,
) {
    match stmt {
        Stmt::Continue(None) => {
            for expr in update {
                out.push(RustStmt::Expr(lower_expr(expr, env)));
            }
            out.push(RustStmt::Continue(None));
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => out.push(RustStmt::If {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition),
            then_body: lower_block_for_body(then_block, update, &mut env.clone(), return_type),
            else_body: else_block
                .as_ref()
                .map(|block| lower_block_for_body(block, update, &mut env.clone(), return_type))
                .unwrap_or_default(),
        }),
        Stmt::While { condition, body } => out.push(RustStmt::While {
            condition: lower_expr_for_slot(condition, env, crate::coercion::Slot::Condition),
            body: lower_block(body, &mut env.clone(), return_type),
        }),
        other => lower_stmt(other, env, return_type, out),
    }
}

fn binary_op_to_rust(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

fn default_expr_for_type(ty: &JavaType) -> RustExpr {
    RustExpr::Raw(default_rust_literal_for_java_type(ty).to_string())
}

fn env_for_type(ty: &TypeDecl) -> TypeEnv {
    let mut env = TypeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field(field.name.clone(), field.ty.clone());
        }
    }
    env
}

fn assignment_slot_for_target(target: &Expr, env: &TypeEnv) -> crate::coercion::Slot {
    match target {
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .map(crate::coercion::Slot::Assignment)
            .unwrap_or(crate::coercion::Slot::Value),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => env
            .field_type(field)
            .cloned()
            .map(crate::coercion::Slot::Assignment)
            .unwrap_or(crate::coercion::Slot::Value),
        Expr::ArrayAccess(array, _) => {
            let array_ty = infer_expr_type(array, env);
            if let JavaType::Array(inner) = array_ty {
                crate::coercion::Slot::Assignment(*inner)
            } else {
                crate::coercion::Slot::Value
            }
        }
        _ => crate::coercion::Slot::Value,
    }
}

fn method_mutates_self(block: &Block) -> bool {
    block.statements.iter().any(stmt_mutates_self)
}

fn stmt_mutates_self(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Assign { target, .. } => target_is_self_field(target),
        Stmt::Expr(Expr::Unary { expr, .. }) => target_is_self_field(expr),
        Stmt::If {
            then_block,
            else_block,
            ..
        } => {
            method_mutates_self(then_block) || else_block.as_ref().is_some_and(method_mutates_self)
        }
        Stmt::While { body, .. } | Stmt::For { body, .. } | Stmt::EnhancedFor { body, .. } => {
            method_mutates_self(body)
        }
        _ => false,
    }
}

fn target_is_self_field(expr: &Expr) -> bool {
    matches!(expr, Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This))
}

fn block_reassigns_name(_name: &str, _stmt: &Stmt) -> bool {
    true
}
