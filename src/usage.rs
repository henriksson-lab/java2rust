use std::collections::{HashMap, HashSet};

use crate::facts::{FactSet, TrivialAccessorKind};
use crate::java_ir::{
    ConstructorInvocationKind, Expr, JavaType, Member, Modifier, Program, ResolvedCall,
    ResolvedConstructor, Stmt, TypeDecl,
};
use crate::stdlib::is_content_read_method;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageFacts {
    pub methods: HashMap<UsageMethodKey, MethodUsageFacts>,
    pub constructors: HashMap<UsageConstructorKey, MethodUsageFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageMethodKey {
    pub type_name: String,
    pub method_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageConstructorKey {
    pub type_name: String,
    pub rust_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MethodUsageFacts {
    pub bindings: HashMap<String, BindingUseFacts>,
    pub call_sites: Vec<CallSiteUse>,
    pub constructor_call_sites: Vec<ConstructorCallSiteUse>,
    pub returned_call_results: Vec<ReturnedCallResultUse>,
    pub field_uses: Vec<FieldUse>,
    pub mutates_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingUseFacts {
    pub name: String,
    pub java_ty: JavaType,
    pub uses: Vec<BindingUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingUse {
    ContentRead,
    AssignedToLocal {
        local_name: String,
    },
    AssignedFromCall {
        name: String,
        boundary: CallArgumentBoundary,
    },
    PassedToCall {
        name: String,
        arg_index: usize,
        boundary: CallArgumentBoundary,
    },
    ReceiverForCall {
        name: String,
        boundary: CallArgumentBoundary,
    },
    StoredInField {
        field_name: String,
    },
    Returned,
    Mutated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldUse {
    AssignedFromCall {
        field_name: String,
        name: String,
        boundary: CallArgumentBoundary,
    },
    PassedToCall {
        field_name: String,
        name: String,
        arg_index: usize,
        boundary: CallArgumentBoundary,
    },
    Returned {
        field_name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallArgumentBoundary {
    InternalSameType,
    KnownContentRead,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSiteUse {
    pub name: String,
    pub arg_count: usize,
    pub result_returned: bool,
    pub boundary: CallArgumentBoundary,
    pub resolved: Option<ResolvedCall>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorCallSiteUse {
    pub name: String,
    pub arg_count: usize,
    pub boundary: CallArgumentBoundary,
    pub resolved: Option<ResolvedConstructor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReturnedCallResultUse {
    pub name: String,
    pub boundary: CallArgumentBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallResultBoundaryPolicy {
    pub name: String,
    pub boundary: CallArgumentBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstructorInvocationBoundaryPolicy {
    pub kind: ConstructorInvocationKind,
    pub boundary: CallArgumentBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallArgumentBoundaryPolicy {
    pub name: String,
    pub boundary: CallArgumentBoundary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodApiBoundaryPolicy {
    pub public_api: bool,
}

pub fn analyze_usage(program: &Program) -> UsageFacts {
    analyze_usage_with_facts(program, &FactSet::default())
}

pub fn analyze_usage_with_facts(program: &Program, semantic_facts: &FactSet) -> UsageFacts {
    let mut facts = UsageFacts::default();
    for ty in &program.types {
        analyze_type_usage(ty, &mut facts, semantic_facts);
    }
    facts
}

fn analyze_type_usage(ty: &TypeDecl, facts: &mut UsageFacts, semantic_facts: &FactSet) {
    let internal_methods = internal_method_names(ty);
    let field_names = field_names(ty);
    for member in &ty.members {
        match member {
            Member::Method(method) => {
                let mut method_facts = MethodUsageFacts::default();
                for param in &method.params {
                    method_facts.bindings.insert(
                        param.name.clone(),
                        BindingUseFacts {
                            name: param.name.clone(),
                            java_ty: param.ty.clone(),
                            uses: Vec::new(),
                        },
                    );
                }
                if let Some(body) = &method.body {
                    for stmt in &body.statements {
                        analyze_stmt(
                            stmt,
                            &mut method_facts,
                            &ty.name,
                            &internal_methods,
                            &field_names,
                            semantic_facts,
                        );
                    }
                }
                facts.methods.insert(
                    UsageMethodKey {
                        type_name: ty.name.clone(),
                        method_name: method.name.clone(),
                    },
                    method_facts,
                );
            }
            Member::Constructor(ctor) => {
                let mut ctor_facts = MethodUsageFacts::default();
                for param in &ctor.params {
                    ctor_facts.bindings.insert(
                        param.name.clone(),
                        BindingUseFacts {
                            name: param.name.clone(),
                            java_ty: param.ty.clone(),
                            uses: Vec::new(),
                        },
                    );
                }
                if let Some(body) = &ctor.body {
                    for stmt in &body.statements {
                        analyze_stmt(
                            stmt,
                            &mut ctor_facts,
                            &ty.name,
                            &internal_methods,
                            &field_names,
                            semantic_facts,
                        );
                    }
                }
                facts.constructors.insert(
                    UsageConstructorKey {
                        type_name: ty.name.clone(),
                        rust_name: ctor.rust_name.clone().unwrap_or_else(|| "new".to_string()),
                    },
                    ctor_facts,
                );
            }
            Member::Type(nested) => analyze_type_usage(nested, facts, semantic_facts),
            _ => {}
        }
    }
}

fn field_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Field(field) => Some(field.name.clone()),
            _ => None,
        })
        .collect()
}

fn internal_method_names(ty: &TypeDecl) -> HashSet<String> {
    ty.members
        .iter()
        .filter_map(|member| match member {
            Member::Method(method) if !method_api_boundary_policy(&method.modifiers).public_api => {
                Some(method.name.clone())
            }
            _ => None,
        })
        .collect()
}

pub fn method_api_boundary_policy(modifiers: &[Modifier]) -> MethodApiBoundaryPolicy {
    MethodApiBoundaryPolicy {
        public_api: modifiers
            .iter()
            .any(|modifier| matches!(modifier, Modifier::Public | Modifier::Protected)),
    }
}

fn analyze_stmt(
    stmt: &Stmt,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    semantic_facts: &FactSet,
) {
    match stmt {
        Stmt::VarDecl {
            ty,
            name,
            initializer,
        } => {
            facts
                .bindings
                .entry(name.clone())
                .or_insert(BindingUseFacts {
                    name: name.clone(),
                    java_ty: ty.clone(),
                    uses: Vec::new(),
                });
            if let Some(initializer) = initializer {
                if let Expr::Var(source_name) = initializer {
                    record_use(
                        facts,
                        source_name,
                        BindingUse::AssignedToLocal {
                            local_name: name.clone(),
                        },
                    );
                }
                if let Some(policy) = call_result_boundary_policy(initializer, internal_methods) {
                    record_use(
                        facts,
                        name,
                        BindingUse::AssignedFromCall {
                            name: policy.name,
                            boundary: policy.boundary,
                        },
                    );
                }
                analyze_expr(
                    initializer,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Stmt::Assign { target, value } => {
            if let Expr::Var(name) = target {
                if is_unshadowed_field_name(facts, field_names, name) {
                    facts.mutates_self = true;
                    if let Some(policy) = call_result_boundary_policy(value, internal_methods) {
                        facts.field_uses.push(FieldUse::AssignedFromCall {
                            field_name: name.clone(),
                            name: policy.name,
                            boundary: policy.boundary,
                        });
                    }
                    analyze_stored_expr(value, facts, name);
                } else {
                    record_use(facts, name, BindingUse::Mutated);
                    if let Expr::Var(source_name) = value {
                        record_use(
                            facts,
                            source_name,
                            BindingUse::AssignedToLocal {
                                local_name: name.clone(),
                            },
                        );
                    }
                    if let Some(policy) = call_result_boundary_policy(value, internal_methods) {
                        record_use(
                            facts,
                            name,
                            BindingUse::AssignedFromCall {
                                name: policy.name,
                                boundary: policy.boundary,
                            },
                        );
                    }
                }
            }
            if let Expr::FieldAccess(_, field_name) = target {
                if target_is_this_field(target) {
                    facts.mutates_self = true;
                }
                if let Some(policy) = call_result_boundary_policy(value, internal_methods) {
                    facts.field_uses.push(FieldUse::AssignedFromCall {
                        field_name: field_name.clone(),
                        name: policy.name,
                        boundary: policy.boundary,
                    });
                }
                analyze_stored_expr(value, facts, field_name);
            }
            analyze_expr(
                target,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            analyze_expr(
                value,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
        }
        Stmt::Return(Some(expr)) => {
            if let Expr::Var(name) = expr {
                if is_unshadowed_field_name(facts, field_names, name) {
                    facts.field_uses.push(FieldUse::Returned {
                        field_name: name.clone(),
                    });
                } else {
                    record_use(facts, name, BindingUse::Returned);
                }
            }
            record_returned_call_result_use(expr, facts, internal_methods);
            if let Expr::FieldAccess(receiver, field_name) = expr
                && matches!(receiver.as_ref(), Expr::This)
            {
                facts.field_uses.push(FieldUse::Returned {
                    field_name: field_name.clone(),
                });
            }
            analyze_return_expr(
                expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
        }
        Stmt::Return(None) => {}
        Stmt::Expr(expr) => {
            if expr_mutates_self_field(expr, facts, field_names) {
                facts.mutates_self = true;
            }
            analyze_expr(
                expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
        }
        Stmt::If {
            condition,
            then_block,
            else_block,
        } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            for stmt in &then_block.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
            if let Some(else_block) = else_block {
                for stmt in &else_block.statements {
                    analyze_stmt(
                        stmt,
                        facts,
                        current_type,
                        internal_methods,
                        field_names,
                        semantic_facts,
                    );
                }
            }
        }
        Stmt::While { condition, body } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            for stmt in &body.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Stmt::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            for stmt in init {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
            if let Some(condition) = condition {
                analyze_expr(
                    condition,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
            for expr in update {
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
            for stmt in &body.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Stmt::EnhancedFor { iterable, body, .. } => {
            analyze_expr(
                iterable,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            for stmt in &body.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Stmt::Switch { selector, cases } => {
            analyze_expr(
                selector,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            for case in cases {
                for stmt in &case.statements {
                    analyze_stmt(
                        stmt,
                        facts,
                        current_type,
                        internal_methods,
                        field_names,
                        semantic_facts,
                    );
                }
            }
        }
        Stmt::Block(block) => {
            for stmt in &block.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Stmt::Try {
            resources, body, ..
        } => {
            for resource in resources {
                analyze_stmt(
                    resource,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
            for stmt in &body.statements {
                analyze_stmt(
                    stmt,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Stmt::Throw(expr) => analyze_expr(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            semantic_facts,
        ),
        Stmt::ConstructorInvocation {
            kind,
            args,
            resolved,
        } => {
            let policy = constructor_invocation_boundary_policy(*kind);
            facts.constructor_call_sites.push(ConstructorCallSiteUse {
                name: "constructor".to_string(),
                arg_count: args.len(),
                boundary: policy.boundary,
                resolved: resolved.clone(),
            });
            for (arg_index, arg) in args.iter().enumerate() {
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    semantic_facts,
                    arg,
                    "constructor",
                    arg_index,
                    policy.boundary,
                );
                analyze_expr(
                    arg,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Stmt::LocalType(_) | Stmt::Break(_) | Stmt::Continue(_) | Stmt::Unsupported(_) => {}
    }
}

pub fn constructor_invocation_boundary_policy(
    kind: ConstructorInvocationKind,
) -> ConstructorInvocationBoundaryPolicy {
    let boundary = match kind {
        ConstructorInvocationKind::This => CallArgumentBoundary::InternalSameType,
        ConstructorInvocationKind::Super => CallArgumentBoundary::Unknown,
    };
    ConstructorInvocationBoundaryPolicy { kind, boundary }
}

fn is_unshadowed_field_name(
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
    name: &str,
) -> bool {
    field_names.contains(name) && !facts.bindings.contains_key(name)
}

pub fn call_result_boundary_policy(
    expr: &Expr,
    internal_methods: &HashSet<String>,
) -> Option<CallResultBoundaryPolicy> {
    match expr {
        Expr::Call { receiver, name, .. } => Some(CallResultBoundaryPolicy {
            name: name.clone(),
            boundary: call_argument_boundary_policy(receiver.as_deref(), name, internal_methods)
                .boundary,
        }),
        Expr::New { .. } => Some(CallResultBoundaryPolicy {
            name: "new".to_string(),
            boundary: CallArgumentBoundary::Unknown,
        }),
        _ => None,
    }
}

fn analyze_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    semantic_facts: &FactSet,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
        internal_methods,
        field_names,
        semantic_facts,
        false,
    );
}

fn analyze_return_expr(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    semantic_facts: &FactSet,
) {
    analyze_expr_with_context(
        expr,
        facts,
        current_type,
        internal_methods,
        field_names,
        semantic_facts,
        true,
    );
}

fn analyze_expr_with_context(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    current_type: &str,
    internal_methods: &HashSet<String>,
    field_names: &HashSet<String>,
    semantic_facts: &FactSet,
    result_returned: bool,
) {
    match expr {
        Expr::FieldAccess(receiver, field) => {
            if field == "length"
                && let Expr::Var(name) = receiver.as_ref()
            {
                record_use(facts, name, BindingUse::ContentRead);
            }
            analyze_expr(
                receiver,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
        }
        Expr::Call {
            receiver,
            name,
            args,
            resolved,
            ..
        } => {
            let policy = call_argument_boundary_policy(receiver.as_deref(), name, internal_methods);
            record_trivial_accessor_call_use(
                facts,
                current_type,
                receiver.as_deref(),
                name,
                args,
                resolved.as_ref(),
                semantic_facts,
                result_returned,
            );
            facts.call_sites.push(CallSiteUse {
                name: name.clone(),
                arg_count: args.len(),
                result_returned,
                boundary: policy.boundary,
                resolved: resolved.clone(),
            });
            if is_content_read_method(name)
                && let Some(receiver) = receiver
                && let Expr::Var(binding) = receiver.as_ref()
            {
                record_use(facts, binding, BindingUse::ContentRead);
            }
            if !is_content_read_method(name)
                && let Some(receiver) = receiver
                && let Expr::Var(binding) = receiver.as_ref()
            {
                record_use(
                    facts,
                    binding,
                    BindingUse::ReceiverForCall {
                        name: name.clone(),
                        boundary: policy.boundary,
                    },
                );
            }
            for (arg_index, arg) in args.iter().enumerate() {
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    semantic_facts,
                    arg,
                    name,
                    arg_index,
                    policy.boundary,
                );
                analyze_expr(
                    arg,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
            if let Some(receiver) = receiver {
                analyze_expr(
                    receiver,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Expr::Binary { left, right, .. } => {
            analyze_expr(
                left,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            analyze_expr(
                right,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
        }
        Expr::Cast { expr, .. } => analyze_expr_with_context(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            semantic_facts,
            result_returned,
        ),
        Expr::Unary { expr, .. } | Expr::InstanceOf { expr, .. } => analyze_expr(
            expr,
            facts,
            current_type,
            internal_methods,
            field_names,
            semantic_facts,
        ),
        Expr::ArrayAccess(array, index) => {
            analyze_expr(
                array,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            analyze_expr(
                index,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
        }
        Expr::ArrayCreation { dimensions, .. } | Expr::ArrayInitializer(dimensions) => {
            for expr in dimensions {
                analyze_expr(
                    expr,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Expr::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            analyze_expr(
                condition,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
            );
            analyze_expr_with_context(
                then_expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
                result_returned,
            );
            analyze_expr_with_context(
                else_expr,
                facts,
                current_type,
                internal_methods,
                field_names,
                semantic_facts,
                result_returned,
            );
        }
        Expr::New { args, .. } => {
            for (arg_index, arg) in args.iter().enumerate() {
                record_call_arg_use(
                    facts,
                    current_type,
                    field_names,
                    semantic_facts,
                    arg,
                    "new",
                    arg_index,
                    CallArgumentBoundary::Unknown,
                );
                analyze_expr(
                    arg,
                    facts,
                    current_type,
                    internal_methods,
                    field_names,
                    semantic_facts,
                );
            }
        }
        Expr::Lambda { .. } => {}
        Expr::Literal(_)
        | Expr::Var(_)
        | Expr::This
        | Expr::Super
        | Expr::MethodReference { .. }
        | Expr::ClassLiteral(_)
        | Expr::Raw(_) => {}
    }
}

fn record_returned_call_result_use(
    expr: &Expr,
    facts: &mut MethodUsageFacts,
    internal_methods: &HashSet<String>,
) {
    match expr {
        Expr::Call { .. } | Expr::New { .. } => {
            let Some(policy) = call_result_boundary_policy(expr, internal_methods) else {
                return;
            };
            facts.returned_call_results.push(ReturnedCallResultUse {
                name: policy.name,
                boundary: policy.boundary,
            });
        }
        Expr::Cast { expr, .. } => record_returned_call_result_use(expr, facts, internal_methods),
        Expr::Ternary {
            then_expr,
            else_expr,
            ..
        } => {
            record_returned_call_result_use(then_expr, facts, internal_methods);
            record_returned_call_result_use(else_expr, facts, internal_methods);
        }
        _ => {}
    }
}

fn record_call_arg_use(
    facts: &mut MethodUsageFacts,
    current_type: &str,
    field_names: &HashSet<String>,
    semantic_facts: &FactSet,
    arg: &Expr,
    call_name: &str,
    arg_index: usize,
    boundary: CallArgumentBoundary,
) {
    if let Some(field_name) = trivial_getter_self_field(arg, current_type, semantic_facts) {
        facts.field_uses.push(FieldUse::PassedToCall {
            field_name,
            name: call_name.to_string(),
            arg_index,
            boundary,
        });
        return;
    }
    match arg {
        Expr::Var(name) if is_unshadowed_field_name(facts, field_names, name) => {
            facts.field_uses.push(FieldUse::PassedToCall {
                field_name: name.clone(),
                name: call_name.to_string(),
                arg_index,
                boundary,
            });
        }
        Expr::Var(binding) => record_use(
            facts,
            binding,
            BindingUse::PassedToCall {
                name: call_name.to_string(),
                arg_index,
                boundary,
            },
        ),
        Expr::FieldAccess(receiver, field_name) if matches!(receiver.as_ref(), Expr::This) => {
            facts.field_uses.push(FieldUse::PassedToCall {
                field_name: field_name.clone(),
                name: call_name.to_string(),
                arg_index,
                boundary,
            });
        }
        _ => {}
    }
}

fn record_trivial_accessor_call_use(
    facts: &mut MethodUsageFacts,
    current_type: &str,
    receiver: Option<&Expr>,
    name: &str,
    args: &[Expr],
    resolved: Option<&ResolvedCall>,
    semantic_facts: &FactSet,
    result_returned: bool,
) {
    let Some(accessor) =
        trivial_accessor_for_self_call(current_type, receiver, name, resolved, semantic_facts)
    else {
        return;
    };
    match &accessor.kind {
        TrivialAccessorKind::Getter { field } if args.is_empty() && result_returned => {
            facts.field_uses.push(FieldUse::Returned {
                field_name: field.field_name.clone(),
            });
        }
        TrivialAccessorKind::Setter { field, .. } if args.len() == 1 => {
            facts.mutates_self = true;
            analyze_stored_expr(&args[0], facts, &field.field_name);
        }
        _ => {}
    }
}

fn trivial_getter_self_field(
    expr: &Expr,
    current_type: &str,
    semantic_facts: &FactSet,
) -> Option<String> {
    let Expr::Call {
        receiver,
        name,
        args,
        resolved,
    } = expr
    else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let accessor = trivial_accessor_for_self_call(
        current_type,
        receiver.as_deref(),
        name,
        resolved.as_ref(),
        semantic_facts,
    )?;
    match &accessor.kind {
        TrivialAccessorKind::Getter { field } => Some(field.field_name.clone()),
        TrivialAccessorKind::Setter { .. } => None,
    }
}

fn trivial_accessor_for_self_call<'a>(
    current_type: &str,
    receiver: Option<&Expr>,
    name: &str,
    resolved: Option<&ResolvedCall>,
    semantic_facts: &'a FactSet,
) -> Option<&'a crate::facts::TrivialAccessorFact> {
    if !receiver.is_none_or(|receiver| matches!(receiver, Expr::This)) {
        return None;
    }
    if !resolved
        .as_ref()
        .is_none_or(|resolved| resolved.declaring_type == current_type)
    {
        return None;
    }
    semantic_facts.trivial_accessor_for(current_type, name)
}

fn target_is_this_field(expr: &Expr) -> bool {
    matches!(expr, Expr::FieldAccess(receiver, _) if matches!(receiver.as_ref(), Expr::This))
}

fn expr_mutates_self_field(
    expr: &Expr,
    facts: &MethodUsageFacts,
    field_names: &HashSet<String>,
) -> bool {
    match expr {
        Expr::Unary { expr, .. } => match expr.as_ref() {
            Expr::Var(name) => is_unshadowed_field_name(facts, field_names, name),
            target => target_is_this_field(target),
        },
        _ => false,
    }
}

fn analyze_stored_expr(expr: &Expr, facts: &mut MethodUsageFacts, field_name: &str) {
    if let Expr::Var(name) = expr {
        record_use(
            facts,
            name,
            BindingUse::StoredInField {
                field_name: field_name.to_string(),
            },
        );
    }
}

pub fn call_argument_boundary_policy(
    receiver: Option<&Expr>,
    name: &str,
    internal_methods: &HashSet<String>,
) -> CallArgumentBoundaryPolicy {
    let boundary = if is_content_read_method(name) {
        CallArgumentBoundary::KnownContentRead
    } else if internal_methods.contains(name)
        && receiver.is_none_or(|receiver| matches!(receiver, Expr::This))
    {
        CallArgumentBoundary::InternalSameType
    } else {
        CallArgumentBoundary::Unknown
    };
    CallArgumentBoundaryPolicy {
        name: name.to_string(),
        boundary,
    }
}

fn record_use(facts: &mut MethodUsageFacts, name: &str, usage: BindingUse) {
    if let Some(binding) = facts.bindings.get_mut(name) {
        binding.uses.push(usage);
    }
}
