use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::facts::FactSet;
use crate::java_ir::{Expr, JavaType, Member, MethodDecl, Modifier, Stmt, TypeDecl};
use crate::representation::RepresentationPlan;
use crate::shape_projection::{
    borrowed_string_content_shape, indexed_assignment_shape_with_facts,
    indexed_read_shape_with_facts,
    project_java_value_shape as project_java_value_shape_with_representations,
    shape_is_erased_object_boundary,
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
    static_field_owners: HashMap<String, String>,
    static_storage_field_names: HashMap<String, String>,
    type_static_storage_field_names: HashMap<(String, String), String>,
    instance_field_names: HashSet<String>,
    local_plan_occurrences: Rc<RefCell<HashMap<(String, String), usize>>>,
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
        self.types.insert_field(name.clone(), ty);
        self.instance_field_names
            .insert(crate::parser::to_snake_case(&name));
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
        self.instance_field_names.insert(name.clone());
        self.types
            .insert_field(alias.clone(), shape.java_ty.clone());
        self.field_shapes.insert(alias, shape);
        self.instance_field_names
            .insert(crate::parser::to_snake_case(&name));
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

    pub fn insert_type_static_storage_field_shape(
        &mut self,
        type_name: impl Into<String>,
        field_name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let type_name = type_name.into();
        let field_name = field_name.into();
        let type_name_alias = crate::parser::to_snake_case(&type_name);
        let alias = crate::parser::to_snake_case(&field_name);
        self.insert_type_field_shape(type_name.clone(), field_name.clone(), shape);
        self.type_static_storage_field_names
            .insert((type_name.clone(), field_name.clone()), field_name.clone());
        self.type_static_storage_field_names
            .insert((type_name.clone(), alias.clone()), field_name.clone());
        if type_name_alias != type_name {
            self.type_static_storage_field_names.insert(
                (type_name_alias.clone(), field_name.clone()),
                field_name.clone(),
            );
            self.type_static_storage_field_names
                .insert((type_name_alias, alias), field_name);
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

    pub fn insert_static_field_shape_for_owner(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let owner = owner.into();
        let name = name.into();
        let alias = crate::parser::to_snake_case(&name);
        let insert_owner_for_name = !self.static_field_names.contains_key(&name);
        let insert_owner_for_alias = !self.static_field_names.contains_key(&alias);
        self.insert_static_field_shape(name.clone(), shape);
        if insert_owner_for_name {
            self.static_field_owners.insert(name.clone(), owner.clone());
        }
        if insert_owner_for_alias {
            self.static_field_owners.insert(alias, owner);
        }
    }

    pub fn insert_static_storage_field_shape(
        &mut self,
        name: impl Into<String>,
        shape: RustSlotShape,
    ) {
        let name = name.into();
        let alias = crate::parser::to_snake_case(&name);
        self.types.insert_field(name.clone(), shape.java_ty.clone());
        self.field_shapes.insert(name.clone(), shape.clone());
        self.types
            .insert_field(alias.clone(), shape.java_ty.clone());
        self.field_shapes.insert(alias.clone(), shape);
        self.static_storage_field_names
            .insert(name.clone(), name.clone());
        self.static_storage_field_names.insert(alias, name);
    }

    pub fn local_type(&self, name: &str) -> Option<&JavaType> {
        self.types.local_type(name)
    }

    pub fn field_type(&self, name: &str) -> Option<&JavaType> {
        self.types.field_type(name).or_else(|| {
            name.strip_prefix("this.")
                .and_then(|name| self.types.field_type(name))
        })
    }

    pub fn local_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.local_shapes.get(name)
    }

    pub fn next_local_plan_for_declared_type<'a>(
        &mut self,
        locals: &'a [LocalPlan],
        name: &str,
        declared_ty: &JavaType,
    ) -> Option<&'a LocalPlan> {
        let exact_matches = locals
            .iter()
            .filter(|local| local.name == name && local.java_ty == *declared_ty)
            .collect::<Vec<_>>();
        if !exact_matches.is_empty() {
            if java_type_is_erased_object_boundary(declared_ty)
                && let Some(local) = exact_matches
                    .iter()
                    .find(|local| shape_is_erased_object_boundary(&local.shape))
            {
                return Some(*local);
            }
            let index = self.consume_local_plan_occurrence(name, declared_ty);
            return exact_matches
                .get(index)
                .copied()
                .or_else(|| exact_matches.last().copied());
        }

        let name_matches = locals
            .iter()
            .filter(|local| local.name == name)
            .collect::<Vec<_>>();
        if name_matches.is_empty() {
            return None;
        }
        let index = self.consume_local_plan_occurrence(name, &JavaType::Unknown("*".to_string()));
        name_matches
            .get(index)
            .copied()
            .or_else(|| name_matches.last().copied())
    }

    fn consume_local_plan_occurrence(&mut self, name: &str, declared_ty: &JavaType) -> usize {
        let key = (name.to_string(), java_type_occurrence_key(declared_ty));
        let mut occurrences = self.local_plan_occurrences.borrow_mut();
        let entry = occurrences.entry(key).or_default();
        let index = *entry;
        *entry += 1;
        index
    }

    pub fn field_shape(&self, name: &str) -> Option<&RustSlotShape> {
        self.field_shapes.get(name).or_else(|| {
            name.strip_prefix("this.")
                .and_then(|name| self.field_shapes.get(name))
        })
    }

    pub fn is_static_field(&self, name: &str) -> bool {
        self.static_field_names.contains_key(name)
            || self.static_storage_field_names.contains_key(name)
    }

    pub fn is_instance_field(&self, name: &str) -> bool {
        self.instance_field_names.contains(name)
    }

    pub fn static_field_access_name(&self, name: &str) -> Option<&str> {
        self.static_field_names.get(name).map(String::as_str)
    }

    pub fn static_field_access_owner(&self, name: &str) -> Option<&str> {
        self.static_field_owners.get(name).map(String::as_str)
    }

    pub fn static_storage_field_access_name(&self, name: &str) -> Option<&str> {
        self.static_storage_field_names
            .get(name)
            .map(String::as_str)
    }

    pub fn type_static_storage_field_access_name(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&str> {
        let alias = crate::parser::to_snake_case(field_name);
        [
            (type_name.to_string(), field_name.to_string()),
            (type_name.to_string(), alias.clone()),
            (
                crate::parser::to_snake_case(type_name),
                field_name.to_string(),
            ),
            (crate::parser::to_snake_case(type_name), alias),
        ]
        .into_iter()
        .find_map(|key| {
            self.type_static_storage_field_names
                .get(&key)
                .map(String::as_str)
        })
    }
}

fn is_this_expr(expr: &Expr) -> bool {
    matches!(expr, Expr::This) || matches!(expr, Expr::Var(name) if name == "this")
}

fn java_type_is_erased_object_boundary(ty: &JavaType) -> bool {
    matches!(ty, JavaType::Named { name, .. } | JavaType::Unknown(name)
        if name == "Object" || name == "java.lang.Object")
}

fn java_type_occurrence_key(ty: &JavaType) -> String {
    match ty {
        JavaType::Void => "void".to_string(),
        JavaType::Primitive(primitive) => format!("primitive:{primitive:?}"),
        JavaType::String => "String".to_string(),
        JavaType::Array(inner) => format!("array:{}", java_type_occurrence_key(inner)),
        JavaType::Named { name, args } => {
            let args = args
                .iter()
                .map(java_type_occurrence_key)
                .collect::<Vec<_>>()
                .join(",");
            format!("named:{name}<{args}>")
        }
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Extends(bound))) => {
            format!("? extends {}", java_type_occurrence_key(bound))
        }
        JavaType::Wildcard(Some(crate::java_ir::WildcardBound::Super(bound))) => {
            format!("? super {}", java_type_occurrence_key(bound))
        }
        JavaType::Wildcard(None) => "?".to_string(),
        JavaType::Unknown(name) => format!("unknown:{name}"),
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
        Expr::FieldAccess(receiver, field) if is_this_expr(receiver) => {
            env.field_shape(field).cloned()
        }
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx),
        Expr::ArrayAccess(array, _) if matches!(array.as_ref(), Expr::ArrayAccess(_, _)) => Some(
            RustSlotShape::for_storage(infer_expr_type(expr, env.type_env())),
        ),
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
        Expr::FieldAccess(receiver, field) if is_this_expr(receiver) => env
            .field_shape(field)
            .cloned()
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env.type_env()))),
        Expr::FieldAccess(receiver, field) => infer_field_access_shape(receiver, field, env, ctx)
            .or_else(|| planned_member_access_shape(receiver, field, env, ctx))
            .unwrap_or_else(|| RustSlotShape::conservative(infer_expr_type(expr, env.type_env()))),
        Expr::ArrayAccess(array, _) => infer_indexed_expr_shape(array, env, ctx),
        Expr::Call {
            receiver,
            name,
            args,
            resolved: Some(call),
        } => {
            native_java_call_expr_shape(receiver.as_deref(), name, args.len(), Some(call), env, ctx)
                .or_else(|| {
                    external_call_expr_shape(receiver.as_deref(), name, args.len(), env, ctx)
                })
                .or_else(|| borrowed_string_view_call_shape(receiver.as_deref(), name, args, env))
                .or_else(|| {
                    java_string_runtime_call_expr_shape(receiver.as_deref(), name, args.len(), ctx)
                })
                .or_else(|| {
                    ctx.signatures.and_then(|signatures| {
                        signatures
                            .method_return_shape_by_rust_name(&call.declaring_type, &call.rust_name)
                    })
                })
                .unwrap_or_else(|| {
                    value_shape_or_conservative(ctx, infer_expr_type(expr, env.type_env()))
                })
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved: None,
        } => native_java_call_expr_shape(receiver.as_deref(), name, args.len(), None, env, ctx)
            .or_else(|| external_call_expr_shape(receiver.as_deref(), name, args.len(), env, ctx))
            .or_else(|| borrowed_string_view_call_shape(receiver.as_deref(), name, args, env))
            .or_else(|| {
                java_string_runtime_call_expr_shape(receiver.as_deref(), name, args.len(), ctx)
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

fn native_java_call_expr_shape(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    resolved: Option<&crate::java_ir::ResolvedCall>,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let default_facts;
    let facts = match ctx.facts {
        Some(facts) => facts,
        None => {
            default_facts = FactSet::default();
            &default_facts
        }
    };
    let target = crate::call_plan::plan_call_target_with_facts_and_arity(
        receiver,
        name,
        arg_count,
        resolved,
        facts,
        env.type_env(),
    );
    if let crate::call_plan::CallTargetPlan::NativeCollectionMethod { receiver, method } = &target
        && let Some(lowered) =
            crate::call_plan::native_collection_lowered_type(receiver, *method, env.type_env())
    {
        return Some(lowered.into_slot_shape());
    }
    crate::call_plan::native_java_call_lowered_type(&target)
        .map(|lowered| lowered.into_slot_shape())
}

fn external_call_expr_shape(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let facts = ctx.facts?;
    crate::call_plan::external_call_lowered_type(
        receiver,
        name,
        arg_count,
        facts,
        env.type_env(),
        None,
    )
    .map(|lowered| lowered.into_slot_shape())
}

fn java_string_runtime_call_expr_shape(
    receiver: Option<&Expr>,
    name: &str,
    arg_count: usize,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    receiver?;
    crate::stdlib::java_string_runtime_method_return_type(name, arg_count)
        .map(|java_ty| value_shape_or_conservative(ctx, java_ty))
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
    let receiver = receiver?;
    let receiver_shape = infer_expr_shape(receiver, env);
    let receiver_ty = if matches!(receiver_shape.java_ty, JavaType::String) {
        receiver_shape.java_ty
    } else {
        infer_expr_type(receiver, env.type_env())
    };
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
            if crate::item_assembly::static_final_field_has_associated_const(ty, field) {
                env.insert_type_field_shape(&ty.name, field.name.clone(), shape.clone());
                env.insert_static_field_shape(field.name.clone(), shape);
            } else if field.modifiers.contains(&Modifier::Static) {
                env.insert_type_static_storage_field_shape(
                    &ty.name,
                    field.name.clone(),
                    shape.clone(),
                );
                env.insert_static_storage_field_shape(field.name.clone(), shape);
            } else if include_instance_fields {
                env.insert_type_field_shape(&ty.name, field.name.clone(), shape.clone());
                env.insert_field_shape(field.name.clone(), shape);
            } else {
                env.insert_type_field_shape(&ty.name, field.name.clone(), shape);
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
    if let Some(shape) = source_java_enum_static_field_shape(receiver, field, env, ctx) {
        return Some(shape);
    }
    if let Some(signatures) = ctx.signatures
        && let Some(crate::member_access_plan::MemberAccessPlan {
            op: crate::member_access_plan::MemberAccessOp::StaticConstant { owner },
            ..
        }) = crate::member_access_plan::plan_member_access_with_facts(
            receiver,
            field,
            env.type_env(),
            ctx.facts.unwrap_or(&FactSet::default()),
        )
    {
        if let Some(shape) = signatures
            .field_shape(&owner, field)
            .or_else(|| signatures.field_shape(&owner, &crate::parser::to_snake_case(field)))
        {
            return Some(field_access_shape_from_signature_shape(shape));
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
            .map(field_access_shape_from_signature_shape)
    })
}

fn source_java_enum_static_field_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    let Expr::Var(receiver_name) = receiver else {
        return None;
    };
    if env.local_type(receiver_name).is_some() || env.field_type(receiver_name).is_some() {
        return None;
    }
    let representations = ctx.representations?;
    source_java_enum_static_field_owner_candidates(receiver_name, env)
        .into_iter()
        .find_map(|owner| {
            representations
                .java_enum_variant_for_label(&owner, field)
                .map(|_| {
                    project_java_value_shape_with_representations(
                        Some(representations),
                        JavaType::Named {
                            name: owner,
                            args: Vec::new(),
                        },
                    )
                })
        })
}

fn source_java_enum_static_field_owner_candidates(
    receiver_name: &str,
    env: &ShapeEnv,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(alias) = env.type_env().static_type_alias(receiver_name) {
        push_unique_type_name(&mut candidates, alias.to_string());
    }
    push_unique_type_name(&mut candidates, receiver_name.to_string());
    push_unique_type_name(
        &mut candidates,
        upper_camel_type_name_from_static_receiver(receiver_name),
    );
    candidates
}

fn push_unique_type_name(out: &mut Vec<String>, candidate: String) {
    if !candidate.is_empty() && !out.iter().any(|existing| existing == &candidate) {
        out.push(candidate);
    }
}

fn upper_camel_type_name_from_static_receiver(receiver_name: &str) -> String {
    receiver_name
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn field_access_shape_from_signature_shape(shape: &RustSlotShape) -> RustSlotShape {
    if matches!(shape.java_ty, JavaType::String) {
        borrowed_string_content_shape()
    } else {
        shape.clone()
    }
}

fn planned_member_access_shape(
    receiver: &Expr,
    field: &str,
    env: &ShapeEnv,
    ctx: ExprShapeContext<'_>,
) -> Option<RustSlotShape> {
    crate::member_access_plan::plan_member_access_with_facts(
        receiver,
        field,
        env.type_env(),
        ctx.facts.unwrap_or(&FactSet::default()),
    )
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
