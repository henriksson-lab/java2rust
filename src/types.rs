use std::collections::HashMap;

use crate::java_ir::{BinaryOp, Expr, JavaType, Literal, PrimitiveType, Stmt};

#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    locals: HashMap<String, JavaType>,
    fields: HashMap<String, JavaType>,
    type_param_bounds: HashMap<String, JavaType>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_local(&mut self, name: impl Into<String>, ty: JavaType) {
        self.locals.insert(name.into(), ty);
    }

    pub fn insert_field(&mut self, name: impl Into<String>, ty: JavaType) {
        self.fields.insert(name.into(), ty);
    }

    pub fn insert_type_param_bound(&mut self, name: impl Into<String>, bound: JavaType) {
        self.type_param_bounds.insert(name.into(), bound);
    }

    pub fn local_type(&self, name: &str) -> Option<&JavaType> {
        self.locals.get(name)
    }

    pub fn field_type(&self, name: &str) -> Option<&JavaType> {
        self.fields.get(name)
    }

    pub fn type_param_bound(&self, name: &str) -> Option<&JavaType> {
        self.type_param_bounds.get(name)
    }

    pub fn learn_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { ty, name, .. } => self.insert_local(name.clone(), ty.clone()),
            Stmt::For { init, .. } => {
                for stmt in init {
                    self.learn_stmt(stmt);
                }
            }
            Stmt::EnhancedFor {
                var_type, var_name, ..
            } => self.insert_local(var_name.clone(), var_type.clone()),
            Stmt::Try {
                resources, body, ..
            } => {
                for resource in resources {
                    self.learn_stmt(resource);
                }
                for stmt in &body.statements {
                    self.learn_stmt(stmt);
                }
            }
            _ => {}
        }
    }
}

pub fn infer_expr_type(expr: &Expr, env: &TypeEnv) -> JavaType {
    match expr {
        Expr::Literal(lit) => literal_type(lit),
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .or_else(|| env.field_type(name).cloned())
            .unwrap_or_else(|| JavaType::Unknown(name.clone())),
        Expr::This => JavaType::Unknown("this".to_string()),
        Expr::Super => JavaType::Unknown("super".to_string()),
        Expr::FieldAccess(receiver, field) => {
            if let Some(java_ty) = infer_member_access_type(receiver, field, env) {
                java_ty
            } else if matches!(receiver.as_ref(), Expr::This) {
                env.field_type(field)
                    .cloned()
                    .unwrap_or_else(|| JavaType::Unknown(field.clone()))
            } else {
                JavaType::Unknown(field.clone())
            }
        }
        Expr::ArrayAccess(array, _) => sequence_element_type(&infer_expr_type(array, env))
            .unwrap_or_else(|| JavaType::Unknown("array_access".to_string())),
        Expr::ArrayCreation {
            element_type,
            dimensions,
        } => dimensions
            .iter()
            .fold(element_type.clone(), |ty, _| JavaType::Array(Box::new(ty))),
        Expr::ArrayInitializer(elements) => elements
            .first()
            .map(|element| JavaType::Array(Box::new(infer_expr_type(element, env))))
            .unwrap_or_else(|| {
                JavaType::Array(Box::new(JavaType::Unknown("array_initializer".to_string())))
            }),
        Expr::Binary { op, left, right } => infer_binary_type(*op, left, right, env),
        Expr::Unary { expr, .. } => infer_expr_type(expr, env),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => common_type(
            &infer_expr_type(then_expr, env),
            &infer_expr_type(else_expr, env),
        ),
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
        } => {
            if let Some(call) = resolved {
                call.return_type.clone()
            } else if name == "nonNull" && !args.is_empty() {
                infer_expr_type(&args[0], env)
            } else {
                let receiver_type = receiver
                    .as_deref()
                    .map(|receiver| infer_expr_type(receiver, env));
                let arg_types = args
                    .iter()
                    .map(|arg| infer_expr_type(arg, env))
                    .collect::<Vec<_>>();
                crate::stdlib::known_external_method_return_type_for_call(
                    receiver_type.as_ref(),
                    name,
                    &arg_types,
                )
                .or_else(|| external_pack_method_return_type(receiver_type.as_ref(), name))
                .unwrap_or_else(|| JavaType::Unknown(name.clone()))
            }
        }
        Expr::New { ty, .. } => ty.clone(),
        Expr::Cast { ty, .. } => ty.clone(),
        Expr::InstanceOf { .. } => JavaType::Primitive(PrimitiveType::Boolean),
        Expr::Lambda { .. } => JavaType::Unknown("lambda".to_string()),
        Expr::MethodReference { .. } => JavaType::Unknown("method_reference".to_string()),
        Expr::ClassLiteral(ty) => class_literal_type(ty),
        Expr::Raw(raw) => JavaType::Unknown(raw.clone()),
    }
}

fn external_pack_method_return_type(
    receiver_type: Option<&JavaType>,
    name: &str,
) -> Option<JavaType> {
    let receiver_name = match receiver_type {
        Some(JavaType::Named { name, .. }) | Some(JavaType::Unknown(name)) => Some(name.as_str()),
        _ => None,
    };
    crate::external_packs::external_api_return_type_for_receiver(receiver_name, name)
}

pub fn class_literal_type(ty: &JavaType) -> JavaType {
    JavaType::Named {
        name: "Class".to_string(),
        args: vec![ty.clone()],
    }
}

pub fn literal_type(lit: &Literal) -> JavaType {
    match lit {
        Literal::Null => JavaType::Unknown("null".to_string()),
        Literal::Bool(_) => JavaType::Primitive(PrimitiveType::Boolean),
        Literal::Int(_) => JavaType::Primitive(PrimitiveType::Int),
        Literal::Long(_) => JavaType::Primitive(PrimitiveType::Long),
        Literal::Float(_) => JavaType::Primitive(PrimitiveType::Float),
        Literal::Double(_) => JavaType::Primitive(PrimitiveType::Double),
        Literal::String(_) => JavaType::String,
        Literal::Char(_) => JavaType::Primitive(PrimitiveType::Char),
    }
}

pub fn sequence_element_type(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Array(inner) => Some((**inner).clone()),
        JavaType::Named { name, args } if name == "List" && args.len() == 1 => {
            Some(args[0].clone())
        }
        _ => None,
    }
}

pub fn optional_element_type(java_ty: &JavaType) -> Option<JavaType> {
    match java_ty {
        JavaType::Named { name, args } if name == "Optional" && args.len() == 1 => {
            Some(args[0].clone())
        }
        _ => None,
    }
}

pub fn list_java_type(element_ty: JavaType) -> JavaType {
    JavaType::Named {
        name: "List".to_string(),
        args: vec![element_ty],
    }
}

pub fn optional_java_type(element_ty: JavaType) -> JavaType {
    JavaType::Named {
        name: "Optional".to_string(),
        args: vec![element_ty],
    }
}

pub fn stream_element_type(source_ty: &JavaType) -> Option<JavaType> {
    sequence_element_type(source_ty).map(normalize_stream_element_type)
}

fn normalize_stream_element_type(ty: JavaType) -> JavaType {
    match ty {
        JavaType::Named { name, args } if name == "Integer" && args.is_empty() => {
            JavaType::Primitive(PrimitiveType::Int)
        }
        other => other,
    }
}

pub fn is_final_by_default(java_ty: &JavaType) -> bool {
    matches!(java_ty, JavaType::Primitive(_) | JavaType::String)
}

pub fn infer_member_access_type(receiver: &Expr, member: &str, env: &TypeEnv) -> Option<JavaType> {
    let receiver_ty = infer_expr_type(receiver, env);
    if member == "length" && is_java_length_receiver(&receiver_ty) {
        Some(JavaType::Primitive(PrimitiveType::Int))
    } else {
        None
    }
}

fn is_java_length_receiver(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Array(_) | JavaType::String)
}

fn infer_binary_type(op: BinaryOp, left: &Expr, right: &Expr, env: &TypeEnv) -> JavaType {
    match op {
        BinaryOp::Assign => {
            assignment_target_type(left, env).unwrap_or_else(|| infer_expr_type(right, env))
        }
        BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge
        | BinaryOp::And
        | BinaryOp::Or => JavaType::Primitive(PrimitiveType::Boolean),
        BinaryOp::Add
            if crate::string_concat::is_string_concat(
                &Expr::Binary {
                    op,
                    left: Box::new(left.clone()),
                    right: Box::new(right.clone()),
                },
                env,
            ) =>
        {
            JavaType::String
        }
        BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
            integral_binary_type(&infer_expr_type(left, env), &infer_expr_type(right, env))
        }
        BinaryOp::Shl | BinaryOp::Shr => infer_expr_type(left, env),
        _ => numeric_binary_type(&infer_expr_type(left, env), &infer_expr_type(right, env)),
    }
}

fn assignment_target_type(target: &Expr, env: &TypeEnv) -> Option<JavaType> {
    match target {
        Expr::Var(name) => env
            .local_type(name)
            .cloned()
            .or_else(|| env.field_type(name).cloned()),
        Expr::FieldAccess(receiver, field) => {
            if matches!(receiver.as_ref(), Expr::This) {
                env.field_type(field).cloned()
            } else {
                None
            }
        }
        Expr::ArrayAccess(array, _) => sequence_element_type(&infer_expr_type(array, env)),
        _ => None,
    }
}

fn integral_binary_type(left: &JavaType, right: &JavaType) -> JavaType {
    use PrimitiveType::*;
    if matches!(left, JavaType::Primitive(Long)) || matches!(right, JavaType::Primitive(Long)) {
        JavaType::Primitive(Long)
    } else {
        JavaType::Primitive(Int)
    }
}

fn numeric_binary_type(left: &JavaType, right: &JavaType) -> JavaType {
    use PrimitiveType::*;
    let rank = |ty: &JavaType| match ty {
        JavaType::Primitive(Double) => 5,
        JavaType::Primitive(Float) => 4,
        JavaType::Primitive(Long) => 3,
        JavaType::Primitive(Int | Short | Byte | Char) => 2,
        _ => 0,
    };
    match rank(left).max(rank(right)) {
        5 => JavaType::Primitive(Double),
        4 => JavaType::Primitive(Float),
        3 => JavaType::Primitive(Long),
        _ => JavaType::Primitive(Int),
    }
}

fn common_type(left: &JavaType, right: &JavaType) -> JavaType {
    if left == right {
        left.clone()
    } else if matches!(left, JavaType::Primitive(_)) && matches!(right, JavaType::Primitive(_)) {
        numeric_binary_type(left, right)
    } else {
        JavaType::Unknown("common_type".to_string())
    }
}
