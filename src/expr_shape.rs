use std::collections::HashMap;

use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, Member, MethodDecl, Modifier, Stmt, TypeDecl};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    indexed_assignment_shape_with_facts, indexed_read_shape_with_facts,
    project_java_value_shape as project_java_value_shape_with_representations,
};
use crate::signature::{
    ConstructorSignaturePlan, LocalPlan, MethodSignaturePlan, ParamPlan, SignaturePlan,
};
use crate::slot_shape::RustSlotShape;
use crate::types::{TypeEnv, infer_expr_type};

#[derive(Debug, Clone, Default)]
pub struct ShapeEnv {
    types: TypeEnv,
    local_shapes: HashMap<String, RustSlotShape>,
    field_shapes: HashMap<String, RustSlotShape>,
}

impl ShapeEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn type_env(&self) -> &TypeEnv {
        &self.types
    }

    pub fn insert_local(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.local_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_local(name, ty);
    }

    pub fn insert_field(&mut self, name: impl Into<String>, ty: JavaType) {
        let name = name.into();
        self.field_shapes
            .insert(name.clone(), RustSlotShape::for_storage(ty.clone()));
        self.types.insert_field(name, ty);
    }

    pub fn insert_local_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.types.insert_local(name.clone(), shape.java_ty.clone());
        self.local_shapes.insert(name, shape);
    }

    pub fn insert_field_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        self.types.insert_field(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name, shape);
    }

    pub fn local_type(&self, name: &str) -> Option<&JavaType> {
        self.types.local_type(name)
    }

    pub fn field_type(&self, name: &str) -> Option<&JavaType> {
        self.types.field_type(name)
    }

    pub fn local_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.local_shapes.get(name)
    }

    pub fn field_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.field_shapes.get(name)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ExprShapeContext<'a> {
    pub signatures: Option<&'a SignaturePlan>,
    pub representations: Option<&'a RepresentationPlan>,
    pub facts: Option<&'a FactSet>,
}

pub fn infer_expr_shape(expr: &Expr, env: &ShapeEnv) -> RustSlotShape {
    infer_expr_shape_with_context(expr, env, ExprShapeContext::default())
}

pub fn infer_assignment_target_shape(expr: &Expr, env: &ShapeEnv) -> Option<RustSlotShape> {
    infer_assignment_target_shape_with_context(expr, env, ExprShapeContext::default())
}

pub fn infer_assignment_target_shape_with_context(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    match expr {
        Expr::Var(name) => env.local_shape(name).cloned(),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => {
            env.field_shape(field).cloned()
        }
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx),
        Expr::ArrayAccess(array, _) => infer_indexed_assignment_target_shape(array, env, ctx),
        _ => None,
    }
}

pub fn infer_expr_shape_with_context(
    expr: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    match expr {
        Expr::Literal(lit) => RustSlotShape::conservative(crate::types::literal_type(lit)),
        Expr::Var(name) => env
            .local_shape(name)
            .cloned()
            .or_else(|| env.field_shape(name).cloned())
            .unwrap_or_else(|| RustSlotShape::conservative(JavaType::Unknown(name.clone()))),
        Expr::FieldAccess(receiver, field) if matches!(receiver.as_ref(), Expr::This) => env
            .field_shape(field)
            .cloned()
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env.type_env()))),
        Expr::FieldAccess(receiver, field)
            if planned_member_access_shape(receiver, field, env).is_some() =>
        {
            planned_member_access_shape(receiver, field, env)
                .expect("checked planned member access")
        }
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx)
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env.type_env()))),
        Expr::ArrayAccess(array, _) => infer_indexed_expr_shape(array, env, ctx),
        Expr::Call {
            resolved: Some(call),
            ..
        } => ctx
            .signatures
            .and_then(|signatures| {
                signatures.method_return_shape_by_rust_name(&call.declaring_type, &call.rust_name)
            })
            .unwrap_or_else(|| {
                value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env()))
            }),
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
        _ => value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env())),
    }
}

pub fn shape_env_for_type(ty: &TypeDecl, signatures: &SignaturePlan) -> ShapeEnv {
    let mut env = ShapeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            env.insert_field_shape(
                field.name.clone(),
                signatures.required_field_shape(&ty.name, &field.name),
            );
        }
    }
    env
}

pub fn shape_env_for_constructor(
    ty: &TypeDecl,
    constructor_signature: &ConstructorSignaturePlan,
    signatures: &SignaturePlan,
) -> ShapeEnv {
    let mut env = shape_env_for_type(ty, signatures);
    insert_param_shapes(&mut env, &constructor_signature.params);
    env
}

pub fn shape_env_for_method(
    ty: &TypeDecl,
    method: &MethodDecl,
    method_signature: &MethodSignaturePlan,
    signatures: &SignaturePlan,
) -> ShapeEnv {
    let mut env = if method.modifiers.contains(&Modifier::Static) {
        ShapeEnv::new()
    } else {
        shape_env_for_type(ty, signatures)
    };
    insert_param_shapes(&mut env, &method_signature.params);
    env
}

pub fn learn_stmt_shapes(
    stmt: &Stmt,
    env: &mut ShapeEnv,
    locals: &[LocalPlan],
    signatures: &SignaturePlan,
) {
    match stmt {
        Stmt::VarDecl { name, .. } => {
            let shape = signatures.required_local_shape(locals, name);
            env.insert_local_shape(name.clone(), shape);
        }
        Stmt::For { init, .. } => {
            for stmt in init {
                learn_stmt_shapes(stmt, env, locals, signatures);
            }
        }
        Stmt::EnhancedFor { var_name, .. } => {
            let shape = signatures.required_local_shape(locals, var_name);
            env.insert_local_shape(var_name.clone(), shape);
        }
        Stmt::Try {
            resources, body, ..
        } => {
            for resource in resources {
                learn_stmt_shapes(resource, env, locals, signatures);
            }
            for stmt in &body.statements {
                learn_stmt_shapes(stmt, env, locals, signatures);
            }
        }
        _ => {}
    }
}

fn insert_param_shapes(env: &mut ShapeEnv, params: &[ParamPlan]) {
    for param in params {
        env.insert_local_shape(param.name.clone(), param.shape.clone());
    }
}

fn value_shape_or_conservative(ctx: ExprShapeContext<'_>, java_ty: JavaType) -> RustSlotShape {
    project_java_value_shape_with_representations(ctx.representations, java_ty)
}

fn infer_field_access_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let JavaType::Named { name, .. } = infer_expr_type(receiver, env.type_env()) else {
        return None;
    };
    ctx.signatures
        .and_then(|signatures| signatures.field_shape(&name, field).cloned())
}

fn planned_member_access_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
) -> Option<RustSlotShape> {
    crate::member_access_plan::plan_member_access(receiver, field, env.type_env())
        .map(|plan| plan.shape)
}

fn infer_indexed_expr_shape(
    array: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> RustSlotShape {
    indexed_read_shape_with_facts(
        &infer_expr_shape_with_context(array, env, ctx),
        ctx.facts.unwrap_or(&FactSet::default()),
    )
}

fn infer_indexed_assignment_target_shape(
    array: &Expr,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    indexed_assignment_shape_with_facts(
        &infer_expr_shape_with_context(array, env, ctx),
        ctx.facts.unwrap_or(&FactSet::default()),
    )
}
