use std::collections::HashMap;

use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, Member, MethodDecl, Modifier, Stmt, TypeDecl};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    borrowed_string_content_shape, indexed_assignment_shape_with_facts,
    indexed_read_shape_with_facts,
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
    static_field_names: HashMap<String, String>,
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

    pub fn insert_static_type_alias(
        &mut self,
        alias: impl Into<String>,
        type_name: impl Into<String>,
    ) {
        self.types.insert_static_type_alias(alias, type_name);
    }

    pub fn insert_static_member_import(
        &mut self,
        member: impl Into<String>,
        owner_type: impl Into<String>,
    ) {
        self.types.insert_static_member_import(member, owner_type);
    }

    pub fn insert_field_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.types.insert_field(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name.clone(), shape.clone());
        self.types
            .insert_field(alias.clone(), shape.java_ty.clone());
        self.field_shapes.insert(alias, shape);
    }

    pub fn insert_type_field_shape(
        &mut self,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let type_name = type_name.into();
        let field_name = field_name.into();
        let type_name_alias = crate::parser::to_snake_case(&type_name);
        let alias = crate::parser::to_snake_case(&field_name);
        self.types
            .insert_type_field(type_name.clone(), field_name.clone(), shape.java_ty.clone());
        self.types
            .insert_type_field(type_name.clone(), alias.clone(), shape.java_ty.clone());
        if type_name_alias != type_name {
            self.types.insert_type_field(
                type_name_alias.clone(),
                field_name,
                shape.java_ty.clone(),
            );
            self.types
                .insert_type_field(type_name_alias, alias, shape.java_ty.clone());
        }
    }

    pub fn insert_static_field_shape(&mut self, name: impl Into<String>, shape: RustSlotShape) {
        let name = name.into();
        let shape = if matches!(shape.java_ty, JavaType::String) {
            borrowed_string_content_shape()
        } else {
            shape
        };
        let alias = crate::parser::to_snake_case(&name);
        self.types.insert_field(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name.clone(), shape.clone());
        self.types
            .insert_field(alias.clone(), shape.java_ty.clone());
        self.field_shapes.insert(alias.clone(), shape);
        self.static_field_names.insert(name.clone(), name.clone());
        self.static_field_names.insert(alias, name);
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

    pub fn is_static_field(&self, name: &str) -> bool {
        self.static_field_names.contains_key(name)
    }

    pub fn static_field_access_name(&self, name: &str) -> Option<&str> {
        self.static_field_names.get(name).map(String::as_str)
    }
}

pub fn fallback_local_shape_from_declared_type(ty: &JavaType) -> RustSlotShape {
    RustSlotShape::conservative(ty.clone())
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
        Expr::Var(name) => env
            .local_shape(name)
            .cloned()
            .or_else(|| env.field_shape(name).cloned()),
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
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx)
            .or_else(|| planned_member_access_shape(receiver, field, env))
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env.type_env()))),
        Expr::ArrayAccess(array, _) => infer_indexed_expr_shape(array, env, ctx),
        Expr::Call {
            receiver,
            name,
            args,
            resolved: Some(call),
        } => borrowed_string_view_call_shape(receiver.as_deref(), name, args, env)
            .or_else(|| {
                ctx.signatures.and_then(|signatures| {
                    signatures
                        .method_return_shape_by_rust_name(&call.declaring_type, &call.rust_name)
                })
            })
            .unwrap_or_else(|| {
                value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env()))
            }),
        Expr::Call {
            receiver,
            name,
            args,
            resolved: None,
        } => borrowed_string_view_call_shape(receiver.as_deref(), name, args, env).unwrap_or_else(
            || value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env())),
        ),
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

fn borrowed_string_view_call_shape(
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    env: &ShapeEnv,
) -> Option<RustSlotShape> {
    if !crate::stdlib::is_borrowed_string_view_method(name, args.len()) {
        return None;
    }
    let receiver_ty = receiver.map(|receiver| infer_expr_type(receiver, env.type_env()))?;
    let effective_receiver_ty =
        crate::types::optional_element_type(&receiver_ty).unwrap_or(receiver_ty);
    if !matches!(effective_receiver_ty, JavaType::String) {
        return None;
    }
    Some(crate::shape_projection::borrowed_string_content_shape())
}

pub fn shape_env_for_type(ty: &TypeDecl, signatures: &SignaturePlan) -> ShapeEnv {
    shape_env_for_type_with_fields(ty, signatures, true)
}

fn shape_env_for_type_with_fields(
    ty: &TypeDecl,
    signatures: &SignaturePlan,
    include_instance_fields: bool,
) -> ShapeEnv {
    let mut env = ShapeEnv::new();
    for member in &ty.members {
        if let Member::Field(field) = member {
            let shape = signatures.required_field_shape(&ty.name, &field.name);
            env.insert_type_field_shape(&ty.name, field.name.clone(), shape.clone());
            if is_static_final_field(field) {
                env.insert_static_field_shape(field.name.clone(), shape);
            } else if include_instance_fields {
                env.insert_field_shape(field.name.clone(), shape);
            }
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
        shape_env_for_type_with_fields(ty, signatures, false)
    } else {
        shape_env_for_type(ty, signatures)
    };
    insert_param_shapes(&mut env, &method_signature.params);
    env
}

fn is_static_final_field(field: &crate::java_ir::FieldDecl) -> bool {
    field.modifiers.contains(&Modifier::Static) && field.modifiers.contains(&Modifier::Final)
}

pub fn learn_stmt_shapes(
    stmt: &Stmt,
    env: &mut ShapeEnv,
    locals: &[LocalPlan],
    signatures: &SignaturePlan,
) {
    match stmt {
        Stmt::VarDecl { ty, name, .. } => {
            let shape = local_shape_for_declared_type(signatures, locals, name, ty)
                .cloned()
                .unwrap_or_else(|| fallback_local_shape_from_declared_type(ty));
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

fn local_shape_for_declared_type<'a>(
    signatures: &'a SignaturePlan,
    locals: &'a [LocalPlan],
    name: &str,
    declared_ty: &JavaType,
) -> Option<&'a RustSlotShape> {
    locals
        .iter()
        .find(|local| local.name == name && local.java_ty == *declared_ty)
        .map(|local| &local.shape)
        .or_else(|| signatures.local_shape(locals, name))
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
    if let Some(signatures) = ctx.signatures
        && let Some(crate::member_access_plan::MemberAccessPlan {
            op: crate::member_access_plan::MemberAccessOp::StaticConstant { owner },
            ..
        }) = crate::member_access_plan::plan_member_access(receiver, field, env.type_env())
    {
        if let Some(shape) = signatures
            .field_shape(&owner, field)
            .or_else(|| signatures.field_shape(&owner, &crate::parser::to_snake_case(field)))
        {
            if matches!(shape.java_ty, JavaType::String) {
                return Some(borrowed_string_content_shape());
            }
            return Some(shape.clone());
        }
    }
    let receiver_ty = infer_expr_type(receiver, env.type_env());
    let name = match receiver_ty {
        JavaType::Named { name, .. } | JavaType::Unknown(name) => name,
        _ => return None,
    };
    if env.local_type(&name).is_some() || env.field_type(&name).is_some() {
        return None;
    }
    ctx.signatures.and_then(|signatures| {
        signatures
            .field_shape(&name, field)
            .or_else(|| signatures.field_shape(&name, &crate::parser::to_snake_case(field)))
            .cloned()
    })
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
