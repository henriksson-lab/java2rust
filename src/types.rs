use std::collections::HashMap;

use crate::java_ir::{BinaryOp, Expr, JavaType, Literal, PrimitiveType, Stmt};
use crate::representation::RepresentationPlan;
use crate::signature::SignaturePlan;
use crate::slot_shape::{OwnershipShape, RustSlotShape, TypeCapabilities, ViewShape};

#[derive(Debug, Clone, Copy, Default)]
pub struct ExprShapeContext<'a> {
    pub signatures: Option<&'a SignaturePlan>,
    pub representations: Option<&'a RepresentationPlan>,
}

#[derive(Debug, Clone, Default)]
pub struct TypeEnv {
    locals: HashMap<String, JavaType>,
    fields: HashMap<String, JavaType>,
    local_shapes: HashMap<String, RustSlotShape>,
    field_shapes: HashMap<String, RustSlotShape>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_local(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.local_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.locals.insert(name, ty);
    }

    pub fn insert_field(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.field_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.fields.insert(name, ty);
    }

    pub fn insert_local_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.locals.insert(name.clone(), shape.java_ty.clone());
        self.local_shapes.insert(name, shape);
    }

    pub fn insert_field_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.fields.insert(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name, shape);
    }

    pub fn local_type(&self, name: &str) -> Option<&JavaType> {
        self.locals.get(name)
    }

    pub fn field_type(&self, name: &str) -> Option<&JavaType> {
        self.fields.get(name)
    }

    pub fn local_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.local_shapes.get(name)
    }

    pub fn field_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.field_shapes.get(name)
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
        Expr::ArrayAccess(array, _) => RustSlotShape::conservative(infer_expr_type(array, env))
            .sequence_element_type()
            .unwrap_or_else(|| JavaType::Unknown("array_access".to_string())),
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

pub fn infer_expr_shape(expr: &Expr, env: &TypeEnv) -> RustSlotShape {
    infer_expr_shape_with_context(expr, env, ExprShapeContext::default())
}

pub fn infer_assignment_target_shape(expr: &Expr, env: &TypeEnv) -> Option<RustSlotShape> {
    infer_assignment_target_shape_with_context(expr, env, ExprShapeContext::default())
}

pub fn infer_assignment_target_shape_with_context(
    expr: &Expr,
    env: &TypeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    match expr {
        Expr::Var(name) => env.local_shape(name).cloned(),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            env.field_shape(field).cloned()
        }
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx),
        Expr::ArrayAccess(array, _) => infer_indexed_assignment_target_shape(array, env),
        _ => None,
    }
}

pub fn infer_stream_lambda_body_shape(expr: &Expr, default: &RustSlotShape) -> RustSlotShape {
    if stream_lambda_body_is_borrowed_str(expr) {
        let mut shape = RustSlotShape::conservative(JavaType::String);
        shape.ownership = OwnershipShape::SharedBorrow;
        shape.view = ViewShape::Str;
        return shape;
    }
    default.clone()
}

pub fn infer_stream_map_to_int_shape(_expr: &Expr) -> RustSlotShape {
    RustSlotShape::conservative(JavaType::Primitive(PrimitiveType::Int))
}

pub fn infer_stream_source_element_shape(source: &Expr, env: &TypeEnv) -> Option<RustSlotShape> {
    stream_source_element_type(source, env).map(stream_source_element_shape)
}

pub fn infer_expr_shape_with_context(
    expr: &Expr,
    env: &TypeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    match expr {
        Expr::Literal(lit) => RustSlotShape::conservative(literal_type(lit)),
        Expr::Var(name) => env
            .local_shape(name)
            .cloned()
            .or_else(|| env.field_shape(name).cloned())
            .unwrap_or_else(|| RustSlotShape::conservative(JavaType::Unknown(name.clone()))),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => env
            .field_shape(field)
            .cloned()
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env))),
        Expr::FieldAccess(receiver, field) if field == "length" => RustSlotShape::conservative(
            infer_expr_type(&Expr::FieldAccess(receiver.clone(), field.clone()), env),
        ),
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx)
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env))),
        Expr::ArrayAccess(array, _) => infer_indexed_expr_shape(array, env, ctx),
        Expr::Call {
            resolved: Some(call),
            ..
        } => ctx
            .signatures
            .and_then(|signatures| {
                signatures.method_return_shape_by_rust_name(&call.declaring_type, &call.rust_name)
            })
            .unwrap_or_else(|| value_shape_or_conservative(ctx, infer_expr_type(expr, env))),
        Expr::New {
            ty,
            resolved: Some(resolved),
            ..
        } => {
            let java_ty = if matches!(ty, JavaType::Unknown(_)) {
                JavaType::Named {
                    name: resolved.declaring_type.clone(),
                    args: Vec::new(),
                }
            } else {
                ty.clone()
            };
            value_shape_or_conservative(ctx, java_ty)
        }
        Expr::Cast { ty, .. } => value_shape_or_conservative(ctx, ty.clone()),
        _ => value_shape_or_conservative(ctx, infer_expr_type(expr, env)),
    }
}

fn value_shape_or_conservative(ctx: ExprShapeContext<'_>, java_ty: JavaType) -> RustSlotShape {
    ctx.representations
        .map(|representations| representations.value_shape_for_java_type(java_ty.clone()))
        .unwrap_or_else(|| RustSlotShape::conservative(java_ty))
}

fn infer_field_access_shape(
    receiver: &Expr,
    field: &str,
    env: &TypeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let JavaType::Named { name, .. } = infer_expr_type(receiver, env) else {
        return None;
    };
    ctx.signatures
        .and_then(|signatures| signatures.field_shape(&name, field).cloned())
}

fn infer_indexed_expr_shape(
    array: &Expr,
    env: &TypeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    let array_shape = infer_expr_shape_with_context(array, env, ctx);
    let element_ty = sequence_element_type_from_java(&infer_expr_type(array, env))
        .unwrap_or_else(|| JavaType::Unknown("array_access".to_string()));
    let mut element_shape = RustSlotShape::conservative(element_ty.clone());

    if (array_shape.ownership == OwnershipShape::SharedBorrow
        || array_shape.view == ViewShape::Slice)
        && !TypeCapabilities::conservative(&element_ty).copy
    {
        element_shape.ownership = OwnershipShape::SharedBorrow;
    }

    element_shape
}

fn stream_lambda_body_is_borrowed_str(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Call {
            name,
            args,
            ..
        } if name == "trim" && args.is_empty()
    )
}

fn stream_source_element_type(source: &Expr, env: &TypeEnv) -> Option<JavaType> {
    RustSlotShape::conservative(infer_expr_type(source, env))
        .sequence_element_type()
        .map(normalize_stream_element_type)
}

fn normalize_stream_element_type(ty: JavaType) -> JavaType {
    match ty {
        JavaType::Named { name, args } if name == "Integer" && args.is_empty() => {
            JavaType::Primitive(PrimitiveType::Int)
        }
        other => other,
    }
}

fn stream_source_element_shape(ty: JavaType) -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(ty);
    if !shape.capabilities().copy {
        shape.ownership = OwnershipShape::SharedBorrow;
    }
    shape
}

fn infer_indexed_assignment_target_shape(array: &Expr, env: &TypeEnv) -> Option<RustSlotShape> {
    sequence_element_type_from_java(&infer_expr_type(array, env)).map(RustSlotShape::for_storage)
}

fn sequence_element_type_from_java(ty: &JavaType) -> Option<JavaType> {
    RustSlotShape::conservative(ty.clone()).sequence_element_type()
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
