use std::collections::HashMap;

use crate::java_ir::{BinaryOp, Expr, JavaType, Literal, PrimitiveType, Stmt};
use crate::rust_ir::RustType;

#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    locals: HashMap<String, JavaType>,
    fields: HashMap<String, JavaType>,
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

    pub fn local_type(&self, name: &str) -> Option<&JavaType> {
        self.locals.get(name)
    }

    pub fn field_type(&self, name: &str) -> Option<&JavaType> {
        self.fields.get(name)
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
            Stmt::Try { body, .. } => {
                for stmt in &body.statements {
                    self.learn_stmt(stmt);
                }
            }
            _ => {}
        }
    }
}

pub fn java_type_to_rust(ty: &JavaType) -> RustType {
    match ty {
        JavaType::Void => RustType::Unit,
        JavaType::Primitive(PrimitiveType::Boolean) => RustType::Bool,
        JavaType::Primitive(PrimitiveType::Byte) => RustType::I8,
        JavaType::Primitive(PrimitiveType::Short) => RustType::I16,
        JavaType::Primitive(PrimitiveType::Char) => RustType::Char,
        JavaType::Primitive(PrimitiveType::Int) => RustType::I32,
        JavaType::Primitive(PrimitiveType::Long) => RustType::I64,
        JavaType::Primitive(PrimitiveType::Float) => RustType::F32,
        JavaType::Primitive(PrimitiveType::Double) => RustType::F64,
        JavaType::String => RustType::String,
        JavaType::Array(inner) => RustType::Vec(Box::new(java_type_to_rust(inner))),
        JavaType::Named { name, args } if name == "List" && args.len() == 1 => {
            RustType::Vec(Box::new(java_type_to_rust(&args[0])))
        }
        JavaType::Named { name, args } if name == "Optional" && args.len() == 1 => {
            RustType::Option(Box::new(java_type_to_rust(&args[0])))
        }
        JavaType::Named { name, .. } => RustType::Named(name.clone()),
        JavaType::Wildcard(_) => RustType::Named("JavaObject".to_string()),
        JavaType::Unknown(raw) => RustType::Raw(raw.clone()),
    }
}

pub fn rust_return_type(ty: &JavaType) -> Option<RustType> {
    let rust = java_type_to_rust(ty);
    if rust == RustType::Unit {
        None
    } else {
        Some(rust)
    }
}

pub fn rust_param_type(ty: &JavaType, is_varargs: bool) -> RustType {
    if is_varargs {
        let inner = match ty {
            JavaType::Array(inner) => java_type_to_rust(inner),
            _ => java_type_to_rust(ty),
        };
        return RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Slice(Box::new(inner))),
        };
    }
    match ty {
        JavaType::Array(inner) => RustType::Ref {
            mutable: false,
            inner: Box::new(RustType::Slice(Box::new(java_type_to_rust(inner)))),
        },
        JavaType::String => RustType::String,
        _ => java_type_to_rust(ty),
    }
}

pub fn default_rust_literal_for_java_type(ty: &JavaType) -> &'static str {
    match ty {
        JavaType::Primitive(PrimitiveType::Boolean) => "false",
        JavaType::Primitive(_) => "0",
        JavaType::String => "String::new()",
        JavaType::Array(_) => "Vec::new()",
        _ => "Default::default()",
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
            if field == "length" && is_length_receiver(&infer_expr_type(receiver, env)) {
                JavaType::Primitive(PrimitiveType::Int)
            } else if matches!(receiver.as_ref(), Expr::This) {
                env.field_type(field)
                    .cloned()
                    .unwrap_or_else(|| JavaType::Unknown(field.clone()))
            } else {
                JavaType::Unknown(field.clone())
            }
        }
        Expr::ArrayAccess(array, _) => match infer_expr_type(array, env) {
            JavaType::Array(inner) => *inner,
            JavaType::Named { name, mut args } if name == "List" && args.len() == 1 => {
                args.remove(0)
            }
            _ => JavaType::Unknown("array_access".to_string()),
        },
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
        Expr::Call { name, resolved, .. } => resolved
            .as_ref()
            .map(|call| call.return_type.clone())
            .unwrap_or_else(|| JavaType::Unknown(name.clone())),
        Expr::New { ty, .. } => ty.clone(),
        Expr::Cast { ty, .. } => ty.clone(),
        Expr::InstanceOf { .. } => JavaType::Primitive(PrimitiveType::Boolean),
        Expr::Lambda { .. } => JavaType::Unknown("lambda".to_string()),
        Expr::MethodReference { .. } => JavaType::Unknown("method_reference".to_string()),
        Expr::ClassLiteral(_) => JavaType::Named {
            name: "Class".to_string(),
            args: Vec::new(),
        },
        Expr::Raw(raw) => JavaType::Unknown(raw.clone()),
    }
}

pub fn is_string_concat(expr: &Expr, env: &TypeEnv) -> bool {
    let mut parts = Vec::new();
    flatten_add(expr, &mut parts);
    parts.len() > 1
        && parts
            .iter()
            .any(|part| matches!(infer_expr_type(part, env), JavaType::String))
}

pub fn flatten_add<'a>(expr: &'a Expr, out: &mut Vec<&'a Expr>) {
    if let Expr::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = expr
    {
        flatten_add(left, out);
        flatten_add(right, out);
    } else {
        out.push(expr);
    }
}

fn literal_type(lit: &Literal) -> JavaType {
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

fn infer_binary_type(op: BinaryOp, left: &Expr, right: &Expr, env: &TypeEnv) -> JavaType {
    match op {
        BinaryOp::Eq
        | BinaryOp::Ne
        | BinaryOp::Lt
        | BinaryOp::Le
        | BinaryOp::Gt
        | BinaryOp::Ge
        | BinaryOp::And
        | BinaryOp::Or => JavaType::Primitive(PrimitiveType::Boolean),
        BinaryOp::Add
            if is_string_concat(
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
        BinaryOp::Shl | BinaryOp::Shr => infer_expr_type(left, env),
        _ => numeric_binary_type(&infer_expr_type(left, env), &infer_expr_type(right, env)),
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

fn is_length_receiver(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Array(_) | JavaType::String)
}
