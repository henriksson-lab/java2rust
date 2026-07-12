use std::collections::{HashMap, HashSet};

use crate::java_ir::{
    Block, ConstructorDecl, Expr, JavaType, MethodDecl, Modifier, Param, Program, Stmt, TypeDecl,
    UnaryOp,
};
use crate::representation::RepresentationPlan;
use crate::rust_ir::RustType;
use crate::slot_shape::{OwnershipShape, RustSlotShape, TypeCapabilities, ViewShape};
use crate::usage::{BindingUse, MethodUsageFacts, UsageConstructorKey, UsageFacts, UsageMethodKey};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SignaturePlan {
    methods: HashMap<MethodKey, MethodSignaturePlan>,
    constructors: HashMap<ConstructorKey, ConstructorSignaturePlan>,
    fields: HashMap<FieldKey, RustSlotShape>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodKey {
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConstructorKey {
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldKey {
    pub type_name: String,
    pub field_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodSignaturePlan {
    pub params: Vec<ParamPlan>,
    pub return_shape: Option<RustSlotShape>,
    pub locals: Vec<LocalPlan>,
    pub receiver: ReceiverPlan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorSignaturePlan {
    pub params: Vec<ParamPlan>,
    pub return_ty: RustType,
    pub locals: Vec<LocalPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamPlan {
    pub name: String,
    pub java_ty: JavaType,
    pub is_varargs: bool,
    pub shape: RustSlotShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPlan {
    pub name: String,
    pub java_ty: JavaType,
    pub shape: RustSlotShape,
    pub binding_mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiverPlan {
    None,
    Shared,
    Mutable,
}

pub fn default_param_shape(param: &Param) -> RustSlotShape {
    let mut shape = RustSlotShape::conservative(param.ty.clone());
    if param.is_varargs || matches!(param.ty, JavaType::Array(_)) {
        shape.ownership = OwnershipShape::SharedBorrow;
        shape.view = ViewShape::Slice;
    }
    shape
}

pub fn default_param_plan(param: &Param) -> ParamPlan {
    param_plan_with_shape(param, default_param_shape(param))
}

pub fn default_return_shape(java_ty: &JavaType) -> Option<RustSlotShape> {
    if matches!(java_ty, JavaType::Void) {
        None
    } else {
        Some(RustSlotShape::conservative(java_ty.clone()))
    }
}

pub fn solve_signatures(
    program: &Program,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
) -> SignaturePlan {
    let mut plan = SignaturePlan::default();
    for ty in &program.types {
        collect_type_signatures(ty, representations, usage, &mut plan);
    }
    plan
}

impl SignaturePlan {
    pub fn method_signature(
        &self,
        ty: &TypeDecl,
        method: &MethodDecl,
    ) -> Option<&MethodSignaturePlan> {
        self.methods.get(&MethodKey {
            type_name: ty.name.clone(),
            rust_name: method_rust_name(method),
        })
    }

    pub fn method_signature_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<&MethodSignaturePlan> {
        self.methods.get(&MethodKey {
            type_name: type_name.to_string(),
            rust_name: rust_name.to_string(),
        })
    }

    pub fn constructor_signature(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> Option<&ConstructorSignaturePlan> {
        self.constructors.get(&ConstructorKey {
            type_name: ty.name.clone(),
            rust_name: constructor_rust_name(ctor),
        })
    }

    pub fn constructor_signature_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<&ConstructorSignaturePlan> {
        self.constructors.get(&ConstructorKey {
            type_name: type_name.to_string(),
            rust_name: rust_name.to_string(),
        })
    }

    pub fn method_arg_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
        index: usize,
    ) -> Option<RustSlotShape> {
        self.method_signature_by_rust_name(type_name, rust_name)
            .and_then(|signature| signature.params.get(index))
            .map(|param| param.shape.clone())
    }

    pub fn constructor_arg_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
        index: usize,
    ) -> Option<RustSlotShape> {
        self.constructor_signature_by_rust_name(type_name, rust_name)
            .and_then(|signature| signature.params.get(index))
            .map(|param| param.shape.clone())
    }

    pub fn method_return_shape_by_rust_name(
        &self,
        type_name: &str,
        rust_name: &str,
    ) -> Option<RustSlotShape> {
        self.method_signature_by_rust_name(type_name, rust_name)
            .and_then(|signature| signature.return_shape.clone())
    }

    pub fn field_shape(&self, type_name: &str, field_name: &str) -> Option<&RustSlotShape> {
        self.fields.get(&FieldKey {
            type_name: type_name.to_string(),
            field_name: field_name.to_string(),
        })
    }

    pub fn constructor_param_plans(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> Option<&[ParamPlan]> {
        self.constructor_signature(ty, ctor)
            .map(|signature| signature.params.as_slice())
    }

    pub fn constructor_return_type(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> Option<RustType> {
        self.constructor_signature(ty, ctor)
            .map(|signature| signature.return_ty.clone())
    }

    pub fn method_param_plans(&self, ty: &TypeDecl, method: &MethodDecl) -> Option<&[ParamPlan]> {
        self.method_signature(ty, method)
            .map(|signature| signature.params.as_slice())
    }

    pub fn method_return_shape(&self, ty: &TypeDecl, method: &MethodDecl) -> Option<RustSlotShape> {
        self.method_signature(ty, method)
            .and_then(|signature| signature.return_shape.clone())
    }

    pub fn method_receiver(&self, ty: &TypeDecl, method: &MethodDecl) -> Option<ReceiverPlan> {
        self.method_signature(ty, method)
            .map(|signature| signature.receiver)
    }

    pub fn method_local_shapes_or_empty(&self, ty: &TypeDecl, method: &MethodDecl) -> &[LocalPlan] {
        self.method_signature(ty, method)
            .map(|signature| signature.locals.as_slice())
            .unwrap_or(&[])
    }

    pub fn constructor_local_shapes_or_empty(
        &self,
        ty: &TypeDecl,
        ctor: &ConstructorDecl,
    ) -> &[LocalPlan] {
        self.constructor_signature(ty, ctor)
            .map(|signature| signature.locals.as_slice())
            .unwrap_or(&[])
    }

    pub fn local_shape<'a>(
        &self,
        locals: &'a [LocalPlan],
        local_name: &str,
    ) -> Option<&'a RustSlotShape> {
        locals
            .iter()
            .find(|local| local.name == local_name)
            .map(|local| &local.shape)
    }

    pub fn local_binding_mutability(&self, locals: &[LocalPlan], local_name: &str) -> Option<bool> {
        locals
            .iter()
            .find(|local| local.name == local_name)
            .map(|local| local.binding_mutable)
    }

    pub fn param_rust_type(&self, param: &ParamPlan) -> RustType {
        param.shape.rust_type()
    }
}

fn collect_type_signatures(
    ty: &TypeDecl,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
    plan: &mut SignaturePlan,
) {
    for member in &ty.members {
        match member {
            crate::java_ir::Member::Field(field) => {
                plan.fields.insert(
                    FieldKey {
                        type_name: ty.name.clone(),
                        field_name: field.name.clone(),
                    },
                    storage_shape(field.ty.clone(), representations),
                );
            }
            crate::java_ir::Member::Method(method) => {
                plan.methods.insert(
                    MethodKey {
                        type_name: ty.name.clone(),
                        rust_name: method_rust_name(method),
                    },
                    MethodSignaturePlan {
                        params: method
                            .params
                            .iter()
                            .map(|param| {
                                method_param_plan(ty, method, param, representations, usage)
                            })
                            .collect(),
                        return_shape: method_return_shape(ty, method, representations, usage),
                        locals: method
                            .body
                            .as_ref()
                            .map(|body| collect_local_plans(body, representations))
                            .unwrap_or_default(),
                        receiver: default_receiver_plan(method),
                    },
                );
            }
            crate::java_ir::Member::Constructor(ctor) => {
                plan.constructors.insert(
                    ConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: constructor_rust_name(ctor),
                    },
                    ConstructorSignaturePlan {
                        params: ctor
                            .params
                            .iter()
                            .map(|param| {
                                constructor_param_plan(ty, ctor, param, representations, usage)
                            })
                            .collect(),
                        return_ty: RustType::Named(ty.name.clone()),
                        locals: ctor
                            .body
                            .as_ref()
                            .map(|body| collect_local_plans(body, representations))
                            .unwrap_or_default(),
                    },
                );
            }
            crate::java_ir::Member::Type(nested) => {
                collect_type_signatures(nested, representations, usage, plan);
            }
            crate::java_ir::Member::Unsupported { .. } => {}
        }
    }
}

fn method_param_plan(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
) -> ParamPlan {
    let mut plan = param_plan(param, representations);
    if can_use_borrowed_str_param(ty, method, param, usage)
        || can_return_borrowed_str_from_param(ty, method, param, usage)
    {
        plan.shape.ownership = OwnershipShape::SharedBorrow;
        plan.shape.view = ViewShape::Str;
    }
    plan
}

fn constructor_param_plan(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
) -> ParamPlan {
    let mut plan = param_plan(param, representations);
    if can_use_borrowed_str_constructor_param(ty, ctor, param, usage) {
        plan.shape.ownership = OwnershipShape::SharedBorrow;
        plan.shape.view = ViewShape::Str;
    }
    plan
}

fn param_plan(param: &Param, representations: &RepresentationPlan) -> ParamPlan {
    let mut shape = default_param_shape(param);
    shape.dispatch = representations.dispatch_for(&param.ty);
    param_plan_with_shape(param, shape)
}

fn param_plan_with_shape(param: &Param, shape: RustSlotShape) -> ParamPlan {
    ParamPlan {
        name: param.name.clone(),
        java_ty: param.ty.clone(),
        is_varargs: param.is_varargs,
        shape,
    }
}

fn can_use_borrowed_str_param(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
) -> bool {
    if !TypeCapabilities::conservative(&param.ty).string_like
        || method.modifiers.contains(&Modifier::Public)
    {
        return false;
    }
    let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) else {
        return false;
    };
    usage_allows_borrowed_str(method_usage, &param.name)
}

fn can_return_borrowed_str_from_param(
    ty: &TypeDecl,
    method: &MethodDecl,
    param: &Param,
    usage: &UsageFacts,
) -> bool {
    if !TypeCapabilities::conservative(&param.ty).string_like
        || !TypeCapabilities::conservative(&method.return_type).string_like
        || method.modifiers.contains(&Modifier::Public)
        || !method.modifiers.contains(&Modifier::Static)
    {
        return false;
    }
    let Some(method_usage) = usage.methods.get(&UsageMethodKey {
        type_name: ty.name.clone(),
        method_name: method.name.clone(),
    }) else {
        return false;
    };
    let Some(binding) = method_usage.bindings.get(&param.name) else {
        return false;
    };
    !binding.uses.is_empty()
        && binding
            .uses
            .iter()
            .all(|usage| matches!(usage, BindingUse::Returned))
}

fn can_use_borrowed_str_constructor_param(
    ty: &TypeDecl,
    ctor: &ConstructorDecl,
    param: &Param,
    usage: &UsageFacts,
) -> bool {
    if !TypeCapabilities::conservative(&param.ty).string_like
        || ctor.modifiers.contains(&Modifier::Public)
    {
        return false;
    }
    let rust_name = ctor.rust_name.clone().unwrap_or_else(|| "new".to_string());
    let Some(ctor_usage) = usage.constructors.get(&UsageConstructorKey {
        type_name: ty.name.clone(),
        rust_name,
    }) else {
        return false;
    };
    usage_allows_borrowed_str(ctor_usage, &param.name)
}

fn usage_allows_borrowed_str(usage: &MethodUsageFacts, param_name: &str) -> bool {
    let Some(binding) = usage.bindings.get(param_name) else {
        return false;
    };
    !binding.uses.is_empty()
        && binding.uses.iter().all(|usage| {
            matches!(
                usage,
                BindingUse::ContentRead | BindingUse::PassedToCall { .. }
            )
        })
}

fn storage_shape(java_ty: JavaType, representations: &RepresentationPlan) -> RustSlotShape {
    let mut shape = RustSlotShape::for_storage(java_ty.clone());
    shape.dispatch = representations.dispatch_for(&java_ty);
    shape
}

fn collect_local_plans(
    block: &crate::java_ir::Block,
    representations: &RepresentationPlan,
) -> Vec<LocalPlan> {
    let mut locals = Vec::new();
    let mutated = collect_mutated_local_names(block);
    collect_local_plans_from_stmts(&block.statements, representations, &mutated, &mut locals);
    locals
}

fn collect_local_plans_from_stmts(
    statements: &[crate::java_ir::Stmt],
    representations: &RepresentationPlan,
    mutated: &HashSet<String>,
    locals: &mut Vec<LocalPlan>,
) {
    use crate::java_ir::Stmt;
    for stmt in statements {
        match stmt {
            Stmt::VarDecl { ty, name, .. } => locals.push(LocalPlan {
                name: name.clone(),
                java_ty: ty.clone(),
                shape: storage_shape(ty.clone(), representations),
                binding_mutable: mutated.contains(name),
            }),
            Stmt::For { init, body, .. } => {
                collect_local_plans_from_stmts(init, representations, mutated, locals);
                collect_local_plans_from_stmts(&body.statements, representations, mutated, locals);
            }
            Stmt::EnhancedFor {
                var_type,
                var_name,
                body,
                ..
            } => {
                locals.push(LocalPlan {
                    name: var_name.clone(),
                    java_ty: var_type.clone(),
                    shape: storage_shape(var_type.clone(), representations),
                    binding_mutable: mutated.contains(var_name),
                });
                collect_local_plans_from_stmts(&body.statements, representations, mutated, locals);
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_local_plans_from_stmts(
                    &then_block.statements,
                    representations,
                    mutated,
                    locals,
                );
                if let Some(else_block) = else_block {
                    collect_local_plans_from_stmts(
                        &else_block.statements,
                        representations,
                        mutated,
                        locals,
                    );
                }
            }
            Stmt::While { body, .. } | Stmt::Block(body) => {
                collect_local_plans_from_stmts(&body.statements, representations, mutated, locals);
            }
            Stmt::Switch { cases, .. } => {
                for case in cases {
                    collect_local_plans_from_stmts(
                        &case.statements,
                        representations,
                        mutated,
                        locals,
                    );
                }
            }
            Stmt::Try { body, .. } => {
                collect_local_plans_from_stmts(&body.statements, representations, mutated, locals);
            }
            Stmt::LocalType(_)
            | Stmt::Assign { .. }
            | Stmt::Expr(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_mutated_local_names(block: &Block) -> HashSet<String> {
    let mut mutated = HashSet::new();
    collect_mutated_local_names_from_stmts(&block.statements, &mut mutated);
    mutated
}

fn collect_mutated_local_names_from_stmts(statements: &[Stmt], mutated: &mut HashSet<String>) {
    for stmt in statements {
        match stmt {
            Stmt::VarDecl { initializer, .. } => {
                if let Some(expr) = initializer {
                    collect_mutated_local_names_from_expr(expr, mutated);
                }
            }
            Stmt::Assign { target, value } => {
                collect_assignment_target_name(target, mutated);
                collect_mutated_local_names_from_expr(value, mutated);
            }
            Stmt::Expr(expr) | Stmt::Return(Some(expr)) | Stmt::Throw(expr) => {
                collect_mutated_local_names_from_expr(expr, mutated);
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
            } => {
                collect_mutated_local_names_from_expr(condition, mutated);
                collect_mutated_local_names_from_stmts(&then_block.statements, mutated);
                if let Some(else_block) = else_block {
                    collect_mutated_local_names_from_stmts(&else_block.statements, mutated);
                }
            }
            Stmt::While { condition, body } => {
                collect_mutated_local_names_from_expr(condition, mutated);
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
                ..
            } => {
                collect_mutated_local_names_from_stmts(init, mutated);
                if let Some(condition) = condition {
                    collect_mutated_local_names_from_expr(condition, mutated);
                }
                for expr in update {
                    collect_mutated_local_names_from_expr(expr, mutated);
                }
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
            }
            Stmt::EnhancedFor { iterable, body, .. } => {
                collect_mutated_local_names_from_expr(iterable, mutated);
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
            }
            Stmt::Switch { selector, cases } => {
                collect_mutated_local_names_from_expr(selector, mutated);
                for case in cases {
                    collect_mutated_local_names_from_stmts(&case.statements, mutated);
                }
            }
            Stmt::Try {
                body,
                catches,
                finally_block,
            } => {
                collect_mutated_local_names_from_stmts(&body.statements, mutated);
                for catch in catches {
                    collect_mutated_local_names_from_stmts(&catch.body.statements, mutated);
                }
                if let Some(finally_block) = finally_block {
                    collect_mutated_local_names_from_stmts(&finally_block.statements, mutated);
                }
            }
            Stmt::Block(block) => {
                collect_mutated_local_names_from_stmts(&block.statements, mutated);
            }
            Stmt::Return(None)
            | Stmt::LocalType(_)
            | Stmt::ConstructorInvocation { .. }
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Unsupported(_) => {}
        }
    }
}

fn collect_mutated_local_names_from_expr(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Unary {
            op: UnaryOp::PostInc | UnaryOp::PostDec | UnaryOp::PreInc | UnaryOp::PreDec,
            expr,
        } => collect_assignment_target_name(expr, mutated),
        Expr::Unary { expr, .. } | Expr::Cast { expr, .. } | Expr::InstanceOf { expr, .. } => {
            collect_mutated_local_names_from_expr(expr, mutated);
        }
        Expr::Binary { left, right, .. } => {
            collect_mutated_local_names_from_expr(left, mutated);
            collect_mutated_local_names_from_expr(right, mutated);
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_mutated_local_names_from_expr(condition, mutated);
            collect_mutated_local_names_from_expr(then_expr, mutated);
            collect_mutated_local_names_from_expr(else_expr, mutated);
        }
        Expr::FieldAccess(receiver, _) => collect_mutated_local_names_from_expr(receiver, mutated),
        Expr::ArrayAccess(array, index) => {
            collect_mutated_local_names_from_expr(array, mutated);
            collect_mutated_local_names_from_expr(index, mutated);
        }
        Expr::Call { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                collect_mutated_local_names_from_expr(receiver, mutated);
            }
            for arg in args {
                collect_mutated_local_names_from_expr(arg, mutated);
            }
        }
        Expr::New { args, .. } => {
            for arg in args {
                collect_mutated_local_names_from_expr(arg, mutated);
            }
        }
        Expr::Lambda { .. }
        | Expr::MethodReference { .. }
        | Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn collect_assignment_target_name(expr: &Expr, mutated: &mut HashSet<String>) {
    match expr {
        Expr::Var(name) => {
            mutated.insert(name.clone());
        }
        Expr::ArrayAccess(array, index) => {
            collect_mutated_local_names_from_expr(array, mutated);
            collect_mutated_local_names_from_expr(index, mutated);
        }
        Expr::FieldAccess(receiver, _) => collect_mutated_local_names_from_expr(receiver, mutated),
        _ => collect_mutated_local_names_from_expr(expr, mutated),
    }
}

pub fn default_receiver_plan(method: &MethodDecl) -> ReceiverPlan {
    if method.modifiers.contains(&Modifier::Static) {
        ReceiverPlan::None
    } else if method.body.as_ref().is_some_and(block_mutates_self) {
        ReceiverPlan::Mutable
    } else {
        ReceiverPlan::Shared
    }
}

fn block_mutates_self(block: &Block) -> bool {
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
        } => block_mutates_self(then_block) || else_block.as_ref().is_some_and(block_mutates_self),
        Stmt::While { body, .. } | Stmt::For { body, .. } | Stmt::EnhancedFor { body, .. } => {
            block_mutates_self(body)
        }
        Stmt::Switch { cases, .. } => cases
            .iter()
            .any(|case| case.statements.iter().any(stmt_mutates_self)),
        Stmt::Try {
            body,
            catches,
            finally_block,
        } => {
            block_mutates_self(body)
                || catches.iter().any(|catch| block_mutates_self(&catch.body))
                || finally_block.as_ref().is_some_and(block_mutates_self)
        }
        _ => false,
    }
}

fn target_is_self_field(expr: &Expr) -> bool {
    matches!(expr, Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This))
}

fn return_shape(java_ty: &JavaType, representations: &RepresentationPlan) -> Option<RustSlotShape> {
    let mut shape = default_return_shape(java_ty)?;
    shape.dispatch = representations.dispatch_for(java_ty);
    Some(shape)
}

fn method_return_shape(
    ty: &TypeDecl,
    method: &MethodDecl,
    representations: &RepresentationPlan,
    usage: &UsageFacts,
) -> Option<RustSlotShape> {
    if let Some(param) = method
        .params
        .iter()
        .find(|param| can_return_borrowed_str_from_param(ty, method, param, usage))
    {
        let mut shape = RustSlotShape::conservative(param.ty.clone());
        shape.ownership = OwnershipShape::SharedBorrow;
        shape.view = ViewShape::Str;
        return Some(shape);
    }
    return_shape(&method.return_type, representations)
}

fn method_rust_name(method: &MethodDecl) -> String {
    method
        .rust_name
        .clone()
        .unwrap_or_else(|| crate::parser::to_snake_case(&method.name))
}

fn constructor_rust_name(ctor: &ConstructorDecl) -> String {
    ctor.rust_name.clone().unwrap_or_else(|| "new".to_string())
}
